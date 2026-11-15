#![cfg(test)]

use super::*;
use serde_json::{Value, json};

fn packet() -> Value {
    json!({
        "protocol": PROTOCOL,
        "tools": ArchiveTool::ALL.map(|tool| json!({
            "tool": tool.name(),
            "path": format!("/fixture/归档 tools/{}", tool.name()),
            "version": format!("{} 1.0", tool.name()),
            "version_status": "passed",
            "stream_check": if matches!(tool, ArchiveTool::Tar | ArchiveTool::Gzip | ArchiveTool::Python3) {
                "passed"
            } else {
                "not_tested"
            },
        })),
    })
}

fn encoded(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_else(|error| panic!("{error}"))
}

fn assert_invalid(value: &Value) {
    assert!(
        matches!(
            parse_report(&encoded(value)),
            Err(SessionError::Invalid(INVALID_RESPONSE))
        ),
        "must reject {value}"
    );
}

#[test]
fn preview_embeds_the_entire_helper_as_one_fixed_shell_argument() {
    let command = validation_command();
    assert_eq!(
        command,
        format!("python3 -I -S -c '{}'", HELPER.replace('\'', "'\\''"))
    );
    assert!(command.contains("keelshell-probe.txt"));
    assert!(command.contains("start_new_session=True"));
    assert!(!command.contains("base64"));
}

#[test]
fn parser_preserves_safe_unicode_paths_and_version_observations() {
    let mut value = packet();
    value["tools"][0]["path"] = json!("/opt/归档 \"tools\"/tar");
    value["tools"][0]["version"] = json!("tar 归档 \"1.0\"");
    let report = parse_report(&encoded(&value)).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(report.tools.len(), 7);
    assert_eq!(
        report.tools.iter().map(|row| row.tool).collect::<Vec<_>>(),
        ArchiveTool::ALL
    );
    assert_eq!(
        report.tools[0].path.as_deref(),
        Some("/opt/归档 \"tools\"/tar")
    );
    assert_eq!(report.tools[0].version.as_deref(), Some("tar 归档 \"1.0\""));
}

#[test]
fn missing_tools_preserve_explicit_untested_functional_boundaries() {
    let mut value = packet();
    for (index, tool) in ArchiveTool::ALL.into_iter().enumerate() {
        value["tools"][index]["path"] = Value::Null;
        value["tools"][index]["version"] = Value::Null;
        value["tools"][index]["version_status"] = json!("unavailable");
        if matches!(
            tool,
            ArchiveTool::Tar | ArchiveTool::Gzip | ArchiveTool::Python3
        ) {
            value["tools"][index]["stream_check"] = json!("unavailable");
        }
    }
    let report = parse_report(&encoded(&value)).unwrap_or_else(|error| panic!("{error}"));
    assert!(report.tools.iter().all(|row| row.path.is_none()
        && row.version.is_none()
        && row.version_status == ToolCheckStatus::Unavailable));
    assert_eq!(report.tools[2].stream_check, ToolCheckStatus::NotTested);
    assert_eq!(report.tools[3].stream_check, ToolCheckStatus::NotTested);
    assert_eq!(report.tools[5].stream_check, ToolCheckStatus::NotTested);
    assert_eq!(report.tools[6].stream_check, ToolCheckStatus::NotTested);
}

#[test]
fn version_and_stream_outcomes_remain_independent_and_typed() {
    for (text, status) in [
        ("failed", ToolCheckStatus::Failed),
        ("unavailable", ToolCheckStatus::Unavailable),
        ("timed_out", ToolCheckStatus::TimedOut),
        ("output_limit", ToolCheckStatus::OutputLimit),
    ] {
        let mut value = packet();
        value["tools"][0]["version"] = Value::Null;
        value["tools"][0]["version_status"] = json!(text);
        let report = parse_report(&encoded(&value)).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(report.tools[0].version_status, status);
        assert_eq!(report.tools[0].stream_check, ToolCheckStatus::Passed);
        value["tools"][0]["stream_check"] = json!(text);
        let report = parse_report(&encoded(&value)).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(report.tools[0].stream_check, status);
    }
}

#[test]
fn parser_rejects_unknown_missing_duplicate_or_mistyped_fields() {
    let base = packet();
    for key in ["protocol", "tools"] {
        let mut value = base.clone();
        value
            .as_object_mut()
            .unwrap_or_else(|| panic!("object"))
            .remove(key);
        assert_invalid(&value);
    }
    for key in ["tool", "path", "version", "version_status", "stream_check"] {
        let mut value = base.clone();
        value["tools"][0]
            .as_object_mut()
            .unwrap_or_else(|| panic!("row"))
            .remove(key);
        assert_invalid(&value);
        let mut value = base.clone();
        value["tools"][0][key] = json!(42);
        assert_invalid(&value);
    }
    let mut value = base.clone();
    value["extra"] = json!(true);
    assert_invalid(&value);
    let mut value = base.clone();
    value["tools"][0]["extra"] = json!(true);
    assert_invalid(&value);
    for (from, to) in [
        ("\"protocol\":", "\"protocol\":\"wrong\",\"protocol\":"),
        ("\"path\":", "\"path\":null,\"path\":"),
    ] {
        let text = String::from_utf8(encoded(&base)).unwrap_or_else(|error| panic!("{error}"));
        let bytes = text.replacen(from, to, 1).into_bytes();
        assert!(matches!(
            parse_report(&bytes),
            Err(SessionError::Invalid(_))
        ));
    }
}

#[test]
fn parser_rejects_unknown_protocol_tools_statuses_order_and_count() {
    let mut value = packet();
    value["protocol"] = json!("keelshell.archive-validation.v2");
    assert_invalid(&value);
    for (field, text) in [
        ("tool", "rar"),
        ("tool", "Tar"),
        ("version_status", "Passed"),
        ("version_status", "success"),
        ("stream_check", "timeout"),
        ("stream_check", "unknown"),
    ] {
        let mut value = packet();
        value["tools"][0][field] = json!(text);
        assert_invalid(&value);
    }
    let mut value = packet();
    value["tools"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("tools"))
        .swap(0, 1);
    assert_invalid(&value);
    let mut value = packet();
    value["tools"][1] = value["tools"][0].clone();
    assert_invalid(&value);
    let mut value = packet();
    value["tools"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("tools"))
        .pop();
    assert_invalid(&value);
    let mut value = packet();
    let extra = value["tools"][0].clone();
    value["tools"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("tools"))
        .push(extra);
    assert_invalid(&value);
}

#[test]
fn parser_rejects_inconsistent_paths_versions_and_functional_claims() {
    for path in ["", ".", "./tar", "tar", "/", "/bin/", "/bin/gzip"] {
        let mut value = packet();
        value["tools"][0]["path"] = json!(path);
        assert_invalid(&value);
    }
    for version in ["", " ", " tar 1", "tar 1 "] {
        let mut value = packet();
        value["tools"][0]["version"] = json!(version);
        assert_invalid(&value);
    }
    for field in ["path", "version"] {
        let mut value = packet();
        value["tools"][0][field] = Value::Null;
        assert_invalid(&value);
    }
    for status in [
        "failed",
        "unavailable",
        "not_tested",
        "timed_out",
        "output_limit",
    ] {
        let mut value = packet();
        value["tools"][0]["version_status"] = json!(status);
        assert_invalid(&value);
    }
    let mut value = packet();
    value["tools"][0]["version"] = Value::Null;
    value["tools"][0]["version_status"] = json!("not_tested");
    assert_invalid(&value);
    let mut value = packet();
    value["tools"][0]["stream_check"] = json!("not_tested");
    assert_invalid(&value);
    for index in [2, 3, 5, 6] {
        for status in [
            "passed",
            "failed",
            "unavailable",
            "timed_out",
            "output_limit",
        ] {
            let mut value = packet();
            value["tools"][index]["stream_check"] = json!(status);
            assert_invalid(&value);
        }
    }
}

#[test]
fn parser_rejects_control_bidi_replacement_and_hidden_text() {
    for character in [
        '\0', '\n', '\r', '\t', '\u{1b}', '\u{7f}', '\u{85}', '\u{061c}', '\u{200b}', '\u{2028}',
        '\u{202e}', '\u{2066}', '\u{feff}', '\u{fffd}',
    ] {
        for (field, text) in [
            ("path", format!("/bin/{character}/tar")),
            ("version", format!("tar {character}1.0")),
        ] {
            let mut value = packet();
            value["tools"][0][field] = json!(text);
            assert_invalid(&value);
        }
    }
}

#[test]
fn exact_utf8_field_limits_fit_the_response_and_overruns_are_rejected() {
    let mut value = packet();
    for (index, tool) in ArchiveTool::ALL.into_iter().enumerate() {
        let suffix = format!("/{}", tool.name());
        value["tools"][index]["path"] = json!(format!(
            "/{}{}",
            "x".repeat(MAX_PATH_BYTES - 1 - suffix.len()),
            suffix,
        ));
        value["tools"][index]["version"] = json!("v".repeat(MAX_VERSION_BYTES));
    }
    let bytes = encoded(&value);
    assert!(bytes.len() <= MAX_RESPONSE_BYTES);
    assert!(parse_report(&bytes).is_ok());
    value["tools"][0]["path"] = json!(format!("/{}tar", "档".repeat(1365)));
    assert_invalid(&value);
    let mut value = packet();
    value["tools"][0]["version"] = json!("档".repeat(86));
    assert_invalid(&value);
    assert!(matches!(
        parse_report(&vec![b' '; MAX_RESPONSE_BYTES + 1]),
        Err(SessionError::OutputLimit(MAX_RESPONSE_BYTES))
    ));
}

#[test]
fn every_truncated_document_invalid_utf8_banner_and_extra_document_is_rejected() {
    let bytes = encoded(&packet());
    for end in 0..bytes.len() {
        assert!(
            matches!(parse_report(&bytes[..end]), Err(SessionError::Invalid(_))),
            "accepted truncated prefix {end}"
        );
    }
    let mut invalid = bytes.clone();
    invalid[1] = 0xff;
    assert!(matches!(
        parse_report(&invalid),
        Err(SessionError::Invalid(_))
    ));
    for prefix in [b"banner\n".as_slice(), b"\0"] {
        let mut value = prefix.to_vec();
        value.extend_from_slice(&bytes);
        assert!(matches!(
            parse_report(&value),
            Err(SessionError::Invalid(_))
        ));
    }
    for suffix in [b"{}".as_slice(), b"\0", b"\nextra"] {
        let mut value = bytes.clone();
        value.extend_from_slice(suffix);
        assert!(matches!(
            parse_report(&value),
            Err(SessionError::Invalid(_))
        ));
    }
    let mut whitespace = b" \n".to_vec();
    whitespace.extend_from_slice(&bytes);
    whitespace.extend_from_slice(b"\r\n");
    assert!(parse_report(&whitespace).is_ok());
}

#[cfg(unix)]
#[test]
fn actual_archive_validation_python_safety_suite_runs_under_the_default_gate() {
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/archive_validation/test_archive_validation.py"
    );
    let mut child = std::process::Command::new("python3")
        .args(["-I", "-S", "-B", script])
        .spawn()
        .unwrap_or_else(|error| panic!("Python 3.8+ prerequisite: {error}"));
    let until = std::time::Instant::now() + Duration::from_secs(45);
    loop {
        if let Some(status) = child.try_wait().unwrap_or_else(|error| panic!("{error}")) {
            assert!(
                status.success(),
                "Python archive validation suite: {status}"
            );
            break;
        }
        if std::time::Instant::now() >= until {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Python archive validation suite exceeded 45 seconds");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
