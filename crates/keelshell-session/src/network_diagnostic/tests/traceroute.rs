#![cfg(test)]

//! Real pinned loopback SSH with controlled frames, plus the real supervisor.
//! These tests do not establish native UI or real Linux packet acceptance.
use super::*;
use keelshell_core::{TracerouteFamily, TracerouteProtocol};

fn request() -> NetworkDiagnosticRequest {
    NetworkDiagnosticRequest::traceroute(
        "fixture.invalid",
        TracerouteProtocol::Udp,
        TracerouteFamily::Ipv4,
        2,
        2,
        500,
    )
    .unwrap_or_else(|error| panic!("bounded traceroute: {error}"))
}

fn body() -> serde_json::Value {
    serde_json::json!({
        "version":1, "status":"success",
        "addresses":[{"family":"IPv4","address":"127.0.0.1"}],
        "peer":null, "tls":null, "http":null, "icmp":null, "limited":false,
        "timing":{"resolve_ms":1,"connect_ms":null,"tls_ms":null,"headers_ms":null,"total_ms":5},
        "traceroute":{"reason":"destination_responded","implementation":"linux_traceroute_2_1_5",
            "address":"127.0.0.1","hops":[{"ttl":1,"probes":[
                {"address":"127.0.0.1","rtt_us":10,"annotation":null},
                {"address":null,"rtt_us":null,"annotation":null}]}]}
    })
}

fn frame(value: &serde_json::Value) -> Vec<u8> {
    format!("{FRAME}{value}\n").into_bytes()
}

#[test]
fn traceroute_wire_preserves_responses_stars_hop_limit_and_icmp_markers() {
    let report = parse_report(request(), &frame(&body()))
        .unwrap_or_else(|error| panic!("valid destination response: {error}"));
    assert_eq!(report.status, NetworkDiagnosticStatus::Success);
    let trace = report
        .traceroute
        .unwrap_or_else(|| panic!("traceroute result"));
    assert_eq!(trace.hops[0].probes[1].rtt_us, None);
    let mut limit = body();
    limit["status"] = "traceroute_hop_limit".into();
    limit["traceroute"]["reason"] = "hop_limit".into();
    let row = serde_json::json!({"ttl":1,"probes":[
        {"address":null,"rtt_us":null,"annotation":null},
        {"address":null,"rtt_us":null,"annotation":null}]});
    limit["traceroute"]["hops"] = serde_json::json!([row.clone(), row]);
    limit["traceroute"]["hops"][1]["ttl"] = 2.into();
    assert_eq!(
        parse_report(request(), &frame(&limit))
            .unwrap_or_else(|error| panic!("reviewed limit: {error}"))
            .status,
        NetworkDiagnosticStatus::TracerouteHopLimit
    );
    let mut unreachable = body();
    unreachable["status"] = "traceroute_unreachable".into();
    unreachable["traceroute"]["reason"] = "reported_unreachable".into();
    unreachable["traceroute"]["hops"][0]["probes"][0]["annotation"] = "!X".into();
    assert_eq!(
        parse_report(request(), &frame(&unreachable))
            .unwrap_or_else(|error| panic!("ICMP error: {error}"))
            .status,
        NetworkDiagnosticStatus::TracerouteUnreachable
    );
}

#[test]
fn traceroute_wire_rejects_request_limits_family_timing_and_forged_success() {
    for (pointer, value) in [
        ("/traceroute/hops/0/ttl", serde_json::json!(2)),
        (
            "/traceroute/hops/0/probes/0/rtt_us",
            serde_json::json!(500001),
        ),
        (
            "/traceroute/hops/0/probes/0/rtt_us",
            serde_json::Value::Null,
        ),
        (
            "/traceroute/hops/0/probes/0/address",
            serde_json::json!("::1"),
        ),
        (
            "/traceroute/hops/0/probes/0/annotation",
            serde_json::json!("!X private"),
        ),
        (
            "/traceroute/hops/0/probes/0/annotation",
            serde_json::json!("!<256>"),
        ),
        (
            "/traceroute/hops/0/probes/0/annotation",
            serde_json::json!("!F-4294967296"),
        ),
        ("/traceroute/implementation", serde_json::json!("busybox")),
        ("/traceroute/implementation", serde_json::Value::Null),
        ("/traceroute/address", serde_json::json!("192.0.2.1")),
        ("/traceroute/address", serde_json::Value::Null),
        ("/traceroute/reason", serde_json::json!("hop_limit")),
        ("/traceroute/hops", serde_json::json!([])),
        ("/peer", serde_json::json!("127.0.0.1")),
        ("/timing/resolve_ms", serde_json::Value::Null),
        ("/timing/connect_ms", serde_json::json!(1)),
        ("/timing/total_ms", serde_json::json!(0)),
    ] {
        let mut candidate = body();
        *candidate
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("wire field")) = value;
        assert!(
            parse_report(request(), &frame(&candidate)).is_err(),
            "{pointer}"
        );
    }
    let mut extra = body();
    extra["traceroute"]["hops"][0]["probes"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("probes"))
        .pop();
    assert!(parse_report(request(), &frame(&extra)).is_err());
    extra = body();
    extra["traceroute"]["stderr"] = "untrusted".into();
    assert!(parse_report(request(), &frame(&extra)).is_err());
    assert!(parse_report(dns_request(), &frame(&body())).is_err());
    let literal = NetworkDiagnosticRequest::traceroute(
        "192.0.2.1",
        TracerouteProtocol::Udp,
        TracerouteFamily::Ipv4,
        2,
        2,
        500,
    )
    .unwrap_or_else(|error| panic!("literal request: {error}"));
    assert!(parse_report(literal, &frame(&body())).is_err());
}

#[test]
fn traceroute_wire_missing_timeout_cancel_and_capability_results_remain_unknown() {
    for (status, reason) in [
        ("timeout", "deadline"),
        ("interrupted", "interrupted"),
        ("cleanup_unknown", "cleanup_unknown"),
        ("utility_unavailable", "utility_missing"),
        ("permission_denied", "permission_denied"),
        ("unsupported_environment", "unsupported_platform"),
        ("unsupported_environment", "unsupported_implementation"),
        ("unsupported_environment", "unsupported_options"),
        ("dns_failure", "resolution_failed"),
        ("traceroute_failure", "process_failed"),
        ("traceroute_failure", "invalid_output"),
        ("traceroute_failure", "output_limit"),
    ] {
        let mut candidate = body();
        candidate["status"] = status.into();
        candidate["addresses"] = serde_json::json!([]);
        candidate["timing"]["resolve_ms"] = serde_json::Value::Null;
        candidate["traceroute"] = serde_json::json!({
            "reason":reason,"implementation":null,"address":null,"hops":[]});
        let report = parse_report(request(), &frame(&candidate))
            .unwrap_or_else(|error| panic!("{reason}: {error}"));
        assert!(
            report
                .traceroute
                .unwrap_or_else(|| panic!("fixed reason"))
                .hops
                .is_empty()
        );
        candidate["status"] = "success".into();
        assert!(parse_report(request(), &frame(&candidate)).is_err());
    }
}

#[tokio::test]
async fn traceroute_reviewed_request_uses_authenticated_ssh_and_only_encoded_parameters()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(Mode::Body(frame(&body()), 0)).await?;
    let request = request();
    let report = fixture
        .session
        .diagnose_remote(request.clone(), &AtomicBool::new(false))
        .await?;
    assert_eq!(report.request, request);
    {
        let seen = fixture
            .control
            .seen
            .lock()
            .map_err(|_| "owned command lock")?;
        let command = std::str::from_utf8(&seen[0])?;
        let encoded = command
            .lines()
            .nth(1)
            .and_then(|line| line.split_whitespace().nth(5))
            .ok_or("missing encoded request")?
            .trim_matches('\'');
        let dispatched: serde_json::Value = serde_json::from_slice(&STANDARD.decode(encoded)?)?;
        assert_eq!(dispatched, serde_json::to_value(request)?);
        assert!(!command.contains("fixture.invalid"));
        assert!(command.contains("shell=False"));
        assert!(command.contains("TRACE_VERSION"));
        assert!(!command.contains("sudo "));
    }
    fixture.finish().await?;
    Ok(())
}

#[tokio::test]
async fn traceroute_precancel_opening_cancel_and_late_ssh_reply_never_adopt()
-> Result<(), Box<dyn std::error::Error>> {
    for mode in [Mode::HoldOpen, Mode::HoldReply] {
        let opening = matches!(mode, Mode::HoldOpen);
        let fixture = Fixture::new(mode).await?;
        assert_eq!(
            fixture
                .session
                .diagnose_remote(request(), &AtomicBool::new(true))
                .await,
            Err(NetworkDiagnosticError::Cancelled)
        );
        assert_eq!(fixture.control.opened.load(Ordering::Acquire), 0);
        let cancel = Arc::new(AtomicBool::new(false));
        let captured = cancel.clone();
        let session = fixture.session.clone();
        let job = tokio::spawn(async move { session.diagnose_remote(request(), &captured).await });
        wait_counter(if opening {
            &fixture.control.opened
        } else {
            &fixture.control.commands
        })
        .await?;
        cancel.store(true, Ordering::Release);
        fixture.control.gate.add_permits(1);
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(3), job).await??,
            Err(NetworkDiagnosticError::Cancelled)
        );
        if opening {
            assert_eq!(fixture.control.commands.load(Ordering::Acquire), 0);
        }
        fixture.finish().await?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn traceroute_real_supervisor_over_pinned_ssh_honestly_refuses_remote_macos()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(Mode::Execute).await?;
    let report = fixture
        .session
        .diagnose_remote(request(), &AtomicBool::new(false))
        .await?;
    assert_eq!(
        report.status,
        NetworkDiagnosticStatus::UnsupportedEnvironment
    );
    assert_eq!(
        report
            .traceroute
            .unwrap_or_else(|| panic!("platform refusal"))
            .reason,
        NetworkDiagnosticTracerouteReason::UnsupportedPlatform
    );
    fixture.finish().await?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn traceroute_offline_process_contract_enforces_limits_and_actual_wait() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/network_diagnostic/tests/test_traceroute.py");
    let driver = "import subprocess,sys; r=subprocess.run([sys.executable,'-I','-S','-B',sys.argv[1]],timeout=25); sys.exit(r.returncode)";
    let output = std::process::Command::new("python3")
        .args(["-I", "-S", "-B", "-c", driver])
        .arg(path)
        .output()
        .unwrap_or_else(|error| panic!("offline Python prerequisite: {error}"));
    assert!(
        output.status.success(),
        "offline traceroute suite: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
