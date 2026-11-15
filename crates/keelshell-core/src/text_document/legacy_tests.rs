#![cfg(test)]

use super::*;

const LEGACY_ENCODINGS: [TextEncoding; 5] = [
    TextEncoding::Gbk,
    TextEncoding::Gb18030,
    TextEncoding::Big5,
    TextEncoding::ShiftJis,
    TextEncoding::Windows1252,
];

const UNICODE_ENCODINGS: [TextEncoding; 3] = [
    TextEncoding::Utf8,
    TextEncoding::Utf16Le,
    TextEncoding::Utf16Be,
];

// Each character has three UTF-8 bytes, independently of its source width.
const EXPANDING_FIXTURES: [(TextEncoding, &[u8], &str); 5] = [
    (TextEncoding::Gbk, b"\xd6\xd0", "中"),
    (TextEncoding::Gb18030, b"\xd6\xd0", "中"),
    (TextEncoding::Big5, b"\xa4\xa4", "中"),
    (TextEncoding::ShiftJis, b"\xa6", "ｦ"),
    (TextEncoding::Windows1252, b"\x80", "€"),
];

fn checked<T>(result: Result<T, TextDocumentError>) -> T {
    result.unwrap_or_else(|error| panic!("text codec fixture: {error}"))
}

fn rejected<T>(result: Result<T, TextDocumentError>) -> TextDocumentError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("text codec fixture unexpectedly accepted"),
    }
}

fn legacy_format(encoding: TextEncoding) -> TextFormat {
    checked(TextDocument::decode_with_encoding(b"", encoding)).format()
}

fn assert_fixture(encoding: TextEncoding, raw: &[u8], body: &str) {
    let document = checked(TextDocument::decode_with_encoding(raw, encoding));
    assert_eq!(document.body(), body);
    assert_eq!(document.format().encoding(), encoding);
    assert!(!document.format().has_bom());
    assert_eq!(checked(document.format().decode_body(raw)), body);
    assert_eq!(checked(document.format().prepare_save(body)), raw);
    assert!(document.format().matches_bytes(body, raw));
}

fn unicode_bytes(body: &str, encoding: TextEncoding) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => body.as_bytes().to_vec(),
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
        _ => panic!("unicode_bytes requires a Unicode encoding"),
    }
}

#[test]
fn legacy_encoding_names_and_classification_are_explicit() {
    for (encoding, name) in [
        (TextEncoding::Gbk, "GBK"),
        (TextEncoding::Gb18030, "GB18030"),
        (TextEncoding::Big5, "Big5"),
        (TextEncoding::ShiftJis, "Shift_JIS"),
        (TextEncoding::Windows1252, "Windows-1252"),
    ] {
        assert_eq!(encoding.name(), name);
        assert!(encoding.is_legacy());
    }
    for encoding in UNICODE_ENCODINGS {
        assert!(!encoding.is_legacy());
    }
}

#[test]
fn gbk_known_chinese_and_euro_bytes_round_trip() {
    assert_fixture(
        TextEncoding::Gbk,
        b"\xd6\xd0\xce\xc4 \x80\r\nlast",
        "中文 €\r\nlast",
    );
}

#[test]
fn gb18030_known_chinese_and_four_byte_scalar_round_trip() {
    assert_fixture(
        TextEncoding::Gb18030,
        b"\xd6\xd0\xce\xc4 \x94\x39\xda\x33 \x81\x30\x81\x30\r\nlast",
        "中文 💩 \u{0080}\r\nlast",
    );
}

#[test]
fn big5_known_chinese_bytes_round_trip() {
    assert_fixture(
        TextEncoding::Big5,
        b"\xa4\xa4\xa4\xe5\r\nlast",
        "中文\r\nlast",
    );
}

#[test]
fn shift_jis_known_japanese_and_halfwidth_bytes_round_trip() {
    assert_fixture(
        TextEncoding::ShiftJis,
        b"\x93\xfa\x96\x7b\x8c\xea\xa6\r\nlast\\~",
        "日本語ｦ\r\nlast\\~",
    );
}

#[test]
fn windows1252_known_accents_quotes_and_euro_round_trip() {
    assert_fixture(
        TextEncoding::Windows1252,
        b"caf\xe9 \x93quote\x94 \x80\r\nlast",
        "café “quote” €\r\nlast",
    );
}

#[test]
fn legacy_empty_and_ascii_documents_keep_the_explicit_encoding() {
    for encoding in LEGACY_ENCODINGS {
        for (raw, body) in [
            (b"".as_slice(), ""),
            (b"plain", "plain"),
            (b"a\r\nlast", "a\r\nlast"),
        ] {
            assert_fixture(encoding, raw, body);
        }
    }
}

#[test]
fn default_decoder_stays_unicode_only_after_legacy_support() {
    for raw in [
        b"\xd6\xd0\xce\xc4".as_slice(),
        b"\x94\x39\xda\x33",
        b"\xa4\xa4\xa4\xe5",
        b"\x93\xfa\x96\x7b",
        b"caf\xe9 \x80",
    ] {
        assert_eq!(
            rejected(TextDocument::decode(raw)),
            TextDocumentError::InvalidUtf8
        );
    }
    let document = checked(TextDocument::decode(b"plain"));
    assert_eq!(document.format().encoding(), TextEncoding::Utf8);
}

#[test]
fn explicit_legacy_selection_does_not_prefer_valid_utf8() {
    let raw = b"\xc3\xa9";
    assert_fixture(TextEncoding::Windows1252, raw, "Ã©");
    assert_eq!(checked(TextDocument::decode(raw)).body(), "é");
}

#[test]
fn legacy_reads_and_known_formats_reject_all_unicode_boms() {
    for encoding in LEGACY_ENCODINGS {
        let format = legacy_format(encoding);
        for raw in [
            UTF8_BOM,
            b"\xef\xbb\xbfplain",
            UTF16_LE_BOM,
            b"\xff\xfea\0",
            UTF16_BE_BOM,
            b"\xfe\xff\0a",
            b"\xff\xfe\0\0",
            b"\xff\xfe\0\0a\0\0\0",
            b"\0\0\xfe\xff",
            b"\0\0\xfe\xff\0\0\0a",
        ] {
            assert_eq!(
                rejected(TextDocument::decode_with_encoding(raw, encoding)),
                TextDocumentError::UnsupportedEncoding
            );
            assert_eq!(
                format.decode_body(raw),
                Err(TextDocumentError::UnsupportedEncoding)
            );
            assert!(!format.matches_bytes("plain", raw));
        }
    }
}

#[test]
fn legacy_keeps_internal_signature_bytes_as_selected_codec_content() {
    assert_fixture(
        TextEncoding::Windows1252,
        b"a\xef\xbb\xbf\xff\xfe\xfe\xff",
        "aï»¿ÿþþÿ",
    );
}

#[test]
fn legacy_multibyte_decoding_rejects_malformed_and_truncated_sequences() {
    for (encoding, raw) in [
        (TextEncoding::Gbk, b"\x81".as_slice()),
        (TextEncoding::Gbk, b"\x81\x7f"),
        (TextEncoding::Gbk, b"\xff"),
        (TextEncoding::Gb18030, b"\x81"),
        (TextEncoding::Gb18030, b"\x81\x30"),
        (TextEncoding::Gb18030, b"\x81\x30\x81"),
        (TextEncoding::Gb18030, b"\xe3\x32\x9a\x36"),
        (TextEncoding::Big5, b"\xa4"),
        (TextEncoding::Big5, b"\x81\x40"),
        (TextEncoding::Big5, b"\xff"),
        (TextEncoding::ShiftJis, b"\x82"),
        (TextEncoding::ShiftJis, b"\x82\x7f"),
        (TextEncoding::ShiftJis, b"\xa0"),
        (TextEncoding::ShiftJis, b"\xfc\x4c"),
    ] {
        assert_eq!(
            rejected(TextDocument::decode_with_encoding(raw, encoding)),
            TextDocumentError::InvalidLegacyEncoding
        );
        assert_eq!(
            legacy_format(encoding).decode_body(raw),
            Err(TextDocumentError::InvalidLegacyEncoding)
        );
    }
}

#[test]
fn windows1252_defined_c1_control_mappings_are_not_replacement_errors() {
    assert_fixture(
        TextEncoding::Windows1252,
        b"\x81\x8d\x8f\x90\x9d",
        "\u{0081}\u{008d}\u{008f}\u{0090}\u{009d}",
    );
}

#[test]
fn legacy_rejects_aliases_and_decode_only_mappings_as_edit_baselines() {
    for (encoding, raw) in [
        // GBK and GB18030 have different canonical encodings of the euro.
        (TextEncoding::Gbk, b"\xa2\xe3".as_slice()),
        (TextEncoding::Gb18030, b"\x80"),
        (TextEncoding::Gbk, b"\xa3\xa0"),
        (TextEncoding::Gb18030, b"\xa3\xa0"),
        (TextEncoding::Gb18030, b"\x82\x35\x91\x32"),
        (TextEncoding::Gb18030, b"\x84\x31\x83\x30"),
        (TextEncoding::Gbk, b"\x81\x30\x81\x30"),
        // These Big5 decomposed scalars and Shift_JIS private-use bytes decode
        // successfully, but the same encoder cannot reproduce their bytes.
        (TextEncoding::Big5, b"\x88\x62"),
        (TextEncoding::ShiftJis, b"\xf0\x40"),
        (TextEncoding::ShiftJis, b"\xee\xfc"),
    ] {
        assert_eq!(
            rejected(TextDocument::decode_with_encoding(raw, encoding)),
            TextDocumentError::NonCanonicalEncoding
        );
        assert_eq!(
            legacy_format(encoding).decode_body(raw),
            Err(TextDocumentError::NonCanonicalEncoding)
        );
    }
    assert_fixture(TextEncoding::Gbk, b"\x80", "€");
    assert_fixture(TextEncoding::Gb18030, b"\xa2\xe3", "€");
    assert_fixture(TextEncoding::ShiftJis, b"\xfa\x57", "＂");
}

#[test]
fn shift_jis_rejects_yen_overline_and_minus_best_fit_without_encoder_errors() {
    let format = legacy_format(TextEncoding::ShiftJis);
    for (draft, lossy_bytes) in [("¥", b"\\".as_slice()), ("‾", b"~"), ("−", b"\x81\x7c")] {
        // The codec regards these as encodable; only a Unicode round-trip
        // comparison can distinguish them from the decoded output.
        let (encoded, _, had_errors) = encoding_rs::SHIFT_JIS.encode(draft);
        assert!(!had_errors);
        assert_eq!(encoded.as_ref(), lossy_bytes);
        assert_eq!(
            format.prepare_save(draft),
            Err(TextDocumentError::Unrepresentable)
        );
        assert!(!format.matches_bytes(draft, lossy_bytes));
    }
    assert_fixture(TextEncoding::ShiftJis, b"\\~\x81\x7c", "\\~－");
}

#[test]
fn gb18030_rejects_revision_best_fit_without_encoder_errors() {
    let format = legacy_format(TextEncoding::Gb18030);
    for (draft, lossy_bytes) in [
        ("\u{e843}", b"\xfe\x7e".as_slice()),
        ("\u{e791}", b"\xa6\xdd"),
    ] {
        let (encoded, _, had_errors) = encoding_rs::GB18030.encode(draft);
        assert!(!had_errors);
        assert_eq!(encoded.as_ref(), lossy_bytes);
        assert_eq!(
            format.prepare_save(draft),
            Err(TextDocumentError::Unrepresentable)
        );
        assert!(!format.matches_bytes(draft, lossy_bytes));
    }
    assert_fixture(
        TextEncoding::Gb18030,
        b"\xfe\x7e\xa6\xdd",
        "\u{9fb9}\u{fe14}",
    );
}

#[test]
fn legacy_unrepresentable_drafts_never_become_numeric_character_references() {
    for (encoding, draft, ncr) in [
        (TextEncoding::Gbk, "😀", b"&#128512;".as_slice()),
        (TextEncoding::Gb18030, "\u{e5e5}", b"&#58853;"),
        (TextEncoding::Big5, "😀", b"&#128512;"),
        (TextEncoding::ShiftJis, "😀", b"&#128512;"),
        (TextEncoding::Windows1252, "😀", b"&#128512;"),
    ] {
        let format = legacy_format(encoding);
        assert_eq!(
            format.prepare_save(draft),
            Err(TextDocumentError::Unrepresentable)
        );
        assert!(!format.matches_bytes(draft, ncr));
        assert_eq!(checked(format.prepare_save("&#128512;")), b"&#128512;");
    }
}

#[test]
fn legacy_nul_is_rejected_on_read_save_readback_and_comparison() {
    for encoding in LEGACY_ENCODINGS {
        let format = legacy_format(encoding);
        for raw in [b"\0".as_slice(), b"private\0text"] {
            assert_eq!(
                rejected(TextDocument::decode_with_encoding(raw, encoding)),
                TextDocumentError::Binary
            );
            assert_eq!(format.decode_body(raw), Err(TextDocumentError::Binary));
        }
        assert_eq!(
            format.prepare_save("private\0text"),
            Err(TextDocumentError::Binary)
        );
        assert!(!format.matches_bytes("private\0text", b"private\0text"));
    }
}

#[test]
fn legacy_raw_and_draft_limits_accept_exactly_one_mib() {
    let body = "a".repeat(MAX_TEXT_EDIT_BYTES);
    let oversized = format!("{body}a");
    for encoding in LEGACY_ENCODINGS {
        let document = checked(TextDocument::decode_with_encoding(
            body.as_bytes(),
            encoding,
        ));
        let format = document.format();
        assert_eq!(document.body().len(), MAX_TEXT_EDIT_BYTES);
        assert_eq!(checked(format.decode_body(body.as_bytes())), body);
        assert_eq!(checked(format.prepare_save(&body)), body.as_bytes());
        assert!(format.matches_bytes(&body, body.as_bytes()));
        assert_eq!(
            rejected(TextDocument::decode_with_encoding(
                oversized.as_bytes(),
                encoding
            )),
            TextDocumentError::Limit
        );
        assert_eq!(
            format.decode_body(oversized.as_bytes()),
            Err(TextDocumentError::Limit)
        );
        assert_eq!(
            format.prepare_save(&oversized),
            Err(TextDocumentError::Limit)
        );
        assert!(!format.matches_bytes(&oversized, oversized.as_bytes()));
        assert!(!format.matches_bytes(&body, oversized.as_bytes()));
    }
}

#[test]
fn legacy_decoded_utf8_expansion_has_an_independent_exact_limit() {
    for (encoding, unit, character) in EXPANDING_FIXTURES {
        let mut raw = unit.repeat(MAX_TEXT_EDIT_BYTES / 3);
        raw.extend_from_slice(&[b'x'; MAX_TEXT_EDIT_BYTES % 3]);
        let mut body = character.repeat(MAX_TEXT_EDIT_BYTES / 3);
        body.push_str(&"x".repeat(MAX_TEXT_EDIT_BYTES % 3));
        assert_eq!(body.len(), MAX_TEXT_EDIT_BYTES);
        assert!(raw.len() < MAX_TEXT_EDIT_BYTES);
        let document = checked(TextDocument::decode_with_encoding(&raw, encoding));
        assert_eq!(document.body(), body);
        assert_eq!(checked(document.format().decode_body(&raw)), body);
        assert_eq!(checked(document.format().prepare_save(&body)), raw);

        raw.push(b'x');
        body.push('x');
        assert!(raw.len() < MAX_TEXT_EDIT_BYTES);
        assert_eq!(
            rejected(TextDocument::decode_with_encoding(&raw, encoding)),
            TextDocumentError::Limit
        );
        assert_eq!(
            document.format().decode_body(&raw),
            Err(TextDocumentError::Limit)
        );
        assert_eq!(
            document.format().prepare_save(&body),
            Err(TextDocumentError::Limit)
        );
        assert!(!document.format().matches_bytes(&body, &raw));
    }
}

#[test]
fn gb18030_encoded_expansion_has_an_independent_exact_raw_limit() {
    let format = legacy_format(TextEncoding::Gb18030);
    let mut body = "\u{0080}".repeat(MAX_TEXT_EDIT_BYTES / 4);
    let raw = b"\x81\x30\x81\x30".repeat(MAX_TEXT_EDIT_BYTES / 4);
    assert!(body.len() < MAX_TEXT_EDIT_BYTES);
    assert_eq!(checked(format.prepare_save(&body)), raw);
    assert_eq!(checked(format.decode_body(&raw)), body);
    assert!(format.matches_bytes(&body, &raw));
    body.push('\u{0080}');
    assert!(body.len() < MAX_TEXT_EDIT_BYTES);
    assert_eq!(format.prepare_save(&body), Err(TextDocumentError::Limit));
    assert!(!format.matches_bytes(&body, &raw));
}

#[test]
fn legacy_crlf_expansion_counts_towards_the_final_raw_budget() {
    for encoding in LEGACY_ENCODINGS {
        let document = checked(TextDocument::decode_with_encoding(b"a\r\n", encoding));
        let format = document.format();
        let mut draft = format!("{}\n", "x".repeat(MAX_TEXT_EDIT_BYTES - 2));
        let raw = checked(format.prepare_save(&draft));
        assert_eq!(raw.len(), MAX_TEXT_EDIT_BYTES);
        assert!(raw.ends_with(b"\r\n"));
        assert!(format.matches_bytes(&draft, &raw));
        draft.push('x');
        assert_eq!(draft.len(), MAX_TEXT_EDIT_BYTES);
        assert_eq!(format.prepare_save(&draft), Err(TextDocumentError::Limit));
        assert!(!format.matches_bytes(&draft, &raw));
    }
}

#[test]
fn legacy_crlf_expansion_counts_towards_the_decoded_utf8_budget() {
    for (encoding, _, character) in EXPANDING_FIXTURES {
        let format = checked(TextDocument::decode_with_encoding(b"a\r\n", encoding)).format();
        let mut draft = character.repeat((MAX_TEXT_EDIT_BYTES - 2) / 3);
        draft.push_str(&"x".repeat((MAX_TEXT_EDIT_BYTES - 2) % 3));
        draft.push('\n');
        assert_eq!(draft.len(), MAX_TEXT_EDIT_BYTES - 1);
        let raw = checked(format.prepare_save(&draft));
        assert!(raw.len() < MAX_TEXT_EDIT_BYTES);
        assert_eq!(checked(format.decode_body(&raw)).len(), MAX_TEXT_EDIT_BYTES);

        draft.insert(0, 'x');
        assert_eq!(draft.len(), MAX_TEXT_EDIT_BYTES);
        assert_eq!(format.prepare_save(&draft), Err(TextDocumentError::Limit));
        assert!(!format.matches_bytes(&draft, &raw));
    }
}

#[test]
fn legacy_line_limit_accepts_4096_and_keeps_4097_line_observations_clean() {
    for encoding in LEGACY_ENCODINGS {
        let format = checked(TextDocument::decode_with_encoding(b"a\r\n", encoding)).format();
        let boundary = "a\n".repeat(MAX_TEXT_EDIT_LINES);
        let boundary_raw = b"a\r\n".repeat(MAX_TEXT_EDIT_LINES);
        assert_eq!(checked(format.prepare_save(&boundary)), boundary_raw);
        let no_final_newline = format!("{}tail", "a\n".repeat(MAX_TEXT_EDIT_LINES - 1));
        assert!(format.prepare_save(&no_final_newline).is_ok());
        assert_eq!(
            format.prepare_save(&format!("{boundary}tail")),
            Err(TextDocumentError::Limit)
        );

        let raw = b"a\r\n".repeat(MAX_TEXT_EDIT_LINES + 1);
        let document = checked(TextDocument::decode_with_encoding(&raw, encoding));
        assert_eq!(
            checked(document.format().decode_body(&raw)),
            document.body()
        );
        assert!(document.format().matches_bytes(document.body(), &raw));
        assert_eq!(
            document.format().prepare_save(document.body()),
            Err(TextDocumentError::Limit)
        );
    }
}

#[test]
fn legacy_line_profiles_preserve_crlf_mixed_cr_and_final_newline_policy() {
    for (encoding, unit, character) in EXPANDING_FIXTURES {
        for (original, draft, expected, ending) in [
            ("", "\nnew", "\nnew", TextLineEnding::None),
            ("\na", "\r\nnew\na", "\nnew\na", TextLineEnding::Lf),
            ("\r\na", "\r\nnew\na", "\r\nnew\r\na", TextLineEnding::CrLf),
            (
                "\r\na\n",
                "\r\nnew\na\n",
                "\r\nnew\na\n",
                TextLineEnding::Mixed,
            ),
            ("\ra\r", "\ra\rnew\n", "\ra\rnew\n", TextLineEnding::Cr),
            (
                "\r\na",
                "\rnew\nlast",
                "\rnew\r\nlast",
                TextLineEnding::CrLf,
            ),
        ] {
            let raw = [unit, original.as_bytes()].concat();
            let draft = format!("{character}{draft}");
            let expected = [unit, expected.as_bytes()].concat();
            let document = checked(TextDocument::decode_with_encoding(&raw, encoding));
            assert_eq!(document.format().line_ending(), ending);
            assert_eq!(checked(document.format().prepare_save(&draft)), expected);
            assert!(document.format().matches_bytes(&draft, &expected));
        }
    }
}

#[test]
fn legacy_continuing_no_break_saves_retain_the_existing_insertion_policy() {
    for encoding in LEGACY_ENCODINGS {
        for original in ["a\n", "a\r\n", "a\r", "a\r\nb\n"] {
            let previous = checked(TextDocument::decode_with_encoding(
                original.as_bytes(),
                encoding,
            ))
            .format();
            let mut format = previous;
            for draft in ["", "plain", "", "next"] {
                let saved = checked(format.prepare_save(draft));
                assert_eq!(saved, draft.as_bytes());
                let observation = checked(TextDocument::decode_with_encoding(&saved, encoding));
                assert_eq!(observation.format().line_ending(), TextLineEnding::None);
                format = observation.format().retain_insertion_policy(format);
                assert_eq!(format.encoding(), encoding);
                assert!(!format.has_bom());
                assert_eq!(format.line_ending(), previous.line_ending());
            }
            let expected = if previous.line_ending() == TextLineEnding::CrLf {
                b"a\r\nb".as_slice()
            } else {
                b"a\nb"
            };
            assert_eq!(checked(format.prepare_save("a\nb")), expected);
            assert_eq!(
                checked(legacy_format(encoding).prepare_save("a\nb")),
                b"a\nb"
            );
        }
    }
}

#[test]
fn legacy_insertion_policy_never_crosses_an_encoding_change() {
    let encodings = [UNICODE_ENCODINGS.as_slice(), LEGACY_ENCODINGS.as_slice()].concat();
    for &encoding in &encodings {
        let original = if encoding.is_legacy() {
            b"a\r\n".to_vec()
        } else {
            unicode_bytes("a\r\n", encoding)
        };
        let previous = checked(TextDocument::decode_with_encoding(&original, encoding)).format();
        for &other_encoding in &encodings {
            if encoding == other_encoding {
                continue;
            }
            let raw = if other_encoding.is_legacy() {
                Vec::new()
            } else {
                unicode_bytes("", other_encoding)
            };
            let observation =
                checked(TextDocument::decode_with_encoding(&raw, other_encoding)).format();
            assert_eq!(observation.retain_insertion_policy(previous), observation);
            assert!(!observation.is_compatible_with(previous));
        }
    }
}

#[test]
fn legacy_format_compatibility_requires_the_same_encoding_and_line_policy() {
    for encoding in LEGACY_ENCODINGS {
        let observe =
            |bytes: &[u8]| checked(TextDocument::decode_with_encoding(bytes, encoding)).format();
        assert!(observe(b"a").is_compatible_with(observe(b"b\n")));
        assert!(observe(b"a\r\n").is_compatible_with(observe(b"b\r\n")));
        for other in [b"a\r\n".as_slice(), b"a\r", b"a\r\nb\n"] {
            assert!(!observe(b"a\n").is_compatible_with(observe(other)));
        }
        for other_encoding in LEGACY_ENCODINGS {
            if encoding != other_encoding {
                let other =
                    checked(TextDocument::decode_with_encoding(b"b\n", other_encoding)).format();
                assert!(!observe(b"a\n").is_compatible_with(other));
            }
        }
    }
}

#[test]
fn legacy_with_line_ending_changes_only_the_authoritative_insertion_policy() {
    for encoding in LEGACY_ENCODINGS {
        let original = checked(TextDocument::decode_with_encoding(b"a\r\n", encoding)).format();
        let verbatim = original.with_line_ending(TextLineEnding::Mixed);
        assert_eq!(verbatim.encoding(), encoding);
        assert!(!verbatim.has_bom());
        assert_eq!(
            checked(verbatim.prepare_save("a\r\nb\nlast")),
            b"a\r\nb\nlast"
        );
        assert_eq!(
            checked(original.prepare_save("a\nb\nlast")),
            b"a\r\nb\r\nlast"
        );
    }
}

#[test]
fn legacy_streaming_preserves_multibyte_characters_across_output_buffers() {
    for (encoding, unit, character) in EXPANDING_FIXTURES.into_iter().chain([(
        TextEncoding::Gb18030,
        b"\x94\x39\xda\x33".as_slice(),
        "💩",
    )]) {
        let mut raw = vec![b'a'; 4095];
        raw.extend_from_slice(&unit.repeat(2050));
        raw.extend_from_slice(b"\r\nlast");
        let body = format!("{}{}\r\nlast", "a".repeat(4095), character.repeat(2050));
        assert_fixture(encoding, &raw, &body);
    }
}

#[test]
fn legacy_streaming_rejects_malformed_input_after_full_output_buffers() {
    for encoding in [
        TextEncoding::Gbk,
        TextEncoding::Gb18030,
        TextEncoding::Big5,
        TextEncoding::ShiftJis,
    ] {
        let mut raw = vec![b'a'; 8192];
        raw.push(0x81);
        assert_eq!(
            rejected(TextDocument::decode_with_encoding(&raw, encoding)),
            TextDocumentError::InvalidLegacyEncoding
        );
    }
    for encoding in LEGACY_ENCODINGS {
        let raw = format!("{}\0", "a".repeat(8192));
        assert_eq!(
            rejected(TextDocument::decode_with_encoding(raw.as_bytes(), encoding)),
            TextDocumentError::Binary
        );
    }
}

#[test]
fn legacy_streaming_rejects_best_fit_and_unrepresentable_draft_tails() {
    let prefix = "a".repeat(8192);
    for (encoding, tail, lossy_tail) in [
        (TextEncoding::ShiftJis, "¥", b"\\".as_slice()),
        (TextEncoding::ShiftJis, "‾", b"~"),
        (TextEncoding::ShiftJis, "−", b"\x81\x7c"),
        (TextEncoding::Gbk, "😀", b"&#128512;"),
        (TextEncoding::Gb18030, "\u{e5e5}", b"&#58853;"),
        (TextEncoding::Big5, "😀", b"&#128512;"),
        (TextEncoding::Windows1252, "😀", b"&#128512;"),
    ] {
        let format = legacy_format(encoding);
        let draft = format!("{prefix}{tail}");
        assert_eq!(
            format.prepare_save(&draft),
            Err(TextDocumentError::Unrepresentable)
        );
        let lossy = [prefix.as_bytes(), lossy_tail].concat();
        assert!(!format.matches_bytes(&draft, &lossy));
    }
}

#[test]
fn legacy_saves_never_unicode_normalize_a_draft_to_make_it_representable() {
    let windows = legacy_format(TextEncoding::Windows1252);
    assert_eq!(checked(windows.prepare_save("café")), b"caf\xe9");
    assert_eq!(
        windows.prepare_save("cafe\u{0301}"),
        Err(TextDocumentError::Unrepresentable)
    );
    assert!(!windows.matches_bytes("cafe\u{0301}", b"caf\xe9"));

    let gb18030 = legacy_format(TextEncoding::Gb18030);
    let draft = "cafe\u{0301}\rstandalone";
    let raw = checked(gb18030.prepare_save(draft));
    let document = checked(TextDocument::decode_with_encoding(
        &raw,
        TextEncoding::Gb18030,
    ));
    assert_eq!(document.body(), draft);
    assert_ne!(document.body(), "café\rstandalone");
}

#[test]
fn legacy_saves_reject_output_that_would_introduce_a_unicode_signature() {
    let format = legacy_format(TextEncoding::Windows1252);
    for (draft, raw) in [
        ("ï»¿body", b"\xef\xbb\xbfbody".as_slice()),
        ("ÿþbody", b"\xff\xfebody"),
        ("þÿbody", b"\xfe\xffbody"),
    ] {
        assert_eq!(
            format.prepare_save(draft),
            Err(TextDocumentError::UnsupportedEncoding)
        );
        assert!(!format.matches_bytes(draft, raw));
    }
}

#[test]
fn gb18030_preserves_literal_leading_and_internal_feff_as_content() {
    let format = legacy_format(TextEncoding::Gb18030);
    let body = "\u{feff}\u{feff}a\u{feff}";
    let raw = checked(format.prepare_save(body));
    let document = checked(TextDocument::decode_with_encoding(
        &raw,
        TextEncoding::Gb18030,
    ));
    assert_eq!(document.body(), body);
    assert!(!document.format().has_bom());
    assert_eq!(checked(document.format().decode_body(&raw)), body);
    assert!(document.format().matches_bytes(body, &raw));
}

#[test]
fn explicit_unicode_selection_matches_existing_bom_and_content_semantics() {
    for encoding in UNICODE_ENCODINGS {
        for body in ["", "中文😀", "a\r\nlast", "\u{feff}\u{feff}a\u{feff}"] {
            let raw = unicode_bytes(body, encoding);
            let default = checked(TextDocument::decode(&raw));
            let selected = checked(TextDocument::decode_with_encoding(&raw, encoding));
            assert_eq!(selected.body(), default.body());
            assert_eq!(selected.format(), default.format());
            assert_eq!(
                checked(selected.format().prepare_save(selected.body())),
                raw
            );
        }
    }
    let raw = b"\xef\xbb\xbfbody";
    let selected = checked(TextDocument::decode_with_encoding(raw, TextEncoding::Utf8));
    assert!(selected.format().has_bom());
    assert_eq!(selected.body(), "body");
}

#[test]
fn explicit_unicode_selection_rejects_missing_or_wrong_encoding_boms() {
    for encoding in UNICODE_ENCODINGS {
        for other_encoding in UNICODE_ENCODINGS {
            if encoding == other_encoding {
                continue;
            }
            let raw = unicode_bytes("a", other_encoding);
            assert_eq!(
                rejected(TextDocument::decode_with_encoding(&raw, encoding)),
                TextDocumentError::UnsupportedEncoding
            );
        }
    }
    for encoding in [TextEncoding::Utf16Le, TextEncoding::Utf16Be] {
        assert_eq!(
            rejected(TextDocument::decode_with_encoding(b"", encoding)),
            TextDocumentError::UnsupportedEncoding
        );
        assert_eq!(
            rejected(TextDocument::decode_with_encoding(b"a", encoding)),
            TextDocumentError::UnsupportedEncoding
        );
    }
}

#[test]
fn explicit_unicode_selection_keeps_utf32_and_malformed_unicode_rejected() {
    for encoding in UNICODE_ENCODINGS {
        for raw in [
            b"\xff\xfe\0\0".as_slice(),
            b"\xff\xfe\0\0a\0\0\0",
            b"\0\0\xfe\xff",
            b"\0\0\xfe\xff\0\0\0a",
        ] {
            assert_eq!(
                rejected(TextDocument::decode_with_encoding(raw, encoding)),
                TextDocumentError::UnsupportedEncoding
            );
        }
    }
    for (encoding, raw, expected) in [
        (
            TextEncoding::Utf8,
            b"\xc0\x80".as_slice(),
            TextDocumentError::InvalidUtf8,
        ),
        (
            TextEncoding::Utf8,
            b"\xef\xbb",
            TextDocumentError::InvalidUtf8,
        ),
        (
            TextEncoding::Utf8,
            b"private\0text",
            TextDocumentError::Binary,
        ),
        (
            TextEncoding::Utf16Le,
            b"\xff\xfex",
            TextDocumentError::InvalidUtf16,
        ),
        (
            TextEncoding::Utf16Le,
            b"\xff\xfe\x00\xd8",
            TextDocumentError::InvalidUtf16,
        ),
        (
            TextEncoding::Utf16Be,
            b"\xfe\xff\xd8\x00",
            TextDocumentError::InvalidUtf16,
        ),
        (
            TextEncoding::Utf16Be,
            b"\xfe\xff\x00\x00",
            TextDocumentError::Binary,
        ),
    ] {
        assert_eq!(
            rejected(TextDocument::decode_with_encoding(raw, encoding)),
            expected
        );
    }
}

#[test]
fn explicit_unicode_selection_keeps_bomless_feff_readback_authority() {
    let document = checked(TextDocument::decode_with_encoding(
        b"plain",
        TextEncoding::Utf8,
    ));
    let format = document.format();
    let draft = "\u{feff}body";
    let raw = checked(format.prepare_save(draft));
    assert!(!format.has_bom());
    assert_eq!(checked(format.decode_body(&raw)), draft);
    assert!(format.matches_bytes(draft, &raw));
    let reopened = checked(TextDocument::decode_with_encoding(&raw, TextEncoding::Utf8));
    assert!(reopened.format().has_bom());
    assert_eq!(reopened.body(), "body");
}

#[test]
fn legacy_errors_display_and_debug_never_include_document_contents() {
    let sentinel = "private-text-codec-sentinel";
    let malformed = [sentinel.as_bytes(), b"\x81"].concat();
    let alias = [sentinel.as_bytes(), b"\xa2\xe3"].concat();
    let errors = [
        rejected(TextDocument::decode_with_encoding(
            &malformed,
            TextEncoding::Gbk,
        )),
        rejected(TextDocument::decode_with_encoding(
            &alias,
            TextEncoding::Gbk,
        )),
        rejected(legacy_format(TextEncoding::ShiftJis).prepare_save(&format!("{sentinel}¥"))),
        rejected(legacy_format(TextEncoding::Windows1252).prepare_save(&format!("{sentinel}😀"))),
    ];
    assert_eq!(errors[0], TextDocumentError::InvalidLegacyEncoding);
    assert_eq!(errors[1], TextDocumentError::NonCanonicalEncoding);
    assert_eq!(errors[2], TextDocumentError::Unrepresentable);
    assert_eq!(errors[3], TextDocumentError::Unrepresentable);
    for error in errors {
        assert!(!error.to_string().contains(sentinel));
        assert!(!format!("{error:?}").contains(sentinel));
        assert!(!error.to_string().contains('¥'));
        assert!(!error.to_string().contains('😀'));
    }
}
