//! Real local components in a bounded GPUI scene; this is not native acceptance.

use crate::files::{
    FilesPanel, Operation,
    fixture_group::FixtureGroup,
    test_server::{Checked, Server},
};
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Context, Entity, InputEvent, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Point, Render, ScrollDelta, ScrollEvent, Styled, Subscription, TestAppContext, TestSupportExt,
    Window, WindowBounds, WindowOptions, accesskit,
    component::input::InputState,
    div, point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{Language, Settings, Theme, ThemeId};
use std::{path::PathBuf, sync::Arc, time::Duration};

const CONTROLS: [&str; 4] = [
    "choose-local-folder",
    "parent-local-folder",
    "refresh-local-folder",
    "toggle-local-hidden",
];

struct LocalScene {
    panel: Entity<FilesPanel>,
    browser_width: f32,
    _subscription: Subscription,
}

impl Render for LocalScene {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.panel.update(cx, |panel, cx| {
            div().size_full().child(
                div()
                    .id("local-browser-scene")
                    .w(px(self.browser_width))
                    .h(px(300.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .w_full()
                            .h(px(if panel.pending.is_some() || panel.suspended {
                                29.
                            } else {
                                61.
                            }))
                            .flex_shrink_0()
                            .border_b_1()
                            .flex()
                            .child(panel.local_navigation(cx)),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(panel.local_browser_view(cx)),
                    )
                    .test_support(),
            )
        })
    }
}

struct Fixture {
    window: AnyWindowHandle,
    scene: Entity<LocalScene>,
    panel: Entity<FilesPanel>,
    _server: Server,
    _runtime: Arc<tokio::runtime::Runtime>,
    _group: Arc<FixtureGroup>,
}

impl Fixture {
    fn new(cx: &mut TestAppContext) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .checked("local navigation fixture runtime"),
        );
        let group = FixtureGroup::acquire_for_context(cx, &runtime);
        let server = Server::new(&runtime);
        let session = server.connect(&runtime);
        let (window, scene) = cx.update(|cx| {
            gpui_kit::init(cx);
            crate::i18n::set_language(Language::ZhCn, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(1200.), px(440.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    crate::design::apply_settings(&Settings::default(), Some(window), cx);
                    let panel = cx.new(|cx| {
                        FilesPanel::new(
                            session,
                            "local navigation fixture".into(),
                            runtime.clone(),
                            window,
                            cx,
                        )
                    });
                    cx.new(|cx| LocalScene {
                        _subscription: cx.observe(&panel, |_, _, cx| cx.notify()),
                        panel,
                        browser_width: 480.,
                    })
                },
            )
            .checked("mount real local navigation and table")
        });
        let panel = scene.read_with(cx, |scene, _| scene.panel.clone());
        Self {
            window,
            scene,
            panel,
            _server: server,
            _runtime: runtime,
            _group: group,
        }
    }

    async fn idle(&self, cx: &mut TestAppContext) {
        cx.wait_for(self.window, Duration::from_secs(12), |_, cx| {
            let panel = self.panel.read(cx);
            !panel.busy && !panel.local_browser.reading() && !panel.local_browser.choosing()
        })
        .await;
    }

    fn path(&self, cx: &mut TestAppContext) -> Entity<InputState> {
        self.panel
            .read_with(cx, |panel, _| panel.local_browser.path.clone())
    }

    fn click(&self, id: &'static str, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
            let target = window.find(id);
            assert!(target.visible(), "{id} must be painted");
            assert!(contained(
                target.bounds(),
                window.find("local-folder-navigation").bounds()
            ));
            let position = target.bounds().center();
            // Keep the painted listeners intact across a physical pointer pair.
            for input in [
                MouseMoveEvent {
                    position,
                    ..Default::default()
                }
                .to_platform_input(),
                MouseDownEvent {
                    position,
                    button: MouseButton::Left,
                    click_count: 1,
                    ..Default::default()
                }
                .to_platform_input(),
                MouseUpEvent {
                    position,
                    button: MouseButton::Left,
                    click_count: 1,
                    ..Default::default()
                }
                .to_platform_input(),
            ] {
                window.dispatch_event(input, cx);
            }
            window.render_frame(cx);
        })
        .checked("click the painted local navigation control");
    }

    fn directory(&self, cx: &mut TestAppContext) -> Option<PathBuf> {
        self.panel.read_with(cx, |panel, _| {
            panel
                .local_browser
                .listing
                .as_ref()
                .map(|listing| listing.directory.clone())
        })
    }
}

fn contained(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.x >= outer.origin.x
        && inner.origin.y >= outer.origin.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

#[gpui_kit::test]
async fn local_navigation_retains_full_width_draft_and_focus_across_theme_language_and_width(
    cx: &mut TestAppContext,
) {
    let fixture = Fixture::new(cx);
    fixture.idle(cx).await;
    let path = fixture.path(cx);
    let draft = format!("/local-draft/{}/目录", "long-segment/".repeat(20));
    fixture.click("local-folder-path-input", cx);
    cx.simulate_input(fixture.window, &draft);

    for (browser_width, expected_local_width) in [(480., 168.), (700., 224.), (1100., 290.)] {
        for family in crate::design::registry() {
            for theme in [Theme::Light, Theme::Dark] {
                for language in [Language::ZhCn, Language::En] {
                    cx.update_window(fixture.window, |_, window, cx| {
                        let settings = Settings {
                            theme_family: ThemeId::new(family.id).checked("registered theme ID"),
                            theme,
                            ..Default::default()
                        };
                        crate::design::apply_settings(&settings, Some(window), cx);
                        crate::i18n::set_language(language, cx);
                        fixture
                            .panel
                            .update(cx, |panel, cx| panel.refresh_locale(window, cx));
                        fixture.scene.update(cx, |scene, cx| {
                            scene.browser_width = browser_width;
                            cx.notify();
                        });
                        window.render_frame(cx);

                        let navigation = window.find("local-folder-navigation").bounds();
                        let table = window.find("local-files-pane").bounds();
                        let controls = window.find("local-folder-controls").bounds();
                        let label = window.find("local-folder-label").bounds();
                        let input = window.find("local-folder-path-input");
                        assert_eq!(navigation.size.height, px(60.));
                        assert_eq!(navigation.size.width, px(expected_local_width));
                        assert_eq!(table.size.width, navigation.size.width);
                        assert_eq!(table.origin.x, navigation.origin.x);
                        assert!(table.origin.y >= navigation.bottom());
                        assert!(contained(controls, navigation));
                        assert!(contained(input.bounds(), navigation) && input.visible());
                        assert!(input.bounds().origin.y >= controls.bottom());
                        assert!(
                            input.bounds().size.width >= navigation.size.width - px(10.),
                            "the editable path must use the whole local width: {input:?}"
                        );
                        assert!(label.size.width >= px(32.), "the Local label must fit");
                        let mut previous = label;
                        for id in CONTROLS {
                            let button = window.find(id);
                            assert!(button.visible() && contained(button.bounds(), controls));
                            assert!(button.bounds().origin.x >= previous.right());
                            assert!(button.bounds().size.width >= px(24.));
                            assert!(button.bounds().size.height >= px(24.));
                            assert!(button.label().is_some_and(|label| !label.is_empty()));
                            previous = button.bounds();
                        }
                        assert_eq!(input.focused(), Some(true));
                        assert_eq!(input.value(), Some(draft.as_str()));
                        assert_eq!(fixture.panel.read(cx).local_browser.path, path);
                        assert_eq!(path.read(cx).value().as_str(), draft);
                        assert!(fixture.panel.read(cx).local_browser.listing.is_none());
                        assert!(!fixture.panel.read(cx).local_browser.reading());
                    })
                    .checked("measure real local geometry and retained draft");
                }
            }
        }
    }
    cx.simulate_input(fixture.window, "-still-focused");
    path.read_with(cx, |path, _| {
        assert_eq!(path.value().as_str(), format!("{draft}-still-focused"));
    });
}

#[gpui_kit::test]
async fn minimum_local_width_keeps_all_four_navigation_actions_reachable(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().checked("isolated local navigation data");
    let root = directory
        .path()
        .canonicalize()
        .checked("canonical local root");
    let child = root.join("child");
    std::fs::create_dir(&child).checked("local child directory");
    let mut hidden_options = std::fs::OpenOptions::new();
    hidden_options.write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Windows uses the native hidden attribute, rather than a dot prefix.
        hidden_options.attributes(0x0000_0002);
    }
    let mut hidden_file = hidden_options
        .open(child.join(".hidden"))
        .checked("local hidden file");
    std::io::Write::write_all(&mut hidden_file, b"hidden").checked("local hidden contents");
    drop(hidden_file);
    std::fs::write(child.join("visible"), b"visible").checked("local visible file");
    let fixture = Fixture::new(cx);
    fixture.idle(cx).await;

    fixture.click("choose-local-folder", cx);
    assert!(cx.did_prompt_for_paths());
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("choose-local-folder").disabled(), Some(true));
        assert_eq!(window.find("parent-local-folder").disabled(), Some(true));
    })
    .checked("picker pending and empty listing retain disabled states");
    cx.simulate_path_prompt_response(|options| {
        assert!(options.directories && !options.files && !options.multiple);
        Some(vec![child.clone()])
    });
    cx.run_until_parked();
    fixture.idle(cx).await;
    assert_eq!(fixture.directory(cx), Some(child.clone()));

    let hidden_index = fixture.panel.read_with(cx, |panel, _| {
        panel
            .local_browser
            .listing
            .as_ref()
            .unwrap_or_else(|| panic!("actual directory listing"))
            .entries
            .iter()
            .position(|entry| entry.path == child.join(".hidden"))
            .unwrap_or_else(|| panic!("actual hidden file"))
    });
    for shown in [true, false] {
        fixture.click("toggle-local-hidden", cx);
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            let toggle = window.find("toggle-local-hidden");
            assert_eq!(toggle.role(), Some(accesskit::Role::Button));
            assert_eq!(toggle.checked(), Some(shown));
            assert_eq!(
                window.try_find(("local-entry", hidden_index)).is_some(),
                shown
            );
            assert_eq!(fixture.panel.read(cx).local_browser.show_hidden, shown);
        })
        .checked("hidden toggle changes real rows and accessible state");
    }

    fixture.click("parent-local-folder", cx);
    fixture.idle(cx).await;
    assert_eq!(fixture.directory(cx), Some(root));
    fixture.click("local-folder-path-input", cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
            cx,
        );
        window.input(
            child
                .to_str()
                .unwrap_or_else(|| panic!("temporary UTF-8 path")),
            cx,
        );
    })
    .checked("edit the full-width local path");
    fixture.click("refresh-local-folder", cx);
    fixture.idle(cx).await;
    assert_eq!(fixture.directory(cx), Some(child));
    fixture.panel.read_with(cx, |panel, cx| {
        assert!(panel.local.read(cx).value().is_empty());
        assert!(panel.pending.is_none() && panel.transfer_jobs.is_empty());
    });
}

#[gpui_kit::test]
async fn suspended_local_navigation_rejects_picker_completion_and_retains_the_path(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().checked("isolated suspended picker directory");
    let root = directory
        .path()
        .canonicalize()
        .checked("canonical picker root");
    let fixture = Fixture::new(cx);
    fixture.idle(cx).await;
    fixture.click("local-folder-path-input", cx);
    cx.simulate_input(fixture.window, "retained navigation draft");
    fixture.click("choose-local-folder", cx);
    assert!(cx.did_prompt_for_paths());
    fixture.panel.update(cx, |panel, cx| panel.suspend(cx));
    cx.simulate_path_prompt_response(|_| Some(vec![root]));
    cx.run_until_parked();
    fixture.idle(cx).await;

    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let navigation = window.find("local-folder-navigation").bounds();
        let input = window.find("local-folder-path-input");
        assert_eq!(navigation.size.height, px(28.));
        assert!(input.visible() && contained(input.bounds(), navigation));
        assert!(input.bounds().size.width >= navigation.size.width - px(10.));
        assert!(window.try_find("local-folder-controls").is_none());
        for id in CONTROLS {
            assert!(window.try_find(id).is_none());
        }
    })
    .checked("suspended local navigation retains its compact path without actionable controls");
    fixture.panel.read_with(cx, |panel, cx| {
        assert!(panel.local_browser.listing.is_none());
        assert!(!panel.local_browser.reading() && !panel.local_browser.choosing());
        assert!(!panel.local_browser.show_hidden);
        assert_eq!(
            panel.local_browser.path.read(cx).value().as_str(),
            "retained navigation draft"
        );
    });
}

#[gpui_kit::test]
async fn compact_pending_navigation_keeps_the_focused_path_and_typing_withdraws_review(
    cx: &mut TestAppContext,
) {
    let fixture = Fixture::new(cx);
    fixture.idle(cx).await;
    let path = fixture.path(cx);
    fixture.click("local-folder-path-input", cx);
    cx.simulate_input(fixture.window, "/retained/local/draft");
    fixture.panel.update(cx, |panel, cx| {
        panel.pending = Some((
            crate::i18n::Message::new("测试待审核操作", "Fixture pending review"),
            Operation::Mkdir("/unissued-review".into()),
        ));
        cx.notify();
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let navigation = window.find("local-folder-navigation").bounds();
        let input = window.find("local-folder-path-input");
        assert_eq!(navigation.size.height, px(28.));
        assert_eq!(navigation.size.width, px(168.));
        assert!(input.visible() && contained(input.bounds(), navigation));
        assert!(input.bounds().size.width >= navigation.size.width - px(10.));
        assert_eq!(input.focused(), Some(true));
        assert_eq!(input.value(), Some("/retained/local/draft"));
        assert_eq!(fixture.panel.read(cx).local_browser.path, path);
        assert!(window.try_find("local-folder-controls").is_none());
        for id in CONTROLS {
            assert!(window.try_find(id).is_none());
        }
    })
    .checked("pending review keeps the real path editor in one row");

    cx.simulate_input(fixture.window, "-changed");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let panel = fixture.panel.read(cx);
        assert!(panel.pending.is_none());
        assert!(panel.operation_id.is_none() && !panel.busy);
        assert_eq!(panel.local_browser.path, path);
        assert_eq!(
            path.read(cx).value().as_str(),
            "/retained/local/draft-changed"
        );
        assert_eq!(window.find("local-folder-path-input").focused(), Some(true));
        assert_eq!(
            window.find("local-folder-navigation").bounds().size.height,
            px(60.)
        );
        for id in CONTROLS {
            assert!(window.find(id).visible());
        }
    })
    .checked("typing withdraws review without issuing its operation or replacing the input");
}

fn scroll_at(window: &mut Window, cx: &mut App, position: Point<Pixels>, delta: ScrollDelta) {
    window.dispatch_event(
        ScrollEvent {
            position,
            delta,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

#[gpui_kit::test]
async fn local_file_rows_ignore_horizontal_wheel_and_remain_vertically_scrollable(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().checked("isolated local wheel files");
    let root = directory
        .path()
        .canonicalize()
        .checked("canonical wheel root");
    for index in 0..12 {
        std::fs::write(root.join(format!("row-{index:02}.txt")), b"local wheel")
            .checked("seed local wheel row");
    }
    let fixture = Fixture::new(cx);
    fixture.idle(cx).await;
    fixture.click("choose-local-folder", cx);
    cx.simulate_path_prompt_response(|_| Some(vec![root.clone()]));
    cx.run_until_parked();
    fixture.idle(cx).await;
    assert_eq!(fixture.directory(cx), Some(root));
    let first_index = fixture.panel.read_with(cx, |panel, _| {
        let listing = panel
            .local_browser
            .listing
            .as_ref()
            .unwrap_or_else(|| panic!("loaded local wheel files"));
        assert_eq!(listing.entries.len(), 12);
        *super::browser::local_indices(
            &listing.entries,
            panel.local_browser.show_hidden,
            panel.local_browser.sort,
        )
        .first()
        .unwrap_or_else(|| panic!("first rendered local entry"))
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let list = window.find("local-files").bounds();
        let position = point(list.origin.x + px(60.), list.origin.y + px(14.));
        let original = window.find(("local-entry", first_index)).bounds().origin;
        for pixels in [true, false] {
            let horizontal = if pixels {
                ScrollDelta::Pixels(point(px(-120.), px(0.)))
            } else {
                ScrollDelta::Lines(point(-3., 0.))
            };
            scroll_at(window, cx, position, horizontal);
            assert_eq!(
                window.find(("local-entry", first_index)).bounds().origin,
                original,
                "horizontal input must not be remapped onto local rows"
            );
            let vertical = if pixels {
                ScrollDelta::Pixels(point(px(0.), px(-120.)))
            } else {
                ScrollDelta::Lines(point(0., -3.))
            };
            scroll_at(window, cx, position, vertical);
            let moved = window.find(("local-entry", first_index)).bounds().origin;
            assert_eq!(moved.x, original.x);
            assert!(moved.y < original.y, "vertical input must move local rows");
            scroll_at(
                window,
                cx,
                position,
                ScrollDelta::Pixels(point(px(0.), px(100_000.))),
            );
            assert_eq!(
                window.find(("local-entry", first_index)).bounds().origin,
                original
            );
        }
        let panel = fixture.panel.read(cx);
        assert!(panel.pending.is_none() && panel.transfer_jobs.is_empty());
        assert_eq!(panel.local_browser.selection.len(), 0);
    })
    .checked("scroll actual local file rows without starting an operation");
    assert_eq!(fixture._server.filesystem.atomic_writes_started(), 0);
}
