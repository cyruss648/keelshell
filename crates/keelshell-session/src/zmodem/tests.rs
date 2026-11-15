use super::*;

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn zmodem_review_and_absolute_deadlines_do_not_depend_on_peer_traffic() -> TestResult {
    let now = Instant::now();
    let reviewed = now.checked_sub(REVIEW_TIMEOUT).ok_or("clock range")?;
    let started = now.checked_sub(MAX_ELAPSED).ok_or("clock range")?;
    assert!(expired(Some(now), reviewed, true, now));
    assert!(!expired(Some(now), reviewed, false, now));
    assert!(expired(Some(started), now, false, now));
    assert!(expired(Some(started), now, true, now));
    assert!(!expired(Some(now), now, true, now));
    Ok(())
}

#[tokio::test]
async fn zmodem_abandoned_source_observation_marks_actual_blocking_worker_cancelled() -> TestResult
{
    let stop = Arc::new(AtomicBool::new(false));
    let (release, gate) = std::sync::mpsc::sync_channel(1);
    let (observed, completed) = tokio::sync::oneshot::channel();
    let mut observation = Box::pin(blocking(stop.clone(), move |stop| {
        gate.recv_timeout(Duration::from_secs(3))
            .map_err(|_| ZmodemFailure::Timeout)?;
        let _ = observed.send(stop.load(Ordering::Acquire));
        Ok(())
    }));
    std::future::poll_fn(|cx| {
        assert!(observation.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(observation);
    assert!(stop.load(Ordering::Acquire));
    release.send(())?;
    assert!(tokio::time::timeout(Duration::from_secs(3), completed).await??);
    Ok(())
}

#[tokio::test]
async fn zmodem_publication_worker_hold_survives_a_dropped_wait_until_actual_worker_exit()
-> TestResult {
    let running = Arc::new(AtomicBool::new(true));
    let worker_running = running.clone();
    let (release, gate) = std::sync::mpsc::sync_channel(1);
    let (finished, observed) = tokio::sync::oneshot::channel();
    let task = tokio::task::spawn_blocking(move || {
        let guard = RunningOnDrop(worker_running);
        let result = gate.recv_timeout(Duration::from_secs(3));
        drop(guard);
        let _ = finished.send(result.is_ok());
    });
    drop(task);
    assert!(running.load(Ordering::Acquire));
    release.send(())?;
    assert!(tokio::time::timeout(Duration::from_secs(3), observed).await??);
    assert!(!running.load(Ordering::Acquire));
    Ok(())
}

// Exercise actual ticket disposition without filesystem claims or global
// quarantine records; backend reservation behavior is covered by SSH fixtures.
fn publication_ticket(hold: &Arc<PublicationHold>, mutated: bool) -> PublicationTicket {
    hold.begin();
    PublicationTicket {
        ticket: None,
        mutated: Arc::new(AtomicBool::new(mutated)),
        hold: Some(hold.clone()),
    }
}

#[test]
fn zmodem_publication_hold_counts_old_unobserved_results_after_new_success() {
    for old_mutated in [false, true] {
        let hold = Arc::new(PublicationHold::default());
        let old = publication_ticket(&hold, old_mutated);
        let mut new = publication_ticket(&hold, true);
        new.known();
        assert!(hold.pending.load(Ordering::Acquire));
        // A resolved ticket's second known()/drop must not consume another ticket.
        new.known();
        drop(new);
        assert!(hold.pending.load(Ordering::Acquire));
        drop(old);
        assert_eq!(hold.pending.load(Ordering::Acquire), old_mutated);
    }
}

#[test]
fn zmodem_publication_unknown_survives_later_success_and_unmutated_drop() {
    let hold = Arc::new(PublicationHold::default());
    drop(publication_ticket(&hold, true));
    assert!(hold.pending.load(Ordering::Acquire));
    let mut success = publication_ticket(&hold, true);
    success.known();
    drop(success);
    assert!(hold.pending.load(Ordering::Acquire));
    drop(publication_ticket(&hold, false));
    assert!(hold.pending.load(Ordering::Acquire));
}

#[test]
fn zmodem_publication_poison_keeps_hold_after_known_and_unmutated_disposition() {
    let hold = Arc::new(PublicationHold::default());
    let mut success = publication_ticket(&hold, true);
    let untouched = publication_ticket(&hold, false);
    let poisoned = std::panic::catch_unwind(|| {
        let Ok(_guard) = hold.state.lock() else {
            panic!("publication state unexpectedly poisoned before fixture");
        };
        panic!("test interruption of publication accounting");
    });
    assert!(poisoned.is_err());
    success.known();
    drop(success);
    drop(untouched);
    assert!(hold.pending.load(Ordering::Acquire));
    let mut later = publication_ticket(&hold, false);
    later.known();
    drop(later);
    assert!(hold.pending.load(Ordering::Acquire));
    assert!(hold.state.is_poisoned());
}

#[test]
fn zmodem_publication_late_unknown_races_new_success_without_clearing_hold() -> TestResult {
    race_publication_dispositions(true)
}

#[test]
fn zmodem_publication_late_unknown_races_unmutated_drop_without_clearing_hold() -> TestResult {
    race_publication_dispositions(false)
}

fn race_publication_dispositions(success: bool) -> TestResult {
    let hold = Arc::new(PublicationHold::default());
    let pending = hold.pending.clone();
    let running = Arc::new(AtomicBool::new(true));
    let old = publication_ticket(&hold, true);
    // Reproduce the reachable gap: the worker has exited, but its result still
    // owns a mutated ticket when the next publication is admitted.
    drop(RunningOnDrop(running.clone()));
    assert!(!running.load(Ordering::Acquire));
    let mut new = publication_ticket(&hold, success);
    let state = hold.state.lock().map_err(|_| "publication lock poisoned")?;
    let (ready, started) = std::sync::mpsc::channel();
    let (finished, completed) = std::sync::mpsc::channel();
    let old_ready = ready.clone();
    let old_finished = finished.clone();
    let _old_disposition = std::thread::spawn(move || {
        let _ = old_ready.send(());
        drop(old);
        let _ = old_finished.send(());
    });
    let _new_disposition = std::thread::spawn(move || {
        let _ = ready.send(());
        if success {
            new.known();
        }
        drop(new);
        let _ = finished.send(());
    });
    // Both dispositions contend for the same state; neither can publish from
    // an earlier snapshot while this lock is held. All waits are bounded.
    started.recv_timeout(Duration::from_secs(3))?;
    started.recv_timeout(Duration::from_secs(3))?;
    assert!(pending.load(Ordering::Acquire));
    drop(state);
    completed.recv_timeout(Duration::from_secs(3))?;
    completed.recv_timeout(Duration::from_secs(3))?;
    assert!(!running.load(Ordering::Acquire));
    assert!(pending.load(Ordering::Acquire));
    let state = hold.state.lock().map_err(|_| "publication lock poisoned")?;
    assert_eq!(state.active, 0);
    assert!(state.unresolved);
    Ok(())
}
