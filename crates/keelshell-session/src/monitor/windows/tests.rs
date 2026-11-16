#![cfg(test)]
#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "Controlled fixtures and success assertions fail immediately on unexpected values"
)]

use super::*;
use serde_json::{Value, json};

const PROBE: &str = include_str!("fixtures/probe.json");
const RESOURCES: &str = include_str!("fixtures/resources.json");
const UNKNOWN: &str = include_str!("fixtures/resources-unknown.json");
const PROCESSES: &str = include_str!("fixtures/processes.json");
const EMPTY_PROCESSES: &str = include_str!("fixtures/processes-empty.json");
const SOCKETS: &str = include_str!("fixtures/sockets.json");
const EMPTY_SOCKETS: &str = include_str!("fixtures/sockets-empty.json");
const DENIED_SOCKETS: &str = include_str!("fixtures/sockets-permission-denied.json");

fn output(input: &str) -> ExecOutput {
    ExecOutput {
        stdout: input.as_bytes().to_vec(),
        stderr: Vec::new(),
        exit_status: Some(0),
    }
}

fn value(input: &str) -> Value {
    serde_json::from_str(input).expect("valid test fixture")
}

fn encoded(value: &Value) -> ExecOutput {
    output(&serde_json::to_string(value).expect("serializable fixture"))
}

fn assert_invalid<T>(result: MonitorResult<T>) {
    assert!(matches!(result, Err(MonitorError::InvalidData(_))));
}

fn operations() -> [Operation; 5] {
    [
        Operation::Probe,
        Operation::Resources {
            skip_capacity: false,
        },
        Operation::Resources {
            skip_capacity: true,
        },
        Operation::Processes,
        Operation::Sockets,
    ]
}

#[test]
fn commands_are_fixed_ascii_under_default_shell_bound() {
    for operation in operations() {
        let command = command(operation).expect("bounded fixed command");
        assert!(command.is_ascii());
        assert!(command.len() < 8000, "{operation:?}: {}", command.len());
        assert!(command.starts_with(COMMAND_PREFIX));
        let encoded = command.strip_prefix(COMMAND_PREFIX).unwrap();
        assert!(!encoded.bytes().any(|byte| byte.is_ascii_whitespace()));
        assert_eq!(STANDARD.decode(encoded).unwrap().len() % 2, 0);
    }
}

#[test]
fn encoded_commands_round_trip_exact_production_files_without_bom() {
    for operation in operations() {
        let command = command(operation).unwrap();
        let bytes = STANDARD
            .decode(command.strip_prefix(COMMAND_PREFIX).unwrap())
            .unwrap();
        assert!(!bytes.starts_with(&[0xff, 0xfe]));
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        let (prefix, body) = operation.parts();
        assert_eq!(
            String::from_utf16(&units).unwrap(),
            [prefix, COMMON, body, SUFFIX].concat()
        );
    }
}

#[test]
fn fixed_scripts_use_read_only_sources_and_both_socket_commands() {
    let resources = include_str!("resources.ps1");
    let processes = include_str!("processes.ps1");
    let sockets = include_str!("sockets.ps1");
    assert!(COMMON.contains("$ErrorActionPreference='Stop'"));
    assert!(COMMON.contains("[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)"));
    assert!(resources.contains("Win32_OperatingSystem"));
    assert!(resources.contains("Win32_Processor"));
    assert!(resources.contains("GetIPStatistics()"));
    assert!(resources.contains("TotalFreeSpace"));
    assert!(resources.contains("AvailableFreeSpace"));
    assert!(processes.contains("Get-Process|Sort-Object Id"));
    assert!(sockets.contains("Get-NetTCPConnection -State Listen"));
    assert!(sockets.contains("Get-NetUDPEndpoint"));
    for script in [COMMON, resources, processes, sockets] {
        for forbidden in [
            "Set-",
            "Remove-",
            "Start-",
            "Stop-",
            "Invoke-",
            "Out-File",
            "ExecutionPolicy",
            "GetOwner",
            "CommandLine",
            "CPUSeconds",
            "Write-Error",
        ] {
            assert!(!script.contains(forbidden), "{forbidden}");
        }
    }
}

#[test]
fn probe_accepts_complete_lf_crlf_and_unterminated_json() {
    for input in [
        PROBE.to_owned(),
        PROBE.replace('\n', "\r\n"),
        PROBE.trim_end_matches('\n').to_owned(),
    ] {
        assert_eq!(parse_probe(output(&input)).unwrap(), [5, 1, 26100, 123]);
    }
}

#[test]
fn probe_requires_actual_windows_identity_and_four_desktop_version_components() {
    for os in ["Linux", "Windows", "win32nt", "", "Win32NT\n"] {
        let mut frame = value(PROBE);
        frame["data"]["os"] = json!(os);
        assert_invalid(parse_probe(encoded(&frame)));
    }
    for version in [
        "7.4.0.0",
        "5.0.26100.123",
        "5.1",
        "5.1.26100",
        "5.1.26100.123.0",
        "05.1.26100.123",
        "5.1.-1.123",
        "5.1.4294967296.0",
        "5.1.1.18446744073709551616",
        "5.1.1.+1",
        "5.1.１.1",
        "5.1.1.0 ",
    ] {
        let mut frame = value(PROBE);
        frame["runtime_version"] = json!(version);
        assert_invalid(parse_probe(encoded(&frame)));
    }
}

#[test]
fn every_envelope_field_is_required_and_unknown_fields_fail_closed() {
    for key in [
        "version",
        "operation",
        "platform",
        "runtime",
        "runtime_version",
        "units",
        "status",
        "data",
    ] {
        let mut frame = value(PROBE);
        frame.as_object_mut().unwrap().remove(key);
        assert_invalid(parse_probe(encoded(&frame)));
    }
    let mut frame = value(PROBE);
    frame["unrecognized"] = json!(true);
    assert_invalid(parse_probe(encoded(&frame)));
}

#[test]
fn duplicate_envelope_fields_are_rejected_before_value_normalization() {
    for key in [
        "version",
        "operation",
        "platform",
        "runtime",
        "runtime_version",
        "units",
        "status",
        "data",
    ] {
        let frame = value(PROBE);
        let prefix = format!("\"{key}\":");
        let duplicate = format!("{prefix}{},{prefix}", frame[key]);
        let duplicated = PROBE.replacen(&prefix, &duplicate, 1);
        assert_ne!(duplicated, PROBE);
        assert_invalid(parse_probe(output(&duplicated)));
    }
    assert_invalid(parse_probe(output(&PROBE.replace(
        "\"os\":\"Win32NT\"",
        "\"os\":\"Win32NT\",\"os\":\"Win32NT\"",
    ))));
}

#[test]
fn envelope_rejects_wrong_units_versions_operations_runtime_and_types() {
    for (key, wrong) in [
        ("version", json!(0)),
        ("version", json!(2)),
        ("version", json!("1")),
        ("version", json!(1.0)),
        ("version", json!(4294967296_u64)),
        ("operation", json!("resources")),
        ("operation", json!("Probe")),
        ("platform", json!("Darwin")),
        ("runtime", json!("PowerShell")),
        ("runtime", json!("pwsh")),
        ("units", json!("kilobytes seconds percent")),
        ("units", json!("bytes seconds ticks")),
        ("status", json!("partial")),
        ("data", Value::Null),
        ("data", json!([])),
    ] {
        let mut frame = value(PROBE);
        frame[key] = wrong;
        assert_invalid(parse_probe(encoded(&frame)));
    }
}

#[test]
fn complete_frame_rejects_banners_unrelated_whitespace_and_multiple_frames() {
    let frame = PROBE.trim_end_matches('\n');
    for input in [
        format!(" {frame}"),
        format!("\n{frame}"),
        format!("\u{feff}{frame}"),
        format!("banner\n{frame}\n"),
        format!("{frame} "),
        format!("{frame}\t"),
        format!("{frame}\n\n"),
        format!("{frame}\r"),
        format!("{frame}\ntrailing"),
        format!("{frame}{frame}"),
        format!("{frame}\n{frame}\n"),
        frame.replace(",\"data\":", ",\r\"data\":"),
    ] {
        assert_invalid(parse_probe(output(&input)));
    }
}

#[test]
fn malformed_truncated_non_utf8_and_oversize_frames_fail_closed() {
    for input in ["", "{}", "{", "null", "[]", "{\"version\":", "NaN"] {
        assert_invalid(parse_probe(output(input)));
    }
    let frame = PROBE.trim_end_matches('\n');
    for removed in [1, 2, 10, frame.len() - 1] {
        assert_invalid(parse_probe(output(&frame[..frame.len() - removed])));
    }
    let mut non_utf8 = output(PROBE);
    non_utf8.stdout.push(0xff);
    assert_invalid(parse_probe(non_utf8));
    let mut too_large = output(PROBE);
    too_large.stdout.resize(MAX_BYTES + 1, b' ');
    assert_invalid(parse_probe(too_large));
    let mut combined = output(PROBE);
    combined.stderr.resize(MAX_BYTES, b'x');
    assert_invalid(parse_probe(combined));
}

#[test]
fn no_exit_or_success_stderr_never_establishes_a_collection() {
    let mut missing = output(PROBE);
    missing.exit_status = None;
    assert!(matches!(
        parse_probe(missing),
        Err(MonitorError::RemoteFailure {
            exit_status: None,
            ..
        })
    ));
    let mut stderr = output(PROBE);
    stderr.stderr = b"\r\n".to_vec();
    assert_invalid(parse_probe(stderr));
    let mut failed = output(PROBE);
    failed.exit_status = Some(1);
    assert!(matches!(
        parse_probe(failed),
        Err(MonitorError::RemoteFailure {
            exit_status: Some(1),
            ..
        })
    ));
}

#[test]
fn typed_failures_require_null_data_nonzero_exit_and_empty_stderr() {
    for (status, expected) in [
        ("command_missing", CapabilityReason::CommandMissing),
        ("permission_denied", CapabilityReason::PermissionDenied),
        ("command_failed", CapabilityReason::CommandFailed),
        ("unsupported_options", CapabilityReason::UnsupportedOptions),
    ] {
        let mut frame = value(DENIED_SOCKETS);
        frame["status"] = json!(status);
        let mut result = encoded(&frame);
        result.exit_status = Some(9);
        assert!(matches!(
            parse_sockets(result),
            Err(MonitorError::CapabilityUnavailable {
                capability: Capability::Sockets,
                reason,
            }) if reason == expected
        ));
        assert_invalid(parse_sockets(encoded(&frame)));
        let mut stderr = encoded(&frame);
        stderr.exit_status = Some(1);
        stderr.stderr = b"Access denied".to_vec();
        assert_invalid(parse_sockets(stderr));
        frame["data"] = value(EMPTY_SOCKETS)["data"].clone();
        let mut non_null = encoded(&frame);
        non_null.exit_status = Some(1);
        assert_invalid(parse_sockets(non_null));
    }
}

#[test]
fn malformed_error_frames_cannot_claim_a_localized_capability_reason() {
    let mut malformed = output("Access denied: command not found");
    malformed.exit_status = Some(127);
    assert_invalid(parse_sockets(malformed));
    let mut wrong_operation = value(DENIED_SOCKETS);
    wrong_operation["operation"] = json!("processes");
    let mut result = encoded(&wrong_operation);
    result.exit_status = Some(1);
    assert_invalid(parse_sockets(result));
    let mut missing_data = value(DENIED_SOCKETS);
    missing_data.as_object_mut().unwrap().remove("data");
    let mut result = encoded(&missing_data);
    result.exit_status = Some(1);
    assert_invalid(parse_sockets(result));
}

#[test]
fn required_nullable_fields_accept_explicit_null_and_reject_absence() {
    let mut resources = value(RESOURCES);
    resources["data"]["cpu_percent"] = Value::Null;
    let (snapshot, _) = parse_resources(encoded(&resources)).unwrap();
    assert!(matches!(snapshot.cpu, CpuSample::Unavailable));
    resources["data"]
        .as_object_mut()
        .unwrap()
        .remove("cpu_percent");
    assert_invalid(parse_resources(encoded(&resources)));

    let mut processes = value(PROCESSES);
    processes["data"]["processes"][0]["working_set_bytes"] = Value::Null;
    assert_eq!(
        parse_processes(encoded(&processes)).unwrap()[0].working_set_bytes,
        None
    );
    processes["data"]["processes"][0]
        .as_object_mut()
        .unwrap()
        .remove("working_set_bytes");
    assert_invalid(parse_processes(encoded(&processes)));

    let mut failure = value(DENIED_SOCKETS);
    let mut explicit_null = encoded(&failure);
    explicit_null.exit_status = Some(1);
    assert!(matches!(
        parse_sockets(explicit_null),
        Err(MonitorError::CapabilityUnavailable {
            capability: Capability::Sockets,
            reason: CapabilityReason::PermissionDenied,
        })
    ));
    failure.as_object_mut().unwrap().remove("data");
    let mut absent = encoded(&failure);
    absent.exit_status = Some(1);
    assert_invalid(parse_sockets(absent));
}

#[test]
fn combined_output_accepts_exact_byte_bound_and_rejects_one_extra() {
    let mut input = PROBE.to_owned();
    input.insert_str(1, &" ".repeat(MAX_BYTES - input.len()));
    assert_eq!(input.len(), MAX_BYTES);
    assert!(parse_probe(output(&input)).is_ok());
    input.insert(1, ' ');
    assert_invalid(parse_probe(output(&input)));
}

#[test]
fn resources_preserve_unicode_wide_counters_and_reported_cpu_provenance() {
    let (snapshot, capacity) = parse_resources(output(RESOURCES)).unwrap();
    assert_eq!(snapshot.platform, MonitorPlatform::Windows);
    assert_eq!(snapshot.boot_time, 1_794_787_200);
    assert_eq!(snapshot.uptime_seconds, 3600.25);
    assert_eq!(snapshot.cpu.reported_percent(), Some(12.5));
    assert!(snapshot.cpu.cumulative().is_none());
    assert_eq!(snapshot.networks[0].name, "以太网 适配器");
    assert_eq!(snapshot.networks[0].received_bytes, 9_007_199_254_740_993);
    assert_eq!(snapshot.networks[0].transmitted_bytes, u64::MAX);
    assert_eq!(capacity, CapabilityState::Available);
}

#[test]
fn unknown_cpu_available_memory_load_swap_and_disk_stay_unknown() {
    let (snapshot, capacity) = parse_resources(output(UNKNOWN)).unwrap();
    assert!(matches!(snapshot.cpu, CpuSample::Unavailable));
    assert_eq!(snapshot.cpu.reported_percent(), None);
    assert_eq!(snapshot.cpu.cumulative(), None);
    assert_eq!(snapshot.load_average, None);
    assert_eq!(snapshot.memory.available_bytes, None);
    assert_eq!(snapshot.memory.used_bytes(), None);
    assert_eq!(snapshot.memory.free_bytes, Some(4_294_967_296));
    assert_eq!(snapshot.memory.swap_total_bytes, None);
    assert_eq!(snapshot.memory.swap_free_bytes, None);
    assert!(matches!(snapshot.disk_io, Err(DiskIoError::MissingData)));
    assert!(snapshot.networks.is_empty());
    assert!(snapshot.filesystems.is_empty());
    assert_eq!(capacity, CapabilityState::Unknown);
}

#[test]
fn capacity_uses_total_free_for_used_and_keeps_quota_available_distinct() {
    let (snapshot, _) = parse_resources(output(RESOURCES)).unwrap();
    let filesystem = &snapshot.filesystems[0];
    assert_eq!(filesystem.total_bytes, 1000);
    assert_eq!(filesystem.used_bytes, 600);
    assert_eq!(filesystem.available_bytes, 300);
    assert_eq!(filesystem.used_percent, 60);
}

#[test]
fn capacity_percent_uses_checked_wide_integer_arithmetic() {
    let mut frame = value(RESOURCES);
    frame["data"]["filesystems"][0]["total_bytes"] = json!(u64::MAX.to_string());
    frame["data"]["filesystems"][0]["total_free_bytes"] = json!("0");
    frame["data"]["filesystems"][0]["available_bytes"] = json!("0");
    let (snapshot, _) = parse_resources(encoded(&frame)).unwrap();
    assert_eq!(snapshot.filesystems[0].used_bytes, u64::MAX);
    assert_eq!(snapshot.filesystems[0].used_percent, 100);
    frame["data"]["filesystems"][0]["total_bytes"] = json!("3");
    frame["data"]["filesystems"][0]["total_free_bytes"] = json!("2");
    let (snapshot, _) = parse_resources(encoded(&frame)).unwrap();
    assert_eq!(snapshot.filesystems[0].used_percent, 33);
}

#[test]
fn typed_capacity_failure_preserves_core_and_never_reports_empty_available() {
    for (status, expected) in [
        ("command_missing", CapabilityReason::CommandMissing),
        ("permission_denied", CapabilityReason::PermissionDenied),
        ("command_failed", CapabilityReason::CommandFailed),
        ("unsupported_options", CapabilityReason::UnsupportedOptions),
    ] {
        let mut frame = value(RESOURCES);
        frame["data"]["capacity_status"] = json!(status);
        frame["data"]["filesystem_count"] = json!(0);
        frame["data"]["filesystems"] = json!([]);
        let (snapshot, capacity) = parse_resources(encoded(&frame)).unwrap();
        assert_eq!(snapshot.cpu.reported_percent(), Some(12.5));
        assert_eq!(snapshot.networks.len(), 1);
        assert!(snapshot.filesystems.is_empty());
        assert_eq!(capacity, CapabilityState::Unavailable(expected));
    }
}

#[test]
fn empty_successful_capacity_is_distinct_from_skipped_and_partial_failure() {
    let mut frame = value(UNKNOWN);
    frame["data"]["capacity_status"] = json!("ok");
    let (_, capacity) = parse_resources(encoded(&frame)).unwrap();
    assert_eq!(capacity, CapabilityState::Available);
    for status in ["skipped", "command_failed", "permission_denied"] {
        let mut partial = value(RESOURCES);
        partial["data"]["capacity_status"] = json!(status);
        assert_invalid(parse_resources(encoded(&partial)));
    }
}

#[test]
fn resources_require_every_field_including_nullable_cpu() {
    let original = value(RESOURCES);
    let keys: Vec<_> = original["data"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    for key in keys {
        let mut frame = original.clone();
        frame["data"].as_object_mut().unwrap().remove(&key);
        assert_invalid(parse_resources(encoded(&frame)));
    }
    let mut frame = original.clone();
    frame["data"]["unexpected"] = json!(1);
    assert_invalid(parse_resources(encoded(&frame)));
}

#[test]
fn nested_resources_reject_unknown_missing_and_duplicate_keys() {
    for path in ["networks", "filesystems"] {
        let original = value(RESOURCES);
        let keys: Vec<_> = original["data"][path][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        for key in keys {
            let mut frame = original.clone();
            frame["data"][path][0].as_object_mut().unwrap().remove(&key);
            assert_invalid(parse_resources(encoded(&frame)));
        }
        let mut frame = original.clone();
        frame["data"][path][0]["unexpected"] = json!(true);
        assert_invalid(parse_resources(encoded(&frame)));
    }
    for (from, to) in [
        (
            "\"network_count\":1",
            "\"network_count\":1,\"network_count\":1",
        ),
        (
            "\"received_bytes\":\"9007199254740993\"",
            "\"received_bytes\":\"9007199254740993\",\"received_bytes\":\"9007199254740993\"",
        ),
        (
            "\"total_free_bytes\":\"400\"",
            "\"total_free_bytes\":\"400\",\"total_free_bytes\":\"400\"",
        ),
    ] {
        let duplicate = RESOURCES.replace(from, to);
        assert_ne!(duplicate, RESOURCES);
        assert_invalid(parse_resources(output(&duplicate)));
    }
}

#[test]
fn resources_reject_noncanonical_overflow_and_numeric_byte_counters() {
    for field in ["boot_time", "memory_total_bytes", "memory_free_bytes"] {
        for invalid in [
            json!(""),
            json!("00"),
            json!("01"),
            json!("-1"),
            json!("+1"),
            json!("1.0"),
            json!("1e3"),
            json!(" 1"),
            json!("1 "),
            json!("１"),
            json!("18446744073709551616"),
            json!(1),
            Value::Null,
        ] {
            let mut frame = value(RESOURCES);
            frame["data"][field] = invalid;
            assert_invalid(parse_resources(encoded(&frame)));
        }
    }
    let mut frame = value(RESOURCES);
    frame["data"]["networks"][0]["transmitted_bytes"] = json!("18446744073709551616");
    assert_invalid(parse_resources(encoded(&frame)));
    frame = value(RESOURCES);
    frame["data"]["filesystems"][0]["total_bytes"] = json!(1000);
    assert_invalid(parse_resources(encoded(&frame)));
}

#[test]
fn resources_reject_invalid_percent_uptime_memory_and_capacity_boundaries() {
    for cpu in [json!(-0.1), json!(100.1), json!("12.5"), json!(true)] {
        let mut frame = value(RESOURCES);
        frame["data"]["cpu_percent"] = cpu;
        assert_invalid(parse_resources(encoded(&frame)));
    }
    for uptime in [json!(-0.1), json!("1"), Value::Null] {
        let mut frame = value(RESOURCES);
        frame["data"]["uptime_seconds"] = uptime;
        assert_invalid(parse_resources(encoded(&frame)));
    }
    for (key, wrong) in [
        ("memory_total_bytes", "0"),
        ("memory_free_bytes", "17179869185"),
    ] {
        let mut frame = value(RESOURCES);
        frame["data"][key] = json!(wrong);
        assert_invalid(parse_resources(encoded(&frame)));
    }
    for (key, wrong) in [
        ("total_bytes", "0"),
        ("total_free_bytes", "1001"),
        ("available_bytes", "401"),
    ] {
        let mut frame = value(RESOURCES);
        frame["data"]["filesystems"][0][key] = json!(wrong);
        assert_invalid(parse_resources(encoded(&frame)));
    }
    assert_invalid(parse_resources(output(
        &RESOURCES.replace("\"cpu_percent\":12.5", "\"cpu_percent\":1e999"),
    )));
}

#[test]
fn resources_accept_zero_uptime_and_cpu_percentage_endpoints() {
    for cpu in [0.0, 100.0] {
        let mut frame = value(RESOURCES);
        frame["data"]["uptime_seconds"] = json!(0);
        frame["data"]["cpu_percent"] = json!(cpu);
        frame["data"]["memory_free_bytes"] = frame["data"]["memory_total_bytes"].clone();
        let (snapshot, _) = parse_resources(encoded(&frame)).unwrap();
        assert_eq!(snapshot.cpu.reported_percent(), Some(cpu));
        assert_eq!(snapshot.uptime_seconds, 0.0);
        assert_eq!(snapshot.memory.available_bytes, None);
    }
}

#[test]
fn resources_validate_counts_duplicates_and_independent_array_bounds() {
    for field in ["network_count", "filesystem_count"] {
        for wrong in [json!(0), json!(2), json!(-1), json!(1.0), json!("1")] {
            let mut frame = value(RESOURCES);
            frame["data"][field] = wrong;
            assert_invalid(parse_resources(encoded(&frame)));
        }
    }
    for (count, rows) in [
        ("network_count", "networks"),
        ("filesystem_count", "filesystems"),
    ] {
        let mut frame = value(RESOURCES);
        let row = frame["data"][rows][0].clone();
        frame["data"][count] = json!(2);
        frame["data"][rows].as_array_mut().unwrap().push(row);
        assert_invalid(parse_resources(encoded(&frame)));
    }
    let mut frame = value(RESOURCES);
    frame["data"]["filesystem_count"] = json!(2);
    let mut duplicate = frame["data"]["filesystems"][0].clone();
    duplicate["source"] = json!("c:\\");
    duplicate["mount"] = json!("c:\\");
    frame["data"]["filesystems"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    assert_invalid(parse_resources(encoded(&frame)));
}

#[test]
fn exact_network_and_filesystem_limits_accept_and_one_extra_rejects() {
    for (rows, count, limit) in [
        ("networks", "network_count", MAX_NETWORKS),
        ("filesystems", "filesystem_count", MAX_FILESYSTEMS),
    ] {
        let mut frame = value(RESOURCES);
        let template = frame["data"][rows][0].clone();
        let records: Vec<_> = (0..limit)
            .map(|index| {
                let mut row = template.clone();
                if rows == "networks" {
                    row["name"] = json!(format!("Ethernet {index}"));
                } else {
                    row["source"] = json!(format!("volume-{index}"));
                    row["mount"] = json!(format!("mount-{index}"));
                }
                row
            })
            .collect();
        frame["data"][rows] = json!(records);
        frame["data"][count] = json!(limit);
        assert!(parse_resources(encoded(&frame)).is_ok());
        frame["data"][count] = json!(limit + 1);
        frame["data"][rows].as_array_mut().unwrap().push(template);
        assert_invalid(parse_resources(encoded(&frame)));
    }
}

#[test]
fn display_fields_reject_controls_escape_bidi_and_hidden_text() {
    for text in [
        "",
        " ",
        "line\nbreak",
        "carriage\rreturn",
        "tab\ttext",
        "nul\0text",
        "esc\u{1b}[31m",
        "c1\u{9b}31m",
        "bidi\u{202e}text",
        "isolate\u{2066}text",
        "line\u{2028}separator",
        "zero\u{200b}width",
        "bom\u{feff}text",
        "replacement\u{fffd}",
    ] {
        for field in ["name", "source", "mount"] {
            let mut frame = value(RESOURCES);
            let row = if field == "name" {
                "networks"
            } else {
                "filesystems"
            };
            frame["data"][row][0][field] = json!(text);
            assert_invalid(parse_resources(encoded(&frame)));
        }
        let mut frame = value(PROCESSES);
        frame["data"]["processes"][0]["name"] = json!(text);
        assert_invalid(parse_processes(encoded(&frame)));
    }
}

#[test]
fn display_bounds_count_utf8_bytes_and_preserve_safe_unicode() {
    let mut frame = value(PROCESSES);
    frame["data"]["processes"][0]["name"] = json!("界".repeat(MAX_TEXT / 3));
    assert!(parse_processes(encoded(&frame)).is_ok());
    frame["data"]["processes"][0]["name"] = json!("a".repeat(MAX_TEXT));
    assert!(parse_processes(encoded(&frame)).is_ok());
    frame["data"]["processes"][0]["name"] = json!("a".repeat(MAX_TEXT + 1));
    assert_invalid(parse_processes(encoded(&frame)));
    frame["data"]["processes"][0]["name"] = json!("界".repeat(MAX_TEXT / 3 + 1));
    assert_invalid(parse_processes(encoded(&frame)));
}

#[test]
fn processes_preserve_pid_zero_unicode_and_optional_working_set_only() {
    let rows = parse_processes(output(PROCESSES)).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].pid, 0);
    assert_eq!(rows[0].working_set_bytes, None);
    assert_eq!(rows[1].command, "编辑器");
    assert_eq!(rows[1].working_set_bytes, Some(9_007_199_254_740_993));
    for row in rows {
        assert_eq!(row.user, None);
        assert_eq!(row.cpu_percent, None);
        assert_eq!(row.memory_percent, None);
    }
}

#[test]
fn processes_accept_explicit_empty_singleton_and_maximum_pid() {
    assert!(parse_processes(output(EMPTY_PROCESSES)).unwrap().is_empty());
    let mut frame = value(PROCESSES);
    frame["data"]["count"] = json!(1);
    frame["data"]["processes"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    assert_eq!(parse_processes(encoded(&frame)).unwrap()[0].pid, 0);
    frame["data"]["processes"][0]["pid"] = json!(u32::MAX.to_string());
    frame["data"]["processes"][0]["working_set_bytes"] = json!(u64::MAX.to_string());
    let row = parse_processes(encoded(&frame)).unwrap().remove(0);
    assert_eq!(row.pid, u32::MAX);
    assert_eq!(row.working_set_bytes, Some(u64::MAX));
}

#[test]
fn processes_reject_duplicate_unordered_overflow_and_noncanonical_pids() {
    for wrong in [
        json!("0"),
        json!("00"),
        json!("-1"),
        json!("4294967296"),
        json!(42),
        json!("4.2"),
        json!("18446744073709551616"),
    ] {
        let mut frame = value(PROCESSES);
        frame["data"]["processes"][1]["pid"] = wrong;
        assert_invalid(parse_processes(encoded(&frame)));
    }
    let mut frame = value(PROCESSES);
    frame["data"]["processes"].as_array_mut().unwrap().reverse();
    assert_invalid(parse_processes(encoded(&frame)));
}

#[test]
fn processes_reject_missing_nullable_working_set_and_duplicate_record_fields() {
    let original = value(PROCESSES);
    for key in ["pid", "name", "working_set_bytes"] {
        let mut frame = original.clone();
        frame["data"]["processes"][0]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert_invalid(parse_processes(encoded(&frame)));
    }
    let mut frame = original.clone();
    frame["data"]["processes"][0]["cpu"] = json!(5);
    assert_invalid(parse_processes(encoded(&frame)));
    for (from, to) in [
        ("\"count\":2", "\"count\":2,\"count\":2"),
        ("\"pid\":\"0\"", "\"pid\":\"0\",\"pid\":\"0\""),
        (
            "\"working_set_bytes\":null",
            "\"working_set_bytes\":null,\"working_set_bytes\":null",
        ),
    ] {
        let duplicated = PROCESSES.replace(from, to);
        assert_ne!(duplicated, PROCESSES);
        assert_invalid(parse_processes(output(&duplicated)));
    }
}

#[test]
fn processes_reject_bad_counts_null_arrays_and_working_set_overflow() {
    for (field, wrong) in [
        ("count", json!(0)),
        ("count", json!(3)),
        ("count", json!(-1)),
        ("count", json!(2.0)),
        ("count", json!("2")),
        ("processes", Value::Null),
        ("processes", json!({})),
    ] {
        let mut frame = value(PROCESSES);
        frame["data"][field] = wrong;
        assert_invalid(parse_processes(encoded(&frame)));
    }
    for wrong in [json!("-1"), json!("18446744073709551616"), json!(0)] {
        let mut frame = value(PROCESSES);
        frame["data"]["processes"][0]["working_set_bytes"] = wrong;
        assert_invalid(parse_processes(encoded(&frame)));
    }
}

#[test]
fn processes_accept_exact_inventory_bound_and_reject_one_extra() {
    let mut frame = value(EMPTY_PROCESSES);
    let records: Vec<_> = (0..MAX_ROWS)
        .map(|pid| json!({"pid":pid.to_string(),"name":"p","working_set_bytes":null}))
        .collect();
    frame["data"]["count"] = json!(MAX_ROWS);
    frame["data"]["processes"] = json!(records);
    assert_eq!(parse_processes(encoded(&frame)).unwrap().len(), MAX_ROWS);
    frame["data"]["count"] = json!(MAX_ROWS + 1);
    frame["data"]["processes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"pid":MAX_ROWS.to_string(),"name":"p","working_set_bytes":null}));
    assert_invalid(parse_processes(encoded(&frame)));
}

#[test]
fn sockets_preserve_numeric_endpoints_and_display_only_pid_metadata() {
    let sockets = parse_sockets(output(SOCKETS)).unwrap();
    assert_eq!(sockets.len(), 2);
    assert_eq!(sockets[0].protocol, "tcp");
    assert_eq!(sockets[0].state, "LISTEN");
    assert_eq!(sockets[0].local, "127.0.0.1:1234");
    assert_eq!(sockets[1].protocol, "udp");
    assert_eq!(sockets[1].state, "BOUND");
    assert_eq!(sockets[1].local, "[fe80::1%12]:65535");
    assert_eq!(sockets[0].process.as_deref(), Some("pid=4"));
    assert!(sockets.iter().all(|socket| socket.peer == "*"));
}

#[test]
fn sockets_require_explicit_empty_success_and_accept_singleton_crlf() {
    assert!(parse_sockets(output(EMPTY_SOCKETS)).unwrap().is_empty());
    assert_invalid(parse_sockets(output("")));
    let mut frame = value(SOCKETS);
    frame["data"]["count"] = json!(1);
    frame["data"]["sockets"].as_array_mut().unwrap().truncate(1);
    let input = format!("{}\r\n", serde_json::to_string(&frame).unwrap());
    assert_eq!(parse_sockets(output(&input)).unwrap().len(), 1);
}

#[test]
fn sockets_accept_ipv4_ipv6_wildcards_scopes_and_numeric_pid_boundaries() {
    for (address, expected) in [
        ("0.0.0.0", "0.0.0.0:1234"),
        ("::", "[::]:1234"),
        ("::1", "[::1]:1234"),
        ("2001:DB8:0:0:0:0:0:1", "[2001:db8::1]:1234"),
        ("fe80::1%0", "[fe80::1%0]:1234"),
        ("fe80::1%4294967295", "[fe80::1%4294967295]:1234"),
        ("::ffff:127.0.0.1", "[::ffff:127.0.0.1]:1234"),
    ] {
        let mut frame = value(SOCKETS);
        frame["data"]["sockets"][0]["address"] = json!(address);
        frame["data"]["sockets"][0]["pid"] = json!(u32::MAX.to_string());
        assert_eq!(parse_sockets(encoded(&frame)).unwrap()[0].local, expected);
    }
    let mut frame = value(SOCKETS);
    frame["data"]["sockets"][0]["pid"] = json!("0");
    assert_eq!(
        parse_sockets(encoded(&frame)).unwrap()[0]
            .process
            .as_deref(),
        Some("pid=0")
    );
}

#[test]
fn sockets_reject_names_injection_invalid_addresses_and_invalid_scopes() {
    for invalid in [
        "",
        "*",
        "localhost",
        "example.test",
        "127.0.0.1:1234",
        "127.0.0.1%1",
        "127.000.0.1",
        "256.0.0.1",
        " 127.0.0.1",
        "127.0.0.1 ",
        "[::1]",
        "[::1]:1234",
        "::1%eth0",
        "::1%01",
        "::1%-1",
        "::1%+1",
        "::1%4294967296",
        "::1%",
        "::1%1%2",
        "::1\n",
        "::g",
        "127.0.0.1;whoami",
        "界",
    ] {
        let mut frame = value(SOCKETS);
        frame["data"]["sockets"][0]["address"] = json!(invalid);
        assert_invalid(parse_sockets(encoded(&frame)));
    }
}

#[test]
fn sockets_reject_invalid_protocol_port_and_pid_fields() {
    for wrong in ["TCP", "UDP", "raw", "", "tcp\n"] {
        let mut frame = value(SOCKETS);
        frame["data"]["sockets"][0]["protocol"] = json!(wrong);
        assert_invalid(parse_sockets(encoded(&frame)));
    }
    for wrong in [
        json!("0"),
        json!("65536"),
        json!("01234"),
        json!("-1"),
        json!("+1"),
        json!("1234.0"),
        json!(1234),
        Value::Null,
    ] {
        let mut frame = value(SOCKETS);
        frame["data"]["sockets"][0]["port"] = wrong;
        assert_invalid(parse_sockets(encoded(&frame)));
    }
    let mut frame = value(SOCKETS);
    frame["data"]["sockets"][0]["pid"] = json!("4294967296");
    assert_invalid(parse_sockets(encoded(&frame)));
}

#[test]
fn sockets_reject_partial_mismatched_duplicate_and_missing_records() {
    let original = value(SOCKETS);
    for field in ["protocol", "address", "port", "pid"] {
        let mut frame = original.clone();
        frame["data"]["sockets"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_invalid(parse_sockets(encoded(&frame)));
    }
    let mut frame = original.clone();
    frame["data"]["sockets"][0]["peer"] = json!("*");
    assert_invalid(parse_sockets(encoded(&frame)));
    for (field, wrong) in [
        ("count", json!(0)),
        ("count", json!(1)),
        ("count", json!(2.0)),
        ("count", json!("2")),
        ("sockets", Value::Null),
        ("sockets", json!({})),
    ] {
        let mut frame = original.clone();
        frame["data"][field] = wrong;
        assert_invalid(parse_sockets(encoded(&frame)));
    }
    let mut frame = original.clone();
    frame["data"]["sockets"][1] = frame["data"]["sockets"][0].clone();
    assert_invalid(parse_sockets(encoded(&frame)));
    let duplicated = SOCKETS.replace("\"pid\":\"4\"", "\"pid\":\"4\",\"pid\":\"4\"");
    assert_invalid(parse_sockets(output(&duplicated)));
}

#[test]
fn sockets_normalize_addresses_for_duplicates_and_keep_distinct_owners() {
    let mut frame = value(SOCKETS);
    frame["data"]["sockets"][0]["address"] = json!("2001:db8::1");
    frame["data"]["sockets"][1] = frame["data"]["sockets"][0].clone();
    frame["data"]["sockets"][1]["address"] = json!("2001:0DB8:0:0:0:0:0:1");
    assert_invalid(parse_sockets(encoded(&frame)));
    frame["data"]["sockets"][1]["pid"] = json!("5");
    assert_eq!(parse_sockets(encoded(&frame)).unwrap().len(), 2);
}

#[test]
fn sockets_accept_exact_inventory_bound_and_reject_one_extra() {
    let mut frame = value(EMPTY_SOCKETS);
    let records: Vec<_> = (0..MAX_ROWS)
        .map(|pid| json!({"protocol":"udp","address":"::","port":"1","pid":pid.to_string()}))
        .collect();
    frame["data"]["count"] = json!(MAX_ROWS);
    frame["data"]["sockets"] = json!(records);
    assert_eq!(parse_sockets(encoded(&frame)).unwrap().len(), MAX_ROWS);
    frame["data"]["count"] = json!(MAX_ROWS + 1);
    frame["data"]["sockets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"protocol":"udp","address":"::","port":"1","pid":MAX_ROWS.to_string()}));
    assert_invalid(parse_sockets(encoded(&frame)));
}
