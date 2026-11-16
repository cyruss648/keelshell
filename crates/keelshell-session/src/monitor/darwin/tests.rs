#![cfg(test)]
#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "Test fixtures and success assertions must fail immediately on unexpected values"
)]

use super::*;

const SAMPLE: &str = include_str!("fixtures/schema1-skipped.txt");
const DF: &str = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
                 /dev/disk3s1s1 994662584 12000000 280000000 5% /\n\
                 /dev/disk3s5 994662584 600000000 280000000 69% /System/Volumes/Data\n\
                 /dev/disk4s1 1000000 200000 800000 20% /Volumes/测试 disk\n";

fn replace_field(input: &str, key: &str, value: &str) -> String {
    let prefix = format!("{key}\t");
    let old = input
        .lines()
        .find(|line| line.starts_with(&prefix))
        .expect("fixture field");
    input.replacen(old, &format!("{key}\t{value}"), 1)
}

fn with_capacity(status: &str, df: &str) -> String {
    let input = replace_field(SAMPLE, "df_status", status);
    let input = replace_field(&input, "df_bytes", &df.len().to_string());
    input.replace(
        "\n@@KS:df@@\n\n@@KS:end@@\n",
        &format!("\n@@KS:df@@\n{df}\n@@KS:end@@\n"),
    )
}

fn snapshot(input: &str) -> Snapshot {
    parse(input, &[]).expect("valid Darwin frame").0
}

fn assert_invalid(input: &str) {
    assert!(
        matches!(parse(input, &[]), Err(MonitorError::InvalidData(_))),
        "invalid frame was accepted"
    );
}

#[test]
fn realistic_apple_silicon_sample_uses_named_mach_cpu_states() {
    assert_eq!(
        snapshot(SAMPLE).cpu,
        CpuSample::Counters(CpuCounters {
            ticks: [1000, 10, 500, 8000, 0, 0, 0, 0]
        }),
    );
}

#[test]
fn sample_identifies_darwin_and_keeps_boot_seconds() {
    let sample = snapshot(SAMPLE);
    assert_eq!(
        (sample.platform, sample.boot_time),
        (MonitorPlatform::Darwin, 1794787200),
    );
}

#[test]
fn free_pages_are_not_available_or_used_memory() {
    let memory = snapshot(SAMPLE).memory;
    assert_eq!(
        (
            memory.total_bytes,
            memory.free_bytes,
            memory.available_bytes,
            memory.used_bytes(),
            memory.swap_total_bytes,
            memory.swap_free_bytes,
        ),
        (17179869184, Some(1073741824), None, None, None, None),
    );
}

#[test]
fn unsupported_disk_io_is_explicitly_missing() {
    assert_eq!(snapshot(SAMPLE).disk_io, Err(DiskIoError::MissingData));
}

#[test]
fn skipped_capacity_is_unobserved_and_empty() {
    let (sample, state) = parse(SAMPLE, &[]).unwrap();
    assert_eq!(
        (sample.filesystems, state),
        (Vec::new(), CapabilityState::Unknown)
    );
}

#[test]
fn valid_capacity_converts_only_1024_byte_blocks_and_preserves_mount_spaces() {
    let (sample, state) = parse(&with_capacity("ok", DF), &[]).unwrap();
    assert_eq!(
        (
            state,
            sample.filesystems.len(),
            sample.filesystems[2].mount.as_str(),
            sample.filesystems[2].total_bytes,
        ),
        (
            CapabilityState::Available,
            3,
            "/Volumes/测试 disk",
            1024000000
        ),
    );
}

#[test]
fn optional_command_failures_preserve_all_core_counters() {
    for (status, reason) in [
        ("missing", CapabilityReason::CommandMissing),
        ("permission_denied", CapabilityReason::PermissionDenied),
        ("failed", CapabilityReason::CommandFailed),
        ("timeout", CapabilityReason::CommandFailed),
        ("invalid_output", CapabilityReason::InvalidOutput),
    ] {
        let (sample, state) = parse(&with_capacity(status, ""), b"private diagnostic").unwrap();
        assert_eq!(state, CapabilityState::Unavailable(reason), "{status}");
        assert_eq!(sample.cpu, snapshot(SAMPLE).cpu, "{status}");
        assert!(sample.filesystems.is_empty(), "{status}");
    }
}

#[test]
fn invalid_optional_df_rows_do_not_discard_core_resources() {
    for df in [
        "",
        "Filesystem 512-blocks Used Available Capacity Mounted on\nx 1 1 0 100% /\n",
        "Filesystem 1024-blocks Used Available Capacity Mounted on\nx -1 1 0 1% /\n",
        "Filesystem 1024-blocks Used Available Capacity Mounted on\nx +1 1 0 1% /\n",
        "Filesystem 1024-blocks Used Available Capacity Mounted on\nx 18446744073709551615 1 0 1% /\n",
        "Filesystem 1024-blocks Used Available Capacity Mounted on\nx 1 1 0 NaN% /\n",
        "Filesystem 1024-blocks Used Available Capacity Mounted on\nx 1 1 0 4294967296% /\n",
        "Filesystem 1024-blocks Used Available Capacity Mounted on\nx 1 1 0 1% relative\n",
        "Filesystem 1024-blocks Used Available Capacity Mounted on\nx 1 1 0 1% /\ny 1 1 0 1% /\n",
        "Filesystem 1024-blocks Used Available Capacity Mounted on\nx 1 1 0 1% /\n\n",
    ] {
        let (sample, state) = parse(&with_capacity("ok", df), &[]).unwrap();
        assert_eq!(sample.memory.free_bytes, Some(1073741824));
        assert_eq!(
            state,
            CapabilityState::Unavailable(CapabilityReason::InvalidOutput),
        );
    }
}

#[test]
fn automount_source_spaces_and_mount_spaces_preserve_other_filesystems() {
    let df = format!("{DF}map auto_home 0 0 0 100% /System/Volumes/Data/home\n");
    let (sample, state) = parse(&with_capacity("ok", &df), &[]).unwrap();
    assert_eq!(state, CapabilityState::Available);
    assert_eq!(sample.filesystems.len(), 4);
    assert_eq!(sample.filesystems[2].mount, "/Volumes/测试 disk");
    assert_eq!(sample.filesystems[3].source, "map auto_home");
    assert_eq!(sample.filesystems[3].mount, "/System/Volumes/Data/home");
    assert_eq!(sample.filesystems[3].total_bytes, 0);
}

#[test]
fn ambiguous_unescaped_df_names_and_overlong_rows_are_rejected() {
    for row in [
        "source 1 2 3 4% /also-source 5 6 7 8% /actual",
        &format!("{} 1 2 3 4% /", "x".repeat(8192)),
    ] {
        assert!(filesystem_columns(row).is_err());
    }
    let df = "Filesystem 1024-blocks Used Available Capacity Mounted on\nsource 1 2 3 4% /also-source 5 6 7 8% /actual\n";
    let (sample, state) = parse(&with_capacity("ok", df), &[]).unwrap();
    assert!(sample.filesystems.is_empty());
    assert_eq!(
        state,
        CapabilityState::Unavailable(CapabilityReason::InvalidOutput)
    );
}

#[test]
fn capacity_percent_over_100_is_not_invented_as_utilization() {
    let df = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
              /dev/disk4s1 1000 1100 0 110% /Volumes/data\n";
    assert_eq!(
        snapshot(&with_capacity("ok", df)).filesystems[0].used_percent,
        110
    );
}

#[test]
fn failed_or_skipped_capacity_cannot_smuggle_a_payload() {
    for status in ["skipped", "failed", "missing"] {
        assert_invalid(&with_capacity(status, DF));
    }
}

#[test]
fn unknown_capacity_status_is_rejected() {
    for status in ["unknown", "0", "127", "OK", "unsupported_options"] {
        assert_invalid(&replace_field(SAMPLE, "df_status", status));
    }
}

#[test]
fn capacity_length_is_bytes_including_multibyte_mounts() {
    let frame = with_capacity("ok", DF);
    assert_invalid(&replace_field(
        &frame,
        "df_bytes",
        &DF.chars().count().to_string(),
    ));
}

#[test]
fn truncated_frames_are_never_accepted_at_any_byte_boundary() {
    for end in 0..SAMPLE.len() {
        assert_invalid(&SAMPLE[..end]);
    }
}

#[test]
fn capacity_truncation_is_not_misclassified_as_optional_failure() {
    let frame = with_capacity("ok", DF);
    for (end, _) in frame.char_indices().skip(START.len()) {
        assert_invalid(&frame[..end]);
    }
}

#[test]
fn preambles_carriage_returns_and_trailing_data_are_rejected() {
    for input in [
        format!("notice\n{SAMPLE}"),
        SAMPLE.replace('\n', "\r\n"),
        format!("{SAMPLE}\n"),
        format!("{SAMPLE}private tail"),
        SAMPLE.replace("schema\t1", "schema\t1\0"),
        SAMPLE.replace("@@KS:darwin@@", "@@KS:Darwin@@"),
    ] {
        assert_invalid(&input);
    }
}

#[test]
fn duplicate_completion_or_capacity_markers_are_rejected() {
    assert_invalid(&format!("{SAMPLE}@@KS:end@@\n"));
    assert_invalid(&SAMPLE.replace("@@KS:df@@", "@@KS:df@@\n@@KS:df@@"));
}

#[test]
fn frame_markers_cannot_be_hidden_inside_counted_capacity_payload() {
    for marker in ["@@KS:darwin@@", "@@KS:df@@", "@@KS:end@@"] {
        assert_invalid(&with_capacity("ok", &format!("{DF}{marker}\n")));
    }
}

#[test]
fn missing_or_duplicate_singleton_fields_are_rejected() {
    for key in KEYS {
        let line = SAMPLE
            .lines()
            .find(|line| line.starts_with(&format!("{key}\t")))
            .unwrap();
        assert_invalid(&SAMPLE.replace(&format!("{line}\n"), ""));
        assert_invalid(&SAMPLE.replace(line, &format!("{line}\n{line}")));
    }
}

#[test]
fn unknown_fields_schemas_platforms_and_units_are_rejected() {
    for input in [
        SAMPLE.replace("schema\t1", "schema\t1\navailable_bytes\t1000"),
        replace_field(SAMPLE, "schema", "2"),
        replace_field(SAMPLE, "schema", "01"),
        replace_field(SAMPLE, "platform", "Linux"),
        replace_field(SAMPLE, "platform", "macOS"),
        replace_field(SAMPLE, "units", "KiB seconds ticks pages nanoseconds"),
        replace_field(
            SAMPLE,
            "units",
            "bytes milliseconds ticks pages nanoseconds",
        ),
        SAMPLE.replace("schema\t1", "schema\t1\textra"),
    ] {
        assert_invalid(&input);
    }
}

#[test]
fn untrusted_diagnostics_never_enter_parser_errors() {
    let error = parse("private remote preamble", b"private remote stderr").unwrap_err();
    assert!(matches!(
        error,
        MonitorError::InvalidData("missing Darwin frame")
    ));
}

#[test]
fn combined_stdout_and_stderr_limit_is_enforced() {
    let stderr = vec![b'x'; MAX_BYTES - SAMPLE.len()];
    assert!(parse(SAMPLE, &stderr).is_ok());
    let stderr = vec![b'x'; MAX_BYTES - SAMPLE.len() + 1];
    assert!(parse(SAMPLE, &stderr).is_err());
}

#[test]
fn oversized_frame_is_rejected_before_line_parsing() {
    assert_invalid(&"x".repeat(MAX_BYTES + 1));
}

#[test]
fn unsigned_fields_reject_signs_whitespace_nondecimal_and_overflow() {
    for key in [
        "boot_time",
        "boot_time_usec",
        "mach_ticks",
        "timebase_numer",
        "timebase_denom",
        "uptime_ns",
        "page_size",
        "sysconf_page_size",
        "memory_total_bytes",
        "memory_free_pages",
        "memory_free_bytes",
        "cpu_user",
        "cpu_nice",
        "cpu_system",
        "cpu_idle",
        "network_count",
        "df_bytes",
    ] {
        for value in [
            "-1",
            "+1",
            " 1",
            "1 ",
            "NaN",
            "inf",
            "1e3",
            "18446744073709551616",
        ] {
            assert_invalid(&replace_field(SAMPLE, key, value));
        }
    }
}

#[test]
fn cpu_words_follow_the_32_bit_mach_abi() {
    assert_eq!(
        snapshot(&replace_field(SAMPLE, "cpu_user", "4294967295"))
            .cpu
            .cumulative()
            .expect("Darwin Mach counters")
            .ticks[0],
        u32::MAX as u64,
    );
    assert_invalid(&replace_field(SAMPLE, "cpu_user", "4294967296"));
}

#[test]
fn boot_time_must_fit_positive_lp64_timeval_seconds() {
    for value in ["0", "9223372036854775808"] {
        assert_invalid(&replace_field(SAMPLE, "boot_time", value));
    }
    assert_invalid(&replace_field(SAMPLE, "boot_time_usec", "1000000"));
}

#[test]
fn load_averages_reject_nonfinite_negative_and_extra_tokens() {
    for key in ["load_1", "load_5", "load_15"] {
        for value in [
            "NaN", "inf", "-inf", "-0", "-1.0", "1e999", "1 2", "1..2", "1\t2",
        ] {
            assert_invalid(&replace_field(SAMPLE, key, value));
        }
        assert_invalid(&replace_field(SAMPLE, key, &"9".repeat(400)));
    }
}

#[test]
fn zero_load_and_free_memory_are_valid_measurements() {
    let frame = replace_field(SAMPLE, "load_1", "0.00000000000000000");
    let frame = replace_field(&frame, "memory_free_pages", "0");
    let frame = replace_field(&frame, "memory_free_bytes", "0");
    let sample = snapshot(&frame);
    assert_eq!(
        (
            sample.load_average.expect("Darwin load")[0],
            sample.memory.free_bytes
        ),
        (0.0, Some(0))
    );
}

#[test]
fn mach_clock_is_converted_from_integer_ticks_and_timebase() {
    assert_eq!(snapshot(SAMPLE).uptime_seconds, 1000.0);
}

#[test]
fn mach_conversion_floors_integer_nanoseconds_before_float_conversion() {
    let frame = replace_field(SAMPLE, "mach_ticks", "24000000001");
    let frame = replace_field(&frame, "uptime_ns", "1000000000041");
    assert_eq!(snapshot(&frame).uptime_seconds, 1000.000000041);
}

#[test]
fn inconsistent_zero_or_oversized_mach_timebases_are_rejected() {
    for key in [
        "mach_ticks",
        "timebase_numer",
        "timebase_denom",
        "uptime_ns",
    ] {
        assert_invalid(&replace_field(SAMPLE, key, "0"));
    }
    for key in ["timebase_numer", "timebase_denom"] {
        assert_invalid(&replace_field(SAMPLE, key, "4294967296"));
    }
    assert_invalid(&replace_field(SAMPLE, "uptime_ns", "999999999999"));
}

#[test]
fn mach_clock_products_do_not_wrap_into_a_valid_u64() {
    let frame = replace_field(SAMPLE, "mach_ticks", "18446744073709551615");
    let frame = replace_field(&frame, "timebase_numer", "4294967295");
    let frame = replace_field(&frame, "timebase_denom", "1");
    let frame = replace_field(&frame, "uptime_ns", "18446744069414584321");
    assert_invalid(&frame);
}

#[test]
fn intel_4k_pages_and_unit_timebase_are_supported() {
    let frame = replace_field(SAMPLE, "page_size", "4096");
    let frame = replace_field(&frame, "sysconf_page_size", "4096");
    let frame = replace_field(&frame, "memory_free_bytes", "268435456");
    let frame = replace_field(&frame, "timebase_numer", "1");
    let frame = replace_field(&frame, "timebase_denom", "1");
    let frame = replace_field(&frame, "mach_ticks", "1000000000000");
    assert_eq!(snapshot(&frame).memory.free_bytes, Some(268435456));
}

#[test]
fn page_size_must_match_kernel_and_memory_units() {
    for value in ["0", "1024", "16385", "131072"] {
        let frame = replace_field(SAMPLE, "page_size", value);
        let frame = replace_field(&frame, "sysconf_page_size", value);
        assert_invalid(&frame);
    }
    assert_invalid(&replace_field(SAMPLE, "sysconf_page_size", "4096"));
    assert_invalid(&replace_field(SAMPLE, "memory_free_bytes", "1073741825"));
    assert_invalid(&replace_field(SAMPLE, "memory_total_bytes", "17179869185"));
}

#[test]
fn missing_free_memory_and_free_over_total_are_rejected() {
    assert_invalid(&SAMPLE.replace("memory_free_pages\t65536\n", ""));
    assert_invalid(&SAMPLE.replace("memory_free_bytes\t1073741824\n", ""));
    let frame = replace_field(SAMPLE, "memory_free_pages", "1048577");
    let frame = replace_field(&frame, "memory_free_bytes", "17179885568");
    assert_invalid(&frame);
    assert_invalid(&replace_field(SAMPLE, "memory_total_bytes", "0"));
}

#[test]
fn pegged_legacy_vm_word_cannot_establish_real_free_pages() {
    let frame = replace_field(SAMPLE, "memory_free_pages", "4294967295");
    let frame = replace_field(&frame, "memory_free_bytes", "70368744161280");
    let frame = replace_field(&frame, "memory_total_bytes", "70368744177664");
    assert_invalid(&frame);
}

#[test]
fn network_counter_resets_are_preserved_for_parent_rate_guards() {
    let before = snapshot(SAMPLE);
    let after = snapshot(&SAMPLE.replace("network\ten0\t100000\t200000", "network\ten0\t1\t2"));
    assert!(
        after.networks[0].received_bytes < before.networks[0].received_bytes
            && after.networks[0].transmitted_bytes < before.networks[0].transmitted_bytes,
    );
}

#[test]
fn duplicate_network_names_are_rejected_even_with_different_counters() {
    let frame = replace_field(SAMPLE, "network_count", "2").replace(
        "network\ten0\t100000\t200000",
        "network\ten0\t1\t2\nnetwork\ten0\t3\t4",
    );
    assert_invalid(&frame);
}

#[test]
fn network_count_must_be_nonempty_exact_and_bounded() {
    for count in ["0", "2", "257"] {
        assert_invalid(&replace_field(SAMPLE, "network_count", count));
    }
    let rows = (0..257)
        .map(|index| format!("network\ten{index}\t1\t2"))
        .collect::<Vec<_>>()
        .join("\n");
    let frame = replace_field(SAMPLE, "network_count", "257")
        .replace("network\ten0\t100000\t200000", &rows);
    assert_invalid(&frame);
}

#[test]
fn exactly_256_distinct_interfaces_are_accepted() {
    let rows = (0..256)
        .map(|index| format!("network\ten{index}\t1\t2"))
        .collect::<Vec<_>>()
        .join("\n");
    let frame = replace_field(SAMPLE, "network_count", "256")
        .replace("network\ten0\t100000\t200000", &rows);
    assert_eq!(snapshot(&frame).networks.len(), 256);
}

#[test]
fn network_names_and_counter_columns_are_strict() {
    for row in [
        "network\t\t1\t2",
        "network\t0123456789012345\t1\t2",
        "network\tprivate host\t1\t2",
        "network\tfe80::1\t1\t2",
        "network\ten0\t-1\t2",
        "network\ten0\t1\tNaN",
        "network\ten0\t18446744073709551616\t2",
        "network\ten0\t1",
        "network\ten0\t1\t2\t3",
    ] {
        assert_invalid(&SAMPLE.replace("network\ten0\t100000\t200000", row));
    }
}

#[test]
fn max_u64_network_counters_are_not_rounded_through_floats() {
    let sample = snapshot(&SAMPLE.replace(
        "network\ten0\t100000\t200000",
        "network\ten0\t18446744073709551615\t18446744073709551614",
    ));
    assert_eq!(sample.networks[0].received_bytes, u64::MAX);
}

#[test]
fn generated_commands_use_isolated_stdin_python_and_explicit_skip_flags() {
    for (skip, arg) in [(true, "1"), (false, "0")] {
        let command = script(skip);
        assert!(command.contains(&format!("\"$ks_python\" -I -S -B - {arg} ")));
        assert!(command.contains("for ks_candidate in /usr/bin/python3 /opt/homebrew/bin/python3 /usr/local/bin/python3; do"));
        assert!(command.contains("[ -n \"$ks_python\" ] || exit 66"));
        assert!(!command.contains("command -v python3"));
        assert!(command.ends_with("\nKS_DARWIN_PY\n"));
    }
}

#[test]
#[cfg(unix)]
fn earlier_path_python_cannot_select_the_collector_interpreter()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    let private = tempfile::tempdir()?;
    let fake = private.path().join("python3");
    std::fs::write(&fake, "#!/bin/sh\nprintf 'UNTRUSTED_PYTHON_SELECTED\\n'\n")?;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700))?;
    let command = script(true);
    let (selection, _) = command
        .split_once("\n\"$ks_python\" -I -S -B -")
        .ok_or("missing fixed interpreter invocation")?;
    let mut process = std::process::Command::new("/bin/sh")
        .args(["-c", &format!("{selection}\nprintf '%s\\n' \"$ks_python\"")])
        .env_clear()
        .env("PATH", private.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while process.try_wait()?.is_none() {
        if std::time::Instant::now() >= deadline {
            process.kill()?;
            process.wait()?;
            return Err("interpreter selection exceeded its deadline".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let result = process.wait_with_output()?;
    let expected = [
        "/usr/bin/python3",
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
    ]
    .into_iter()
    .find(|path| {
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    });
    if let Some(path) = expected {
        assert!(result.status.success());
        assert_eq!(result.stdout, format!("{path}\n").as_bytes());
    } else {
        assert_eq!(result.status.code(), Some(66));
        assert!(result.stdout.is_empty());
    }
    Ok(())
}
