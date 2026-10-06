//! Non-author checks of actual panel submission, cancellation, and delivery.
use super::{Action, ProfileSyncPanel, observation::ResultCategory};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size, test::TestAppContextExt,
};
use keelshell_core::{Connection, ProfileSyncChoice, ProfileSyncService, StateStore};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool, mpsc},
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;
use zeroize::Zeroizing;

const PASSWORD: &str = "independent-stage-fixture-password";

struct Slot {
    release: Option<mpsc::SyncSender<()>>,
    finished: mpsc::Receiver<()>,
}
impl Slot {
    fn hold(runtime: &Runtime) -> Self {
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release, released) = mpsc::sync_channel(1);
        let (finished_tx, finished) = mpsc::sync_channel(1);
        runtime.spawn_blocking(move || {
            entered_tx.send(()).checked();
            let _ = released.recv_timeout(Duration::from_secs(18));
            let _ = finished_tx.send(());
        });
        entered_rx.recv_timeout(Duration::from_secs(2)).checked();
        Self {
            release: Some(release),
            finished,
        }
    }
    fn release(&mut self) {
        if let Some(sender) = self.release.take() {
            let _ = sender.send(());
            self.finished.recv_timeout(Duration::from_secs(2)).checked();
        }
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        if let Some(sender) = self.release.take() {
            let _ = sender.send(());
            let _ = self.finished.recv_timeout(Duration::from_secs(2));
        }
    }
}

fn runtime() -> Arc<Runtime> {
    Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .checked(),
    )
}
fn mount(
    cx: &mut TestAppContext,
    store: Arc<StateStore>,
    runtime: Arc<Runtime>,
) -> (AnyWindowHandle, Entity<ProfileSyncPanel>) {
    let state = store.load().checked();
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(800.), px(600.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| ProfileSyncPanel::new(store, &state, runtime, window, cx)),
        )
        .checked()
    })
}
fn inspect(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    panel: &Entity<ProfileSyncPanel>,
    directory: &std::path::Path,
) {
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |p, cx| {
            p.directory.update(cx, |field, cx| {
                field.set_value(directory.to_string_lossy(), window, cx)
            });
            p.password
                .update(cx, |field, cx| field.set_value(PASSWORD, window, cx));
            p.submit(Action::Inspect, window, cx);
        });
    })
    .checked();
}
fn wait_snapshot(
    observation: &Arc<super::observation::OperationObservation>,
    predicate: impl Fn(super::observation::Snapshot) -> bool,
) -> super::observation::Snapshot {
    let deadline = Instant::now() + Duration::from_secs(18);
    loop {
        let snapshot = observation.snapshot();
        if predicate(snapshot) {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "actual stage deadline: {snapshot:?}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[gpui_kit::test]
async fn actual_queued_cancel_return_and_new_submit_keep_observers_separate(
    cx: &mut TestAppContext,
) {
    let temporary = tempfile::tempdir().checked();
    let directory = temporary.path().join("shared");
    std::fs::create_dir(&directory).checked();
    let store = Arc::new(StateStore::new(temporary.path().join("state.json")));
    let runtime = runtime();
    let (window, panel) = mount(cx, store, runtime.clone());
    let mut slot = Slot::hold(&runtime);
    inspect(cx, window, &panel, &directory);
    let old = panel.read_with(cx, |p, _| p.observation.clone()).checked();
    let queued = old.snapshot();
    assert!(panel.read_with(cx, |p, _| p.busy()));
    assert!(queued.blocking_started.is_none() && queued.service_returned.is_none());
    assert!(queued.foreground_completed.is_none());
    panel.update(cx, |p, cx| p.cancel(cx));
    slot.release();
    let returned = wait_snapshot(&old, |s| s.service_returned.is_some());
    assert_eq!(returned.service_result, Some(ResultCategory::Cancelled));
    assert!(returned.blocking_started.is_some());
    assert!(
        returned.foreground_completed.is_none(),
        "foreground was not pumped: {returned:?}"
    );
    assert!(
        panel.read_with(cx, |p, _| p.busy()),
        "observation cannot clear business pending state"
    );
    eprintln!("[independent-profile-stages] cancelled-before-foreground={returned:?}");
    cx.wait_for(window, Duration::from_secs(18), |_, cx| {
        !panel.read_with(cx, |p, _| p.busy())
    })
    .await;
    let completed = old.snapshot();
    assert_eq!(completed.foreground_result, Some(ResultCategory::Cancelled));
    assert!(completed.submitted <= completed.blocking_started.checked());
    assert!(completed.blocking_started <= completed.service_returned);
    assert!(completed.service_returned <= completed.foreground_completed);
    let mut next_slot = Slot::hold(&runtime);
    inspect(cx, window, &panel, &directory);
    let current = panel.read_with(cx, |p, _| p.observation.clone()).checked();
    assert_ne!(completed.operation, current.snapshot().operation);
    old.foreground_completed(Some(&current), ResultCategory::Stale);
    let new_queued = current.snapshot();
    assert!(new_queued.blocking_started.is_none());
    assert!(new_queued.service_returned.is_none() && new_queued.foreground_completed.is_none());
    assert_eq!(
        old.snapshot().foreground_result,
        Some(ResultCategory::Cancelled)
    );
    next_slot.release();
    cx.wait_for(window, Duration::from_secs(18), |_, cx| {
        !panel.read_with(cx, |p, _| p.busy())
    })
    .await;
    assert_eq!(
        current.snapshot().foreground_result,
        Some(ResultCategory::Review)
    );
    assert!(panel.read_with(cx, |p, _| p.review.is_some()));
    eprintln!(
        "[independent-profile-stages] next-completed={:?}",
        current.snapshot()
    );
}

#[gpui_kit::test]
async fn actual_stale_apply_reports_stable_error_and_preserves_newer_store(
    cx: &mut TestAppContext,
) {
    let temporary = tempfile::tempdir().checked();
    let directory = temporary.path().join("shared");
    std::fs::create_dir(&directory).checked();
    let store = Arc::new(StateStore::new(temporary.path().join("state.json")));
    let mut state = store.load().checked();
    state
        .connections
        .push(Connection::new("Fixture", "fixture.invalid", "fixture"));
    store.save(&state).checked();
    let (window, panel) = mount(cx, store.clone(), runtime());
    inspect(cx, window, &panel, &directory);
    cx.wait_for(window, Duration::from_secs(18), |_, cx| {
        !panel.read_with(cx, |p, _| p.busy())
    })
    .await;
    let id = panel.read_with(cx, |p, _| p.review.as_ref().checked().rows()[0].id);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |p, cx| {
            p.choose(id, ProfileSyncChoice::Local, cx);
            p.password
                .update(cx, |field, cx| field.set_value(PASSWORD, window, cx));
        });
    })
    .checked();
    let mut newer = store.load().checked();
    newer.connections[0].name = "Owned concurrent save".into();
    let newer = store.save(&newer).checked();
    let bytes = std::fs::read(store.path()).checked();
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |p, cx| {
            assert!(p.preview.is_some() && p.effects_acknowledged);
            p.submit(Action::Apply, window, cx);
        })
    })
    .checked();
    let observation = panel.read_with(cx, |p, _| p.observation.clone()).checked();
    cx.wait_for(window, Duration::from_secs(18), |_, cx| {
        !panel.read_with(cx, |p, _| p.busy())
    })
    .await;
    let snapshot = observation.snapshot();
    assert_eq!(snapshot.service_result, Some(ResultCategory::Stale));
    assert_eq!(snapshot.foreground_result, Some(ResultCategory::Stale));
    assert!(snapshot.foreground_completed.is_some());
    assert!(panel.read_with(cx, |p, _| p.review.is_none()));
    assert_eq!(std::fs::read(store.path()).checked(), bytes);
    assert_eq!(store.load().checked(), newer);
    assert!(!directory.join("keelshell-profiles.ksync").exists());
    eprintln!("[independent-profile-stages] legal-stale={snapshot:?}");
}

#[gpui_kit::test]
async fn actual_encrypted_inspect_exposes_running_service_before_return(cx: &mut TestAppContext) {
    let temporary = tempfile::tempdir().checked();
    let directory = temporary.path().join("shared");
    std::fs::create_dir(&directory).checked();
    let peer = Arc::new(StateStore::new(temporary.path().join("peer/state.json")));
    let mut state = peer.load().checked();
    state
        .connections
        .push(Connection::new("Peer", "fixture.invalid", "fixture"));
    peer.save(&state).checked();
    let service = ProfileSyncService::new(peer);
    let review = service
        .inspect(
            directory.clone(),
            Zeroizing::new(PASSWORD.into()),
            &AtomicBool::new(false),
        )
        .checked();
    let choices = review
        .rows()
        .iter()
        .map(|row| (row.id, ProfileSyncChoice::Local))
        .collect::<BTreeMap<_, _>>();
    assert!(
        service
            .apply(
                review,
                choices,
                Zeroizing::new(PASSWORD.into()),
                &AtomicBool::new(false)
            )
            .checked()
            .published
    );
    let store = Arc::new(StateStore::new(temporary.path().join("local/state.json")));
    let (window, panel) = mount(cx, store, runtime());
    inspect(cx, window, &panel, &directory);
    let observation = panel.read_with(cx, |p, _| p.observation.clone()).checked();
    let running = wait_snapshot(&observation, |s| s.blocking_started.is_some());
    assert!(
        running.service_returned.is_none(),
        "actual encrypted service was still running: {running:?}"
    );
    assert!(running.foreground_completed.is_none());
    assert!(panel.read_with(cx, |p, _| p.busy()));
    eprintln!("[independent-profile-stages] actual-core-running={running:?}");
    let returned = wait_snapshot(&observation, |s| s.service_returned.is_some());
    assert_eq!(returned.service_result, Some(ResultCategory::Review));
    assert!(returned.foreground_completed.is_none());
    cx.wait_for(window, Duration::from_secs(18), |_, cx| {
        !panel.read_with(cx, |p, _| p.busy())
    })
    .await;
    assert_eq!(
        observation.snapshot().foreground_result,
        Some(ResultCategory::Review)
    );
}

trait Checked<T> {
    fn checked(self) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    #[track_caller]
    fn checked(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => panic!("independent stage fixture: {e:?}"),
        }
    }
}
impl<T> Checked<T> for Option<T> {
    #[track_caller]
    fn checked(self) -> T {
        match self {
            Some(v) => v,
            None => panic!("missing independent stage fixture value"),
        }
    }
}
