#![cfg(test)]

//! Exact-format editor saves through the shared real TCP/SSH/SFTP fixture.

use std::collections::HashMap;
use std::error::Error;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use keelshell_core::{
    MAX_TEXT_EDIT_BYTES, MAX_TEXT_EDIT_LINES, TextDocument, TextEncoding, TextFormat,
    TextLineEnding,
};
use russh::keys::{HashAlg, PrivateKey, ssh_key::Algorithm};
use russh::{Channel, ChannelId, server};
use tokio::net::TcpListener;
use tokio::task::{JoinHandle, JoinSet};
use zeroize::Zeroizing;

use super::{RegularFileSnapshot, SftpSession};
use crate::{SessionError, SshAuth, SshOptions, SshSession};

// Other users of this packet fixture exercise its unrelated transfer controls.
#[allow(dead_code)]
#[path = "../../tests/fixtures/sftp.rs"]
mod sftp_fixture;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const PATH: &str = "/editor.txt";
const LEGACY_ENCODINGS: [TextEncoding; 5] = [
    TextEncoding::Gbk,
    TextEncoding::Gb18030,
    TextEncoding::Big5,
    TextEncoding::ShiftJis,
    TextEncoding::Windows1252,
];

struct Peer {
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    filesystem: sftp_fixture::Filesystem,
    jobs: JoinSet<()>,
}

impl server::Handler for Peer {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(
            if user == "fixture" && password == "ephemeral-test-password" {
                server::Auth::Accept
            } else {
                server::Auth::reject()
            },
        )
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if name != "sftp" {
            session.channel_failure(id)?;
            return Ok(());
        }
        let Some(channel) = self.channels.remove(&id) else {
            return Err(russh::Error::Disconnect);
        };
        session.channel_success(id)?;
        let filesystem = self.filesystem.clone();
        self.jobs.spawn(async move {
            russh_sftp::server::run(channel.into_stream(), filesystem).await;
        });
        Ok(())
    }
}

struct Fixture {
    filesystem: sftp_fixture::Filesystem,
    session: SshSession,
    sftp: SftpSession,
    server: tokio::sync::Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(server) = self.server.get_mut().take() {
            server.abort();
        }
    }
}

impl Fixture {
    async fn start() -> TestResult<Self> {
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::from_millis(1),
            auth_rejection_time_initial: Some(Duration::from_millis(1)),
            ..Default::default()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let filesystem = sftp_fixture::Filesystem::default();
        let peer = Peer {
            channels: HashMap::new(),
            filesystem: filesystem.clone(),
            jobs: JoinSet::new(),
        };
        let mut server = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await
                && let Ok(running) = server::run_stream(config, stream, peer).await
            {
                let _ = running.await;
            }
        });
        let mut options = SshOptions::new("127.0.0.1", "fixture");
        options.port = address.port();
        options.expected_host_key = Some(fingerprint);
        options.auth = SshAuth::Password(Zeroizing::new("ephemeral-test-password".into()));
        options.timeout = Duration::from_secs(5);
        let connected = tokio::time::timeout(Duration::from_secs(8), async {
            let session = SshSession::connect(options).await?;
            let sftp = session.sftp().await?;
            Ok::<_, SessionError>((session, sftp))
        })
        .await;
        let (session, sftp) = match connected {
            Ok(Ok(connected)) => connected,
            failed => {
                server.abort();
                let _ = (&mut server).await;
                return Err(match failed {
                    Ok(Err(error)) => error.into(),
                    Err(error) => error.into(),
                    Ok(Ok(_)) => unreachable!("successful setup is handled above"),
                });
            }
        };
        Ok(Self {
            filesystem,
            session,
            sftp,
            server: tokio::sync::Mutex::new(Some(server)),
        })
    }

    async fn snapshot(&self, bytes: &[u8]) -> TestResult<RegularFileSnapshot> {
        // Setup belongs to this isolated in-memory filesystem, so every
        // observed mutating packet comes from the API under examination.
        self.filesystem.replace_external_file(PATH, bytes)?;
        Ok(self
            .sftp
            .read_regular_snapshot(PATH, bytes.len().max(1))
            .await?)
    }

    async fn assert_no_mutations(&self, reviewed: &RegularFileSnapshot) -> TestResult {
        assert_eq!(self.filesystem.atomic_writes_started(), 0);
        assert_eq!(self.filesystem.transfer_writes_started(), 0);
        assert_eq!(self.filesystem.writable_closes_started(), 0);
        let current = self
            .sftp
            .read_regular_snapshot(PATH, reviewed.content.len().max(1))
            .await?;
        assert_eq!(current.content, reviewed.content);
        assert_eq!(current.entry.size, reviewed.entry.size);
        assert_eq!(current.entry.permissions, reviewed.entry.permissions);
        assert_eq!(current.entry.modified, reviewed.entry.modified);
        let entries = self.sftp.list("/").await?;
        assert_eq!(entries.len(), 1, "no backup or temporary may be created");
        assert_eq!(entries[0].path, PATH);
        Ok(())
    }

    async fn assert_saved(
        &self,
        reviewed: &RegularFileSnapshot,
        replacement: &[u8],
        backup: &str,
    ) -> TestResult {
        assert!(backup.starts_with("/.editor.txt.keelshell-backup-"));
        assert!(backup.ends_with(".txt"));
        assert_eq!(
            self.sftp.read_regular(backup, MAX_TEXT_EDIT_BYTES).await?,
            reviewed.content
        );
        let current = self
            .sftp
            .read_regular_snapshot(PATH, MAX_TEXT_EDIT_BYTES)
            .await?;
        assert_eq!(current.content, replacement);
        assert_eq!(current.entry.permissions, reviewed.entry.permissions);
        let entries = self.sftp.list("/").await?;
        assert_eq!(
            entries.len(),
            2,
            "only the target and completed backup remain"
        );
        assert!(entries.iter().all(|entry| !entry.name.ends_with(".tmp")));
        Ok(())
    }

    async fn backup(&self) -> TestResult<String> {
        self.sftp
            .list("/")
            .await?
            .into_iter()
            .find(|entry| {
                entry.name.starts_with(".editor.txt.keelshell-backup-")
                    && entry.name.ends_with(".txt")
            })
            .map(|entry| entry.path)
            .ok_or_else(|| "no acknowledged original backup".into())
    }

    async fn finish(&self) -> TestResult {
        self.sftp.close().await?;
        self.session.close().await?;
        let server = self.server.lock().await.take();
        if let Some(mut server) = server {
            match tokio::time::timeout(Duration::from_secs(5), &mut server).await {
                Ok(joined) => joined?,
                Err(error) => {
                    server.abort();
                    let _ = server.await;
                    return Err(error.into());
                }
            }
        }
        Ok(())
    }
}

fn legacy_format(encoding: TextEncoding) -> TestResult<TextFormat> {
    Ok(TextDocument::decode_with_encoding(b"old\r\nlast", encoding)?.format())
}

fn assert_invalid(result: crate::Result<String>) {
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("invalid editor input unexpectedly produced a backup"),
    };
    assert!(matches!(error, SessionError::Invalid(_)), "{error:?}");
    assert!(!error.to_string().contains("private"));
    assert!(!format!("{error:?}").contains("private"));
}

async fn reached(condition: impl Fn() -> bool) -> TestResult {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !condition() {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    Ok(())
}

async fn legacy_roundtrip(
    encoding: TextEncoding,
    original: &[u8],
    body: &str,
    draft: &str,
    replacement: &[u8],
) -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(original).await?;
    let document = TextDocument::decode_with_encoding(&reviewed.content, encoding)?;
    assert_eq!(document.body(), body);
    assert_eq!(document.format().encoding(), encoding);
    assert!(!document.format().has_bom());
    assert!(TextDocument::decode(&reviewed.content).is_err());
    assert_eq!(document.format().prepare_save(draft)?, replacement);
    let backup = fixture
        .sftp
        .write_editor_reviewed_with_format_authorized(
            &reviewed,
            replacement,
            document.format(),
            &|| true,
        )
        .await?;
    fixture
        .assert_saved(&reviewed, replacement, &backup)
        .await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_gbk_preserves_exact_source_backup_and_replacement() -> TestResult {
    legacy_roundtrip(
        TextEncoding::Gbk,
        b"\xd6\xd0\xce\xc4\r\nold",
        "中文\r\nold",
        "你好\nnew",
        b"\xc4\xe3\xba\xc3\r\nnew",
    )
    .await
}

#[tokio::test]
async fn editor_encoding_gb18030_preserves_four_byte_source_backup_and_replacement() -> TestResult {
    legacy_roundtrip(
        TextEncoding::Gb18030,
        b"\x90\x30\x81\x30\r\nold",
        "\u{10000}\r\nold",
        "\u{10000}\nnew",
        b"\x90\x30\x81\x30\r\nnew",
    )
    .await
}

#[tokio::test]
async fn editor_encoding_big5_preserves_exact_source_backup_and_replacement() -> TestResult {
    legacy_roundtrip(
        TextEncoding::Big5,
        b"\xa4\xa4\xa4\xe5\r\nold",
        "中文\r\nold",
        "你好\nnew",
        b"\xa7\x41\xa6\x6e\r\nnew",
    )
    .await
}

#[tokio::test]
async fn editor_encoding_shift_jis_preserves_exact_source_backup_and_replacement() -> TestResult {
    legacy_roundtrip(
        TextEncoding::ShiftJis,
        b"\x93\xfa\x96\x7b\r\nold",
        "日本\r\nold",
        "東京\nnew",
        b"\x93\x8c\x8b\x9e\r\nnew",
    )
    .await
}

#[tokio::test]
async fn editor_encoding_windows1252_preserves_exact_source_backup_and_replacement() -> TestResult {
    legacy_roundtrip(
        TextEncoding::Windows1252,
        b"caf\xe9 \x80\r\nold",
        "café €\r\nold",
        "naïve £\nnew",
        b"na\xefve \xa3\r\nnew",
    )
    .await
}

#[tokio::test]
async fn editor_encoding_wrong_source_encoding_is_typed_and_mutation_free() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"private\xd6\xd0\r\nold").await?;
    let wrong = TextDocument::decode(b"old\r\nlast")?.format();
    assert_invalid(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(&reviewed, b"new", wrong, &|| true)
            .await,
    );
    fixture.assert_no_mutations(&reviewed).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_source_must_roundtrip_under_supplied_line_policy() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"private\xd6\xd0\r\nold").await?;
    let wrong = legacy_format(TextEncoding::Gbk)?.with_line_ending(TextLineEnding::Lf);
    assert!(wrong.decode_body(&reviewed.content).is_ok());
    assert_invalid(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(&reviewed, b"new", wrong, &|| true)
            .await,
    );
    fixture.assert_no_mutations(&reviewed).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_invalid_source_never_falls_back_or_replaces_bytes() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"private\x81").await?;
    assert_invalid(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(
                &reviewed,
                b"new",
                legacy_format(TextEncoding::Gbk)?,
                &|| true,
            )
            .await,
    );
    fixture.assert_no_mutations(&reviewed).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_invalid_candidate_is_rejected_before_backup() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    let format = legacy_format(TextEncoding::Gbk)?;
    for candidate in [b"private\x81".as_slice(), b"private\0body"] {
        assert_invalid(
            fixture
                .sftp
                .write_editor_reviewed_with_format_authorized(&reviewed, candidate, format, &|| {
                    true
                })
                .await,
        );
        fixture.assert_no_mutations(&reviewed).await?;
    }
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_candidate_must_equal_prepared_line_policy_bytes() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    let format = legacy_format(TextEncoding::Gbk)?;
    let candidate = b"private\xc4\xe3\nnew";
    assert!(format.decode_body(candidate).is_ok());
    assert_invalid(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(&reviewed, candidate, format, &|| true)
            .await,
    );
    fixture.assert_no_mutations(&reviewed).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_legacy_authority_cannot_reinterpret_unicode_boms() -> TestResult {
    let fixture = Fixture::start().await?;
    for encoding in LEGACY_ENCODINGS {
        let format = legacy_format(encoding)?;
        for unicode in [
            b"\xef\xbb\xbfprivate".as_slice(),
            b"\xff\xfep\0",
            b"\xfe\xff\0p",
        ] {
            let reviewed = fixture.snapshot(b"old\r\nlast").await?;
            assert_invalid(
                fixture
                    .sftp
                    .write_editor_reviewed_with_format_authorized(
                        &reviewed,
                        unicode,
                        format,
                        &|| true,
                    )
                    .await,
            );
            fixture.assert_no_mutations(&reviewed).await?;
            let reviewed = fixture.snapshot(unicode).await?;
            assert_invalid(
                fixture
                    .sftp
                    .write_editor_reviewed_with_format_authorized(
                        &reviewed,
                        b"new",
                        format,
                        &|| true,
                    )
                    .await,
            );
            fixture.assert_no_mutations(&reviewed).await?;
        }
    }
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_noncanonical_legacy_source_and_candidate_are_rejected() -> TestResult {
    let fixture = Fixture::start().await?;
    let format = legacy_format(TextEncoding::ShiftJis)?;
    // This Shift_JIS alias decodes to U+2252 but re-encodes as 0x81 0xE0.
    let alias = b"private\x87\x90";
    let reviewed = fixture.snapshot(alias).await?;
    assert_invalid(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(&reviewed, b"new", format, &|| true)
            .await,
    );
    fixture.assert_no_mutations(&reviewed).await?;
    let reviewed = fixture.snapshot(b"\x93\xfa\x96\x7b\r\nold").await?;
    assert_invalid(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(&reviewed, alias, format, &|| true)
            .await,
    );
    fixture.assert_no_mutations(&reviewed).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_lossy_best_fit_and_unrepresentable_drafts_produce_no_candidate()
-> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"old\r\nlast").await?;
    for (encoding, draft) in [
        (TextEncoding::Gbk, "private😀"),
        (TextEncoding::Big5, "private😀"),
        (TextEncoding::Windows1252, "private中"),
        (TextEncoding::ShiftJis, "private😀"),
        (TextEncoding::ShiftJis, "private¥"),
        (TextEncoding::ShiftJis, "private‾"),
    ] {
        // The byte-only SFTP API cannot recover a draft after somebody has
        // already substituted it. No bytes may be proposed from a failed save.
        let error = legacy_format(encoding)?
            .prepare_save(draft)
            .err()
            .ok_or("lossy draft unexpectedly produced reviewable bytes")?;
        assert!(!error.to_string().contains("private"));
        assert!(!format!("{error:?}").contains("private"));
        fixture.assert_no_mutations(&reviewed).await?;
    }
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_expired_authority_creates_no_backup_or_temporary() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    assert!(matches!(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(
                &reviewed,
                b"\xc4\xe3\r\nnew",
                legacy_format(TextEncoding::Gbk)?,
                &|| false,
            )
            .await,
        Err(SessionError::Closed)
    ));
    fixture.assert_no_mutations(&reviewed).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_authority_revoked_during_source_check_prevents_all_mutations() -> TestResult
{
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    let format = legacy_format(TextEncoding::Gbk)?;
    let hold = fixture.filesystem.hold_metadata_path(PATH)?;
    let allowed = AtomicBool::new(true);
    let authorize = || allowed.load(Ordering::Acquire);
    let writer = fixture.sftp.write_editor_reviewed_with_format_authorized(
        &reviewed,
        b"\xc4\xe3\r\nnew",
        format,
        &authorize,
    );
    tokio::pin!(writer);
    tokio::select! {
        result = &mut writer => return Err(format!("source check was not held: {result:?}").into()),
        observed = reached(|| hold.entered() > 0) => observed?,
    }
    allowed.store(false, Ordering::Release);
    hold.release();
    assert!(matches!(writer.await, Err(SessionError::Closed)));
    fixture.assert_no_mutations(&reviewed).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_raw_source_and_candidate_budgets_fail_before_backup() -> TestResult {
    let fixture = Fixture::start().await?;
    let format = legacy_format(TextEncoding::Windows1252)?;
    let oversized = vec![b'x'; MAX_TEXT_EDIT_BYTES + 1];
    for (original, candidate) in [
        (oversized.as_slice(), b"new".as_slice()),
        (b"old".as_slice(), oversized.as_slice()),
    ] {
        let reviewed = fixture.snapshot(original).await?;
        assert!(matches!(
            fixture
                .sftp
                .write_editor_reviewed_with_format_authorized(&reviewed, candidate, format, &|| {
                    true
                },)
                .await,
            Err(SessionError::OutputLimit(MAX_TEXT_EDIT_BYTES))
        ));
        fixture.assert_no_mutations(&reviewed).await?;
    }
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_decoded_source_and_candidate_budgets_fail_before_backup() -> TestResult {
    let fixture = Fixture::start().await?;
    let format = legacy_format(TextEncoding::Gbk)?;
    let oversized_body = b"\xd6\xd0".repeat(MAX_TEXT_EDIT_BYTES / 3 + 1);
    assert!(oversized_body.len() < MAX_TEXT_EDIT_BYTES);
    for (original, candidate) in [
        (oversized_body.as_slice(), b"new".as_slice()),
        (b"old".as_slice(), oversized_body.as_slice()),
    ] {
        let reviewed = fixture.snapshot(original).await?;
        assert_invalid(
            fixture
                .sftp
                .write_editor_reviewed_with_format_authorized(&reviewed, candidate, format, &|| {
                    true
                })
                .await,
        );
        fixture.assert_no_mutations(&reviewed).await?;
    }
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_saved_line_budget_rejects_candidate_before_backup() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    let candidate = b"\xc4\xe3\r\n".repeat(MAX_TEXT_EDIT_LINES + 1);
    assert_invalid(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(
                &reviewed,
                &candidate,
                legacy_format(TextEncoding::Gbk)?,
                &|| true,
            )
            .await,
    );
    fixture.assert_no_mutations(&reviewed).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_final_crlf_expansion_budget_fails_before_backup() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    let mut candidate = vec![b'x'; MAX_TEXT_EDIT_BYTES - 1];
    candidate.push(b'\n');
    let format = legacy_format(TextEncoding::Gbk)?;
    assert_eq!(format.decode_body(&candidate)?.len(), MAX_TEXT_EDIT_BYTES);
    assert_invalid(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(&reviewed, &candidate, format, &|| true)
            .await,
    );
    fixture.assert_no_mutations(&reviewed).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_source_over_line_limit_can_be_replaced_at_save_line_limit() -> TestResult {
    let fixture = Fixture::start().await?;
    let original = b"\xd6\xd0\r\n".repeat(MAX_TEXT_EDIT_LINES + 1);
    let reviewed = fixture.snapshot(&original).await?;
    let format = TextDocument::decode_with_encoding(&original, TextEncoding::Gbk)?.format();
    let candidate = b"\xc4\xe3\r\n".repeat(MAX_TEXT_EDIT_LINES);
    let backup = fixture
        .sftp
        .write_editor_reviewed_with_format_authorized(&reviewed, &candidate, format, &|| true)
        .await?;
    fixture.assert_saved(&reviewed, &candidate, &backup).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_one_mib_legacy_save_keeps_mcp_at_64_kib() -> TestResult {
    let fixture = Fixture::start().await?;
    let original = b"\x90\x30\x81\x30\r\nold";
    let reviewed = fixture.snapshot(original).await?;
    let format = TextDocument::decode_with_encoding(original, TextEncoding::Gb18030)?.format();
    let candidate = b"\x90\x30\x81\x30".repeat(MAX_TEXT_EDIT_BYTES / 4);
    assert_eq!(candidate.len(), MAX_TEXT_EDIT_BYTES);
    assert_eq!(format.decode_body(&candidate)?.len(), MAX_TEXT_EDIT_BYTES);
    assert!(matches!(
        fixture
            .sftp
            .write_regular_reviewed(&reviewed, &candidate)
            .await,
        Err(SessionError::OutputLimit(65536))
    ));
    fixture.assert_no_mutations(&reviewed).await?;
    let backup = fixture
        .sftp
        .write_editor_reviewed_with_format_authorized(&reviewed, &candidate, format, &|| true)
        .await?;
    fixture.assert_saved(&reviewed, &candidate, &backup).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_bomless_leading_feff_uses_exact_authority_without_reguessing() -> TestResult
{
    let fixture = Fixture::start().await?;
    let format = TextDocument::decode(b"old\r\nlast")?.format();
    assert!(!format.has_bom());
    let original = "\u{feff}old\r\nlast".as_bytes();
    let reviewed = fixture.snapshot(original).await?;
    let candidate = "\u{feff}\u{feff}new\r\nlast".as_bytes();
    assert_eq!(format.decode_body(original)?, "\u{feff}old\r\nlast");
    let backup = fixture
        .sftp
        .write_editor_reviewed_with_format_authorized(&reviewed, candidate, format, &|| true)
        .await?;
    fixture.assert_saved(&reviewed, candidate, &backup).await?;
    assert_eq!(
        format.decode_body(&fixture.sftp.read_regular(PATH, MAX_TEXT_EDIT_BYTES).await?)?,
        "\u{feff}\u{feff}new\r\nlast"
    );
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_unicode_entrypoint_rejects_legacy_sources_and_candidates() -> TestResult {
    let fixture = Fixture::start().await?;
    for (encoding, legacy) in [
        (TextEncoding::Gbk, b"\xd6\xd0".as_slice()),
        (TextEncoding::Gb18030, b"\x90\x30\x81\x30"),
        (TextEncoding::Big5, b"\xa4\xa4"),
        (TextEncoding::ShiftJis, b"\x93\xfa"),
        (TextEncoding::Windows1252, b"caf\xe9"),
    ] {
        assert!(TextDocument::decode_with_encoding(legacy, encoding).is_ok());
        let reviewed = fixture.snapshot(legacy).await?;
        assert_invalid(
            fixture
                .sftp
                .write_editor_reviewed_authorized(&reviewed, b"new", &|| true)
                .await,
        );
        fixture.assert_no_mutations(&reviewed).await?;
        let reviewed = fixture.snapshot(b"old").await?;
        assert_invalid(
            fixture
                .sftp
                .write_editor_reviewed_authorized(&reviewed, legacy, &|| true)
                .await,
        );
        fixture.assert_no_mutations(&reviewed).await?;
    }
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_unicode_compatibility_entrypoint_keeps_encoding_only_check() -> TestResult
{
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"old\nlast").await?;
    let candidate = b"\xef\xbb\xbfnew\r\nlast";
    let backup = fixture
        .sftp
        .write_editor_reviewed_authorized(&reviewed, candidate, &|| true)
        .await?;
    fixture.assert_saved(&reviewed, candidate, &backup).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_missing_posix_rename_has_no_mutations_or_fallback() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    fixture.filesystem.set_atomic_unsupported(true);
    assert!(matches!(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(
                &reviewed,
                b"\xc4\xe3\r\nnew",
                legacy_format(TextEncoding::Gbk)?,
                &|| true,
            )
            .await,
        Err(SessionError::Unsupported(_))
    ));
    fixture.assert_no_mutations(&reviewed).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_backup_close_ack_precedes_target_staging() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    let format = legacy_format(TextEncoding::Gbk)?;
    let candidate = b"\xc4\xe3\r\nnew";
    let hold = fixture
        .filesystem
        .hold_close("/..editor.txt.keelshell-backup-", true, true)?;
    let writer = fixture.sftp.write_editor_reviewed_with_format_authorized(
        &reviewed,
        candidate,
        format,
        &|| true,
    );
    tokio::pin!(writer);
    tokio::select! {
        result = &mut writer => return Err(format!("backup CLOSE was not held: {result:?}").into()),
        observed = reached(|| hold.entered() > 0) => observed?,
    }
    assert_eq!(fixture.filesystem.atomic_writes_started(), 1);
    let entries = fixture.sftp.list("/").await?;
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|entry| {
        entry.name.starts_with("..editor.txt.keelshell-backup-") && entry.name.ends_with(".tmp")
    }));
    assert_eq!(
        fixture.sftp.read_regular(PATH, 1024).await?,
        reviewed.content
    );
    assert!(
        fixture.backup().await.is_err(),
        "backup publication has no ACK yet"
    );
    hold.release();
    let backup = writer.await?;
    fixture.assert_saved(&reviewed, candidate, &backup).await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_backup_close_failure_prevents_target_staging() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    fixture.filesystem.reject_writable_closes(true);
    assert!(
        fixture
            .sftp
            .write_editor_reviewed_with_format_authorized(
                &reviewed,
                b"\xc4\xe3\r\nnew",
                legacy_format(TextEncoding::Gbk)?,
                &|| true,
            )
            .await
            .is_err()
    );
    fixture.filesystem.reject_writable_closes(false);
    assert_eq!(fixture.filesystem.atomic_writes_started(), 1);
    assert_eq!(
        fixture.sftp.read_regular(PATH, 1024).await?,
        reviewed.content
    );
    assert!(fixture.backup().await.is_err());
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_revalidates_source_after_staging_and_excludes_shared_writers() -> TestResult
{
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    let format = legacy_format(TextEncoding::Gbk)?;
    let hold =
        fixture
            .filesystem
            .hold_close_excluding("/.editor.txt.keelshell-", true, true, "backup")?;
    let writer = fixture.sftp.write_editor_reviewed_with_format_authorized(
        &reviewed,
        b"\xc4\xe3\r\nnew",
        format,
        &|| true,
    );
    tokio::pin!(writer);
    tokio::select! {
        result = &mut writer => return Err(format!("target CLOSE was not held: {result:?}").into()),
        observed = reached(|| hold.entered() > 0) => observed?,
    }
    let sibling = fixture.session.sftp().await?;
    assert!(matches!(
        sibling.write(PATH, b"bypass").await,
        Err(SessionError::MutationBusy)
    ));
    let backup = fixture.backup().await?;
    assert_eq!(
        fixture.sftp.read_regular(&backup, 1024).await?,
        reviewed.content
    );
    let external = b"\xce\xc4\r\nold";
    fixture.filesystem.replace_external_file(PATH, external)?;
    hold.release();
    assert!(matches!(writer.await, Err(SessionError::Invalid(_))));
    assert_eq!(fixture.sftp.read_regular(PATH, 1024).await?, external);
    assert_eq!(
        fixture.sftp.read_regular(&backup, 1024).await?,
        reviewed.content
    );
    sibling.close().await?;
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_revocation_before_publication_retains_original_and_backup() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    let format = legacy_format(TextEncoding::Gbk)?;
    let hold =
        fixture
            .filesystem
            .hold_close_excluding("/.editor.txt.keelshell-", true, true, "backup")?;
    let allowed = AtomicBool::new(true);
    let authorize = || allowed.load(Ordering::Acquire);
    let writer = fixture.sftp.write_editor_reviewed_with_format_authorized(
        &reviewed,
        b"\xc4\xe3\r\nnew",
        format,
        &authorize,
    );
    tokio::pin!(writer);
    tokio::select! {
        result = &mut writer => return Err(format!("target CLOSE was not held: {result:?}").into()),
        observed = reached(|| hold.entered() > 0) => observed?,
    }
    let backup = fixture.backup().await?;
    allowed.store(false, Ordering::Release);
    hold.release();
    assert!(matches!(writer.await, Err(SessionError::Closed)));
    assert_eq!(
        fixture.sftp.read_regular(PATH, 1024).await?,
        reviewed.content
    );
    assert_eq!(
        fixture.sftp.read_regular(&backup, 1024).await?,
        reviewed.content
    );
    assert!(matches!(
        fixture.sftp.inspect_remote_mutation_quarantine(PATH).await,
        Err(SessionError::Invalid(
            "no matching unknown transfer destination"
        ))
    ));
    fixture.finish().await
}

#[tokio::test]
async fn editor_encoding_dropped_target_close_keeps_backup_and_shared_quarantine() -> TestResult {
    let fixture = Fixture::start().await?;
    let reviewed = fixture.snapshot(b"\xd6\xd0\r\nold").await?;
    let format = legacy_format(TextEncoding::Gbk)?;
    let hold =
        fixture
            .filesystem
            .hold_close_excluding("/.editor.txt.keelshell-", true, true, "backup")?;
    {
        let writer = fixture.sftp.write_editor_reviewed_with_format_authorized(
            &reviewed,
            b"\xc4\xe3\r\nnew",
            format,
            &|| true,
        );
        tokio::pin!(writer);
        tokio::select! {
            result = &mut writer => return Err(format!("target CLOSE was not held: {result:?}").into()),
            observed = reached(|| hold.entered() > 0) => observed?,
        }
    }
    let backup = fixture.backup().await?;
    assert_eq!(
        fixture.sftp.read_regular(PATH, 1024).await?,
        reviewed.content
    );
    assert_eq!(
        fixture.sftp.read_regular(&backup, 1024).await?,
        reviewed.content
    );
    assert!(
        !fixture
            .sftp
            .inspect_remote_mutation_quarantine(PATH)
            .await?
            .entries()
            .is_empty()
    );
    let sibling = fixture.session.sftp().await?;
    assert!(matches!(
        sibling.write(PATH, b"bypass").await,
        Err(SessionError::MutationQuarantined)
    ));
    hold.release();
    reached(|| hold.pending() == 0).await?;
    sibling.close().await?;
    fixture.finish().await
}
