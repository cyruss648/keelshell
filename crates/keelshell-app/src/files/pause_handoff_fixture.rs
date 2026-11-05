//! Observe the selected job's real worker-to-session pause handoff in tests.
use super::Checked;
use std::sync::{
    Arc, Mutex, OnceLock, Weak,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use tokio::sync::watch;

struct State {
    channel: watch::Receiver<bool>,
    allow_forward: watch::Sender<bool>,
    reached: AtomicBool,
    forwarded_at: OnceLock<Instant>,
    expired: AtomicBool,
}

fn registry() -> &'static Mutex<Vec<Weak<State>>> {
    static REGISTRY: OnceLock<Mutex<Vec<Weak<State>>>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}

/// Own one job's observer. Only the negative control blocks forwarding.
/// Neither the registry nor a worker observation retains a FilesPanel/fixture.
pub(crate) struct PauseHandoff(Arc<State>);

impl PauseHandoff {
    pub(crate) fn install(channel: &watch::Sender<bool>, block_forwarding: bool) -> Self {
        let state = Arc::new(State {
            channel: channel.subscribe(),
            allow_forward: watch::channel(!block_forwarding).0,
            reached: AtomicBool::new(false),
            forwarded_at: OnceLock::new(),
            expired: AtomicBool::new(false),
        });
        let mut observers = registry().lock().checked("pause handoff registry");
        observers.retain(|observer| observer.strong_count() > 0);
        assert!(
            observers
                .iter()
                .filter_map(Weak::upgrade)
                .all(|observer| !observer.channel.same_channel(&state.channel)),
            "a pause channel has exactly one handshake owner"
        );
        observers.push(Arc::downgrade(&state));
        Self(state)
    }

    pub(crate) fn reached(&self) -> bool {
        self.0.reached.load(Ordering::Acquire)
    }

    pub(crate) fn forwarded(&self) -> bool {
        self.forwarded_at().is_some()
    }

    pub(crate) fn forwarded_at(&self) -> Option<Instant> {
        self.0.forwarded_at.get().copied()
    }

    pub(crate) fn expired(&self) -> bool {
        self.0.expired.load(Ordering::Acquire)
    }

    pub(crate) fn allow_forwarding(&self) {
        self.0.allow_forward.send_replace(true);
    }
}

impl Drop for PauseHandoff {
    fn drop(&mut self) {
        // Failed assertions must not strand the worker at the negative gate.
        self.allow_forwarding();
    }
}

/// Returned only for the exact watch channel registered by this test.
pub(crate) struct Forwarding(Arc<State>);

impl Forwarding {
    /// Call immediately after the real TransferHandle::pause() returns.
    pub(crate) fn forwarded(self) {
        self.0.forwarded_at.get_or_init(Instant::now);
    }
}

/// The normal observer never waits. A negative control can withhold this
/// handoff within the existing five-second idle budget; expiry is a failure.
pub(crate) async fn before_forward(channel: &watch::Receiver<bool>) -> Option<Forwarding> {
    if !*channel.borrow() {
        return None;
    }
    let state = {
        let observers = registry().lock().checked("lookup exact pause handoff");
        observers
            .iter()
            .filter_map(Weak::upgrade)
            .find(|observer| observer.channel.same_channel(channel))
    }?;
    let mut permit = state.allow_forward.subscribe();
    state.reached.store(true, Ordering::Release);
    let allowed = *permit.borrow_and_update();
    if !allowed {
        let released =
            tokio::time::timeout(Duration::from_secs(5), permit.wait_for(|allowed| *allowed))
                .await
                .is_ok_and(|result| result.is_ok());
        if !released {
            state.expired.store(true, Ordering::Release);
        }
    }
    Some(Forwarding(state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{future::Future, task::Poll};

    #[tokio::test]
    async fn handoff_is_channel_scoped_and_guard_drop_unblocks_only_its_worker() {
        let (selected, selected_rx) = watch::channel(true);
        let (sibling, sibling_rx) = watch::channel(true);
        let selected_probe = PauseHandoff::install(&selected, true);
        let sibling_probe = PauseHandoff::install(&sibling, true);
        let mut selected_wait = Box::pin(before_forward(&selected_rx));
        let mut sibling_wait = Box::pin(before_forward(&sibling_rx));
        std::future::poll_fn(|cx| {
            assert!(selected_wait.as_mut().poll(cx).is_pending());
            assert!(sibling_wait.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        assert!(selected_probe.reached() && sibling_probe.reached());
        assert!(!selected_probe.forwarded() && !sibling_probe.forwarded());
        drop(selected_probe);
        tokio::time::timeout(Duration::from_secs(5), selected_wait)
            .await
            .checked("dropped observer releases its worker")
            .unwrap_or_else(|| panic!("exact selected handoff"))
            .forwarded();
        std::future::poll_fn(|cx| {
            assert!(sibling_wait.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        assert!(!sibling_probe.expired());
        sibling_probe.allow_forwarding();
        tokio::time::timeout(Duration::from_secs(5), sibling_wait)
            .await
            .checked("sibling explicitly released")
            .unwrap_or_else(|| panic!("exact sibling handoff"))
            .forwarded();
        assert!(sibling_probe.forwarded());
    }
}
