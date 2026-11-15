//! Lossless document envelopes for bounded, explicitly reviewed remote saves.

use encoding_rs::{DecoderResult, EncoderResult, Encoding};

use crate::{MAX_TEXT_EDIT_BYTES, MAX_TEXT_EDIT_LINES};

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";
const UTF16_LE_BOM: &[u8] = b"\xff\xfe";
const UTF16_BE_BOM: &[u8] = b"\xfe\xff";

/// A checked document encoding; legacy encodings require explicit selection.
///
/// UTF-16 requires a byte-order mark. Legacy encodings use the WHATWG mappings
/// from `encoding_rs` and accept only byte sequences that round-trip exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEncoding {
    /// UTF-8, with or without a leading signature.
    Utf8,
    /// UTF-16 in little-endian byte order, with its original signature.
    Utf16Le,
    /// UTF-16 in big-endian byte order, with its original signature.
    Utf16Be,
    /// GBK, selected explicitly without a Unicode signature.
    Gbk,
    /// GB18030, selected explicitly without a Unicode signature.
    Gb18030,
    /// Big5, selected explicitly without a Unicode signature.
    Big5,
    /// Shift_JIS, selected explicitly without a Unicode signature.
    ShiftJis,
    /// Windows-1252, selected explicitly without a Unicode signature.
    Windows1252,
}

impl TextEncoding {
    /// The encoding name suitable for a format description or save review.
    pub fn name(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Gbk => "GBK",
            Self::Gb18030 => "GB18030",
            Self::Big5 => "Big5",
            Self::ShiftJis => "Shift_JIS",
            Self::Windows1252 => "Windows-1252",
        }
    }

    /// Whether decoding this encoding requires an explicit legacy selection.
    pub fn is_legacy(self) -> bool {
        self.legacy_codec().is_some()
    }

    fn legacy_codec(self) -> Option<&'static Encoding> {
        match self {
            Self::Utf8 | Self::Utf16Le | Self::Utf16Be => None,
            Self::Gbk => Some(encoding_rs::GBK),
            Self::Gb18030 => Some(encoding_rs::GB18030),
            Self::Big5 => Some(encoding_rs::BIG5),
            Self::ShiftJis => Some(encoding_rs::SHIFT_JIS),
            Self::Windows1252 => Some(encoding_rs::WINDOWS_1252),
        }
    }
}

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

/// The original encoding, signature and line-ending policy, separate from text.
///
/// This is transient document state, not permission to change a remote file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextFormat {
    encoding: TextEncoding,
    bom: bool,
    line_ending: TextLineEnding,
}

/// One checked Unicode body, excluding its encoding's leading signature BOM.
///
/// Raw files are decoded strictly into this editable body. Their encoding,
/// byte order and BOM remain in the format for subsequent saves.
/// Callers retain the original raw file snapshot for conflict and backup checks.
#[derive(Clone, PartialEq, Eq)]
pub struct TextDocument {
    body: String,
    format: TextFormat,
}

/// A rejected format operation. Errors never include document contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TextDocumentError {
    /// Input is not valid UTF-8.
    #[error("document is not valid UTF-8")]
    InvalidUtf8,
    /// UTF-16 input has an odd byte count or malformed surrogate sequence.
    #[error("document is not valid UTF-16")]
    InvalidUtf16,
    /// Input is malformed in the explicitly selected legacy encoding.
    #[error("document is not valid in the selected legacy encoding")]
    InvalidLegacyEncoding,
    /// Decodable legacy bytes cannot be reproduced exactly by the same codec.
    #[error("document bytes do not round-trip in the selected legacy encoding")]
    NonCanonicalEncoding,
    /// The draft has an unmappable character or would change when decoded.
    #[error("document cannot be represented losslessly in the selected encoding")]
    Unrepresentable,
    /// An unsupported signature, including UTF-32, was supplied.
    #[error("document has an unsupported encoding signature")]
    UnsupportedEncoding,
    /// The editor does not accept NUL-containing text.
    #[error("document contains NUL")]
    Binary,
    /// Raw bytes, the UTF-8 body or a save result exceed the editor budget.
    #[error("document exceeds the 1 MiB or 4096-line save budget")]
    Limit,
}

impl TextDocument {
    /// Decode complete raw bytes without lossy conversion or newline changes.
    ///
    /// UTF-8 may have one leading BOM; UTF-16 requires an explicit LE or BE BOM.
    /// Exactly one signature is removed. No other encoding is guessed.
    /// Both raw bytes and the decoded UTF-8 body must fit within 1 MiB.
    /// The 4096-line bound applies when preparing a save, not when observing
    /// an unchanged file.
    ///
    /// # Errors
    /// Rejects unsupported signatures, malformed UTF-8 or UTF-16, NUL, and
    /// either raw bytes or a decoded UTF-8 body over 1 MiB.
    pub fn decode(bytes: &[u8]) -> Result<Self, TextDocumentError> {
        if bytes.len() > MAX_TEXT_EDIT_BYTES {
            return Err(TextDocumentError::Limit);
        }
        // UTF-32 LE shares the UTF-16 LE prefix and must be rejected first.
        if bytes.starts_with(b"\xff\xfe\x00\x00") || bytes.starts_with(b"\x00\x00\xfe\xff") {
            return Err(TextDocumentError::UnsupportedEncoding);
        }
        let (encoding, bom, body) = if let Some(bytes) = bytes.strip_prefix(UTF16_LE_BOM) {
            (TextEncoding::Utf16Le, true, decode_utf16(bytes, true)?)
        } else if let Some(bytes) = bytes.strip_prefix(UTF16_BE_BOM) {
            (TextEncoding::Utf16Be, true, decode_utf16(bytes, false)?)
        } else {
            let bom = bytes.starts_with(UTF8_BOM);
            let body = bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes);
            let body = std::str::from_utf8(body).map_err(|_| TextDocumentError::InvalidUtf8)?;
            if body.contains('\0') {
                return Err(TextDocumentError::Binary);
            }
            (TextEncoding::Utf8, bom, body.to_owned())
        };
        let line_ending = line_ending(&body);
        Ok(Self {
            body,
            format: TextFormat {
                encoding,
                bom,
                line_ending,
            },
        })
    }

    /// Decode complete raw bytes using an explicitly selected exact encoding.
    ///
    /// Unicode selections retain [`Self::decode`]'s BOM rules: UTF-8 accepts an
    /// optional signature, and UTF-16 requires the selected byte order's BOM.
    /// A different detected encoding is rejected, without conversion.
    ///
    /// Legacy selections reject leading Unicode BOMs and never sniff another
    /// encoding. Decoding must succeed without replacement, and re-encoding
    /// must reproduce every original byte. Both raw bytes and the decoded
    /// UTF-8 body must fit within 1 MiB; the save-only line bound is unchanged.
    ///
    /// # Errors
    /// Rejects mismatched or unsupported signatures, malformed input, legacy
    /// bytes that do not round-trip, NUL, and either size limit.
    /// Errors never contain document contents.
    pub fn decode_with_encoding(
        bytes: &[u8],
        encoding: TextEncoding,
    ) -> Result<Self, TextDocumentError> {
        if let Some(codec) = encoding.legacy_codec() {
            let body = decode_legacy(bytes, codec)?;
            let line_ending = line_ending(&body);
            return Ok(Self {
                body,
                format: TextFormat {
                    encoding,
                    bom: false,
                    line_ending,
                },
            });
        }
        let document = Self::decode(bytes)?;
        if document.format.encoding != encoding {
            return Err(TextDocumentError::UnsupportedEncoding);
        }
        Ok(document)
    }

    /// Exact editable body, with internal U+FEFF and all line terminators retained.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Encoding, signature and line-ending policy of the original raw file.
    pub fn format(&self) -> TextFormat {
        self.format
    }
}

impl TextFormat {
    /// The checked encoding and, for UTF-16, the original byte order.
    pub fn encoding(self) -> TextEncoding {
        self.encoding
    }

    /// Whether the raw file has its encoding's leading signature.
    ///
    /// This is always true for UTF-16, optional for UTF-8 and false for legacy
    /// encodings.
    pub fn has_bom(self) -> bool {
        self.bom
    }

    /// The known terminator profile, retained across saves without line breaks.
    ///
    /// Mixed files are never majority-normalized.
    pub fn line_ending(self) -> TextLineEnding {
        self.line_ending
    }

    /// Replace only the observed line-ending profile while retaining the
    /// authoritative encoding and BOM envelope.
    pub fn with_line_ending(self, line_ending: TextLineEnding) -> Self {
        Self {
            line_ending,
            ..self
        }
    }

    /// Decode bytes using this already observed format.
    ///
    /// For Unicode formats, this differs from [`TextDocument::decode`] for an
    /// important UTF-8 edge case: a BOM-less document may begin with U+FEFF as
    /// editable content, whose bytes are indistinguishable from a UTF-8 BOM.
    /// The observed format is authoritative, so a BOM-less UTF-8 format keeps
    /// that leading character in the returned body.
    /// A known legacy format rejects Unicode BOMs and requires the same strict
    /// byte round-trip as [`TextDocument::decode_with_encoding`].
    ///
    /// # Errors
    /// Rejects bytes whose encoding signature does not match this format,
    /// malformed text, legacy bytes that do not round-trip, NUL, and either
    /// the raw or decoded-body size limit.
    pub fn decode_body(self, bytes: &[u8]) -> Result<String, TextDocumentError> {
        if bytes.len() > MAX_TEXT_EDIT_BYTES {
            return Err(TextDocumentError::Limit);
        }
        match self.encoding {
            TextEncoding::Utf8 => {
                let bytes = if self.bom {
                    bytes
                        .strip_prefix(UTF8_BOM)
                        .ok_or(TextDocumentError::UnsupportedEncoding)?
                } else {
                    bytes
                };
                let body =
                    std::str::from_utf8(bytes).map_err(|_| TextDocumentError::InvalidUtf8)?;
                if body.contains('\0') {
                    return Err(TextDocumentError::Binary);
                }
                Ok(body.to_owned())
            }
            TextEncoding::Utf16Le => {
                let bytes = bytes
                    .strip_prefix(UTF16_LE_BOM)
                    .ok_or(TextDocumentError::UnsupportedEncoding)?;
                decode_utf16(bytes, true)
            }
            TextEncoding::Utf16Be => {
                let bytes = bytes
                    .strip_prefix(UTF16_BE_BOM)
                    .ok_or(TextDocumentError::UnsupportedEncoding)?;
                decode_utf16(bytes, false)
            }
            encoding => {
                let codec = encoding
                    .legacy_codec()
                    .ok_or(TextDocumentError::UnsupportedEncoding)?;
                decode_legacy(bytes, codec)
            }
        }
    }

    /// Retain a known insertion policy while continuing the same open document.
    ///
    /// An observation without terminators cannot select a different policy.
    /// Keep the previous profile in that case only for the same encoding.
    /// Encoding and BOM always belong to this new observation. An explicit new
    /// read uses its decoded format directly.
    pub fn retain_insertion_policy(self, previous: Self) -> Self {
        Self {
            line_ending: if self.encoding == previous.encoding
                && self.line_ending == TextLineEnding::None
            {
                previous.line_ending
            } else {
                self.line_ending
            },
            ..self
        }
    }

    /// Whether two observations permit ordinary content merging.
    ///
    /// No-break and LF files share the LF insertion policy. A changed encoding,
    /// BOM, CRLF policy or verbatim profile requires an explicit new read and review.
    pub fn is_compatible_with(self, other: Self) -> bool {
        let policy = |ending| match ending {
            TextLineEnding::None => TextLineEnding::Lf,
            ending => ending,
        };
        self.encoding == other.encoding
            && self.bom == other.bom
            && policy(self.line_ending) == policy(other.line_ending)
    }

    /// Prepare the exact bytes that must appear in a save approval and readback.
    ///
    /// LF/CRLF documents retain their original policy, including inserted LF.
    /// Standalone CR bytes are never discarded. Mixed and CR-only documents
    /// remain verbatim. No final newline is added or removed. The original
    /// encoding, byte order and BOM are preserved.
    ///
    /// # Errors
    /// Rejects NUL, over 4096 LF-delimited lines, a UTF-8 draft over 1 MiB, or
    /// final raw bytes over 1 MiB including BOM, CRLF and encoding expansion.
    /// Legacy output must also decode to the exact normalized Unicode draft,
    /// whose UTF-8 size is independently bounded. Unmappable characters and
    /// lossy mappings are rejected, without replacement or character references.
    /// It never returns a truncated result.
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
        if let Some(codec) = self.encoding.legacy_codec() {
            if self.bom {
                return Err(TextDocumentError::UnsupportedEncoding);
            }
            // Use the existing line policy before encoding so the final
            // losslessness check covers the complete normalized Unicode draft.
            let normalized = Self {
                encoding: TextEncoding::Utf8,
                ..self
            }
            .encode(draft, check_lines)?;
            let normalized =
                std::str::from_utf8(&normalized).map_err(|_| TextDocumentError::InvalidUtf8)?;
            let result = encode_legacy_body(normalized, codec)?;
            // Some codecs best-fit characters without reporting an unmappable
            // character (notably Shift_JIS yen, overline and minus mappings).
            if decode_legacy_body(&result, codec)? != normalized {
                return Err(TextDocumentError::Unrepresentable);
            }
            return Ok(result);
        }
        let mut result = Vec::with_capacity(draft.len().saturating_add(3).min(MAX_TEXT_EDIT_BYTES));
        if self.bom {
            let signature = match self.encoding {
                TextEncoding::Utf8 => UTF8_BOM,
                TextEncoding::Utf16Le => UTF16_LE_BOM,
                TextEncoding::Utf16Be => UTF16_BE_BOM,
                _ => return Err(TextDocumentError::UnsupportedEncoding),
            };
            extend(&mut result, signature)?;
        }
        let normalized = matches!(
            self.line_ending,
            TextLineEnding::None | TextLineEnding::Lf | TextLineEnding::CrLf
        );
        for (index, line) in draft.split_inclusive('\n').enumerate() {
            if check_lines && index >= MAX_TEXT_EDIT_LINES {
                return Err(TextDocumentError::Limit);
            }
            if normalized && let Some(body) = line.strip_suffix('\n') {
                let body = body.strip_suffix('\r').unwrap_or(body);
                extend_text(&mut result, body, self.encoding)?;
                extend_text(
                    &mut result,
                    if self.line_ending == TextLineEnding::CrLf {
                        "\r\n"
                    } else {
                        "\n"
                    },
                    self.encoding,
                )?;
            } else {
                extend_text(&mut result, line, self.encoding)?;
            }
        }
        Ok(result)
    }
}

fn decode_legacy(bytes: &[u8], codec: &'static Encoding) -> Result<String, TextDocumentError> {
    let body = decode_legacy_body(bytes, codec)?;
    match encode_legacy_body(&body, codec) {
        Ok(encoded) if encoded == bytes => Ok(body),
        _ => Err(TextDocumentError::NonCanonicalEncoding),
    }
}

fn decode_legacy_body(bytes: &[u8], codec: &'static Encoding) -> Result<String, TextDocumentError> {
    if bytes.len() > MAX_TEXT_EDIT_BYTES {
        return Err(TextDocumentError::Limit);
    }
    if bytes.starts_with(UTF8_BOM)
        || bytes.starts_with(UTF16_LE_BOM)
        || bytes.starts_with(UTF16_BE_BOM)
        || bytes.starts_with(b"\x00\x00\xfe\xff")
    {
        return Err(TextDocumentError::UnsupportedEncoding);
    }
    let mut decoder = codec.new_decoder_without_bom_handling();
    let mut remaining = bytes;
    let mut body = String::with_capacity(bytes.len());
    let mut buffer = [0_u8; 4096];
    loop {
        let (result, read, written) =
            decoder.decode_to_utf8_without_replacement(remaining, &mut buffer, true);
        let decoded = std::str::from_utf8(&buffer[..written])
            .map_err(|_| TextDocumentError::InvalidLegacyEncoding)?;
        if body.len().saturating_add(decoded.len()) > MAX_TEXT_EDIT_BYTES {
            return Err(TextDocumentError::Limit);
        }
        if decoded.contains('\0') {
            return Err(TextDocumentError::Binary);
        }
        body.push_str(decoded);
        remaining = &remaining[read..];
        match result {
            DecoderResult::InputEmpty => return Ok(body),
            DecoderResult::OutputFull => {}
            DecoderResult::Malformed(_, _) => return Err(TextDocumentError::InvalidLegacyEncoding),
        }
    }
}

fn encode_legacy_body(text: &str, codec: &'static Encoding) -> Result<Vec<u8>, TextDocumentError> {
    let mut encoder = codec.new_encoder();
    let mut remaining = text;
    let mut output = Vec::with_capacity(text.len());
    let mut buffer = [0_u8; 4096];
    loop {
        let (result, read, written) =
            encoder.encode_from_utf8_without_replacement(remaining, &mut buffer, true);
        extend(&mut output, &buffer[..written])?;
        remaining = &remaining[read..];
        match result {
            EncoderResult::InputEmpty => return Ok(output),
            EncoderResult::OutputFull => {}
            EncoderResult::Unmappable(_) => return Err(TextDocumentError::Unrepresentable),
        }
    }
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, TextDocumentError> {
    let (pairs, remainder) = bytes.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(TextDocumentError::InvalidUtf16);
    }
    let units = pairs.iter().map(|&pair| {
        if little_endian {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        }
    });
    let mut body = String::with_capacity(bytes.len());
    for character in char::decode_utf16(units) {
        let character = character.map_err(|_| TextDocumentError::InvalidUtf16)?;
        if character == '\0' {
            return Err(TextDocumentError::Binary);
        }
        if body.len().saturating_add(character.len_utf8()) > MAX_TEXT_EDIT_BYTES {
            return Err(TextDocumentError::Limit);
        }
        body.push(character);
    }
    Ok(body)
}

fn extend_text(
    output: &mut Vec<u8>,
    text: &str,
    encoding: TextEncoding,
) -> Result<(), TextDocumentError> {
    match encoding {
        TextEncoding::Utf8 => extend(output, text.as_bytes()),
        TextEncoding::Utf16Le | TextEncoding::Utf16Be => {
            for unit in text.encode_utf16() {
                let bytes = if encoding == TextEncoding::Utf16Le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                };
                extend(output, &bytes)?;
            }
            Ok(())
        }
        _ => Err(TextDocumentError::UnsupportedEncoding),
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
mod legacy_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn checked<T>(result: Result<T, TextDocumentError>) -> T {
        result.unwrap_or_else(|error| panic!("text document fixture: {error}"))
    }

    fn raw_with_bom(body: &str, encoding: TextEncoding) -> Vec<u8> {
        match encoding {
            TextEncoding::Utf8 => [UTF8_BOM, body.as_bytes()].concat(),
            TextEncoding::Utf16Le => UTF16_LE_BOM
                .iter()
                .copied()
                .chain(body.encode_utf16().flat_map(u16::to_le_bytes))
                .collect(),
            TextEncoding::Utf16Be => UTF16_BE_BOM
                .iter()
                .copied()
                .chain(body.encode_utf16().flat_map(u16::to_be_bytes))
                .collect(),
            _ => panic!("raw_with_bom is a Unicode-only fixture"),
        }
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
            let document = checked(TextDocument::decode(raw));
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
            let document = checked(TextDocument::decode(raw.as_bytes()));
            assert_eq!(
                checked(document.format().prepare_save(draft)),
                expected.as_bytes()
            );
        }
    }

    #[test]
    fn text_document_only_one_signature_is_removed_and_internal_feff_remains() {
        let document = checked(TextDocument::decode("\u{feff}\u{feff}a\u{feff}".as_bytes()));
        assert!(document.format().has_bom());
        assert_eq!(document.body(), "\u{feff}a\u{feff}");
        assert_eq!(
            checked(document.format().prepare_save(document.body())),
            "\u{feff}\u{feff}a\u{feff}".as_bytes()
        );
    }

    #[test]
    fn text_document_final_raw_budget_includes_bom_and_crlf_expansion() {
        let bom = checked(TextDocument::decode(UTF8_BOM)).format();
        assert_eq!(
            checked(bom.prepare_save(&"x".repeat(MAX_TEXT_EDIT_BYTES - 3))).len(),
            MAX_TEXT_EDIT_BYTES
        );
        assert_eq!(
            bom.prepare_save(&"x".repeat(MAX_TEXT_EDIT_BYTES - 2)),
            Err(TextDocumentError::Limit)
        );
        let crlf = checked(TextDocument::decode(b"a\r\n")).format();
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
        let document = checked(TextDocument::decode(raw.as_bytes()));
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
            (b"\xff\xfe\0\0", TextDocumentError::UnsupportedEncoding),
            (b"\0\0\xfe\xff", TextDocumentError::UnsupportedEncoding),
        ] {
            assert!(matches!(TextDocument::decode(bytes), Err(error) if error == expected));
            assert!(!expected.to_string().contains("private"));
        }
        assert!(matches!(
            TextDocument::decode(&vec![b'a'; MAX_TEXT_EDIT_BYTES + 1]),
            Err(TextDocumentError::Limit)
        ));
    }

    #[test]
    fn text_document_format_changes_require_a_new_observation() {
        let format = |raw: &[u8]| checked(TextDocument::decode(raw)).format();
        assert!(format(b"a").is_compatible_with(format(b"b\n")));
        for raw in [b"a\r\n".as_slice(), b"\xef\xbb\xbfa\n", b"a\r\nb\n", b"a\r"] {
            assert!(!format(b"a\n").is_compatible_with(format(raw)));
        }
        assert!(format(b"a\r\n").is_compatible_with(format(b"B\r\n")));
    }

    #[test]
    fn text_document_continuing_no_break_observation_retains_insertion_policy() {
        for original in [b"a\n".as_slice(), b"a\r\n", b"a\r", b"a\r\nb\n"] {
            let previous = checked(TextDocument::decode(original)).format();
            for raw in [b"".as_slice(), b"ab", b"\xef\xbb\xbfab"] {
                let observation = checked(TextDocument::decode(raw)).format();
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
        let reopened = checked(TextDocument::decode(b"ab")).format();
        assert_eq!(checked(reopened.prepare_save("a\nb")), b"a\nb");
    }

    #[test]
    fn text_document_encoding_names_and_utf16_known_bytes_preserve_byte_order() {
        assert_eq!(TextEncoding::Utf8.name(), "UTF-8");
        for (encoding, name, raw) in [
            (
                TextEncoding::Utf16Le,
                "UTF-16 LE",
                b"\xff\xfe\x2d\x4e\x87\x65\x3d\xd8\x00\xde\xff\xfe\r\0\n\0Z\0".as_slice(),
            ),
            (
                TextEncoding::Utf16Be,
                "UTF-16 BE",
                b"\xfe\xff\x4e\x2d\x65\x87\xd8\x3d\xde\x00\xfe\xff\0\r\0\n\0Z",
            ),
        ] {
            let document = checked(TextDocument::decode(raw));
            assert_eq!(document.body(), "中文😀\u{feff}\r\nZ");
            assert_eq!(document.format().encoding(), encoding);
            assert_eq!(encoding.name(), name);
            assert!(document.format().has_bom());
            assert_eq!(document.format().line_ending(), TextLineEnding::CrLf);
            assert_eq!(
                checked(document.format().prepare_save(document.body())),
                raw
            );
            assert!(document.format().matches_bytes(document.body(), raw));
        }
    }

    #[test]
    fn text_format_decode_body_uses_observed_bom_and_preserves_leading_feff_content() {
        let bomless_utf8 = checked(TextDocument::decode(b"plain")).format();
        assert_eq!(
            checked(bomless_utf8.decode_body("\u{feff}body".as_bytes())),
            "\u{feff}body"
        );

        let bom_utf8 = checked(TextDocument::decode(UTF8_BOM)).format();
        assert_eq!(checked(bom_utf8.decode_body(b"\xef\xbb\xbfbody")), "body");

        for encoding in [TextEncoding::Utf16Le, TextEncoding::Utf16Be] {
            let format = checked(TextDocument::decode(&raw_with_bom("", encoding))).format();
            let raw = raw_with_bom("\u{feff}body", encoding);
            assert_eq!(checked(format.decode_body(&raw)), "\u{feff}body");
        }
    }

    #[test]
    fn text_document_utf16_round_trip_removes_only_one_bom_and_keeps_all_line_profiles() {
        for encoding in [TextEncoding::Utf16Le, TextEncoding::Utf16Be] {
            for body in [
                "",
                "中文😀",
                "\u{feff}",
                "\u{feff}\u{feff}inside\u{feff}\r\nlast",
                "a\nb\n",
                "a\r\nb\r\n",
                "a\r\nb\nlast",
                "a\rb\r",
            ] {
                let raw = raw_with_bom(body, encoding);
                let document = checked(TextDocument::decode(&raw));
                assert_eq!(document.body(), body);
                assert_eq!(
                    checked(document.format().prepare_save(document.body())),
                    raw
                );
                assert!(document.format().matches_bytes(document.body(), &raw));
            }
        }
    }

    #[test]
    fn text_document_utf16_saves_follow_existing_newline_policy_and_eof() {
        for encoding in [TextEncoding::Utf16Le, TextEncoding::Utf16Be] {
            for (original, draft, expected) in [
                ("a\r\nb", "a\r\n😀\nb", "a\r\n😀\r\nb"),
                ("a\nb", "a\r\n😀\nb", "a\n😀\nb"),
                ("single", "single\n😀", "single\n😀"),
                ("a\r\nb\n", "a\r\n😀\nb\n", "a\r\n😀\nb\n"),
                ("a\rb\r", "a\rb\r😀\n", "a\rb\r😀\n"),
            ] {
                let document = checked(TextDocument::decode(&raw_with_bom(original, encoding)));
                assert_eq!(
                    checked(document.format().prepare_save(draft)),
                    raw_with_bom(expected, encoding)
                );
            }
        }
    }

    #[test]
    fn text_document_bomless_input_is_utf8_and_never_guessed_as_utf16() {
        let document = checked(TextDocument::decode(b"-N"));
        assert_eq!(document.body(), "-N");
        assert_eq!(document.format().encoding(), TextEncoding::Utf8);
        assert!(!document.format().has_bom());
        for raw in [b"a\0".as_slice(), b"\0a"] {
            assert!(matches!(
                TextDocument::decode(raw),
                Err(TextDocumentError::Binary)
            ));
        }
        for raw in [b"\x3d\xd8\x00\xde".as_slice(), b"\xd8\x3d\xde\x00"] {
            assert!(matches!(
                TextDocument::decode(raw),
                Err(TextDocumentError::InvalidUtf8)
            ));
        }
    }

    #[test]
    fn text_document_utf32_boms_are_rejected_before_utf16_classification() {
        for raw in [
            b"\xff\xfe\0\0".as_slice(),
            b"\xff\xfe\0\0a\0\0\0",
            b"\0\0\xfe\xff",
            b"\0\0\xfe\xff\0\0\0a",
        ] {
            assert!(matches!(
                TextDocument::decode(raw),
                Err(TextDocumentError::UnsupportedEncoding)
            ));
        }
    }

    #[test]
    fn text_document_utf16_rejects_odd_bytes_and_malformed_surrogates_without_contents() {
        for encoding in [TextEncoding::Utf16Le, TextEncoding::Utf16Be] {
            for units in [
                vec![0xd800_u16],
                vec![0xdbff],
                vec![0xdc00],
                vec![0xdfff],
                vec![0xdc00, 0xd800],
                vec![0xd800, 0x0061],
                vec![0xd800, 0xd800],
                vec![0xd800, 0xdc00, 0xdc00],
            ] {
                let mut raw = raw_with_bom("private", encoding);
                for unit in units {
                    raw.extend_from_slice(&if encoding == TextEncoding::Utf16Le {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    });
                }
                assert!(matches!(
                    TextDocument::decode(&raw),
                    Err(TextDocumentError::InvalidUtf16)
                ));
            }
            for body in ["", "private"] {
                let mut raw = raw_with_bom(body, encoding);
                raw.push(b'x');
                assert!(matches!(
                    TextDocument::decode(&raw),
                    Err(TextDocumentError::InvalidUtf16)
                ));
            }
        }
        assert_eq!(
            TextDocumentError::InvalidUtf16.to_string(),
            "document is not valid UTF-16"
        );
        assert_eq!(
            format!("{:?}", TextDocumentError::InvalidUtf16),
            "InvalidUtf16"
        );
    }

    #[test]
    fn text_document_all_encodings_reject_nul_on_read_save_and_comparison() {
        for encoding in [
            TextEncoding::Utf8,
            TextEncoding::Utf16Le,
            TextEncoding::Utf16Be,
        ] {
            let raw = raw_with_bom("private\0text", encoding);
            assert!(matches!(
                TextDocument::decode(&raw),
                Err(TextDocumentError::Binary)
            ));
            let format = checked(TextDocument::decode(&raw_with_bom("", encoding))).format();
            assert_eq!(
                format.prepare_save("private\0text"),
                Err(TextDocumentError::Binary)
            );
            assert!(!format.matches_bytes("private\0text", &raw));
        }
    }

    #[test]
    fn text_document_utf8_never_replaces_malformed_or_partial_sequences() {
        for body in [
            b"\xc0\x80".as_slice(),
            b"\xed\xa0\x80",
            b"\xf4\x90\x80\x80",
            b"\xef\xbb",
        ] {
            for signature in [b"".as_slice(), UTF8_BOM] {
                assert!(matches!(
                    TextDocument::decode(&[signature, body].concat()),
                    Err(TextDocumentError::InvalidUtf8)
                ));
            }
        }
    }

    #[test]
    fn text_document_utf16_raw_budget_includes_bom_and_newline_expansion() {
        for encoding in [TextEncoding::Utf16Le, TextEncoding::Utf16Be] {
            let format = checked(TextDocument::decode(&raw_with_bom("", encoding))).format();
            let draft = "x".repeat((MAX_TEXT_EDIT_BYTES - 2) / 2);
            let raw = checked(format.prepare_save(&draft));
            assert_eq!(raw.len(), MAX_TEXT_EDIT_BYTES);
            assert_eq!(checked(TextDocument::decode(&raw)).body(), draft);
            assert!(format.matches_bytes(&draft, &raw));
            let oversized_draft = format!("{draft}x");
            let oversized_raw = raw_with_bom(&oversized_draft, encoding);
            assert_eq!(
                format.prepare_save(&oversized_draft),
                Err(TextDocumentError::Limit)
            );
            assert!(!format.matches_bytes(&oversized_draft, &oversized_raw));
            assert!(matches!(
                TextDocument::decode(&oversized_raw),
                Err(TextDocumentError::Limit)
            ));

            let format = checked(TextDocument::decode(&raw_with_bom("a\r\n", encoding))).format();
            let draft = format!("{}\n", "x".repeat((MAX_TEXT_EDIT_BYTES - 2) / 2 - 2));
            let raw = checked(format.prepare_save(&draft));
            assert_eq!(raw.len(), MAX_TEXT_EDIT_BYTES);
            assert!(raw.ends_with(&raw_with_bom("\r\n", encoding)[2..]));
            assert_eq!(
                format.prepare_save(&format!("{draft}x")),
                Err(TextDocumentError::Limit)
            );
        }
    }

    #[test]
    fn text_document_utf16_decoded_body_and_save_draft_have_independent_utf8_budget() {
        for encoding in [TextEncoding::Utf16Le, TextEncoding::Utf16Be] {
            let mut body = "界".repeat(MAX_TEXT_EDIT_BYTES / 3);
            body.push_str(&"x".repeat(MAX_TEXT_EDIT_BYTES % 3));
            let raw = raw_with_bom(&body, encoding);
            assert!(raw.len() < MAX_TEXT_EDIT_BYTES);
            let document = checked(TextDocument::decode(&raw));
            assert_eq!(document.body().len(), MAX_TEXT_EDIT_BYTES);
            assert_eq!(checked(document.format().prepare_save(&body)), raw);
            body.push('x');
            let oversized = raw_with_bom(&body, encoding);
            assert!(oversized.len() < MAX_TEXT_EDIT_BYTES);
            assert!(matches!(
                TextDocument::decode(&oversized),
                Err(TextDocumentError::Limit)
            ));
            assert_eq!(
                document.format().prepare_save(&body),
                Err(TextDocumentError::Limit)
            );
            assert!(!document.format().matches_bytes(&body, &oversized));
        }
    }

    #[test]
    fn text_document_utf16_line_bound_keeps_unchanged_observation_clean() {
        for encoding in [TextEncoding::Utf16Le, TextEncoding::Utf16Be] {
            let body = "😀\r\n".repeat(MAX_TEXT_EDIT_LINES + 1);
            let raw = raw_with_bom(&body, encoding);
            let document = checked(TextDocument::decode(&raw));
            assert!(document.format().matches_bytes(&body, &raw));
            assert_eq!(
                document.format().prepare_save(&body),
                Err(TextDocumentError::Limit)
            );
            assert!(
                document
                    .format()
                    .prepare_save(&"😀\n".repeat(MAX_TEXT_EDIT_LINES))
                    .is_ok()
            );
            assert_eq!(
                document
                    .format()
                    .prepare_save(&format!("{}tail", "😀\n".repeat(MAX_TEXT_EDIT_LINES))),
                Err(TextDocumentError::Limit)
            );
        }
    }

    #[test]
    fn text_document_format_compatibility_checks_encoding_bom_and_line_policy() {
        let encodings = [
            TextEncoding::Utf8,
            TextEncoding::Utf16Le,
            TextEncoding::Utf16Be,
        ];
        for encoding in encodings {
            let format =
                |body| checked(TextDocument::decode(&raw_with_bom(body, encoding))).format();
            assert!(format("single").is_compatible_with(format("a\n")));
            assert!(format("a\r\n").is_compatible_with(format("b\r\n")));
            for body in ["a\r\n", "a\r", "a\r\nb\n"] {
                assert!(!format("a\n").is_compatible_with(format(body)));
            }
            for other_encoding in encodings {
                if encoding == other_encoding {
                    continue;
                }
                let other =
                    checked(TextDocument::decode(&raw_with_bom("a\n", other_encoding))).format();
                assert!(!format("a\n").is_compatible_with(other));
            }
        }
        let plain = checked(TextDocument::decode(b"a\n")).format();
        let bom = checked(TextDocument::decode(&raw_with_bom(
            "a\n",
            TextEncoding::Utf8,
        )))
        .format();
        assert!(!plain.is_compatible_with(bom));
    }

    #[test]
    fn text_document_utf16_continuing_empty_and_plain_saves_retain_original_policy() {
        for encoding in [TextEncoding::Utf16Le, TextEncoding::Utf16Be] {
            for original in ["a\n", "a\r\n", "a\r", "a\r\nb\n"] {
                let document = checked(TextDocument::decode(&raw_with_bom(original, encoding)));
                let mut format = document.format();
                for draft in ["", "plain", "", "😀"] {
                    let saved = checked(format.prepare_save(draft));
                    assert_eq!(saved, raw_with_bom(draft, encoding));
                    let observation = checked(TextDocument::decode(&saved));
                    assert_eq!(observation.format().line_ending(), TextLineEnding::None);
                    format = observation.format().retain_insertion_policy(format);
                    assert_eq!(format.encoding(), encoding);
                    assert!(format.has_bom());
                    assert_eq!(format.line_ending(), document.format().line_ending());
                    assert!(format.matches_bytes(draft, &saved));
                }
                let expected = if format.line_ending() == TextLineEnding::CrLf {
                    "😀\r\nnext"
                } else {
                    "😀\nnext"
                };
                assert_eq!(
                    checked(format.prepare_save("😀\nnext")),
                    raw_with_bom(expected, encoding)
                );
                let reopened = checked(TextDocument::decode(&raw_with_bom("", encoding))).format();
                assert_eq!(
                    checked(reopened.prepare_save("a\nb")),
                    raw_with_bom("a\nb", encoding)
                );
            }
        }
    }

    #[test]
    fn text_document_insertion_policy_does_not_cross_encoding_changes() {
        let encodings = [
            TextEncoding::Utf8,
            TextEncoding::Utf16Le,
            TextEncoding::Utf16Be,
        ];
        for encoding in encodings {
            let previous = checked(TextDocument::decode(&raw_with_bom("a\r\n", encoding))).format();
            for other_encoding in encodings {
                if encoding == other_encoding {
                    continue;
                }
                let observation =
                    checked(TextDocument::decode(&raw_with_bom("", other_encoding))).format();
                let continued = observation.retain_insertion_policy(previous);
                assert_eq!(continued, observation);
                assert_eq!(
                    checked(continued.prepare_save("a\nb")),
                    raw_with_bom("a\nb", other_encoding)
                );
            }
        }
    }
}
