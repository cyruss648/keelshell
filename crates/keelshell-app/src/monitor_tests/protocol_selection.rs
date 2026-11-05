//! Actual workspace transitions against a pinned, gated loopback SSH peer.
use super::*;
use crate::workspace::tests::protocol_diagnostics::{
    assert_selection, change_selection, independent_reconnect_pending_reply_fixture,
    prepare_pointer_panes,
};
use gpui_kit::InputEvent as _;
use keelshell_core::NetworkDiagnosticKind;

fn pointer_press_without_render(
    window: &mut gpui_kit::Window,
    position: gpui_kit::Point<gpui_kit::Pixels>,
    cx: &mut gpui_kit::App,
) {
    window.dispatch_event(
        gpui_kit::MouseMoveEvent {
            position,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        gpui_kit::MouseDownEvent {
            position,
            button: gpui_kit::MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        gpui_kit::MouseUpEvent {
            position,
            button: gpui_kit::MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
}

/// GPUI may draw before a key event. Assertions immediately after key-down
/// inspect the action's result before another event can draw the new focus.
fn key_down(
    window: &mut gpui_kit::Window,
    key: &'static str,
    cx: &mut gpui_kit::App,
) -> gpui_kit::Keystroke {
    let keystroke = gpui_kit::Keystroke::parse(key).checked("parse selection key");
    window.dispatch_event(
        gpui_kit::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        }
        .to_platform_input(),
        cx,
    );
    keystroke
}

fn key_up(window: &mut gpui_kit::Window, keystroke: gpui_kit::Keystroke, cx: &mut gpui_kit::App) {
    window.dispatch_event(gpui_kit::KeyUpEvent { keystroke }.to_platform_input(), cx);
}

fn search_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd-f"
    } else {
        "ctrl-shift-f"
    }
}

#[gpui_kit::test]
async fn protocol_selection_aba_revokes_review_and_fresh_review_can_dispatch(
    cx: &mut TestAppContext,
) {
    let h = Harness::new(cx);
    cx.update_window(h.window, |_, window, _| window.remove_window())
        .checked("retire initial monitor window");
    let (window, monitor, workspace) =
        independent_reconnect_pending_reply_fixture(h.session.clone(), h.port, cx);
    let panel = monitor.update(cx, |monitor, _| {
        monitor.paused = true;
        monitor.protocol.clone()
    });
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        h.control.shells_ready.load(Ordering::Acquire) == 1
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        panel.update(cx, |panel, cx| {
            panel.kind = NetworkDiagnosticKind::Icmp;
            panel.endpoint.update(cx, |input, cx| {
                input.set_value("owned-review.test", window, cx)
            });
            panel.preview(cx);
            assert!(panel.review.is_some());
        });
        workspace.update(cx, |view, cx| change_selection(view, "aba", window, cx));
        // Both transitions occurred in this foreground turn. A queued old
        // confirm must fail without relying on render or a periodic observer.
        panel.update(cx, |panel, cx| {
            assert!(panel.review.is_none());
            assert!(panel.session.is_some());
            panel.confirm(cx);
            assert!(panel.cancel.is_none());
        });
    })
    .checked("ABA immediately retires consent");
    assert_eq!(h.control.requests.load(Ordering::Acquire), 0);
    for transition in ["same", "close_inactive"] {
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.preview(cx);
                assert!(panel.review.is_some());
            });
            workspace.update(cx, |view, cx| {
                change_selection(view, transition, window, cx)
            });
            assert!(
                panel.read(cx).review.is_some(),
                "{transition}: current review lost"
            );
        })
        .checked("same target retains diagnostic consent");
    }
    h.control.gate.add_permits(1);
    panel.update(cx, |panel, cx| panel.confirm(cx));
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).cancel.is_none()
    })
    .await;
    assert!(
        panel.read_with(cx, |panel, _| panel.result.as_ref().is_some_and(|report| {
            report.status == keelshell_session::network_diagnostic::NetworkDiagnosticStatus::Success
        }))
    );
    assert_eq!(h.control.requests.load(Ordering::Acquire), 1);
    cx.update_window(window, |_, window, _| window.remove_window())
        .checked("close selection review workspace");
    h.finish();
}

#[gpui_kit::test]
async fn protocol_selection_aba_cancels_held_reply_and_discards_late_completion(
    cx: &mut TestAppContext,
) {
    for kind in [NetworkDiagnosticKind::Dns, NetworkDiagnosticKind::Icmp] {
        let h = Harness::new(cx);
        cx.update_window(h.window, |_, window, _| window.remove_window())
            .checked("retire held-selection initial window");
        let (window, monitor, workspace) =
            independent_reconnect_pending_reply_fixture(h.session.clone(), h.port, cx);
        let panel = monitor.update(cx, |monitor, _| {
            monitor.paused = true;
            monitor.protocol.clone()
        });
        cx.wait_for(window, Duration::from_secs(3), |_, _| {
            h.control.shells_ready.load(Ordering::Acquire) == 1
        })
        .await;
        cx.update_window(window, |_, window, cx| {
            panel.update(cx, |panel, cx| {
                panel.kind = kind;
                panel.endpoint.update(cx, |input, cx| {
                    input.set_value("held-review.test", window, cx)
                });
                panel.preview(cx);
                assert!(panel.review.is_some());
                panel.confirm(cx);
            });
        })
        .checked("dispatch held diagnostic from current workspace selection");
        cx.wait_for(window, Duration::from_secs(3), |_, _| {
            h.control.requests.load(Ordering::Acquire) == 1
        })
        .await;
        let cancellation = panel.read_with(cx, |panel, _| {
            panel
                .cancel
                .clone()
                .unwrap_or_else(|| panic!("capture running cancellation"))
        });
        cx.update_window(window, |_, window, cx| {
            workspace.update(cx, |view, cx| change_selection(view, "aba", window, cx));
            assert!(cancellation.load(Ordering::Acquire));
        })
        .checked("cancel before any intervening render or reply");
        h.control.gate.add_permits(1);
        cx.wait_for(window, Duration::from_secs(5), |_, cx| {
            panel.read(cx).cancel.is_none()
        })
        .await;
        panel.update(cx, |panel, cx| {
            assert!(panel.result.is_none(), "{kind:?}: stale reply adopted");
            assert!(panel.review.is_none());
            panel.confirm(cx);
        });
        assert_eq!(h.control.requests.load(Ordering::Acquire), 1);
        cx.update_window(window, |_, window, _| window.remove_window())
            .checked("close held-selection workspace");
        h.finish();
    }
}

#[gpui_kit::test]
async fn protocol_icmp_unverified_exit_retires_authority_even_for_stale_completion(
    cx: &mut TestAppContext,
) {
    for mode in [1, 2, 3] {
        for stale in [false, true] {
            let h = Harness::new(cx);
            h.control.exit_mode.store(mode, Ordering::Release);
            h.click("protocol-expand", cx);
            h.click("protocol-mode-icmp", cx);
            h.set_endpoint("fixture.test", cx);
            h.click("protocol-preview", cx);
            h.click("protocol-confirm", cx);
            cx.wait_for(h.window, Duration::from_secs(3), |_, _| {
                h.control.requests.load(Ordering::Acquire) == 1
            })
            .await;
            if stale {
                // Isolate stale correlation from cancellation: the controlled
                // nonzero/signal reply must still reach its own error branch.
                // Actual selection cancellation has separate workspace tests.
                h.panel
                    .update(cx, |panel, _| panel.token = uuid::Uuid::new_v4());
            }
            h.control.gate.add_permits(1);
            h.idle(cx).await;
            h.panel.update(cx, |panel, cx| {
                assert!(panel.session.is_none(), "mode={mode}, stale={stale}");
                assert!(panel.result.is_none());
                panel.preview(cx);
                panel.confirm(cx);
                assert!(panel.review.is_none());
                assert!(panel.cancel.is_none());
            });
            assert_eq!(
                h.control.requests.load(Ordering::Acquire),
                1,
                "mode={mode}, stale={stale}: no second exec after unverified exit"
            );
            h.finish();
        }
    }
}

#[gpui_kit::test]
async fn protocol_terminal_pointer_aba_retires_review_before_any_draw(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    cx.update_window(h.window, |_, window, _| window.remove_window())
        .checked("retire pointer initial window");
    let (window, monitor, workspace) =
        independent_reconnect_pending_reply_fixture(h.session.clone(), h.port, cx);
    let panel = monitor.update(cx, |monitor, _| {
        monitor.paused = true;
        monitor.protocol.clone()
    });
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        h.control.shells_ready.load(Ordering::Acquire) == 1
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        let points = prepare_pointer_panes(&workspace, window, cx);
        panel.update(cx, |panel, cx| {
            panel.kind = NetworkDiagnosticKind::Icmp;
            panel.endpoint.update(cx, |input, cx| {
                input.set_value("pointer-review.test", window, cx)
            });
            panel.preview(cx);
            assert!(panel.review.is_some());
        });
        pointer_press_without_render(window, points[1], cx);
        assert_selection(workspace.read(cx), 1);
        assert!(panel.read(cx).review.is_none());
        pointer_press_without_render(window, points[0], cx);
        assert_selection(workspace.read(cx), 0);
        panel.update(cx, |panel, cx| {
            assert!(panel.review.is_none());
            assert!(panel.session.is_some());
            panel.confirm(cx);
            assert!(panel.cancel.is_none());
            panel.preview(cx);
            assert!(panel.review.is_some(), "fresh review after pointer return");
        });
        assert_eq!(h.control.requests.load(Ordering::Acquire), 0);
    })
    .checked("real terminal pointer ABA without render or focus callbacks");
    h.control.gate.add_permits(1);
    panel.update(cx, |panel, cx| panel.confirm(cx));
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).cancel.is_none()
    })
    .await;
    assert!(panel.read_with(cx, |panel, _| panel.result.is_some()));
    assert_eq!(h.control.requests.load(Ordering::Acquire), 1);
    cx.update_window(window, |_, window, _| window.remove_window())
        .checked("close pointer review workspace");
    h.finish();
}

#[gpui_kit::test]
async fn protocol_terminal_pointer_aba_cancels_held_reply_before_draw(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    cx.update_window(h.window, |_, window, _| window.remove_window())
        .checked("retire pointer held initial window");
    let (window, monitor, workspace) =
        independent_reconnect_pending_reply_fixture(h.session.clone(), h.port, cx);
    let panel = monitor.update(cx, |monitor, _| {
        monitor.paused = true;
        monitor.protocol.clone()
    });
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        h.control.shells_ready.load(Ordering::Acquire) == 1
    })
    .await;
    let points = cx
        .update_window(window, |_, window, cx| {
            let points = prepare_pointer_panes(&workspace, window, cx);
            panel.update(cx, |panel, cx| {
                panel.endpoint.update(cx, |input, cx| {
                    input.set_value("pointer-held.test", window, cx)
                });
                panel.preview(cx);
                assert!(panel.review.is_some());
                panel.confirm(cx);
            });
            points
        })
        .checked("hold a real SSH diagnostic reply");
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        h.control.requests.load(Ordering::Acquire) == 1
    })
    .await;
    let cancel = panel.read_with(cx, |panel, _| {
        panel
            .cancel
            .clone()
            .unwrap_or_else(|| panic!("running pointer diagnostic"))
    });
    cx.update_window(window, |_, window, cx| {
        pointer_press_without_render(window, points[1], cx);
        assert_selection(workspace.read(cx), 1);
        assert!(
            cancel.load(Ordering::Acquire),
            "cancel before the next draw"
        );
        pointer_press_without_render(window, points[0], cx);
        assert_selection(workspace.read(cx), 0);
    })
    .checked("same-frame pointer return keeps the old operation revoked");
    h.control.gate.add_permits(1);
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).cancel.is_none()
    })
    .await;
    panel.update(cx, |panel, cx| {
        assert!(panel.result.is_none());
        assert!(panel.review.is_none());
        panel.confirm(cx);
    });
    assert_eq!(h.control.requests.load(Ordering::Acquire), 1);
    cx.update_window(window, |_, window, _| window.remove_window())
        .checked("close pointer held workspace");
    h.finish();
}

#[gpui_kit::test]
async fn protocol_search_close_retires_peer_review_and_retains_same_owner(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    cx.update_window(h.window, |_, window, _| window.remove_window())
        .checked("retire search initial window");
    let (window, monitor, workspace) =
        independent_reconnect_pending_reply_fixture(h.session.clone(), h.port, cx);
    let panel = monitor.update(cx, |monitor, _| {
        monitor.paused = true;
        monitor.protocol.clone()
    });
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        h.control.shells_ready.load(Ordering::Acquire) == 1
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        let points = prepare_pointer_panes(&workspace, window, cx);
        pointer_press_without_render(window, points[1], cx);
        window.press(search_shortcut(), cx);
        let close = window.find("terminal-search-close").bounds().center();
        pointer_press_without_render(window, points[0], cx);
        panel.update(cx, |panel, cx| {
            panel.endpoint.update(cx, |input, cx| {
                input.set_value("search-close.test", window, cx)
            });
            panel.preview(cx);
            assert!(panel.review.is_some());
        });
        pointer_press_without_render(window, close, cx);
        assert_selection(workspace.read(cx), 1);
        assert!(panel.read(cx).review.is_none());
        pointer_press_without_render(window, points[0], cx);
        assert_selection(workspace.read(cx), 0);
        panel.update(cx, |panel, cx| {
            panel.confirm(cx);
            assert!(panel.cancel.is_none(), "old review cannot execute");
            panel.preview(cx);
            assert!(panel.review.is_some());
        });

        // An owner's own search open/click/close must retain consent, and
        // the selection callback must not pull focus out of the search input.
        window.press(search_shortcut(), cx);
        let search_focus = window.focused(cx).unwrap_or_else(|| panic!("search focus"));
        let input = window.find("terminal-search-input").bounds().center();
        let close = window.find("terminal-search-close").bounds().center();
        pointer_press_without_render(window, input, cx);
        assert!(search_focus.is_focused(window));
        assert!(panel.read(cx).review.is_some());
        pointer_press_without_render(window, close, cx);
        assert_selection(workspace.read(cx), 0);
        assert!(panel.read(cx).review.is_some());
        window.press(search_shortcut(), cx);
        let escape = key_down(window, "escape", cx);
        assert_selection(workspace.read(cx), 0);
        assert!(panel.read(cx).review.is_some());
        key_up(window, escape, cx);
        assert_eq!(h.control.requests.load(Ordering::Acquire), 0);
    })
    .checked("search mouse and same-owner search actions");
    h.control.gate.add_permits(1);
    panel.update(cx, |panel, cx| panel.confirm(cx));
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).cancel.is_none()
    })
    .await;
    assert!(panel.read_with(cx, |panel, _| panel.result.is_some()));
    assert_eq!(h.control.requests.load(Ordering::Acquire), 1);
    cx.update_window(window, |_, window, _| window.remove_window())
        .checked("close search review workspace");
    h.finish();
}

#[gpui_kit::test]
async fn protocol_search_tab_and_escape_select_owner_before_the_next_draw(cx: &mut TestAppContext) {
    for key in ["tab", "shift-tab"] {
        let h = Harness::new(cx);
        cx.update_window(h.window, |_, window, _| window.remove_window())
            .checked("retire tab initial window");
        let (window, monitor, workspace) =
            independent_reconnect_pending_reply_fixture(h.session.clone(), h.port, cx);
        let panel = monitor.update(cx, |monitor, _| {
            monitor.paused = true;
            monitor.protocol.clone()
        });
        cx.wait_for(window, Duration::from_secs(3), |_, _| {
            h.control.shells_ready.load(Ordering::Acquire) == 1
        })
        .await;
        cx.update_window(window, |_, window, cx| {
            let points = prepare_pointer_panes(&workspace, window, cx);
            pointer_press_without_render(window, points[1], cx);
            window.press(search_shortcut(), cx);
            let search_focus = window
                .focused(cx)
                .unwrap_or_else(|| panic!("peer search focus"));
            // Resolve the adjacent real tab stop during preparation. The
            // tested transition below uses the Root's actual Tab key binding.
            if key == "tab" {
                window.focus_prev(cx);
            } else {
                window.focus_next(cx);
            }
            let adjacent = window
                .focused(cx)
                .unwrap_or_else(|| panic!("adjacent tab stop"));
            assert!(!search_focus.is_focused(window));
            pointer_press_without_render(window, points[0], cx);
            panel.update(cx, |panel, cx| {
                panel.endpoint.update(cx, |input, cx| {
                    input.set_value("search-tab.test", window, cx)
                });
                panel.preview(cx);
                assert!(panel.review.is_some());
            });
            adjacent.focus(window, cx);
            let tab = key_down(window, key, cx);
            assert!(
                search_focus.is_focused(window),
                "{key}: real search input entered"
            );
            assert_selection(workspace.read(cx), 1);
            assert!(panel.read(cx).review.is_none(), "{key}: revoke before draw");
            key_up(window, tab, cx);
            let escape = key_down(window, "escape", cx);
            assert!(!search_focus.is_focused(window));
            assert_selection(workspace.read(cx), 1);
            assert!(panel.read(cx).review.is_none());
            key_up(window, escape, cx);
            pointer_press_without_render(window, points[0], cx);
            assert_selection(workspace.read(cx), 0);
            panel.update(cx, |panel, cx| {
                panel.confirm(cx);
                assert!(panel.cancel.is_none());
            });
            assert_eq!(h.control.requests.load(Ordering::Acquire), 0);
        })
        .checked("Tab/Shift-Tab search entry and Escape synchronously select owner");
        cx.update_window(window, |_, window, _| window.remove_window())
            .checked("close tab search workspace");
        h.finish();
    }
}

#[gpui_kit::test]
async fn protocol_search_close_cancels_held_reply_before_the_next_draw(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    cx.update_window(h.window, |_, window, _| window.remove_window())
        .checked("retire held search initial window");
    let (window, monitor, workspace) =
        independent_reconnect_pending_reply_fixture(h.session.clone(), h.port, cx);
    let panel = monitor.update(cx, |monitor, _| {
        monitor.paused = true;
        monitor.protocol.clone()
    });
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        h.control.shells_ready.load(Ordering::Acquire) == 1
    })
    .await;
    let (points, close) = cx
        .update_window(window, |_, window, cx| {
            let points = prepare_pointer_panes(&workspace, window, cx);
            pointer_press_without_render(window, points[1], cx);
            window.press(search_shortcut(), cx);
            let close = window.find("terminal-search-close").bounds().center();
            pointer_press_without_render(window, points[0], cx);
            panel.update(cx, |panel, cx| {
                panel.kind = NetworkDiagnosticKind::Icmp;
                panel.endpoint.update(cx, |input, cx| {
                    input.set_value("search-held.test", window, cx)
                });
                panel.preview(cx);
                assert!(panel.review.is_some());
                panel.confirm(cx);
            });
            (points, close)
        })
        .checked("hold ICMP reply with nonactive peer search open");
    cx.wait_for(window, Duration::from_secs(3), |_, _| {
        h.control.requests.load(Ordering::Acquire) == 1
    })
    .await;
    let cancel = panel.read_with(cx, |panel, _| {
        panel
            .cancel
            .clone()
            .unwrap_or_else(|| panic!("held search cancellation"))
    });
    cx.update_window(window, |_, window, cx| {
        pointer_press_without_render(window, close, cx);
        assert_selection(workspace.read(cx), 1);
        assert!(cancel.load(Ordering::Acquire), "cancel before next draw");
        pointer_press_without_render(window, points[0], cx);
        assert_selection(workspace.read(cx), 0);
    })
    .checked("search close and pointer return retire held operation");
    h.control.gate.add_permits(1);
    cx.wait_for(window, Duration::from_secs(5), |_, cx| {
        panel.read(cx).cancel.is_none()
    })
    .await;
    panel.update(cx, |panel, cx| {
        assert!(panel.result.is_none());
        assert!(panel.review.is_none());
        panel.confirm(cx);
    });
    assert_eq!(h.control.requests.load(Ordering::Acquire), 1);
    cx.update_window(window, |_, window, _| window.remove_window())
        .checked("close held search workspace");
    h.finish();
}

struct SearchButtonFixture {
    harness: Harness,
    window: AnyWindowHandle,
    monitor: Entity<ProtocolPanel>,
    workspace: Entity<crate::workspace::Workspace>,
    points: [gpui_kit::Point<gpui_kit::Pixels>; 2],
}

impl SearchButtonFixture {
    async fn new(cx: &mut TestAppContext) -> Self {
        let harness = Harness::new(cx);
        cx.update_window(harness.window, |_, window, _| window.remove_window())
            .checked("retire search button initial window");
        let (window, monitor, workspace) =
            independent_reconnect_pending_reply_fixture(harness.session.clone(), harness.port, cx);
        let monitor = monitor.update(cx, |monitor, _| {
            monitor.paused = true;
            monitor.protocol.clone()
        });
        cx.wait_for(window, Duration::from_secs(3), |_, _| {
            harness.control.shells_ready.load(Ordering::Acquire) == 1
        })
        .await;
        let points = cx
            .update_window(window, |_, window, cx| {
                prepare_pointer_panes(&workspace, window, cx)
            })
            .checked("mount search button panes");
        Self {
            harness,
            window,
            monitor,
            workspace,
            points,
        }
    }

    fn preview(&self, window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
        self.monitor.update(cx, |panel, cx| {
            panel.kind = NetworkDiagnosticKind::Icmp;
            panel.endpoint.update(cx, |input, cx| {
                input.set_value("search-button.test", window, cx)
            });
            panel.preview(cx);
            assert!(panel.review.is_some());
        });
    }

    fn peer_button(
        &self,
        id: &'static str,
        step: usize,
        window: &mut gpui_kit::Window,
        cx: &mut gpui_kit::App,
    ) -> gpui_kit::FocusHandle {
        pointer_press_without_render(window, self.points[1], cx);
        window.press(search_shortcut(), cx);
        for _ in 0..step {
            window.focus_next(cx);
        }
        window.render_frame(cx);
        assert_eq!(window.find(id).focused(), Some(true), "actual peer button");
        window
            .focused(cx)
            .unwrap_or_else(|| panic!("peer button focus"))
    }

    fn finish(self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, _| window.remove_window())
            .checked("close search button workspace");
        self.harness.finish();
    }
}

#[gpui_kit::test]
async fn protocol_search_buttons_tab_and_space_retire_peer_review(cx: &mut TestAppContext) {
    for (id, step) in [
        ("terminal-search-previous", 1),
        ("terminal-search-next", 2),
        ("terminal-search-close", 3),
    ] {
        for key in ["tab", "shift-tab"] {
            let f = SearchButtonFixture::new(cx).await;
            cx.update_window(f.window, |_, window, cx| {
                let button = f.peer_button(id, step, window, cx);
                if key == "tab" {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
                let adjacent = window
                    .focused(cx)
                    .unwrap_or_else(|| panic!("button neighbor"));
                assert!(!button.is_focused(window));
                pointer_press_without_render(window, f.points[0], cx);
                f.preview(window, cx);
                adjacent.focus(window, cx);
                let tab = key_down(window, key, cx);
                assert!(button.is_focused(window), "{id}/{key}: button entered");
                assert_selection(f.workspace.read(cx), 1);
                assert!(
                    f.monitor.read(cx).review.is_none(),
                    "{id}/{key}: revoke on focus"
                );
                key_up(window, tab, cx);
                let space = key_down(window, "space", cx);
                key_up(window, space, cx);
                assert_selection(f.workspace.read(cx), 1);
                pointer_press_without_render(window, f.points[0], cx);
                f.monitor.update(cx, |panel, cx| panel.confirm(cx));
                assert!(f.monitor.read(cx).cancel.is_none());
                assert_eq!(f.harness.control.requests.load(Ordering::Acquire), 0);

                // The same button operations within A retain A's fresh review.
                f.preview(window, cx);
                window.press(search_shortcut(), cx);
                for _ in 0..step {
                    window.focus_next(cx);
                }
                let space = key_down(window, "space", cx);
                key_up(window, space, cx);
                assert_selection(f.workspace.read(cx), 0);
                assert!(f.monitor.read(cx).review.is_some(), "{id}: same owner");
            })
            .checked("Root Tab into each search button and keyboard activation");
            f.finish(cx);
        }
    }
}

#[gpui_kit::test]
async fn protocol_search_button_space_retires_peer_review_without_tab(cx: &mut TestAppContext) {
    for (id, step) in [("terminal-search-previous", 1), ("terminal-search-next", 2)] {
        let f = SearchButtonFixture::new(cx).await;
        cx.update_window(f.window, |_, window, cx| {
            let button = f.peer_button(id, step, window, cx);
            pointer_press_without_render(window, f.points[0], cx);
            f.preview(window, cx);
            // Prepare focus directly to isolate Keyboard Click from the Tab
            // observer. Space must publish its owner from the actual action.
            button.focus(window, cx);
            assert_selection(f.workspace.read(cx), 0);
            let space = key_down(window, "space", cx);
            key_up(window, space, cx);
            assert_selection(f.workspace.read(cx), 1);
            assert!(f.monitor.read(cx).review.is_none());
            pointer_press_without_render(window, f.points[0], cx);
            f.monitor.update(cx, |panel, cx| panel.confirm(cx));
            assert!(f.monitor.read(cx).cancel.is_none());
            assert_eq!(f.harness.control.requests.load(Ordering::Acquire), 0);
        })
        .checked("search navigation Keyboard Click synchronously selects its owner");
        f.finish(cx);
    }
}

#[gpui_kit::test]
async fn protocol_search_button_tab_cancels_held_reply_before_the_next_draw(
    cx: &mut TestAppContext,
) {
    let f = SearchButtonFixture::new(cx).await;
    let (button, adjacent) = cx
        .update_window(f.window, |_, window, cx| {
            let button = f.peer_button("terminal-search-next", 2, window, cx);
            window.focus_next(cx);
            let adjacent = window
                .focused(cx)
                .unwrap_or_else(|| panic!("next button neighbor"));
            assert!(!button.is_focused(window));
            pointer_press_without_render(window, f.points[0], cx);
            f.preview(window, cx);
            f.monitor.update(cx, |panel, cx| panel.confirm(cx));
            (button, adjacent)
        })
        .checked("hold ICMP reply before entering a peer search button");
    cx.wait_for(f.window, Duration::from_secs(3), |_, _| {
        f.harness.control.requests.load(Ordering::Acquire) == 1
    })
    .await;
    let cancel = f.monitor.read_with(cx, |panel, _| {
        panel
            .cancel
            .clone()
            .unwrap_or_else(|| panic!("held button cancellation"))
    });
    cx.update_window(f.window, |_, window, cx| {
        adjacent.focus(window, cx);
        let tab = key_down(window, "shift-tab", cx);
        assert!(button.is_focused(window));
        assert_selection(f.workspace.read(cx), 1);
        assert!(
            cancel.load(Ordering::Acquire),
            "cancel on button focus before draw"
        );
        key_up(window, tab, cx);
        pointer_press_without_render(window, f.points[0], cx);
        assert_selection(f.workspace.read(cx), 0);
    })
    .checked("Shift-Tab search button entry immediately cancels the old operation");
    f.harness.control.gate.add_permits(1);
    cx.wait_for(f.window, Duration::from_secs(5), |_, cx| {
        f.monitor.read(cx).cancel.is_none()
    })
    .await;
    f.monitor.update(cx, |panel, cx| {
        assert!(panel.result.is_none());
        assert!(panel.review.is_none());
        panel.confirm(cx);
    });
    assert_eq!(f.harness.control.requests.load(Ordering::Acquire), 1);
    f.finish(cx);
}
