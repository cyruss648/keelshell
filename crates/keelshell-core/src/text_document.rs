//! UTF-8 document envelopes for bounded, explicitly reviewed remote saves.

use crate::{MAX_TEXT_EDIT_BYTES, MAX_TEXT_EDIT_LINES};

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";

/// Observed line terminators, without guessing a dominant style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextLineEnding {
    /// No line terminator was present; newly entered breaks use LF.
    None,
    /// All terminators are LF.
    Lf,
    /// All terminators are CRLF.
    CrLf,
    /// All terminators are standalone CR; edits remain verbatim.
    Cr,
    /// More than one kind of terminator; edits remain verbatim.
    Mixed,
}

/// The original UTF-8 signature and line-ending policy, separate from text.
///
/// This is transient document state, not permission to change a remote file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Utf8TextFormat {
    bom: bool,
    line_ending: TextLineEnding,
}

/// One checked UTF-8 body, excluding exactly one leading signature BOM.
///
/// Callers retain the original raw file snapshot for conflict and backup checks.
#[derive(Clone, PartialEq, Eq)]
pub struct Utf8TextDocument {
    body: String,
    format: Utf8TextFormat,
}

/// A rejected format operation. Errors never include document contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TextDocumentError {
    /// Input is not valid UTF-8.
    #[error("document is not valid UTF-8")]
    InvalidUtf8,
    /// A non-UTF-8 signature was supplied; no encoding is guessed or converted.
    #[error("document has an unsupported encoding signature")]
    UnsupportedEncoding,
    /// The editor does not accept NUL-containing text.
    #[error("document contains NUL")]
    Binary,
    /// Raw bytes or a save result exceed the existing editor budget.
    #[error("document exceeds the 1 MiB or 4096-line save budget")]
    Limit,
}

impl Utf8TextDocument {
    /// Decode complete raw bytes without lossy conversion or newline changes.
    ///
    /// Opening retains the existing byte-only bound. The 4096-line bound is
    /// applied when preparing a save, not when observing an unchanged file.
    ///
    /// # Errors
    /// Rejects more than 1 MiB, non-UTF-8 signatures, malformed UTF-8 and NUL.
    pub fn decode(bytes: &[u8]) -> Result<Self, TextDocumentError> {
        if bytes.len() > MAX_TEXT_EDIT_BYTES {
            return Err(TextDocumentError::Limit);
        }
        if [b"\xff\xfe".as_slice(), b"\xfe\xff", b"\x00\x00\xfe\xff"]
            .iter()
            .any(|signature| bytes.starts_with(signature))
        {
            return Err(TextDocumentError::UnsupportedEncoding);
        }
        let bom = bytes.starts_with(UTF8_BOM);
        let body = bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes);
        let body = std::str::from_utf8(body).map_err(|_| TextDocumentError::InvalidUtf8)?;
        if body.contains('\0') {
            return Err(TextDocumentError::Binary);
        }
        Ok(Self {
            body: body.to_owned(),
            format: Utf8TextFormat {
                bom,
                line_ending: line_ending(body),
            },
        })
    }

    /// Exact editable body, with internal U+FEFF and all line bytes retained.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Signature and observed line-ending policy of the original raw file.
    pub fn format(&self) -> Utf8TextFormat {
        self.format
    }
}

impl Utf8TextFormat {
    /// Whether the raw file starts with the three-byte UTF-8 signature.
    pub fn has_bom(self) -> bool {
        self.bom
    }

    /// The known terminator profile, retained across saves without line breaks.
    ///
    /// Mixed files are never majority-normalized.
    pub fn line_ending(self) -> TextLineEnding {
        self.line_ending
    }

    /// Retain a known insertion policy while continuing the same open document.
    ///
    /// An observation without terminators cannot select a different policy.
    /// Keep the previous profile in that case, while the BOM belongs to this
    /// new observation. An explicit new read uses its decoded format directly.
    pub fn retain_insertion_policy(self, previous: Self) -> Self {
        Self {
            line_ending: if self.line_ending == TextLineEnding::None {
                previous.line_ending
            } else {
                self.line_ending
            },
            ..self
        }
    }

    /// Whether two observations permit ordinary content merging.
    ///
    /// No-break and LF files share the LF insertion policy. A changed BOM,
    /// CRLF policy or verbatim profile requires an explicit new read and review.
    pub fn is_compatible_with(self, other: Self) -> bool {
        let policy = |ending| match ending {
            TextLineEnding::None => TextLineEnding::Lf,
            ending => ending,
        };
        self.bom == other.bom && policy(self.line_ending) == policy(other.line_ending)
    }

    /// Prepare the exact bytes that must appear in a save approval and readback.
    ///
    /// LF/CRLF documents retain their original policy, including inserted LF.
    /// Standalone CR bytes are never discarded. Mixed and CR-only documents
    /// remain verbatim. No final newline is added or removed.
    ///
    /// # Errors
    /// Rejects NUL, over 4096 LF-delimited lines, or over 1 MiB after adding the
    /// original BOM and expanding CRLF. It never returns a truncated result.
    pub fn prepare_save(self, draft: &str) -> Result<Vec<u8>, TextDocumentError> {
        self.encode(draft, true)
    }

    /// Compare draft save bytes with a raw observation without authorizing a save.
    ///
    /// The byte bound still applies, but an unchanged observation with more than
    /// 4096 lines must not become falsely dirty merely because it cannot be saved.
    pub fn matches_bytes(self, draft: &str, raw: &[u8]) -> bool {
        self.encode(draft, false).is_ok_and(|bytes| bytes == raw)
    }

    fn encode(self, draft: &str, check_lines: bool) -> Result<Vec<u8>, TextDocumentError> {
        if draft.len() > MAX_TEXT_EDIT_BYTES {
            return Err(TextDocumentError::Limit);
        }
        if draft.contains('\0') {
            return Err(TextDocumentError::Binary);
        }
        let mut result = Vec::with_capacity(draft.len().saturating_add(3).min(MAX_TEXT_EDIT_BYTES));
        if self.bom {
            result.extend_from_slice(UTF8_BOM);
        }
        for (index, line) in draft.split_inclusive('\n').enumerate() {
            if check_lines && index >= MAX_TEXT_EDIT_LINES {
                return Err(TextDocumentError::Limit);
            }
            let normalized = matches!(
                self.line_ending,
                TextLineEnding::None | TextLineEnding::Lf | TextLineEnding::CrLf
            );
            if normalized && let Some(body) = line.strip_suffix('\n') {
                let body = body.strip_suffix('\r').unwrap_or(body);
                extend(&mut result, body.as_bytes())?;
                extend(
                    &mut result,
                    if self.line_ending == TextLineEnding::CrLf {
                        b"\r\n"
                    } else {
                        b"\n"
                    },
                )?;
            } else {
                extend(&mut result, line.as_bytes())?;
            }
        }
        Ok(result)
    }
}

fn extend(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), TextDocumentError> {
    if output.len().saturating_add(bytes.len()) > MAX_TEXT_EDIT_BYTES {
        return Err(TextDocumentError::Limit);
    }
    output.extend_from_slice(bytes);
    Ok(())
}

fn line_ending(text: &str) -> TextLineEnding {
    let bytes = text.as_bytes();
    let (mut lf, mut crlf, mut cr) = (false, false, false);
    let mut offset = 0;
    while offset < bytes.len() {
        match bytes[offset] {
            b'\r' if bytes.get(offset + 1) == Some(&b'\n') => {
                crlf = true;
                offset += 1;
            }
            b'\r' => cr = true,
            b'\n' => lf = true,
            _ => {}
        }
        offset += 1;
    }
    match (lf, crlf, cr) {
        (false, false, false) => TextLineEnding::None,
        (true, false, false) => TextLineEnding::Lf,
        (false, true, false) => TextLineEnding::CrLf,
        (false, false, true) => TextLineEnding::Cr,
        _ => TextLineEnding::Mixed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked<T>(result: Result<T, TextDocumentError>) -> T {
        result.unwrap_or_else(|error| panic!("UTF-8 document fixture: {error}"))
    }

    #[test]
    fn text_document_unchanged_round_trip_keeps_bom_unicode_terminators_and_eof() {
        for raw in [
            "".as_bytes(),
            UTF8_BOM,
            "中文😀".as_bytes(),
            "\u{feff}".as_bytes(),
            "\u{feff}\u{feff}inside\u{feff}\r\nlast".as_bytes(),
            "a\nb\n".as_bytes(),
            "a\r\nb\r\n".as_bytes(),
            "a\r\nb\nlast".as_bytes(),
            "a\rb\r".as_bytes(),
        ] {
            let document = checked(Utf8TextDocument::decode(raw));
            assert_eq!(
                checked(document.format().prepare_save(document.body())),
                raw
            );
            assert!(document.format().matches_bytes(document.body(), raw));
        }
    }

    #[test]
    fn text_document_preserves_uniform_policy_without_double_cr_or_forced_eof() {
        for (raw, draft, expected) in [
            ("a\r\nb", "a\r\nnew\nb", "a\r\nnew\r\nb"),
            ("a\nb", "a\r\nnew\nb", "a\nnew\nb"),
            ("single", "single\nnew", "single\nnew"),
            ("a\r\nb\n", "a\r\nnew\nb\n", "a\r\nnew\nb\n"),
            ("a\rb\r", "a\rb\rnew\n", "a\rb\rnew\n"),
        ] {
            let document = checked(Utf8TextDocument::decode(raw.as_bytes()));
            assert_eq!(
                checked(document.format().prepare_save(draft)),
                expected.as_bytes()
            );
        }
    }

    #[test]
    fn text_document_only_one_signature_is_removed_and_internal_feff_remains() {
        let document = checked(Utf8TextDocument::decode(
            "\u{feff}\u{feff}a\u{feff}".as_bytes(),
        ));
        assert!(document.format().has_bom());
        assert_eq!(document.body(), "\u{feff}a\u{feff}");
        assert_eq!(
            checked(document.format().prepare_save(document.body())),
            "\u{feff}\u{feff}a\u{feff}".as_bytes()
        );
    }

    #[test]
    fn text_document_final_raw_budget_includes_bom_and_crlf_expansion() {
        let bom = checked(Utf8TextDocument::decode(UTF8_BOM)).format();
        assert_eq!(
            checked(bom.prepare_save(&"x".repeat(MAX_TEXT_EDIT_BYTES - 3))).len(),
            MAX_TEXT_EDIT_BYTES
        );
        assert_eq!(
            bom.prepare_save(&"x".repeat(MAX_TEXT_EDIT_BYTES - 2)),
            Err(TextDocumentError::Limit)
        );
        let crlf = checked(Utf8TextDocument::decode(b"a\r\n")).format();
        let mut draft = "x".repeat(MAX_TEXT_EDIT_BYTES - 2);
        draft.push('\n');
        assert_eq!(
            checked(crlf.prepare_save(&draft)).len(),
            MAX_TEXT_EDIT_BYTES
        );
        draft.push('x');
        assert_eq!(crlf.prepare_save(&draft), Err(TextDocumentError::Limit));
    }

    #[test]
    fn text_document_save_line_bound_does_not_mark_an_unchanged_read_dirty() {
        let raw = "a\n".repeat(MAX_TEXT_EDIT_LINES + 1);
        let document = checked(Utf8TextDocument::decode(raw.as_bytes()));
        assert!(
            document
                .format()
                .matches_bytes(document.body(), raw.as_bytes())
        );
        assert_eq!(
            document.format().prepare_save(document.body()),
            Err(TextDocumentError::Limit)
        );
        assert!(
            document
                .format()
                .prepare_save(&"a\n".repeat(MAX_TEXT_EDIT_LINES))
                .is_ok()
        );
    }

    #[test]
    fn text_document_rejects_binary_invalid_and_other_encoding_signatures() {
        for (bytes, expected) in [
            (b"\xff".as_slice(), TextDocumentError::InvalidUtf8),
            (b"private\0text", TextDocumentError::Binary),
            (b"\xff\xfea\0", TextDocumentError::UnsupportedEncoding),
            (b"\xfe\xff\0a", TextDocumentError::UnsupportedEncoding),
            (b"\0\0\xfe\xff", TextDocumentError::UnsupportedEncoding),
        ] {
            assert!(matches!(Utf8TextDocument::decode(bytes), Err(error) if error == expected));
            assert!(!expected.to_string().contains("private"));
        }
        assert!(matches!(
            Utf8TextDocument::decode(&vec![b'a'; MAX_TEXT_EDIT_BYTES + 1]),
            Err(TextDocumentError::Limit)
        ));
    }

    #[test]
    fn text_document_format_changes_require_a_new_observation() {
        let format = |raw: &[u8]| checked(Utf8TextDocument::decode(raw)).format();
        assert!(format(b"a").is_compatible_with(format(b"b\n")));
        for raw in [b"a\r\n".as_slice(), b"\xef\xbb\xbfa\n", b"a\r\nb\n", b"a\r"] {
            assert!(!format(b"a\n").is_compatible_with(format(raw)));
        }
        assert!(format(b"a\r\n").is_compatible_with(format(b"B\r\n")));
    }

    #[test]
    fn text_document_continuing_no_break_observation_retains_insertion_policy() {
        for original in [b"a\n".as_slice(), b"a\r\n", b"a\r", b"a\r\nb\n"] {
            let previous = checked(Utf8TextDocument::decode(original)).format();
            for raw in [b"".as_slice(), b"ab", b"\xef\xbb\xbfab"] {
                let observation = checked(Utf8TextDocument::decode(raw)).format();
                assert_eq!(observation.line_ending(), TextLineEnding::None);
                let continued = observation.retain_insertion_policy(previous);
                assert_eq!(continued.line_ending(), previous.line_ending());
                assert_eq!(continued.has_bom(), observation.has_bom());
                let body = if previous.line_ending() == TextLineEnding::CrLf {
                    "a\r\nb"
                } else {
                    "a\nb"
                };
                let expected = format!(
                    "{}{body}",
                    if observation.has_bom() {
                        "\u{feff}"
                    } else {
                        ""
                    }
                );
                assert_eq!(checked(continued.prepare_save("a\nb")), expected.as_bytes());
            }
        }
        let reopened = checked(Utf8TextDocument::decode(b"ab")).format();
        assert_eq!(checked(reopened.prepare_save("a\nb")), b"a\nb");
    }
}
