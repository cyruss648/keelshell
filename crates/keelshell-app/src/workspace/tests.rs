//! Render the real SSH-only workspace and exercise language/save transitions.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};

use gpui_kit::component::input::InputState;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, ClipboardItem, Entity, Focusable, TestAppContext,
    WindowBounds, WindowOptions, point, px, size,
    test::{TestAppContextExt, TestWindowExt},
};
use keelshell_core::{AppState, Connection, Language, StateStore};
use keelshell_session::SessionEvent;

use super::{KeyboardInteractiveField, KeyboardInteractivePrompt, Workspace};
use crate::i18n;
use crate::terminal::{TerminalCommand, TerminalView};

mod ai_command_review_target;
mod ai_metadata;

trait Checked<T> {
    fn checked(self, operation: &str) -> T;
}

impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    #[track_caller]
    fn checked(self, operation: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{operation}: {error:?}"),
        }
    }
}

struct TemporaryState(PathBuf);

impl Drop for TemporaryState {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    window: AnyWindowHandle,
    workspace: Entity<Workspace>,
    store: Arc<StateStore>,
    _state_directory: TemporaryState,
}

fn mount(cx: &mut TestAppContext, profiles: Vec<Connection>) -> Fixture {
    mount_sized(cx, profiles, 1280., 840.)
}

fn mount_sized(
    cx: &mut TestAppContext,
    profiles: Vec<Connection>,
    width: f32,
    height: f32,
) -> Fixture {
    let directory = std::env::temp_dir().join(format!("keelshell-ui-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).checked("create isolated state directory");
    let guard = TemporaryState(directory.clone());
    let store = Arc::new(StateStore::new(directory.join("state.json")));
    let mut state = store.load().checked("load empty fixture state");
    state.connections = profiles;
    state = store.save(&state).checked("persist fixture profiles");
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .checked("create fixture runtime"),
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        i18n::set_language(state.settings.language, cx);
    });
    let workspace_store = store.clone();
    let (window, workspace) = cx.update(|cx| {
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
                cx.new(|cx| Workspace::new(workspace_store, state, None, runtime, window, cx))
            },
        )
        .checked("mount production workspace")
    });
    cx.update_window(window, |_, window, _| window.activate_window())
        .checked("activate test window for native focus events");
    Fixture {
        window,
        workspace,
        store,
        _state_directory: guard,
    }
}

#[gpui_kit::test]
async fn update_panel_close_has_a_localized_name_and_closes_the_real_panel(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    for (language, label) in [
        (Language::ZhCn, "关闭关于与更新"),
        (Language::En, "Close about and updates"),
    ] {
        cx.update_window(fixture.window, |_, window, cx| {
            i18n::set_language(language, cx);
            window.render_frame(cx);
            window.click("about-updates", cx);
            window.render_frame(cx);
            assert!(fixture.workspace.read(cx).update_panel.is_some());
            assert_eq!(window.find("close-update-panel").label(), Some(label));
            window.click("close-update-panel", cx);
        })
        .checked("close the actual update panel without checking or installing an update");
        cx.wait_for(fixture.window, Duration::from_secs(5), |_, cx| {
            fixture.workspace.read(cx).update_panel.is_none()
        })
        .await;
        cx.update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("close-update-panel").is_none());
        })
        .checked("closed panel no longer exposes its control");
    }
    cx.update(|cx| i18n::set_language(Language::ZhCn, cx));
}

#[gpui_kit::test]
fn first_launch_is_chinese_without_a_local_or_remote_terminal(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let workspace = fixture.workspace.read(cx);
        assert_eq!(workspace.state.settings.language, Language::ZhCn);
        assert_eq!(i18n::language(cx), Language::ZhCn);
        assert!(workspace.tabs.is_empty());
        assert!(workspace.remote_sessions.is_empty());
        assert!(workspace.panels.is_empty());
        assert_eq!(window.find("new-connection").label(), Some("新建连接"));
        assert_eq!(window.find("language").label(), Some("中文 / EN"));
        assert_eq!(window.find("new-session").label(), Some("新建 SSH 会话"));
        assert_eq!(window.find("quick-host").label(), Some("主机或 IP 地址"));
        assert_eq!(window.find("quick-port").label(), Some("端口"));
        assert_eq!(window.find("quick-username").label(), Some("SSH 用户名"));
        assert_eq!(window.find("quick-key").label(), Some("私钥路径（可选）"));
        assert!(window.try_find(("session-tab", 0_usize)).is_none());
        assert!(window.try_find("run-command").is_none());
        i18n::set_language(Language::En, cx);
        window.render_frame(cx);
        assert_eq!(window.find("new-session").label(), Some("New SSH session"));
        assert_eq!(
            window.find("quick-host").label(),
            Some("Host or IP address")
        );
        assert_eq!(window.find("quick-port").label(), Some("Port"));
        assert_eq!(window.find("quick-username").label(), Some("SSH username"));
        assert_eq!(
            window.find("quick-key").label(),
            Some("Private key path (optional)")
        );
        i18n::set_language(Language::ZhCn, cx);
    })
    .checked("draw empty Chinese workspace");
}

#[gpui_kit::test]
fn empty_split_and_new_session_never_create_a_fallback_shell(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("split-session", cx);
        // Also exercise the command defensively, bypassing disabled UI state.
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.split_remote(window, cx);
            workspace.split_remote(window, cx);
        });
        window.render_frame(cx);
        window.click("new-session", cx);
        let workspace = fixture.workspace.read(cx);
        assert!(workspace.show_connections);
        assert!(workspace.tabs.is_empty());
        assert!(workspace.remote_sessions.is_empty());
        assert!(workspace.remote_hosts.is_empty());
        assert!(workspace.panels.is_empty());
        assert!(!workspace.split);
    })
    .checked("empty split and connection-manager actions");
    cx.run_until_parked();
    fixture
        .workspace
        .read_with(cx, |workspace, _| assert!(workspace.tabs.is_empty()));
}

#[gpui_kit::test]
fn quick_connect_uses_the_ephemeral_route_and_does_not_save_a_profile(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.quick_connect.host.update(cx, |input, cx| {
                input.set_value("quick.example.test", window, cx)
            });
            workspace
                .quick_connect
                .port
                .update(cx, |input, cx| input.set_value("2208", window, cx));
            workspace
                .quick_connect
                .username
                .update(cx, |input, cx| input.set_value("operator", window, cx));
            workspace.quick_password = true;
        });
        window.render_frame(cx);
        window.click("quick-connect", cx);
        window.render_frame(cx);
        assert!(
            window.try_find("save-credential-mode").is_none(),
            "one-time authentication must not expose vault save"
        );
        assert!(window.try_find("keyboard-interactive-mode").is_some());
        window.click("keyboard-interactive-mode", cx);
        assert!(
            fixture
                .workspace
                .read(cx)
                .connect_route
                .as_ref()
                .is_some_and(|route| route.keyboard_interactive)
        );
    })
    .checked("start one-time SSH connection from the empty workspace");
    fixture.workspace.read_with(cx, |workspace, _| {
        assert!(workspace.connect_route.is_some());
        let login = workspace
            .login
            .as_ref()
            .checked_option("password mode reuses SSH login UI");
        assert!(workspace.login_is_current(&login.connection));
        assert!(matches!(login.mode, super::vault::LoginMode::Once));
        assert!(workspace.state.connections.is_empty());
        assert!(!workspace.saving);
    });
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            let login = workspace
                .login
                .as_ref()
                .checked_option("quick login remains open");
            login.secret.update(cx, |input, cx| {
                input.set_value("one-time-secret", window, cx)
            });
            workspace.submit_login(window, cx);
            assert!(workspace.connect_route.is_some());
            assert!(workspace.connecting);
        });
    })
    .checked("submit one-time password without rejecting the ephemeral route");
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.cancel_connect_route(window, cx);
        });
    })
    .checked("cancel one-time SSH connection");
    assert!(
        fixture
            .store
            .load()
            .checked("reload quick state")
            .connections
            .is_empty()
    );
}

#[gpui_kit::test]
fn quick_connect_remains_visible_with_saved_profiles(cx: &mut TestAppContext) {
    let profile = Connection::new("Saved SSH", "saved.example.test", "operator");
    let fixture = mount(cx, vec![profile]);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("quick-connect-surface").is_some());
        assert_eq!(window.find("new-connection").label(), Some("新建连接"));
    })
    .checked("keep one-time entry point beside the saved library");
}

#[gpui_kit::test]
fn quick_connect_save_action_only_opens_the_persistent_editor(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.quick_connect.host.update(cx, |input, cx| {
                input.set_value("save.example.test", window, cx)
            });
            workspace
                .quick_connect
                .username
                .update(cx, |input, cx| input.set_value("operator", window, cx));
        });
        window.render_frame(cx);
        window.click("save-quick-profile", cx);
    })
    .checked("open the persistent connection editor explicitly");
    fixture.workspace.read_with(cx, |workspace, cx| {
        let form = match workspace.form.as_ref() {
            Some(form) => form,
            None => panic!("save action opens editor"),
        };
        assert_eq!(form.host.read(cx).value(), "save.example.test");
        assert_eq!(form.username.read(cx).value(), "operator");
        assert!(workspace.state.connections.is_empty());
    });
    assert!(
        fixture
            .store
            .load()
            .checked("reload unsaved quick state")
            .connections
            .is_empty()
    );
}

#[gpui_kit::test]
async fn language_switch_preserves_profiles_and_unsaved_drafts_after_persistence(
    cx: &mut TestAppContext,
) {
    let mut profile = Connection::new("生产 SSH", "example.invalid", "operator");
    profile.group = "研发/应用".into();
    let profiles = vec![profile];
    let fixture = mount(cx, profiles.clone());
    let signature = cx
        .update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("new-connection", cx);
            let name = fixture
                .workspace
                .read(cx)
                .form
                .as_ref()
                .map(|form| form.name.entity_id());
            let Some(name) = name else {
                panic!("New connection must open a draft");
            };
            window.click(("input", name), cx);
            window.input("未保存的连接", cx);
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.command.update(cx, |input, cx| {
                    input.set_value("printf '%s' '$UNSENT'", window, cx)
                });
                workspace
                    .search
                    .update(cx, |input, cx| input.set_value("生产", window, cx));
                workspace.switch_language(window, cx);
                assert!(workspace.saving);
                // A draft edit arriving after save starts must survive its completion.
                let Some(form) = &workspace.form else {
                    panic!("language change removed draft");
                };
                form.name.update(cx, |input, cx| {
                    input.set_value("保存期间继续编辑", window, cx)
                });
                form.host
                    .update(cx, |input, cx| input.set_value("draft.invalid", window, cx));
                form.signature(cx)
            })
        })
        .checked("edit draft and switch language through the production callback");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(workspace.state.settings.language, Language::En);
        assert_eq!(i18n::language(cx), Language::En);
        assert_eq!(workspace.state.connections, profiles);
        assert_eq!(
            workspace.form.as_ref().map(|form| form.signature(cx)),
            Some(signature.clone())
        );
        assert_eq!(workspace.command.read(cx).value(), "printf '%s' '$UNSENT'");
        assert_eq!(workspace.search.read(cx).value(), "生产");
        assert!(workspace.tabs.is_empty());
    });
    let persisted = fixture
        .store
        .load()
        .checked("verify language persisted on disk");
    assert_eq!(persisted.settings.language, Language::En);
    assert_eq!(persisted.connections, profiles);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture
            .workspace
            .update(cx, |workspace, cx| workspace.switch_language(window, cx));
    })
    .checked("switch the same live workspace back to Chinese");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(workspace.state.settings.language, Language::ZhCn);
        assert_eq!(workspace.state.connections, profiles);
        assert_eq!(
            workspace.form.as_ref().map(|form| form.signature(cx)),
            Some(signature)
        );
        assert_eq!(workspace.command.read(cx).value(), "printf '%s' '$UNSENT'");
    });
    assert_eq!(
        fixture
            .store
            .load()
            .checked("verify Chinese persisted")
            .settings
            .language,
        Language::ZhCn
    );
}

#[gpui_kit::test]
fn split_pair_is_stable_across_unrelated_tabs_and_closure(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    cx.update_window(fixture.window, |_, window, cx| {
        let tabs: Vec<_> = ["primary", "unrelated host", "peer"]
            .into_iter()
            .map(|title| {
                let (_, output) = std::sync::mpsc::sync_channel(2);
                let (input, _) = std::sync::mpsc::sync_channel(2);
                cx.new(|cx| {
                    crate::terminal::TerminalView::from_transport(
                        title.into(),
                        14.,
                        1000,
                        output,
                        input,
                        Arc::new(std::sync::atomic::AtomicBool::new(false)),
                        cx,
                    )
                })
            })
            .collect();
        let primary = tabs[0].entity_id();
        let peer = tabs[2].entity_id();
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.tabs = tabs;
            workspace.split = true;
            workspace.split_pair = Some((primary, peer));
            workspace.active = 0;
            assert_eq!(
                workspace.split_companion().map(|tab| tab.entity_id()),
                Some(peer)
            );
            workspace.active = 1;
            assert!(workspace.split_companion().is_none());
            // Removing another host does not change the selected pair's identity.
            workspace.close_tab(&super::CloseTab, window, cx);
            workspace.active = 0;
            assert_eq!(
                workspace.split_companion().map(|tab| tab.entity_id()),
                Some(peer)
            );
            workspace.active = 1;
            assert_eq!(
                workspace.split_companion().map(|tab| tab.entity_id()),
                Some(primary)
            );
            workspace.close_tab(&super::CloseTab, window, cx);
            assert!(workspace.split_companion().is_none());
            assert!(workspace.split_pair.is_none());
            assert!(!workspace.split);
        });
    })
    .checked("keep split bound to its actual remote pane entities");
}

struct RemotePane {
    terminal: Entity<TerminalView>,
    commands: Receiver<TerminalCommand>,
    // Keep the fixture transport connected while layout and focus are exercised.
    _output: SyncSender<SessionEvent>,
}

fn attach_remote_panes(fixture: &Fixture, cx: &mut TestAppContext) -> Vec<RemotePane> {
    cx.update_window(fixture.window, |_, window, cx| {
        let panes: Vec<_> = ["left-pane-output", "right-pane-output"]
            .into_iter()
            .map(|text| {
                let (output, incoming) = mpsc::sync_channel(64);
                let (outgoing, commands) = mpsc::sync_channel(64);
                let terminal = cx.new(|cx| {
                    let mut terminal = TerminalView::from_transport(
                        text.into(),
                        14.,
                        1000,
                        incoming,
                        outgoing,
                        Arc::new(AtomicBool::new(false)),
                        cx,
                    );
                    terminal.emulator.feed(text.as_bytes());
                    terminal
                });
                RemotePane {
                    terminal,
                    commands,
                    _output: output,
                }
            })
            .collect();
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.tabs = panes.iter().map(|pane| pane.terminal.clone()).collect();
            workspace.active = 0;
            for (index, pane) in panes.iter().enumerate() {
                workspace.remote_hosts.insert(
                    pane.terminal.entity_id(),
                    format!("fixture-{index}@example.invalid:22"),
                );
                // Exercise production observers and focus callbacks, rather than
                // updating Workspace.active from inside the test click handler.
                workspace.watch_terminal(&pane.terminal, window, cx);
            }
            cx.notify();
        });
        let focus = panes[0].terminal.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        window.render_frame(cx);
        panes
    })
    .checked("attach deterministic remote transports and production focus listeners")
}

fn writes(pane: &RemotePane) -> Vec<u8> {
    pane.commands
        .try_iter()
        .filter_map(|command| match command {
            TerminalCommand::Write(bytes) => Some(bytes),
            TerminalCommand::Resize(_, _) => None,
        })
        .flatten()
        .collect()
}

#[gpui_kit::test]
fn split_peer_pointer_focus_routes_capture_and_commands_without_reordering_panes(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    let left = panes[0].terminal.entity_id();
    let right = panes[1].terminal.entity_id();
    let original_bounds = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.split = true;
                workspace.split_pair = Some((left, right));
                workspace.show_assistant = true;
                cx.notify();
            });
            window.render_frame(cx);
            let bounds = [
                window.find(("terminal-pane", left)).bounds(),
                window.find(("terminal-pane", right)).bounds(),
            ];
            assert!(bounds[0].origin.x < bounds[1].origin.x);
            // A real pointer event reaches TerminalView's mouse/focus handler.
            window.click(("terminal-pane", right), cx);
            assert!(
                panes[1]
                    .terminal
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window),
                "right terminal did not receive pointer focus; bounds={bounds:?}"
            );
            bounds
        })
        .checked("click the actual right-hand terminal pane");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let workspace = fixture.workspace.read(cx);
        assert_eq!(workspace.tabs[workspace.active].entity_id(), right);
        assert_eq!(
            workspace
                .displayed_terminals()
                .iter()
                .map(Entity::entity_id)
                .collect::<Vec<_>>(),
            vec![left, right]
        );
        assert_eq!(
            window.find(("terminal-pane", left)).bounds(),
            original_bounds[0]
        );
        assert_eq!(
            window.find(("terminal-pane", right)).bounds(),
            original_bounds[1]
        );
        window.click("context-screen", cx);
    })
    .checked("verify stable left/right geometry and explicitly capture focused context");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, cx| {
        let (context, host, session_id) = workspace.assistant.read(cx).captured_context_for_test();
        assert!(context.contains("right-pane-output"));
        assert!(!context.contains("left-pane-output"));
        assert_eq!(host, "fixture-1@example.invalid:22");
        assert_eq!(session_id, format!("{right:?}"));
    });
    cx.update_window(fixture.window, |_, window, cx| {
        let input = fixture.workspace.read(cx).command.entity_id();
        window.click(("input", input), cx);
        window.input("printf focused-right", cx);
    })
    .checked("type a command after focusing the right pane");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, _| {
        assert_eq!(workspace.command_target, Some(right));
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.click("run-command", cx);
    })
    .checked("queue the reviewed command using its production button");
    cx.run_until_parked();
    assert!(writes(&panes[0]).is_empty());
    assert_eq!(writes(&panes[1]), b"printf focused-right\r");
    fixture.workspace.read_with(cx, |workspace, _| {
        assert_eq!(workspace.tabs[workspace.active].entity_id(), right);
        assert_eq!(
            workspace
                .displayed_terminals()
                .iter()
                .map(Entity::entity_id)
                .collect::<Vec<_>>(),
            vec![left, right]
        );
        assert_eq!(workspace.command_target, None);
        let Some(history) = workspace.command_histories.get(&right) else {
            panic!("the active SSH tab owns command history");
        };
        assert_eq!(
            history.newest_first().collect::<Vec<_>>(),
            ["printf focused-right"]
        );
    });
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.visible_panel = Some(super::ToolPanel::Commands);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("clear-command-history").is_some());
        window.click(("use-history", 0_usize), cx);
    })
    .checked("show and reinsert the active SSH session history");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert_eq!(workspace.command.read(cx).value(), "printf focused-right");
        assert_eq!(workspace.command_target, Some(right));
    });
}

#[gpui_kit::test]
fn existing_command_draft_cannot_drift_to_another_tab_and_new_draft_allows_rebinding(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let panes = attach_remote_panes(&fixture, cx);
    let first = panes[0].terminal.entity_id();
    let second = panes[1].terminal.entity_id();
    cx.update_window(fixture.window, |_, window, cx| {
        let input = fixture.workspace.read(cx).command.entity_id();
        window.click(("input", input), cx);
        window.input("printf reviewed-first", cx);
    })
    .checked("type an original command draft on the first tab");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, _| {
        assert_eq!(workspace.command_target, Some(first))
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.click(("session-tab", 1_usize), cx);
    })
    .checked("switch tabs through its production pointer callback");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let workspace = fixture.workspace.read(cx);
        assert_eq!(workspace.tabs[workspace.active].entity_id(), second);
        assert_eq!(workspace.command_target, Some(first));
        assert_eq!(workspace.command.read(cx).value(), "printf reviewed-first");
        window.click("run-command", cx);
        // The method must enforce the same binding even if a future keyboard
        // shortcut bypasses the disabled button.
        fixture
            .workspace
            .update(cx, |workspace, cx| workspace.run_command(window, cx));
    })
    .checked("refuse the old command on the newly selected tab");
    assert!(writes(&panes[0]).is_empty());
    assert!(writes(&panes[1]).is_empty());
    cx.update_window(fixture.window, |_, window, cx| {
        let input = fixture.workspace.read(cx).command.entity_id();
        window.click(("input", input), cx);
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
            cx,
        );
        window.press("backspace", cx);
    })
    .checked("explicitly clear the old draft");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert!(workspace.command.read(cx).value().is_empty());
        assert_eq!(workspace.command_target, Some(first));
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("new-command-draft", cx);
        window.input("printf reviewed-second", cx);
    })
    .checked("begin a fresh draft on the current tab");
    cx.run_until_parked();
    fixture.workspace.read_with(cx, |workspace, _| {
        assert_eq!(workspace.command_target, Some(second))
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("run-command", cx);
    })
    .checked("send only the new explicitly rebound draft");
    cx.run_until_parked();
    assert!(writes(&panes[0]).is_empty());
    assert_eq!(writes(&panes[1]), b"printf reviewed-second\r");
}

#[gpui_kit::test]
async fn connection_library_clipboard_import_favorite_export_and_delete_persist(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let mut source = AppState::default();
    source.connections.push(Connection::new(
        "Imported host",
        "import.example.test",
        "operator",
    ));
    let document = source
        .export_connections()
        .checked("create validated connection export");

    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        cx.write_to_clipboard(ClipboardItem::new_string(document.clone()));
        window.click("import-connections", cx);
    })
    .checked("import connection JSON through the production manager");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert_eq!(workspace.state.connections.len(), 1);
        assert_eq!(workspace.state.connections[0].name, "Imported host");
    });

    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("favorite", 0_usize), cx);
    })
    .checked("toggle the imported connection favorite marker");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert!(workspace.state.connections[0].favorite);
    });

    let exported = cx
        .update_window(fixture.window, |_, window, cx| {
            window.render_frame(cx);
            window.click("export-connections", cx);
            cx.read_from_clipboard().and_then(|item| item.text())
        })
        .checked("read exported JSON from the system clipboard");
    let Some(exported) = exported else {
        panic!("export action did not place text on the clipboard");
    };
    assert!(exported.contains("import.example.test"));
    assert!(!exported.contains("known_hosts"));

    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(("delete", 0_usize), cx);
    })
    .checked("delete the imported connection through the production manager");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert!(workspace.state.connections.is_empty());
    });
    assert!(
        fixture
            .store
            .load()
            .checked("reload deleted connection state")
            .connections
            .is_empty()
    );
}

#[gpui_kit::test]
async fn connection_library_imports_reviewable_openssh_config_from_clipboard(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, Vec::new());
    let document = "Host app\n HostName app.example.test\n User deploy\n Port 2222\nHost web-*\n HostName ignored.example.test\n User deploy\n";
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        cx.write_to_clipboard(ClipboardItem::new_string(document.into()));
        window.click("import-openssh", cx);
    })
    .checked("import OpenSSH text through the production manager");
    fixture.workspace.read_with(cx, |workspace, cx| {
        assert!(workspace.state.connections.is_empty());
        assert!(workspace.openssh_review.is_some());
        assert!(workspace.status.render(cx).contains("审阅"));
    });
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("confirm-openssh-import", cx);
    })
    .checked("confirm the reviewed OpenSSH import");
    cx.wait_for(fixture.window, Duration::from_secs(3), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert_eq!(workspace.state.connections.len(), 1);
        let connection = &workspace.state.connections[0];
        assert_eq!(connection.name, "app");
        assert_eq!(connection.host, "app.example.test");
        assert_eq!(connection.port, 2222);
        assert!(workspace.openssh_review.is_none());
    });
}

#[gpui_kit::test]
async fn cancelling_reviewable_openssh_import_keeps_library_unchanged(cx: &mut TestAppContext) {
    let fixture = mount(cx, Vec::new());
    let document = "Host app\n HostName app.example.test\n User deploy\n";
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        cx.write_to_clipboard(ClipboardItem::new_string(document.into()));
        window.click("import-openssh", cx);
    })
    .checked("open the OpenSSH import review");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("cancel-openssh-import", cx);
    })
    .checked("cancel the OpenSSH import review");
    fixture.workspace.read_with(cx, |workspace, _| {
        assert!(workspace.state.connections.is_empty());
        assert!(workspace.openssh_review.is_none());
    });
}

fn password_profile() -> Connection {
    let mut profile = Connection::new("测试凭据", "127.0.0.1", "fixture-user");
    profile.auth = keelshell_core::AuthMethod::Password;
    profile
}

fn begin_vault_save(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            let profile = workspace.state.connections[0].clone();
            workspace.prepare_login(profile, None, window, cx);
        });
        window.render_frame(cx);
        window.click("save-credential-mode", cx);
        fixture.workspace.update(cx, |workspace, cx| {
            let login = workspace.login.as_ref().checked_option("open login");
            for (input, value) in [
                (&login.secret, "ssh-fixture-secret-中文"),
                (&login.master, "vault-fixture-master"),
                (&login.confirmation, "vault-fixture-master"),
            ] {
                input.update(cx, |input, cx| input.set_value(value, window, cx));
            }
        });
        window.render_frame(cx);
        window.click("submit-login", cx);
    })
    .checked("submit explicit credential save through production buttons");
}

trait CheckedOption<T> {
    fn checked_option(self, message: &str) -> T;
}
impl<T> CheckedOption<T> for Option<T> {
    fn checked_option(self, message: &str) -> T {
        self.unwrap_or_else(|| panic!("{message}"))
    }
}

async fn wait_for_vault(fixture: &Fixture, cx: &mut TestAppContext) {
    cx.wait_for(fixture.window, Duration::from_secs(20), |_, cx| {
        !fixture.workspace.read(cx).saving
    })
    .await;
}

#[gpui_kit::test]
async fn vault_login_saves_locks_unlinks_and_never_connects_on_save(cx: &mut TestAppContext) {
    let fixture = mount(cx, vec![password_profile()]);
    begin_vault_save(&fixture, cx);
    wait_for_vault(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        let workspace = fixture.workspace.read(cx);
        let login = workspace
            .login
            .as_ref()
            .checked_option("save keeps authentication modal open");
        assert!(matches!(login.mode, super::vault::LoginMode::Unlock));
        assert!(login.connection.credential_ref.is_some());
        assert!(!login.busy);
        assert!(!workspace.connecting);
        assert!(workspace.tabs.is_empty());
        for input in [&login.secret, &login.master, &login.confirmation] {
            assert!(input.read(cx).value().is_empty());
        }
        assert!(login.master.read(cx).focus_handle(cx).is_focused(window));
        assert_eq!(window.find("submit-login").label(), Some("解锁并连接"));
        fixture
            .workspace
            .update(cx, |workspace, cx| workspace.switch_language(window, cx));
    })
    .checked("saving leaves vault locked and updates prompt");
    wait_for_vault(&fixture, cx).await;
    let stored = fixture
        .store
        .load()
        .checked("read saved credential reference");
    assert!(stored.connections[0].credential_ref.is_some());
    let vault_path = fixture.store.path().with_file_name("vault.json");
    assert!(vault_path.is_file());
    for path in [fixture.store.path(), vault_path.as_path()] {
        let bytes = std::fs::read_to_string(path).checked("inspect isolated persisted document");
        assert!(!bytes.contains("ssh-fixture-secret"));
        assert!(!bytes.contains("vault-fixture-master"));
    }
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("submit-login").label(),
            Some("Unlock and connect")
        );
        window.click("forget-credential", cx);
    })
    .checked("unlink saved credential in English");
    wait_for_vault(&fixture, cx).await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert!(workspace.state.connections[0].credential_ref.is_none());
        assert!(matches!(
            workspace
                .login
                .as_ref()
                .checked_option("modal remains open")
                .mode,
            super::vault::LoginMode::Once
        ));
        assert!(!workspace.connecting);
    });
    assert!(
        fixture
            .store
            .load()
            .checked("read unlinked profile")
            .connections[0]
            .credential_ref
            .is_none()
    );
    assert!(
        vault_path.exists(),
        "unlinking explicitly retains encrypted entries"
    );
}

#[gpui_kit::test]
async fn vault_wrong_master_and_cancelled_unlock_never_start_ssh(cx: &mut TestAppContext) {
    let fixture = mount(cx, vec![password_profile()]);
    begin_vault_save(&fixture, cx);
    wait_for_vault(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            let login = workspace.login.as_ref().checked_option("saved login");
            login
                .master
                .update(cx, |input, cx| input.set_value("wrong-master", window, cx));
            workspace.submit_login(window, cx);
        });
    })
    .checked("submit incorrect master password");
    wait_for_vault(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        let workspace = fixture.workspace.read(cx);
        let login = workspace
            .login
            .as_ref()
            .checked_option("failed unlock remains retryable");
        assert!(!workspace.connecting);
        assert!(!login.busy);
        assert!(
            login
                .message
                .as_ref()
                .checked_option("unlock error")
                .render(cx)
                .contains("无法解锁")
        );
        assert!(login.master.read(cx).value().is_empty());
        fixture.workspace.update(cx, |workspace, cx| {
            let login = workspace.login.as_ref().checked_option("retry login");
            let master = login.master.clone();
            master.update(cx, |input, cx| {
                input.set_value("vault-fixture-master", window, cx)
            });
            workspace.submit_login(window, cx);
            workspace.cancel_login(window, cx);
            assert!(master.read(cx).value().is_empty());
            assert!(workspace.login.is_none());
        });
    })
    .checked("cancel unlock before foreground delivery");
    wait_for_vault(&fixture, cx).await;
    fixture.workspace.read_with(cx, |workspace, _| {
        assert!(workspace.login.is_none());
        assert!(!workspace.connecting);
        assert!(workspace.tabs.is_empty());
        assert!(workspace.state.connections[0].credential_ref.is_some());
    });
}

#[gpui_kit::test]
async fn vault_state_conflict_keeps_orphan_encrypted_entry_without_link_or_connection(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, vec![password_profile()]);
    let other = StateStore::new(fixture.store.path());
    let mut external = other.load().checked("external snapshot");
    external.settings.font_size = 19.;
    other
        .save(&external)
        .checked("create state conflict before vault save");
    begin_vault_save(&fixture, cx);
    wait_for_vault(&fixture, cx).await;
    fixture.workspace.read_with(cx, |workspace, cx| {
        let login = workspace
            .login
            .as_ref()
            .checked_option("failed save preserves modal");
        assert!(!login.busy);
        let message = login
            .message
            .as_ref()
            .checked_option("save error")
            .render(cx);
        assert!(message.contains("连接配置保存失败"));
        assert!(message.contains("保存其他工作，再重启应用"));
        assert!(message.contains("密码输入已清空"));
        assert!(!message.contains("重新打开配置"));
        assert_eq!(workspace.status.render(cx), message);
        for input in [&login.secret, &login.master, &login.confirmation] {
            assert!(input.read(cx).value().is_empty());
        }
        assert!(workspace.state.connections[0].credential_ref.is_none());
        assert!(!workspace.connecting);
    });
    cx.update(|cx| i18n::set_language(Language::En, cx));
    fixture.workspace.read_with(cx, |workspace, cx| {
        let message = workspace.status.render(cx);
        assert!(message.contains("Save your other work, then restart the app"));
        assert!(message.contains("password inputs were cleared"));
        assert!(!message.contains("input preserved"));
        assert!(!message.contains("Reopen the profile"));
    });
    assert!(fixture.store.path().with_file_name("vault.json").exists());
    let saved = other
        .load()
        .checked("state failure preserves external edit");
    assert_eq!(saved.settings.font_size, 19.);
    assert!(saved.connections[0].credential_ref.is_none());
}

#[gpui_kit::test]
async fn editing_connection_name_preserves_vault_reference_but_target_change_removes_it(
    cx: &mut TestAppContext,
) {
    let mut profile = password_profile();
    profile.credential_ref = Some(uuid::Uuid::new_v4());
    let reference = profile.credential_ref;
    let fixture = mount(cx, vec![profile.clone()]);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.edit_connection(profile, window, cx);
            let form = workspace.form.as_ref().checked_option("profile editor");
            form.name
                .update(cx, |input, cx| input.set_value("已重命名", window, cx));
            workspace.save_connection(window, cx);
        });
    })
    .checked("rename profile");
    wait_for_vault(&fixture, cx).await;
    assert_eq!(
        fixture.store.load().checked("renamed profile").connections[0].credential_ref,
        reference
    );
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            let profile = workspace.state.connections[0].clone();
            workspace.edit_connection(profile, window, cx);
            let form = workspace.form.as_ref().checked_option("profile editor");
            form.host
                .update(cx, |input, cx| input.set_value("other.invalid", window, cx));
            workspace.save_connection(window, cx);
        });
    })
    .checked("edit profile destination");
    wait_for_vault(&fixture, cx).await;
    assert!(
        fixture
            .store
            .load()
            .checked("changed destination")
            .connections[0]
            .credential_ref
            .is_none()
    );
}

#[gpui_kit::test]
fn vault_authentication_focus_and_cancellation_keep_secrets_out_of_remote_terminal(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx, vec![password_profile()]);
    let panes = attach_remote_panes(&fixture, cx);
    let _ = writes(&panes[0]);
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            workspace.prepare_login(workspace.state.connections[0].clone(), None, window, cx);
        });
        window.render_frame(cx);
        let (secret, master) = {
            let workspace = fixture.workspace.read(cx);
            let login = workspace
                .login
                .as_ref()
                .checked_option("authentication prompt");
            assert!(login.secret.read(cx).focus_handle(cx).is_focused(window));
            (login.secret.clone(), login.master.clone())
        };
        window.click("ssh-password", cx);
        window.input("ssh-typed-secret", cx);
        window.click("save-credential-mode", cx);
        window.render_frame(cx);
        assert!(secret.read(cx).value().is_empty());
        window.click(("input", master.entity_id()), cx);
        window.input("typed-master-secret", cx);
        window.click("cancel-login", cx);
        assert!(master.read(cx).value().is_empty());
        assert!(fixture.workspace.read(cx).login.is_none());
    })
    .checked("type masked credentials and cancel via real UI handlers");
    cx.run_until_parked();
    assert!(writes(&panes[0]).is_empty());
    assert!(writes(&panes[1]).is_empty());
    assert!(!fixture.store.path().with_file_name("vault.json").exists());
}

struct VaultSshFixture {
    authenticated: Arc<AtomicBool>,
}

impl russh::server::Handler for VaultSshFixture {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<russh::server::Auth, Self::Error> {
        let valid = user == "fixture-user" && password == "ssh-fixture-secret-中文";
        self.authenticated
            .store(valid, std::sync::atomic::Ordering::Release);
        Ok(if valid {
            russh::server::Auth::Accept
        } else {
            russh::server::Auth::reject()
        })
    }
}

#[gpui_kit::test]
async fn vault_successful_unlock_authenticates_over_real_loopback_ssh(cx: &mut TestAppContext) {
    use russh::keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair};
    let fixture = mount(cx, vec![password_profile()]);
    let runtime = fixture
        .workspace
        .read_with(cx, |workspace, _| workspace.runtime.clone());
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .checked("bind loopback authentication fixture");
    let port = listener.local_addr().checked("fixture port").port();
    // Deterministic public test-only seed; never used by a real SSH endpoint.
    let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x71; 32]));
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::from_millis(1),
        auth_rejection_time_initial: Some(Duration::from_millis(1)),
        ..Default::default()
    });
    let authenticated = Arc::new(AtomicBool::new(false));
    let seen = authenticated.clone();
    let server = runtime.spawn(async move {
        if let Ok(Ok((socket, _))) =
            tokio::time::timeout(Duration::from_secs(20), listener.accept()).await
            && let Ok(session) = russh::server::run_stream(
                config,
                socket,
                VaultSshFixture {
                    authenticated: seen,
                },
            )
            .await
        {
            let _ = tokio::time::timeout(Duration::from_secs(20), session).await;
        }
    });
    cx.update_window(fixture.window, |_, _, cx| {
        fixture.workspace.update(cx, |workspace, _| {
            workspace.state.connections[0].port = port;
        });
    })
    .checked("assign isolated loopback destination");
    begin_vault_save(&fixture, cx);
    wait_for_vault(&fixture, cx).await;
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            let login = workspace
                .login
                .as_mut()
                .checked_option("saved credential prompt");
            // The fixture's generated host identity is explicitly trusted for
            // this authentication test; separate transport tests verify TOFU.
            login.pin = Some(fingerprint);
            login.master.update(cx, |input, cx| {
                input.set_value("vault-fixture-master", window, cx)
            });
        });
        window.render_frame(cx);
        window.click("submit-login", cx);
    })
    .checked("unlock stored credential through production button");
    cx.wait_for(fixture.window, Duration::from_secs(20), |_, cx| {
        !fixture.workspace.read(cx).tabs.is_empty()
    })
    .await;
    assert!(
        authenticated.load(std::sync::atomic::Ordering::Acquire),
        "SSH server must receive the decrypted fixture password, never the master password"
    );
    cx.update_window(fixture.window, |_, window, cx| {
        fixture.workspace.update(cx, |workspace, cx| {
            assert!(workspace.login.is_none());
            assert!(!workspace.connecting);
            assert_eq!(workspace.remote_sessions.len(), 1);
            workspace.close_tab(&super::CloseTab, window, cx);
        });
    })
    .checked("close authenticated fixture session");
    server.abort();
}

#[gpui_kit::test]
fn keyboard_interactive_prompt_is_ephemeral_and_cancelable(cx: &mut TestAppContext) {
    let mut profile = Connection::new("MFA target", "mfa.example.test", "operator");
    profile.auth = keelshell_core::AuthMethod::Password;
    let fixture = mount(cx, vec![profile.clone()]);
    let mut receiver = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                workspace.request_connect(profile.clone(), window, cx);
                let route = workspace
                    .connect_route
                    .as_mut()
                    .checked_option("keyboard-interactive route");
                route.keyboard_interactive = true;
                let route_id = route.id;
                let (sender, receiver) = tokio::sync::oneshot::channel();
                let answer = cx.new(|cx| {
                    InputState::new(window, cx)
                        .masked(true)
                        .placeholder("Enter this response")
                });
                answer.update(cx, |input, cx| input.set_value("123456", window, cx));
                workspace.keyboard_interactive = Some(KeyboardInteractivePrompt {
                    identity: uuid::Uuid::new_v4(),
                    route_id,
                    index: 0,
                    name: "Duo MFA".into(),
                    instructions: "Approve the sign-in, then enter the code.".into(),
                    fields: vec![KeyboardInteractiveField {
                        prompt: "One-time code".into(),
                        answer,
                    }],
                    response: Some(sender),
                });
                receiver
            })
        })
        .checked("prepare keyboard-interactive MFA prompt");
    cx.update_window(fixture.window, |_, window, cx| {
        for language in [Language::ZhCn, Language::En] {
            i18n::set_language(language, cx);
            window.render_frame(cx);
            assert!(window.try_find("keyboard-interactive-content").is_some());
            assert!(window.try_find("submit-keyboard-interactive").is_some());
            assert_eq!(
                window.find("submit-keyboard-interactive").label(),
                Some(if language == Language::En {
                    "Submit and continue"
                } else {
                    "提交并继续"
                })
            );
        }
        window.click("submit-keyboard-interactive", cx);
        fixture.workspace.read_with(cx, |workspace, _| {
            assert!(workspace.keyboard_interactive.is_none());
        });
    })
    .checked("submit keyboard-interactive MFA prompt");
    assert!(matches!(
        receiver.try_recv(),
        Ok(Some(answers)) if answers.len() == 1 && answers[0].as_str() == "123456"
    ));

    let mut receiver = cx
        .update_window(fixture.window, |_, window, cx| {
            fixture.workspace.update(cx, |workspace, cx| {
                let route_id = workspace
                    .connect_route
                    .as_ref()
                    .map(|route| route.id)
                    .checked_option("route after submitted challenge");
                let (sender, receiver) = tokio::sync::oneshot::channel();
                let answer = cx.new(|cx| InputState::new(window, cx).masked(true));
                workspace.keyboard_interactive = Some(KeyboardInteractivePrompt {
                    identity: uuid::Uuid::new_v4(),
                    route_id,
                    index: 0,
                    name: "Duo MFA".into(),
                    instructions: String::new(),
                    fields: vec![KeyboardInteractiveField {
                        prompt: "One-time code".into(),
                        answer,
                    }],
                    response: Some(sender),
                });
                receiver
            })
        })
        .checked("prepare second keyboard-interactive prompt");
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("cancel-keyboard-interactive", cx);
        fixture.workspace.read_with(cx, |workspace, _| {
            assert!(workspace.keyboard_interactive.is_none());
            assert!(workspace.connect_route.is_none());
        });
    })
    .checked("cancel keyboard-interactive MFA prompt");
    assert!(matches!(receiver.try_recv(), Ok(None)));
}

pub(crate) mod batch_peer;
mod command_workflows;
mod commands;
mod credentials;
mod dependency_workflow;
mod library;
mod routes;
mod themes;

mod remote_completion;

mod mcp;
