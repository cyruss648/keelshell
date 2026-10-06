//! Reviewer checks derived effects with actual stores and production UI events.
use super::{ProfileSyncPanel, Report};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{
    AuthMethod, Connection, Language, ProfileSyncChoice, ProfileSyncReview, ProfileSyncService,
    StateStore, Theme,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
};
use uuid::Uuid;
use zeroize::Zeroizing;
const PASSWORD: &str = "review-ui-route-effects-password";
const JUMP: Uuid = Uuid::from_u128(96_000);
const FIRST: Uuid = Uuid::from_u128(96_001);
struct Fixture {
    _temporary: tempfile::TempDir,
    directory: std::path::PathBuf,
    local: Arc<StateStore>,
}
impl Fixture {
    fn new(children: usize, local_folder: bool) -> Self {
        let temporary = tempfile::tempdir().checked();
        let directory = temporary.path().join("shared");
        std::fs::create_dir(&directory).checked();
        let peer = Arc::new(StateStore::new(temporary.path().join("peer/state.json")));
        let local = Arc::new(StateStore::new(temporary.path().join("local/state.json")));
        let mut state = peer.load().checked();
        let mut jump =
            Connection::new("Reviewed parent jump", "old-jump.fixture.invalid", "review");
        jump.id = JUMP;
        state.connections.push(jump);
        for index in 0..children {
            let mut target = Connection::new(
                format!("Target {index:02}"),
                format!("target-{index:02}.fixture.invalid"),
                "review",
            );
            target.id = Uuid::from_u128(96_001 + index as u128);
            target.jump_host = Some(JUMP);
            state.connections.push(target);
        }
        peer.save(&state).checked();
        local.save(&local.load().checked()).checked();
        sync(&peer, &directory, ProfileSyncChoice::Local);
        sync(&local, &directory, ProfileSyncChoice::Remote);
        let mut state = local.load().checked();
        for profile in &mut state.connections {
            profile.auth = AuthMethod::PrivateKey {
                path: "/synthetic/local-ui-key".into(),
            };
            profile.credential_ref = Some(Uuid::from_u128(97_000 + profile.id.as_u128()));
        }
        if local_folder {
            let folder = state
                .create_folder("Device-local folder remains", None)
                .checked();
            state.move_connection(JUMP, Some(folder)).checked();
        }
        local.save(&state).checked();
        let mut state = peer.load().checked();
        state
            .connections
            .iter_mut()
            .find(|p| p.id == JUMP)
            .checked()
            .host = "new-jump.fixture.invalid".into();
        peer.save(&state).checked();
        sync(&peer, &directory, ProfileSyncChoice::Local);
        Self {
            _temporary: temporary,
            directory,
            local,
        }
    }
    fn review(&self) -> ProfileSyncReview {
        ProfileSyncService::new(self.local.clone())
            .inspect(
                self.directory.clone(),
                Zeroizing::new(PASSWORD.into()),
                &AtomicBool::new(false),
            )
            .checked()
    }
}
fn sync(store: &Arc<StateStore>, directory: &std::path::Path, choice: ProfileSyncChoice) {
    let service = ProfileSyncService::new(store.clone());
    let review = service
        .inspect(
            directory.into(),
            Zeroizing::new(PASSWORD.into()),
            &AtomicBool::new(false),
        )
        .checked();
    let choices = review
        .rows()
        .iter()
        .map(|r| (r.id, choice))
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
}
fn mount(
    cx: &mut TestAppContext,
    fixture: &Fixture,
    width: f32,
    height: f32,
) -> (AnyWindowHandle, Entity<ProfileSyncPanel>) {
    let state = fixture.local.load().checked();
    let review = fixture.review();
    let store = fixture.local.clone();
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .checked(),
    );
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
                    let mut panel = ProfileSyncPanel::new(store, &state, runtime, window, cx);
                    panel.complete(Ok(Report::Review(review)), window, cx);
                    panel
                })
            },
        )
        .checked()
    })
}
fn footer(window: &gpui_kit::Window, controls: &[&'static str]) {
    let body = window.find("profile-sync-body").bounds();
    let footer = window.find("profile-sync-footer").bounds();
    let outer = window.bounds();
    assert!(body.size.height >= px(40.) && body.bottom() <= footer.top());
    assert!(
        footer.left() >= outer.left()
            && footer.right() <= outer.right()
            && footer.top() >= outer.top()
            && footer.bottom() <= outer.bottom()
    );
    for id in controls {
        let target = window.find(*id);
        let bounds = target.bounds();
        assert!(target.visible());
        assert!(
            bounds.left() >= footer.left()
                && bounds.right() <= footer.right()
                && bounds.top() >= footer.top()
                && bounds.bottom() <= footer.bottom()
        );
    }
}
#[gpui_kit::test]
async fn route_effects_need_explicit_confirmation_before_production_apply(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let fixture = Fixture::new(1, false);
    let original = std::fs::read(fixture.local.path()).checked();
    let peer = std::fs::read(fixture.directory.join("keelshell-profiles.ksync")).checked();
    let (handle, panel) = mount(cx, &fixture, 900., 580.);
    cx.update_window(handle, |_, window, cx| {
        crate::i18n::set_language(Language::ZhCn, cx);
        window.render_frame(cx);
        super::layout_tests::reveal(window, ("profile-sync-remote", 0_usize), cx);
        window.click(("profile-sync-remote", 0_usize), cx);
        let preview = panel.read(cx).preview.as_ref().checked();
        assert!(
            preview
                .route_changes
                .iter()
                .any(|c| c.id == FIRST && c.resets_authentication)
        );
        assert!(!panel.read(cx).effects_acknowledged);
        panel.update(cx, |p, cx| {
            p.password
                .update(cx, |input, cx| input.set_value(PASSWORD, window, cx))
        });
        window.render_frame(cx);
        window.click("profile-sync-approve", cx);
        assert!(!panel.read(cx).busy());
        assert_eq!(std::fs::read(fixture.local.path()).checked(), original);
        assert_eq!(
            std::fs::read(fixture.directory.join("keelshell-profiles.ksync")).checked(),
            peer
        );
        footer(
            window,
            &["profile-sync-approve", "profile-sync-acknowledge-effects"],
        );
        window.click("profile-sync-acknowledge-effects", cx);
        assert!(panel.read(cx).effects_acknowledged);
        window.render_frame(cx);
        window.click("profile-sync-approve", cx);
        assert!(panel.read(cx).busy());
        assert!(panel.read(cx).password.read(cx).value().is_empty());
    })
    .checked();
    cx.wait_for(handle, std::time::Duration::from_secs(25), |_, cx| {
        !panel.read(cx).busy()
    })
    .await;
    let saved = fixture.local.load().checked();
    let target = saved.connections.iter().find(|p| p.id == FIRST).checked();
    assert_eq!(target.auth, AuthMethod::Agent);
    assert!(target.credential_ref.is_none());
    assert_eq!(
        saved
            .connection_route(FIRST)
            .checked()
            .identity()
            .endpoints()[0]
            .host,
        "new-jump.fixture.invalid"
    );
    assert!(!saved.profile_sync.checked().publication_pending());
}
#[gpui_kit::test]
fn route_effect_pagination_and_confirmation_fit_language_theme_small_window_matrix(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let fixture = Fixture::new(24, true);
    let original = std::fs::read(fixture.local.path()).checked();
    for (width, height) in [(480., 440.), (900., 580.)] {
        for language in [Language::ZhCn, Language::En] {
            for theme in [Theme::System, Theme::Light, Theme::Dark] {
                let (handle, panel) = mount(cx, &fixture, width, height);
                cx.update_window(handle,|_,window,cx|{
            crate::i18n::set_language(language,cx);crate::design::apply(theme,Some(window),cx);window.render_frame(cx);
            let review=panel.read(cx).review.as_ref().checked();assert_eq!(review.rows().len(),1);assert_eq!(review.rows()[0].local_placement.as_deref(),Some("Device-local folder remains"));
            super::layout_tests::reveal(window,("profile-sync-remote",0_usize),cx);window.click(("profile-sync-remote",0_usize),cx);window.render_frame(cx);
            assert_eq!(panel.read(cx).preview.as_ref().checked().route_changes.len(),25);footer(window,&["profile-sync-close","profile-sync-approve","profile-sync-acknowledge-effects"]);
            super::layout_tests::reveal(window,"profile-sync-impact-next",cx);window.click("profile-sync-impact-next",cx);window.render_frame(cx);assert_eq!(panel.read(cx).impact_page,1);assert!(window.try_find(("profile-sync-route-effect",24_usize)).is_some());assert!(window.try_find(("profile-sync-route-effect",0_usize)).is_none());footer(window,&["profile-sync-acknowledge-effects"]);
            window.click("profile-sync-acknowledge-effects",cx);assert!(panel.read(cx).effects_acknowledged);
            super::layout_tests::reveal(window,("profile-sync-local",0_usize),cx);window.click(("profile-sync-local",0_usize),cx);assert!(panel.read(cx).preview.as_ref().checked().route_changes.is_empty());assert!(!panel.read(cx).effects_acknowledged,"changing a choice must revoke prior effect confirmation, including retained local placement");
            assert_eq!(std::fs::read(fixture.local.path()).checked(),original);
        }).checked();
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
            Ok(v) => v,
            Err(e) => panic!("review UI fixture failed: {e:?}"),
        }
    }
}
impl<T> Checked<T> for Option<T> {
    #[track_caller]
    fn checked(self) -> T {
        match self {
            Some(v) => v,
            None => panic!("review UI fixture missing value"),
        }
    }
}
