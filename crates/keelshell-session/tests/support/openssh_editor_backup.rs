//! Verified editor backups against the independent disposable OpenSSH server.
use super::*;
use keelshell_core::{MAX_TEXT_EDIT_BYTES, TextDocument, TextEncoding};

#[tokio::test]
#[ignore = "requires the disposable pinned OpenSSH loopback runner"]
async fn openssh_editor_verified_backups_preserve_unicode_legacy_empty_and_repeated_bytes()
-> TestResult {
    tokio::time::timeout(Duration::from_secs(75), async {
        let fixture = Fixture::connect().await?;
        let cases = [
            (
                "unicode '$(printf never).txt",
                TextEncoding::Utf8,
                "原始\r\n内容".as_bytes(),
                "审核\nUnicode",
            ),
            (
                "utf16-be.txt",
                TextEncoding::Utf16Be,
                b"\xfe\xff\x53\x9f\x00\x0d\x00\x0a\x59\xcb".as_slice(),
                "已审\n内容",
            ),
            (
                "gbk-original.txt",
                TextEncoding::Gbk,
                b"\xd6\xd0\xce\xc4".as_slice(),
                "审核\n内容",
            ),
            (
                "empty.txt",
                TextEncoding::Utf8,
                b"".as_slice(),
                "first save",
            ),
        ];
        let mut originals = Vec::new();
        for (name, encoding, initial, draft) in cases {
            let path = fixture.path(name);
            fixture.sftp.write(&path, initial).await?;
            let format = TextDocument::decode_with_encoding(initial, encoding)?.format();
            let mut prior = initial.to_vec();
            // The second save starts from the first verified target observation.
            // Each backup must contain its own complete prior bytes.
            for text in [draft, ""] {
                let reviewed = fixture
                    .sftp
                    .read_regular_snapshot(&path, MAX_TEXT_EDIT_BYTES)
                    .await?;
                assert_eq!(reviewed.content, prior);
                let expected = format.prepare_save(text)?;
                let backup = fixture
                    .sftp
                    .write_editor_reviewed_with_format_authorized(
                        &reviewed,
                        &expected,
                        format,
                        &|| true,
                    )
                    .await?;
                assert!(
                    backup
                        .starts_with(&format!("{}/.{name}.keelshell-backup-", fixture.directory,))
                );
                assert!(backup.ends_with(".txt"));
                assert!(!originals.iter().any(|(path, _)| path == &backup));
                let observed_backup = fixture
                    .sftp
                    .read_regular_snapshot(&backup, MAX_TEXT_EDIT_BYTES)
                    .await?;
                assert_eq!(observed_backup.content, prior);
                assert_eq!(observed_backup.entry.size, Some(prior.len() as u64));
                assert_eq!(observed_backup.entry.permissions, Some(0o100600));
                let observed_target = fixture
                    .sftp
                    .read_regular_snapshot(&path, MAX_TEXT_EDIT_BYTES)
                    .await?;
                assert_eq!(observed_target.content, expected);
                assert_eq!(
                    observed_target.entry.permissions,
                    reviewed.entry.permissions,
                );
                originals.push((backup, prior));
                prior = expected;
            }
        }
        assert_eq!(originals.len(), 8);
        // Recheck after every save: later target publications cannot overwrite
        // previously returned backups.
        for (path, bytes) in &originals {
            assert_eq!(
                fixture.sftp.read_regular(path, MAX_TEXT_EDIT_BYTES).await?,
                *bytes,
            );
        }
        assert_eq!(
            fixture
                .ssh
                .exec("printf verified-editor-backups-survive")
                .await?
                .stdout,
            b"verified-editor-backups-survive",
        );
        for (path, _) in originals {
            fixture.sftp.remove(&path).await?;
        }
        fixture
            .close(
                &[
                    "unicode '$(printf never).txt",
                    "utf16-be.txt",
                    "gbk-original.txt",
                    "empty.txt",
                ],
                &[],
            )
            .await
    })
    .await?
}
