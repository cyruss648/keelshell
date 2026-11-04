//! Wall-clock waits for tests that bridge GPUI with real Tokio/TCP workers.
use gpui_kit::{AnyWindowHandle, App, AppContext, TestAppContext, Window, test::TestWindowExt};
use std::time::{Duration, Instant};

pub(super) trait TransportTestContextExt {
    async fn wait_for_transport(
        &mut self,
        window: AnyWindowHandle,
        timeout: Duration,
        phase: &'static str,
        predicate: impl FnMut(&mut Window, &mut App) -> bool,
    );
}

impl TransportTestContextExt for TestAppContext {
    async fn wait_for_transport(
        &mut self,
        window: AnyWindowHandle,
        timeout: Duration,
        phase: &'static str,
        predicate: impl FnMut(&mut Window, &mut App) -> bool,
    ) {
        let started = Instant::now();
        eprintln!("updater transport phase={phase:?} begin timeout={timeout:?}");
        let ready = poll_until(self, window, started + timeout, predicate).await;
        assert!(
            ready,
            "updater transport phase={phase:?} timed out after {timeout:?}; elapsed={:?}",
            started.elapsed()
        );
        eprintln!(
            "updater transport phase={phase:?} complete elapsed={:?}",
            started.elapsed()
        );
    }
}

async fn poll_until(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    deadline: Instant,
    mut predicate: impl FnMut(&mut Window, &mut App) -> bool,
) -> bool {
    loop {
        if Instant::now() >= deadline {
            return false;
        }
        let ready = cx
            .update_window(window, |_, window, cx| {
                window.render_frame(cx);
                predicate(window, cx)
            })
            .expect("owned updater test window remains open");
        // Rendering/predicate work and scheduler parking all consume the same
        // deadline; a late success cannot renew it or turn a timeout into a pass.
        if Instant::now() >= deadline {
            return false;
        }
        if ready {
            return true;
        }
        cx.executor()
            .timer(
                Duration::from_millis(10).min(deadline.saturating_duration_since(Instant::now())),
            )
            .await;
    }
}

#[gpui_kit::test]
async fn real_transport_clock_does_not_skip_elapsed_time(cx: &mut TestAppContext) {
    let (_window, _panel) = super::mount_panel(cx, Default::default());
    let delay = Duration::from_millis(100);
    let started = Instant::now();
    cx.executor().timer(delay).await;
    assert!(
        started.elapsed() >= delay,
        "real transport timer advanced before wall time: {:?}",
        started.elapsed()
    );
}

#[gpui_kit::test]
async fn pending_transport_expires_at_one_wall_deadline(cx: &mut TestAppContext) {
    let (window, _panel) = super::mount_panel(cx, Default::default());
    let started = Instant::now();
    let timeout = Duration::from_millis(100);
    assert!(!poll_until(cx, window, started + timeout, |_, _| false).await);
    assert!(started.elapsed() >= timeout);
    assert!(
        started.elapsed() < super::REQUEST_BOUND,
        "a pending condition must not restart its deadline"
    );
}

#[gpui_kit::test]
async fn expired_transport_cannot_accept_a_ready_predicate(cx: &mut TestAppContext) {
    let (window, _panel) = super::mount_panel(cx, Default::default());
    let mut called = false;
    assert!(
        !poll_until(cx, window, Instant::now(), |_, _| {
            called = true;
            true
        })
        .await
    );
    assert!(!called, "expired waits cannot consult a late result");
}
