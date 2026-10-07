//! Real worker/foreground separation without password derivation or external services.

use super::{Action, ProfileSyncPanel, work_trace::Phase};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size, test::TestAppContextExt,
};
use keelshell_core::StateStore;
use std::{sync::Arc, time::Duration};

fn mount(
    cx: &mut TestAppContext,
) -> (
    tempfile::TempDir,
    AnyWindowHandle,
    Entity<ProfileSyncPanel>,
    Arc<tokio::runtime::Runtime>,
) {
    let temporary = tempfile::tempdir().checked("isolated trace fixture");
    let store = Arc::new(StateStore::new(temporary.path().join("state.json")));
    let state = store.load().checked("owned initial state");
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .checked("owned trace runtime"),
    );
    let panel_runtime = runtime.clone();
    let (window, panel) = cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(900.), px(580.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| ProfileSyncPanel::new(store, &state, panel_runtime, window, cx))
            },
        )
        .checked("owned trace window")
    });
    (temporary, window, panel, runtime)
}

fn wait_on_real_worker(predicate: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(
            std::time::Instant::now() < deadline,
            "trace worker deadline"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[gpui_kit::test]
async fn blocking_queue_and_held_foreground_have_distinct_observed_phases(cx: &mut TestAppContext) {
    let (_temporary, window, panel, runtime) = mount(cx);
    let (entered, entry) = std::sync::mpsc::sync_channel(1);
    let (release, released) = std::sync::mpsc::sync_channel(1);
    let blocker = runtime.spawn_blocking(move || {
        entered.send(()).checked("blocker entry");
        released
            .recv_timeout(Duration::from_secs(5))
            .checked("bounded owner release");
    });
    entry
        .recv_timeout(Duration::from_secs(3))
        .checked("actual blocking pool entry");
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| panel.submit(Action::Disable, window, cx));
    })
    .checked("production handler admission");
    let trace = panel.read_with(cx, |panel, _| {
        panel
            .work_trace
            .as_ref()
            .checked("actual worker trace")
            .clone()
    });
    wait_on_real_worker(|| trace.snapshot().offsets_us[Phase::AsyncEntered as usize].is_some());
    let queued = trace.snapshot();
    assert!(queued.offsets_us[Phase::BlockingEntered as usize].is_none());
    assert!(queued.offsets_us[Phase::CoreReturned as usize].is_none());
    assert!(panel.read_with(cx, |panel, _| panel.busy()));
    release.send(()).checked("release owned blocker");
    // Deliberately do not drive GPUI: real Tokio/FS work can finish independently.
    wait_on_real_worker(|| trace.snapshot().offsets_us[Phase::BlockingJoined as usize].is_some());
    assert!(blocker.is_finished());
    let finished = trace.snapshot();
    assert!(
        finished.offsets_us[..=Phase::BlockingJoined as usize]
            .iter()
            .all(Option::is_some)
    );
    assert!(finished.offsets_us[Phase::ForegroundReceived as usize].is_none());
    assert!(finished.offsets_us[Phase::CallbackEntered as usize].is_none());
    assert!(panel.read_with(cx, |panel, _| panel.busy()));
    cx.wait_for(window, Duration::from_secs(3), |_, cx| {
        !panel.read(cx).busy()
    })
    .await;
    let completed = trace.snapshot();
    assert!(completed.offsets_us.iter().all(Option::is_some));
    for pair in completed.offsets_us.windows(2) {
        assert!(pair[0] <= pair[1], "actual phase order: {completed:?}");
    }
}

#[gpui_kit::test]
async fn rejected_admission_does_not_invent_a_worker_observation(cx: &mut TestAppContext) {
    let (_temporary, window, panel, _runtime) = mount(cx);
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| panel.submit(Action::Inspect, window, cx));
        assert!(!panel.read(cx).busy());
        assert!(panel.read(cx).work_trace.is_none());
    })
    .checked("empty password handler rejection");
}

trait Checked<T> {
    fn checked(self, stage: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    #[track_caller]
    fn checked(self, stage: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{stage}: {error:?}"),
        }
    }
}
impl<T> Checked<T> for Option<T> {
    #[track_caller]
    fn checked(self, stage: &str) -> T {
        match self {
            Some(value) => value,
            None => panic!("{stage}: missing owned fixture value"),
        }
    }
}
