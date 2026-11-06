use super::*;
use crate::monitor::{Snapshot, tests::SAMPLE};

fn with_capacity(body: &str, status: &str) -> String {
    let core = SAMPLE
        .split_once("@@KS:df@@")
        .map_or(SAMPLE, |(core, _)| core);
    format!("{core}@@KS:df@@\n{body}\n@@KS:df-status@@\n{status}\n@@KS:end@@\n")
}

#[test]
fn df_failures_preserve_core_resources_and_valid_interval_rates() -> MonitorResult<()> {
    let before = Snapshot::parse(SAMPLE)?;
    for (status, diagnostic, reason) in [
        ("127", "", CapabilityReason::CommandMissing),
        (
            "2",
            "df: invalid option -- P",
            CapabilityReason::UnsupportedOptions,
        ),
        (
            "1",
            "df: /restricted: Permission denied",
            CapabilityReason::PermissionDenied,
        ),
        ("1", "unclassified failure", CapabilityReason::CommandFailed),
    ] {
        let input = with_capacity("", status)
            .replace("100.00 75.00", "105.00 78.00")
            .replace("cpu 100 20 30 400", "cpu 120 20 50 450")
            .replace("eth0: 1000 10", "eth0: 1500 10");
        let (after, state) = Snapshot::parse_with_capacity(&input, diagnostic.as_bytes())?;
        assert_eq!(state, CapabilityState::Unavailable(reason), "{status}");
        assert!(after.filesystems.is_empty());
        assert_eq!(after.memory, before.memory);
        assert_eq!(
            after.rates_since(&before).cpu_busy_percent,
            Some(40.0 / 90.0 * 100.0)
        );
        assert_eq!(
            after.rates_since(&before).networks[1].received_bytes_per_second,
            Some(100.0)
        );
    }
    Ok(())
}

#[test]
fn absent_or_invalid_capacity_never_invalidates_the_core_snapshot() -> MonitorResult<()> {
    for input in [
        with_capacity("unexpected df output", "0"),
        with_capacity("", "nonsense"),
        with_capacity("", "256"),
        format!(
            "{}@@KS:end@@\n",
            SAMPLE
                .split_once("@@KS:df@@")
                .map_or(SAMPLE, |(core, _)| core)
        ),
    ] {
        let (sample, capacity) = Snapshot::parse_with_capacity(&input, &[])?;
        assert_eq!(
            capacity,
            CapabilityState::Unavailable(CapabilityReason::InvalidOutput)
        );
        assert!(sample.memory.total_bytes > 0);
        assert!(!sample.networks.is_empty());
    }
    Ok(())
}

#[test]
fn typed_command_errors_do_not_echo_untrusted_diagnostic_text() {
    for capability in [Capability::Processes, Capability::Sockets] {
        for (status, stderr, reason) in [
            (66, "", CapabilityReason::CommandMissing),
            (
                2,
                "illegal option -- e; private remote text",
                CapabilityReason::UnsupportedOptions,
            ),
            (
                126,
                "private remote text",
                CapabilityReason::PermissionDenied,
            ),
            (
                1,
                "permission denied: private remote text",
                CapabilityReason::PermissionDenied,
            ),
            (1, "private remote text", CapabilityReason::CommandFailed),
        ] {
            let result = command_text(
                ExecOutput {
                    stdout: Vec::new(),
                    stderr: stderr.as_bytes().to_vec(),
                    exit_status: Some(status),
                },
                capability,
            );
            assert!(
                matches!(&result, Err(MonitorError::CapabilityUnavailable { reason: actual, .. }) if *actual == reason)
            );
            let error = result
                .err()
                .map(|error| error.to_string())
                .unwrap_or_default();
            assert!(!error.contains("private remote text"));
        }
    }
}

#[tokio::test]
async fn pending_observations_cannot_repopulate_a_reset_or_retired_cache() {
    let cache = CapabilityCache::default();
    let (first, state) = cache.begin(Capability::Processes).await;
    assert_eq!(state, CapabilityState::Unknown);
    cache.reset().await;
    cache
        .record(first, Capability::Processes, CapabilityState::Available)
        .await;
    assert_eq!(
        cache.report().await.processes.state,
        CapabilityState::Unknown
    );
    let (second, _) = cache.begin(Capability::Processes).await;
    cache.invalidate_pending();
    cache
        .record(second, Capability::Processes, CapabilityState::Available)
        .await;
    assert_eq!(
        cache.report().await.processes.state,
        CapabilityState::Unknown
    );
    let (current, _) = cache.begin(Capability::Processes).await;
    cache
        .record(current, Capability::Processes, CapabilityState::Available)
        .await;
    assert_eq!(
        cache.report().await.processes.state,
        CapabilityState::Available
    );
}

#[tokio::test]
async fn reports_require_the_same_cache_generation_and_latest_revision() {
    let cache = Arc::new(CapabilityCache::default());
    let other = Arc::new(CapabilityCache::default());
    let (generation, _) = cache.begin(Capability::Filesystems).await;
    cache
        .record(
            generation,
            Capability::Filesystems,
            CapabilityState::Unavailable(CapabilityReason::CommandMissing),
        )
        .await;
    let negative = cache.authenticated_report().await;
    assert!(negative.is_current_for(&cache));
    assert!(!negative.is_current_for(&other));
    let unchanged = cache.authenticated_report().await;
    assert_eq!(negative.observations(), unchanged.observations());
    assert!(negative.is_current_for(&cache));

    cache
        .record(
            generation,
            Capability::Resources,
            CapabilityState::Available,
        )
        .await;
    assert!(!negative.is_current_for(&cache));
    let revised = cache.authenticated_report().await;
    assert_eq!(
        revised.observations().filesystems,
        negative.observations().filesystems,
        "collecting another item must not refresh the cached failure's time"
    );
    assert!(revised.is_current_for(&cache));
    cache.invalidate_pending();
    assert!(!revised.is_current_for(&cache));
    let resumed = cache.authenticated_report().await;
    assert!(resumed.is_current_for(&cache));
    cache.reset().await;
    assert!(!resumed.is_current_for(&cache));
    assert_eq!(cache.report().await, MonitorCapabilities::default());
}

#[test]
fn cached_capacity_collector_still_collects_proc_without_invoking_df() {
    let script = super::super::SNAPSHOT_SCRIPT.replace(
        super::super::DF_COLLECTION,
        "printf '\\n@@KS:df-status@@\\ncached\\n'",
    );
    assert!(script.contains("cat /proc/stat"));
    assert!(script.contains("cat /proc/net/dev"));
    assert!(script.contains("cat /proc/diskstats"));
    assert!(!script.contains("df -Pk"));
}
