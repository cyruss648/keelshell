#![cfg(test)]
//! Exact legacy bytes against the independent, disposable OpenSSH runner.
use super::*;
use keelshell_core::{MAX_TEXT_EDIT_BYTES, TextDocument, TextEncoding};

#[tokio::test]
#[ignore = "requires the disposable pinned OpenSSH loopback runner"]
async fn openssh_legacy_editor_preserves_encoding_original_backup_and_rejects_invalid_writes()
-> TestResult {
    tokio::time::timeout(Duration::from_secs(65), async {
        let fixture = Fixture::connect().await?;
        let cases = [
            (
                "gbk.txt",
                TextEncoding::Gbk,
                "中文",
                b"\xd6\xd0\xce\xc4".as_slice(),
            ),
            (
                "gb18030.txt",
                TextEncoding::Gb18030,
                "😀",
                b"\x94\x39\xfc\x36".as_slice(),
            ),
            (
                "big5.txt",
                TextEncoding::Big5,
                "中文",
                b"\xa4\xa4\xa4\xe5".as_slice(),
            ),
            (
                "shift-jis.txt",
                TextEncoding::ShiftJis,
                "日本",
                b"\x93\xfa\x96\x7b".as_slice(),
            ),
            (
                "windows-1252.txt",
                TextEncoding::Windows1252,
                "Café",
                b"Caf\xe9".as_slice(),
            ),
        ];
        for (name, encoding, body, raw) in cases {
            let path = fixture.path(name);
            fixture.sftp.write(&path, raw).await?;
            let snapshot = fixture
                .sftp
                .read_regular_snapshot(&path, MAX_TEXT_EDIT_BYTES)
                .await?;
            assert_eq!(snapshot.content, raw);
            let document = TextDocument::decode_with_encoding(raw, encoding)?;
            assert_eq!(document.body(), body);
            let format = document.format();
            let mut expected = raw.to_vec();
            expected.extend_from_slice(b"\nreviewed");
            let before = fixture.sftp.list(&fixture.directory).await?.len();
            assert!(
                fixture
                    .sftp
                    .write_editor_reviewed_authorized(&snapshot, &expected, &|| true)
                    .await
                    .is_err()
            );
            assert!(
                fixture
                    .sftp
                    .write_editor_reviewed_with_format_authorized(
                        &snapshot,
                        &expected,
                        format,
                        &|| false
                    )
                    .await
                    .is_err()
            );
            assert!(
                fixture
                    .sftp
                    .write_editor_reviewed_with_format_authorized(&snapshot, b"\0", format, &|| {
                        true
                    })
                    .await
                    .is_err()
            );
            let unicode = TextDocument::decode(b"seed")?.format();
            assert!(
                fixture
                    .sftp
                    .write_editor_reviewed_with_format_authorized(
                        &snapshot,
                        &expected,
                        unicode,
                        &|| true
                    )
                    .await
                    .is_err()
            );
            assert_eq!(fixture.sftp.list(&fixture.directory).await?.len(), before);
            let unchanged = fixture
                .sftp
                .read_regular_snapshot(&path, MAX_TEXT_EDIT_BYTES)
                .await?;
            assert_eq!(unchanged.content, raw);
            assert_eq!(unchanged.entry.permissions, snapshot.entry.permissions);
            assert_eq!(unchanged.entry.modified, snapshot.entry.modified);
            let backup = fixture
                .sftp
                .write_editor_reviewed_with_format_authorized(&snapshot, &expected, format, &|| {
                    true
                })
                .await?;
            assert_eq!(
                fixture
                    .sftp
                    .read_regular(&path, MAX_TEXT_EDIT_BYTES)
                    .await?,
                expected
            );
            assert_eq!(
                fixture
                    .sftp
                    .read_regular(&backup, MAX_TEXT_EDIT_BYTES)
                    .await?,
                raw
            );
            assert_eq!(format.decode_body(&expected)?, format!("{body}\nreviewed"));
            fixture.sftp.remove(&backup).await?;
        }
        assert_eq!(
            fixture
                .ssh
                .exec("printf editor-codecs-survive")
                .await?
                .stdout,
            b"editor-codecs-survive"
        );
        fixture
            .close(
                &[
                    "gbk.txt",
                    "gb18030.txt",
                    "big5.txt",
                    "shift-jis.txt",
                    "windows-1252.txt",
                ],
                &[],
            )
            .await
    })
    .await?
}
