//! Production-panel GPUI bounds and pointer pagination, without native acceptance claims.
use super::{ProfileSyncPanel, Report};
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, ElementId, Entity, Pixels, ScrollDelta,
    TestAppContext, Window, WindowBounds, WindowOptions, point, px, size, test::TestWindowExt,
};
use keelshell_core::{
    AppState, Connection, Language, ProfileSyncChoice, ProfileSyncReview, ProfileSyncService,
    StateStore, Theme,
};
use std::sync::{Arc, atomic::AtomicBool};
use tokio::runtime::Runtime;
use uuid::Uuid;
use zeroize::Zeroizing;

struct Fixture {
    _temporary: tempfile::TempDir,
    store: Arc<StateStore>,
    directory: std::path::PathBuf,
    state: AppState,
}

impl Fixture {
    fn new(profiles: usize) -> Self {
        let temporary = tempfile::tempdir().checked();
        let directory = temporary.path().join("shared");
        std::fs::create_dir(&directory).checked();
        let store = Arc::new(StateStore::new(temporary.path().join("state.json")));
        let mut state = store.load().checked();
        for index in 0..profiles {
            let mut profile = Connection::new(
                format!("Profile {index:02} {}", "连接审阅".repeat(20)),
                "long-hostname-for-metadata-review.fixture.invalid",
                "fixture-review-account",
            );
            profile.id = Uuid::from_u128(20_000 + index as u128);
            profile.group = "Long readable grouping ".repeat(4).trim_end().into();
            profile.tags = (0..8)
                .map(|tag| format!("tag-{tag}-{}", "context".repeat(6)))
                .collect();
            state.connections.push(profile);
        }
        let state = store.save(&state).checked();
        Self {
            _temporary: temporary,
            store,
            directory,
            state,
        }
    }

    fn review(&self) -> ProfileSyncReview {
        ProfileSyncService::new(self.store.clone())
            .inspect(
                self.directory.clone(),
                Zeroizing::new("isolated-layout-sync-password".into()),
                &AtomicBool::new(false),
            )
            .checked()
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
    fixture: &Fixture,
    runtime: Arc<Runtime>,
    review: Option<ProfileSyncReview>,
    width: f32,
    height: f32,
) -> (AnyWindowHandle, Entity<ProfileSyncPanel>) {
    cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(width), px(height)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    let mut panel = ProfileSyncPanel::new(
                        fixture.store.clone(),
                        &fixture.state,
                        runtime,
                        window,
                        cx,
                    );
                    if let Some(review) = review {
                        panel.complete(Ok(Report::Review(review)), window, cx);
                    }
                    panel
                })
            },
        )
        .checked()
    })
}

fn contains(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.left() >= outer.left()
        && inner.right() <= outer.right()
        && inner.top() >= outer.top()
        && inner.bottom() <= outer.bottom()
}

fn assert_footer(window: &Window, controls: &[&'static str]) -> Bounds<Pixels> {
    let panel = window.find("profile-sync-panel").bounds();
    let body = window.find("profile-sync-body").bounds();
    let footer = window.find("profile-sync-footer").bounds();
    assert!(
        contains(panel, window.bounds()),
        "panel escapes window: {panel:?}"
    );
    assert!(contains(body, panel), "body escapes panel: {body:?}");
    assert!(contains(footer, panel), "footer escapes panel: {footer:?}");
    assert!(
        body.size.height >= px(40.),
        "scroll viewport collapsed: {body:?}"
    );
    assert!(body.bottom() <= footer.top(), "body overlaps fixed footer");
    for id in controls {
        let target = window.find(*id);
        assert!(target.visible(), "{id} must be visible");
        assert!(
            contains(target.bounds(), footer),
            "{id} escapes footer: {:?}",
            target.bounds()
        );
        assert!(
            contains(target.bounds(), window.bounds()),
            "{id} escapes window"
        );
        assert!(
            target.bounds().size.width > px(20.) && target.bounds().size.height > px(15.),
            "{id} collapsed: {:?}",
            target.bounds()
        );
    }
    footer
}

fn scroll_body(window: &mut Window, delta: f32, cx: &mut App) {
    window.scroll(
        "profile-sync-body",
        ScrollDelta::Pixels(point(px(0.), px(delta))),
        cx,
    );
    window.render_frame(cx);
}

pub(super) fn reveal(window: &mut Window, id: impl Into<ElementId>, cx: &mut App) {
    let id = id.into();
    for _ in 0..6 {
        window.render_frame(cx);
        let viewport = window.find("profile-sync-body").bounds();
        let target = window.find(id.clone());
        if target.visible() && contains(target.bounds(), viewport) {
            return;
        }
        let delta = viewport.top() + viewport.size.height / 2.
            - target.bounds().top()
            - target.bounds().size.height / 2.;
        scroll_body(window, f32::from(delta), cx);
    }
    let viewport = window.find("profile-sync-body").bounds();
    let target = window.find(id);
    assert!(
        target.visible() && contains(target.bounds(), viewport),
        "bounded wheel must reveal target {:?} in {viewport:?}",
        target.bounds()
    );
}

#[gpui_kit::test]
fn body_and_footer_fit_small_windows_in_both_languages_and_themes(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let fixture = Fixture::new(0);
    let runtime = runtime();
    for (width, height) in [(480., 440.), (800., 600.)] {
        for language in [Language::ZhCn, Language::En] {
            for theme in [Theme::Light, Theme::Dark] {
                let (handle, panel) = mount(cx, &fixture, runtime.clone(), None, width, height);
                cx.update_window(handle, |_, window, cx| {
                    crate::i18n::set_language(language, cx);
                    crate::design::apply(theme, Some(window), cx);
                    window.render_frame(cx);
                    assert_footer(window, &["profile-sync-close", "profile-sync-inspect"]);
                    assert!(!panel.read(cx).busy());
                    assert!(!panel.read(cx).configured);
                    assert!(!fixture.directory.join("keelshell-profiles.ksync").exists());

                    // Pending is a controlled render fixture here; the separate
                    // core tests prove actual journal admission/recovery behavior.
                    panel.update(cx, |panel, cx| {
                        panel.configured = true;
                        panel.enabled = true;
                        panel.pending = true;
                        cx.notify();
                    });
                    window.render_frame(cx);
                    assert_footer(
                        window,
                        &[
                            "profile-sync-close",
                            "profile-sync-resume",
                            "profile-sync-discard-pending",
                            "profile-sync-disable",
                        ],
                    );
                })
                .checked();
            }
        }
    }
}

#[gpui_kit::test]
fn long_multi_page_review_keeps_footer_reachable_and_choices_across_real_pagination(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let fixture = Fixture::new(41);
    let runtime = runtime();
    for (width, height) in [(480., 440.), (800., 600.)] {
        for language in [Language::ZhCn, Language::En] {
            for theme in [Theme::Light, Theme::Dark] {
                let (handle, panel) = mount(
                    cx,
                    &fixture,
                    runtime.clone(),
                    Some(fixture.review()),
                    width,
                    height,
                );
                cx.update_window(handle, |_, window, cx| {
                    crate::i18n::set_language(language, cx);
                    crate::design::apply(theme, Some(window), cx);
                    window.render_frame(cx);
                    let actions = [
                        "profile-sync-close",
                        "profile-sync-discard-review",
                        "profile-sync-inspect",
                        "profile-sync-approve",
                    ];
                    let fixed = assert_footer(window, &actions);
                    assert_eq!(panel.read(cx).review.as_ref().checked().rows().len(), 41);
                    window.click("profile-sync-approve", cx);
                    assert!(
                        !panel.read(cx).busy(),
                        "incomplete review cannot start a worker"
                    );
                    assert!(window.try_find(("profile-sync-row", 19_usize)).is_some());
                    assert!(window.try_find(("profile-sync-row", 20_usize)).is_none());

                    reveal(window, ("profile-sync-local", 0_usize), cx);
                    window.click(("profile-sync-local", 0_usize), cx);
                    assert_eq!(panel.read(cx).choices.len(), 1);
                    scroll_body(window, -100_000., cx);
                    assert_eq!(assert_footer(window, &actions), fixed);
                    reveal(window, "profile-sync-page-next", cx);
                    window.click("profile-sync-page-next", cx);
                    window.render_frame(cx);
                    assert_eq!(panel.read(cx).page, 1);
                    assert!(window.try_find(("profile-sync-row", 0_usize)).is_none());
                    assert!(window.try_find(("profile-sync-row", 20_usize)).is_some());
                    assert!(window.try_find(("profile-sync-row", 39_usize)).is_some());

                    reveal(window, ("profile-sync-remote", 20_usize), cx);
                    window.click(("profile-sync-remote", 20_usize), cx);
                    assert_eq!(panel.read(cx).choices.len(), 2);
                    reveal(window, "profile-sync-page-next", cx);
                    window.click("profile-sync-page-next", cx);
                    window.render_frame(cx);
                    assert_eq!(panel.read(cx).page, 2);
                    assert!(window.try_find(("profile-sync-row", 40_usize)).is_some());
                    assert!(window.try_find(("profile-sync-row", 41_usize)).is_none());
                    reveal(window, ("profile-sync-local", 40_usize), cx);
                    window.click(("profile-sync-local", 40_usize), cx);
                    assert_eq!(panel.read(cx).choices.len(), 3);
                    window.click("profile-sync-approve", cx);
                    assert!(
                        !panel.read(cx).busy(),
                        "incomplete review cannot start a worker"
                    );
                    assert_eq!(assert_footer(window, &actions), fixed);
                    window.click("profile-sync-approve", cx);
                    assert!(
                        !panel.read(cx).busy(),
                        "3/41 selections cannot admit a worker"
                    );
                    assert!(!fixture.directory.join("keelshell-profiles.ksync").exists());

                    reveal(window, "profile-sync-page-prev", cx);
                    window.click("profile-sync-page-prev", cx);
                    window.render_frame(cx);
                    assert_eq!(panel.read(cx).page, 1);
                    assert_eq!(panel.read(cx).choices.len(), 3);
                    let choices = &panel.read(cx).choices;
                    assert_eq!(choices[&Uuid::from_u128(20_000)], ProfileSyncChoice::Local);
                    assert_eq!(choices[&Uuid::from_u128(20_020)], ProfileSyncChoice::Remote);
                    assert_eq!(choices[&Uuid::from_u128(20_040)], ProfileSyncChoice::Local);
                    assert_eq!(assert_footer(window, &actions), fixed);
                })
                .checked();
            }
        }
    }
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
