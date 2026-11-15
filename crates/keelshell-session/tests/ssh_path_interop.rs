#![cfg(test)]
//! Opt-in real OpenSSH target/jump checks. The Python runner owns every server,
//! gate, key and process. This test never reads user profiles or writes remotely.

use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use keelshell_session::{
    ProxyKind, SessionError, SshAuth, SshOptions, SshProxy, SshSession,
    path_comparison::{
        PathComparisonPlan, PathComparisonReport, PathComparisonStatus, PathProbeKind,
        PathSampleOutcome, PathSide,
    },
};
use russh::keys::{HashAlg, PublicKey};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

fn environment(name: &str) -> TestResult<String> {
    Ok(std::env::var(format!("KEELSHELL_PATH_{name}"))?)
}

fn options(role: &str, proxy: Option<&str>) -> TestResult<SshOptions> {
    if environment("OPT_IN")? != "localhost-owned-fixture" {
        return Err("Run only through the disposable localhost path lab".into());
    }
    let mut options = SshOptions::new("127.0.0.1", environment("USER")?);
    options.port = environment(&format!("{role}_PORT"))?.parse()?;
    let public = std::fs::read_to_string(environment(&format!("{role}_HOST_KEY"))?)?;
    options.expected_host_key = Some(
        PublicKey::from_openssh(&public)?
            .fingerprint(HashAlg::Sha256)
            .to_string(),
    );
    options.auth = SshAuth::PrivateKey {
        path: PathBuf::from(environment(&format!("{role}_CLIENT_KEY"))?),
        passphrase: None,
    };
    options.timeout = Duration::from_secs(8);
    if let Some(proxy) = proxy {
        options.proxy = Some(SshProxy {
            kind: ProxyKind::HttpConnect,
            host: "127.0.0.1".into(),
            port: environment(proxy)?.parse()?,
            credentials: None,
        });
    }
    Ok(options)
}

struct Routes {
    direct: SshSession,
    relay: SshSession,
    jump: SshSession,
}

impl Routes {
    async fn connect() -> TestResult<Self> {
        let direct = SshSession::connect(options("TARGET", Some("DIRECT_PROXY_PORT"))?).await?;
        let jump = SshSession::connect(options("JUMP", Some("RELAY_PROXY_PORT"))?).await?;
        // The final host/port/user/pin are exactly the same as the direct
        // target. Only transport route metadata differs.
        let relay = SshSession::connect_through(&jump, options("TARGET", None)?).await?;
        Ok(Self {
            direct,
            relay,
            jump,
        })
    }

    async fn check_alive(&self) -> TestResult {
        for session in [&self.direct, &self.relay] {
            let reply = session.exec("printf path-parent-alive").await?;
            assert_eq!(reply.stdout, b"path-parent-alive");
            assert!(reply.stderr.is_empty());
            assert_eq!(reply.exit_status, Some(0));
        }
        // MaxSessions=0 on the jump intentionally prevents exec. A fresh
        // authenticated target channel proves its forwarding parent survived.
        assert!(!self.jump.is_closed());
        let another = SshSession::connect_through(&self.jump, options("TARGET", None)?).await?;
        let reply = another.exec("printf jump-parent-alive").await?;
        let closed = another.close().await;
        assert_eq!(reply.stdout, b"jump-parent-alive");
        assert!(reply.stderr.is_empty());
        assert_eq!(reply.exit_status, Some(0));
        closed?;
        Ok(())
    }

    async fn close(&self) -> TestResult {
        // Attempt each close even when an earlier one fails.
        let relay = self.relay.close().await;
        let direct = self.direct.close().await;
        let jump = self.jump.close().await;
        relay?;
        direct?;
        jump?;
        Ok(())
    }
}

fn side_name(side: PathSide) -> &'static str {
    match side {
        PathSide::A => "A",
        PathSide::B => "B",
    }
}

fn kind_name(kind: PathProbeKind) -> &'static str {
    match kind {
        PathProbeKind::Exec => "Exec",
        PathProbeKind::Upload => "Upload",
        PathProbeKind::Download => "Download",
    }
}

fn report_value(report: &PathComparisonReport, wall_time: Duration) -> Value {
    let mut median_ns = json!({"A": {}, "B": {}});
    for side in [PathSide::A, PathSide::B] {
        for kind in [
            PathProbeKind::Exec,
            PathProbeKind::Upload,
            PathProbeKind::Download,
        ] {
            median_ns[side_name(side)][kind_name(kind)] = report
                .summary(side, kind)
                .map_or(Value::Null, |value| json!(value.as_nanos()));
        }
    }
    let samples: Vec<_> = report
        .samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let (outcome, failure) = match &sample.outcome {
                PathSampleOutcome::Passed => ("Passed", None),
                PathSampleOutcome::Failed(failure) => ("Failed", Some(format!("{failure:?}"))),
            };
            json!({
                "index": index,
                "path": side_name(sample.path),
                "kind": kind_name(sample.kind),
                "round": sample.round,
                "elapsed_ns": sample.elapsed.as_nanos(),
                "bytes": sample.bytes,
                "outcome": outcome,
                "failure": failure,
            })
        })
        .collect();
    json!({
        "schema": 1,
        "scenario": std::env::var("KEELSHELL_PATH_SCENARIO").ok(),
        "status": match report.status {
            PathComparisonStatus::Complete => "Complete",
            PathComparisonStatus::Cancelled => "Cancelled",
            PathComparisonStatus::Incomplete => "Incomplete",
        },
        "cleanup_unconfirmed": report.cleanup_unconfirmed,
        "wall_elapsed_ns": wall_time.as_nanos(),
        "samples": samples,
        "median_ns": median_ns,
        "limits": {"rounds": 3, "sample_seconds": 8, "comparison_seconds": 60,
                   "bytes_per_transfer": 262144, "total_payload_bytes": 3145728},
        "measurement": "SSH channel open + exec startup + synthetic memory I/O + completion; not ICMP RTT",
    })
}

fn evidence_path(name: &str) -> TestResult<PathBuf> {
    let directory = PathBuf::from(environment("REPORT_DIR")?);
    if !directory.is_absolute() || !directory.is_dir() {
        return Err("The runner must supply an existing absolute evidence directory".into());
    }
    Ok(directory.join(name))
}

fn write_new(path: &Path, text: &str) -> TestResult {
    use std::io::Write;
    // Never overwrite an old report if somebody accidentally reuses fixtures.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires scripts/ssh_path_lab.py with disposable pinned target and jump"]
async fn openssh_path_comparison_collects_eighteen_pinned_samples() -> TestResult {
    let routes = Routes::connect().await?;
    let plan = PathComparisonPlan::new(routes.direct.clone(), routes.relay.clone())?;
    assert!(plan.matches_sessions(&routes.direct, &routes.relay));
    let clone = plan.clone();
    assert!(clone.matches_sessions(&routes.direct, &routes.relay));
    assert!(!clone.matches_sessions(&routes.relay, &routes.direct));
    write_new(
        &evidence_path("review.txt")?,
        &format!(
            "A\n{}\nB\n{}\nPLAN\n{}\n",
            routes.direct.connection_info().review_text(),
            routes.relay.connection_info().review_text(),
            plan.review_text(),
        ),
    )?;
    let started = Instant::now();
    let report = plan.run(&AtomicBool::new(false)).await;
    let elapsed = started.elapsed();
    // Persist the actual failed/partial report before any acceptance assertion.
    write_new(
        &evidence_path("report.json")?,
        &serde_json::to_string_pretty(&report_value(&report, elapsed))?,
    )?;
    let alive = routes.check_alive().await;
    let close = routes.close().await;
    assert!(
        matches!(report.status, PathComparisonStatus::Complete),
        "status {:?}; original samples are retained in report.json",
        report.status,
    );
    assert!(!report.cleanup_unconfirmed);
    assert_eq!(report.samples.len(), 18);
    assert!(elapsed < Duration::from_secs(62));
    for side in [PathSide::A, PathSide::B] {
        for kind in [
            PathProbeKind::Exec,
            PathProbeKind::Upload,
            PathProbeKind::Download,
        ] {
            let rows: Vec<_> = report
                .samples
                .iter()
                .filter(|sample| sample.path == side && sample.kind == kind)
                .collect();
            assert_eq!(rows.len(), 3);
            let mut durations = Vec::new();
            let mut rounds = Vec::new();
            for sample in rows {
                assert!(matches!(sample.outcome, PathSampleOutcome::Passed));
                if kind != PathProbeKind::Exec {
                    assert_eq!(sample.bytes, 256 * 1024);
                }
                durations.push(sample.elapsed);
                rounds.push(sample.round);
            }
            rounds.sort_unstable();
            rounds.dedup();
            assert_eq!(rounds.len(), 3);
            durations.sort_unstable();
            assert_eq!(report.summary(side, kind), Some(durations[1]));
        }
    }
    alive?;
    close?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires scripts/ssh_path_lab.py with disposable pinned target and jump"]
async fn openssh_path_comparison_precancel_preserves_routes() -> TestResult {
    let routes = Routes::connect().await?;
    let same = PathComparisonPlan::new(routes.direct.clone(), routes.direct.clone());
    let independent = SshSession::connect(options("TARGET", Some("DIRECT_PROXY_PORT"))?).await?;
    let same_route = PathComparisonPlan::new(routes.direct.clone(), independent.clone());
    let plan = PathComparisonPlan::new(routes.direct.clone(), routes.relay.clone())?;
    let report = plan.run(&AtomicBool::new(true)).await;
    write_new(
        &evidence_path("precancel.json")?,
        &serde_json::to_string_pretty(&report_value(&report, Duration::ZERO))?,
    )?;
    let alive = routes.check_alive().await;
    let other_close = independent.close().await;
    let close = routes.close().await;
    assert!(same.is_err(), "a clone is not a second route");
    assert!(
        same_route.is_err(),
        "independent connections on the same route are rejected"
    );
    assert!(matches!(report.status, PathComparisonStatus::Cancelled));
    assert!(report.samples.is_empty());
    assert!(!report.cleanup_unconfirmed);
    for side in [PathSide::A, PathSide::B] {
        assert_eq!(report.summary(side, PathProbeKind::Exec), None);
    }
    alive?;
    other_close?;
    close?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires scripts/ssh_path_lab.py with disposable pinned target and jump"]
async fn openssh_path_comparison_rejects_unpinned_target_and_jump() -> TestResult {
    // Substituting the OTHER daemon's real key is a valid but wrong pin. This
    // checks fail-closed host verification at both independent SSH layers.
    let mut bad_jump = options("JUMP", Some("RELAY_PROXY_PORT"))?;
    bad_jump.expected_host_key = options("TARGET", None)?.expected_host_key;
    let rejected_jump = SshSession::connect(bad_jump).await;
    let jump = SshSession::connect(options("JUMP", Some("RELAY_PROXY_PORT"))?).await?;
    let mut bad_target = options("TARGET", None)?;
    bad_target.expected_host_key = options("JUMP", None)?.expected_host_key;
    let rejected_target = SshSession::connect_through(&jump, bad_target).await;
    // Wrong target identity must not consume or disconnect the valid parent.
    let target = SshSession::connect_through(&jump, options("TARGET", None)?).await?;
    let output = target.exec("printf pinned-target-alive").await?;
    let target_close = target.close().await;
    let jump_close = jump.close().await;
    assert!(matches!(
        rejected_jump,
        Err(SessionError::ChangedHostKey { .. })
    ));
    assert!(matches!(
        rejected_target,
        Err(SessionError::ChangedHostKey { .. })
    ));
    assert_eq!(output.stdout, b"pinned-target-alive");
    assert!(output.stderr.is_empty());
    assert_eq!(output.exit_status, Some(0));
    target_close?;
    jump_close?;
    Ok(())
}
