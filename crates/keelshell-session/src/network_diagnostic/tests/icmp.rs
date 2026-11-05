#![cfg(test)]
//! Offline wire and authenticated loopback fixtures; no real ICMP is sent.
use super::*;

fn request() -> NetworkDiagnosticRequest {
    NetworkDiagnosticRequest::icmp("fixture.invalid").unwrap_or_else(|error| panic!("{error}"))
}

fn body(status: &str, reason: &str, received: u8) -> serde_json::Value {
    serde_json::json!({
        "version":1, "status":status,
        "addresses":[{"family":"IPv4","address":"127.0.0.1"}],
        "peer":null, "tls":null, "http":null, "limited":false,
        "timing":{"resolve_ms":1,"connect_ms":null,"tls_ms":null,"headers_ms":null,"total_ms":4},
        "icmp":{"reason":reason,"implementation":"iputils_20240117","address":"127.0.0.1",
            "counts":{"transmitted":4,"received":received,"duplicates":0,"corrupted":0,"errors":0},
            "rtt":if received>0 {serde_json::json!({"min_us":10,"avg_us":20,"max_us":30,"mdev_us":8})}
                  else {serde_json::Value::Null}}
    })
}

fn frame(body: &serde_json::Value) -> Vec<u8> {
    format!("{FRAME}{body}\n").into_bytes()
}

#[test]
fn icmp_wire_distinguishes_success_loss_no_reply_timeout_and_reported_errors() {
    for (status, reason, received) in [
        ("success", "completed", 4),
        ("icmp_partial", "packet_loss", 2),
        ("icmp_no_reply", "no_echo_reply", 0),
        ("timeout", "deadline", 2),
        ("icmp_failure", "reported_errors", 2),
    ] {
        let mut value = body(status, reason, received);
        if reason == "reported_errors" {
            value["icmp"]["counts"]["errors"] = 2.into();
        }
        let report = parse_report(request(), &frame(&value))
            .unwrap_or_else(|error| panic!("{status}/{reason}: {error}"));
        assert_eq!(report.request, request());
        let icmp = report.icmp.unwrap_or_else(|| panic!("missing ICMP"));
        assert_eq!(icmp.counts.map(|counts| counts.received), Some(received));
    }
}

#[test]
fn icmp_wire_rejects_counter_status_timing_implementation_and_protocol_forgery() {
    let good = body("success", "completed", 4);
    let mutations = [
        ("/icmp/counts/received", serde_json::json!(2)),
        ("/icmp/counts/transmitted", serde_json::json!(5)),
        ("/icmp/counts/duplicates", serde_json::json!(10000)),
        ("/icmp/counts/errors", serde_json::json!(1)),
        ("/icmp/rtt/min_us", serde_json::json!(40)),
        ("/icmp/rtt/max_us", serde_json::json!(8_000_001)),
        ("/icmp/rtt", serde_json::Value::Null),
        ("/icmp/reason", serde_json::json!("packet_loss")),
        (
            "/icmp/implementation",
            serde_json::json!("iputils_20240905"),
        ),
        ("/icmp/implementation", serde_json::Value::Null),
        ("/icmp/address", serde_json::json!("192.0.2.1")),
        ("/icmp/address", serde_json::Value::Null),
        ("/peer", serde_json::json!("127.0.0.1")),
        ("/timing/connect_ms", serde_json::json!(1)),
        ("/timing/resolve_ms", serde_json::Value::Null),
    ];
    for (pointer, value) in mutations {
        let mut candidate = good.clone();
        *candidate
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("{pointer}")) = value;
        assert!(
            parse_report(request(), &frame(&candidate)).is_err(),
            "{pointer}: {candidate}"
        );
    }
    assert!(parse_report(dns_request(), &frame(&good)).is_err());
    let mut extra = good;
    extra["icmp"]["stderr"] = "private untrusted text".into();
    assert!(parse_report(request(), &frame(&extra)).is_err());
}

#[test]
fn icmp_capability_and_deadline_states_do_not_invent_missing_counters() {
    for (status, reason) in [
        ("permission_denied", "permission_denied"),
        ("utility_unavailable", "utility_missing"),
        ("unsupported_environment", "unsupported_implementation"),
        ("unsupported_environment", "unsupported_platform"),
        ("dns_failure", "resolution_failed"),
        ("icmp_failure", "invalid_output"),
        ("icmp_failure", "process_failed"),
        ("icmp_failure", "output_limit"),
        ("cleanup_unknown", "cleanup_unknown"),
        ("timeout", "deadline"),
        ("interrupted", "interrupted"),
    ] {
        let mut candidate = body(status, reason, 0);
        candidate["addresses"] = serde_json::json!([]);
        candidate["timing"]["resolve_ms"] = serde_json::Value::Null;
        for key in ["counts", "rtt", "implementation", "address"] {
            candidate["icmp"][key] = serde_json::Value::Null;
        }
        let report = parse_report(request(), &frame(&candidate))
            .unwrap_or_else(|error| panic!("{status}/{reason}: {error}"));
        assert!(report.icmp.is_some_and(|icmp| icmp.counts.is_none()));
        candidate["status"] = "success".into();
        assert!(parse_report(request(), &frame(&candidate)).is_err());
    }
}

#[tokio::test]
async fn icmp_approved_request_is_bound_to_authenticated_loopback_frame()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(Mode::Body(
        frame(&body("icmp_partial", "packet_loss", 2)),
        0,
    ))
    .await?;
    let report = fixture
        .session
        .diagnose_remote(request(), &AtomicBool::new(false))
        .await?;
    assert_eq!(report.status, NetworkDiagnosticStatus::IcmpPartial);
    assert_eq!(
        report
            .icmp
            .as_ref()
            .and_then(|icmp| icmp.counts.as_ref())
            .map(|counts| counts.received),
        Some(2)
    );
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
        assert_eq!(dispatched["kind"], "icmp");
        assert_eq!(dispatched["host"], request().host());
        assert!(command.contains("shell=False"));
        assert!(command.contains("LC_ALL"));
        assert!(!command.contains("pip install"));
    }
    fixture.finish().await?;
    Ok(())
}

#[tokio::test]
async fn icmp_cancel_while_opening_and_inflight_discards_late_reply()
-> Result<(), Box<dyn std::error::Error>> {
    for mode in [Mode::HoldOpen, Mode::HoldReply] {
        let opening = matches!(mode, Mode::HoldOpen);
        let fixture = Fixture::new(mode).await?;
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

#[cfg(unix)]
#[test]
fn icmp_offline_fake_process_suite_is_bounded_and_reaps_owned_children() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/network_diagnostic/tests/test_icmp.py");
    let driver = "import subprocess,sys; r=subprocess.run([sys.executable,'-I','-S','-B',sys.argv[1]],timeout=30); sys.exit(r.returncode)";
    let output = std::process::Command::new("python3")
        .args(["-I", "-S", "-B", "-c", driver])
        .arg(path)
        .output()
        .unwrap_or_else(|error| panic!("offline Python process prerequisite: {error}"));
    assert!(
        output.status.success(),
        "offline process suite: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
