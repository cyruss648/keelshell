use super::ProfileSyncPanel;
use gpui_kit::{AppContext, TestAppContext};
use gpui_kit::{
    Bounds, WindowBounds, WindowOptions, point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::StateStore;
use std::sync::Arc;
const MASTER: &str = "isolated-ui-sync-password";
#[gpui_kit::test]
async fn production_handlers_require_every_choice_and_clear_password_before_worker(
    cx: &mut TestAppContext,
) {
    let temporary = tempfile::tempdir().checked();
    let dir = temporary.path().join("shared");
    std::fs::create_dir(&dir).checked();
    let store = Arc::new(StateStore::new(temporary.path().join("state.json")));
    let mut state = store.load().checked();
    state.connections.push(keelshell_core::Connection::new(
        "Fixture",
        "fixture.invalid",
        "fixture",
    ));
    let state = store.save(&state).checked();
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .checked(),
    );
    let path = dir.clone();
    let (window, panel) = cx.update(|cx| {
        gpui_kit::init(cx);
        crate::i18n::set_language(keelshell_core::Language::ZhCn, cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(800.), px(600.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| ProfileSyncPanel::new(store.clone(), &state, runtime, window, cx))
            },
        )
        .checked()
    });
    cx.update_window(window, |_, w, cx| {
        panel.update(cx, |p, cx| {
            p.directory
                .update(cx, |i, cx| i.set_value(path.to_string_lossy(), w, cx));
            p.password.update(cx, |i, cx| i.set_value(MASTER, w, cx));
        });
        w.render_frame(cx);
        w.click("profile-sync-inspect", cx);
        assert!(panel.read(cx).password.read(cx).value().is_empty());
    })
    .checked();
    cx.wait_for(window, std::time::Duration::from_secs(20), |_, cx| {
        !panel.read(cx).busy()
    })
    .await;
    cx.update_window(window, |_, w, cx| {
        assert!(panel.read(cx).review.is_some());
        assert!(panel.read(cx).choices.is_empty());
        assert!(!dir.join("keelshell-profiles.ksync").exists());
        w.render_frame(cx);
        w.click("profile-sync-approve", cx);
        assert!(!panel.read(cx).busy());
        super::layout_tests::reveal(w, ("profile-sync-local", 0_usize), cx);
        w.click(("profile-sync-local", 0_usize), cx);
        assert_eq!(panel.read(cx).choices.len(), 1);
        panel.update(cx, |p, cx| {
            p.password.update(cx, |i, cx| i.set_value(MASTER, w, cx))
        });
        w.render_frame(cx);
        w.click("profile-sync-approve", cx);
        assert!(panel.read(cx).password.read(cx).value().is_empty());
    })
    .checked();
    cx.wait_for(window, std::time::Duration::from_secs(20), |_, cx| {
        !panel.read(cx).busy()
    })
    .await;
    assert!(dir.join("keelshell-profiles.ksync").exists());
    assert!(store.load().checked().profile_sync.checked().enabled());
    cx.update_window(window, |_, w, cx| {
        assert!(panel.read(cx).review.is_none());
        assert!(!panel.read(cx).pending);
        w.render_frame(cx);
    })
    .checked();
}

trait Checked<T> {
    fn checked(self) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    #[track_caller]
    fn checked(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("sync fixture failed: {error:?}"),
        }
    }
}
impl<T> Checked<T> for Option<T> {
    #[track_caller]
    fn checked(self) -> T {
        match self {
            Some(value) => value,
            None => panic!("sync fixture missing expected value"),
        }
    }
}
