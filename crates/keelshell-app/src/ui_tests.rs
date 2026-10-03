//! Real GPUI layout, paint and input dispatch with deterministic transport queues.
//! These tests catch frame-phase panics and routing regressions without a shell,
//! network service, GPU pixel assertion or native OS IME acceptance claim.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::time::Duration;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, ClipboardItem, Context, ElementInputHandler, Entity,
    Focusable, InputEvent, InputHandler, IntoElement, MouseButton, MouseDownEvent, MouseUpEvent,
    Render, TestAppContext, Window, WindowBounds, WindowOptions, div, point, prelude::*, px, rgba,
    size, test::TestWindowExt,
};
use keelshell_session::SessionEvent;

use crate::i18n::Message;
use crate::terminal::{TerminalCommand, TerminalView};

/// GPUI test callbacks return unit. Keep setup failures explicit and report the
/// actual caller rather than suppressing workspace lint rules in test modules.
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

struct TerminalFixture {
    window: AnyWindowHandle,
    terminal: Entity<TerminalView>,
    output: SyncSender<SessionEvent>,
    commands: Receiver<TerminalCommand>,
    cancelled: Arc<AtomicBool>,
}

fn mount(cx: &mut TestAppContext) -> TerminalFixture {
    cx.update(gpui_kit::init);
    let (output, incoming) = mpsc::sync_channel(128);
    let (outgoing, commands) = mpsc::sync_channel(128);
    let cancelled = Arc::new(AtomicBool::new(false));
    let stop = cancelled.clone();
    let (window, terminal) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(800.), px(480.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|cx| {
                    TerminalView::from_transport(
                        "Controlled terminal".into(),
                        14.,
                        1_000,
                        incoming,
                        outgoing,
                        stop,
                        cx,
                    )
                })
            },
        )
        .checked("open the production root and terminal view")
    });
    TerminalFixture {
        window,
        terminal,
        output,
        commands,
        cancelled,
    }
}

fn written(commands: &Receiver<TerminalCommand>) -> Vec<u8> {
    commands
        .try_iter()
        .filter_map(|command| match command {
            TerminalCommand::Write(bytes) => Some(bytes),
            TerminalCommand::Resize(_, _) => None,
        })
        .flatten()
        .collect()
}

fn click_terminal(window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    let position = point(px(30.), px(60.));
    window.dispatch_event(
        MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count: 1,
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

#[gpui_kit::test]
fn terminal_frame_registers_input_in_paint_and_routes_focused_keys(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    fixture
        .output
        .send(SessionEvent::Data(b"\x1b[31mready\x1b[0m\r\n".to_vec()))
        .checked("test operation failed");
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(20));
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        // Running the complete frame is essential: moving handle_input back to
        // canvas prepaint must panic here instead of escaping a compile-only check.
        window.render_frame(cx);
        click_terminal(window, cx);
        assert!(
            fixture
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert!(fixture.terminal.read(cx).visible_text().contains("ready"));
        assert!(fixture.terminal.read(cx).emulator.size.cols > 2);
        window.input("echo 🦀", cx);
        window.press("enter", cx);
        window.press("ctrl-space", cx);
        window.press("alt-left", cx);
        window.render_frame(cx);
    })
    .checked("test operation failed");
    assert_eq!(
        written(&fixture.commands),
        "echo 🦀\r\0\x1b[1;3D".as_bytes()
    );
}

fn search_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd-f"
    } else {
        "ctrl-shift-f"
    }
}

fn input_accelerator(key: &str) -> String {
    format!(
        "{}-{key}",
        if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        }
    )
}

fn deliver_output(fixture: &TerminalFixture, bytes: Vec<u8>, cx: &mut TestAppContext) {
    fixture
        .output
        .send(SessionEvent::Data(bytes))
        .checked("deliver remote output");
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(20));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn terminal_search_shortcut_typing_enter_escape_and_focus_stay_local(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    deliver_output(&fixture, b"first hit\r\nsecond hit\r\n".to_vec(), cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        click_terminal(window, cx);
        window.press(search_shortcut(), cx);
        assert!(window.try_find("terminal-search-bar").is_some());
        assert_eq!(window.find("terminal-search-input").focused(), Some(true));
        window.input("hit", cx);
        assert!(
            written(&fixture.commands).is_empty(),
            "search text leaked into SSH"
        );
    })
    .checked("open search using the platform shortcut and type");
    // InputEvent::Change subscribers run when the native event transaction ends.
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(fixture.terminal.read(cx).selected_text(), "hit");
        let first = fixture
            .terminal
            .read(cx)
            .emulator
            .cells()
            .into_iter()
            .find(|cell| cell.selected)
            .map(|cell| (cell.row, cell.col));
        window.press("enter", cx);
        let second = fixture
            .terminal
            .read(cx)
            .emulator
            .cells()
            .into_iter()
            .find(|cell| cell.selected)
            .map(|cell| (cell.row, cell.col));
        assert_ne!(first, second, "Enter must advance to another match");
        window.press("shift-enter", cx);
        let previous = fixture
            .terminal
            .read(cx)
            .emulator
            .cells()
            .into_iter()
            .find(|cell| cell.selected)
            .map(|cell| (cell.row, cell.col));
        assert_eq!(
            first, previous,
            "Shift+Enter must move to the previous match"
        );
        window.press("escape", cx);
        assert!(window.try_find("terminal-search-bar").is_none());
        assert!(fixture.terminal.read(cx).selected_text().is_empty());
        assert!(
            fixture
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert!(
            written(&fixture.commands).is_empty(),
            "search keys leaked into SSH"
        );
        window.input("id", cx);
        window.press("enter", cx);
        assert_eq!(
            written(&fixture.commands),
            b"id\r",
            "terminal focus must work after closing search"
        );
        window.press(search_shortcut(), cx);
        assert_eq!(
            fixture.terminal.read(cx).selected_text(),
            "hit",
            "reopening must search the retained query again"
        );
    })
    .checked("navigate matches, close search and resume terminal input");
}

#[gpui_kit::test]
fn terminal_search_pointer_controls_and_paste_do_not_emit_ssh_mouse_or_input(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    deliver_output(
        &fixture,
        "中文[ok]\r\n中文[ok]\r\n\x1b[?1003h\x1b[?1006h"
            .as_bytes()
            .to_vec(),
        cx,
    );
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        click_terminal(window, cx);
        written(&fixture.commands); // The initial click belongs to the remote surface.
        window.press(search_shortcut(), cx);
        window.click("terminal-search-input", cx);
        window.input("中文", cx);
        cx.write_to_clipboard(ClipboardItem::new_string("[ok]".into()));
        window.press(&input_accelerator("v"), cx);
        assert_eq!(
            window.find("terminal-search-input").value(),
            Some("中文[ok]")
        );
        window.hover("terminal-search-next", cx);
        window.scroll(
            "terminal-search-input",
            gpui_kit::ScrollDelta::Lines(point(0., 1.)),
            cx,
        );
        assert!(
            written(&fixture.commands).is_empty(),
            "search pointer/typing/paste leaked into SSH"
        );
    })
    .checked("interact with the local search input over an SSH mouse-reporting terminal");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(fixture.terminal.read(cx).selected_text(), "中文[ok]");
        let first = fixture
            .terminal
            .read(cx)
            .emulator
            .cells()
            .into_iter()
            .find(|cell| cell.selected)
            .map(|cell| (cell.row, cell.col));
        window.click("terminal-search-next", cx);
        let next = fixture
            .terminal
            .read(cx)
            .emulator
            .cells()
            .into_iter()
            .find(|cell| cell.selected)
            .map(|cell| (cell.row, cell.col));
        assert_ne!(first, next);
        window.click("terminal-search-previous", cx);
        let previous = fixture
            .terminal
            .read(cx)
            .emulator
            .cells()
            .into_iter()
            .find(|cell| cell.selected)
            .map(|cell| (cell.row, cell.col));
        assert_eq!(first, previous);
        window.click("terminal-search-close", cx);
        assert!(window.try_find("terminal-search-bar").is_none());
        assert!(
            fixture
                .terminal
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert!(!fixture.terminal.read(cx).emulator.has_search_match());
        assert!(
            written(&fixture.commands).is_empty(),
            "search buttons leaked remote mouse events"
        );
    })
    .checked("click search navigation and close without remote mouse events");
}

#[gpui_kit::test]
fn terminal_search_output_invalidation_query_reset_and_locale_keep_input_local(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    // More output than the viewport forces the first match into retained history.
    let mut bytes = b"old needle\r\n".to_vec();
    bytes.extend("filler\r\n".repeat(60).bytes());
    deliver_output(&fixture, bytes, cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        click_terminal(window, cx);
        window.press(search_shortcut(), cx);
        assert_eq!(
            window.find("terminal-search-input").label(),
            Some("搜索远程终端输出…")
        );
        window.input("needle", cx);
    })
    .checked("open search in a terminal with scrollback");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(fixture.terminal.read(cx).selected_text(), "needle");
        assert!(
            fixture
                .terminal
                .read(cx)
                .visible_text()
                .contains("old needle")
        );
        crate::i18n::set_language(keelshell_core::Language::En, cx);
        fixture
            .terminal
            .update(cx, |terminal, cx| terminal.refresh_locale(window, cx));
        window.render_frame(cx);
        assert_eq!(
            window.find("terminal-search-input").label(),
            Some("Search remote output…")
        );
        assert_eq!(window.find("terminal-search-input").value(), Some("needle"));
        assert_eq!(
            window.find("terminal-search-feedback").label(),
            Some("Match found")
        );
        assert_eq!(fixture.terminal.read(cx).selected_text(), "needle");
    })
    .checked("reveal an offscreen match and update locale without losing search state");
    deliver_output(&fixture, b"new needle\r\n".to_vec(), cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!fixture.terminal.read(cx).emulator.has_search_match());
        assert!(fixture.terminal.read(cx).selected_text().is_empty());
        assert_eq!(
            window.find("terminal-search-feedback").label(),
            Some("Output changed; press Enter to search again")
        );
        window.press("enter", cx);
        assert_eq!(fixture.terminal.read(cx).selected_text(), "needle");
        window.press(&input_accelerator("a"), cx);
        window.input("missing", cx);
    })
    .checked("invalidate stale coordinates on output and repeat the search explicitly");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert!(!fixture.terminal.read(cx).emulator.has_search_match());
        assert_eq!(
            window.find("terminal-search-feedback").label(),
            Some("No match")
        );
        window.press(&input_accelerator("a"), cx);
        window.press("backspace", cx);
    })
    .checked("a new query clears the old selected match");
    cx.run_until_parked();
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("terminal-search-feedback").label(),
            Some("Enter a search term")
        );
        assert!(!fixture.terminal.read(cx).emulator.has_search_match());
        assert!(
            written(&fixture.commands).is_empty(),
            "search controls leaked into SSH"
        );
    })
    .checked("empty query resets search without sending remote input");
}

#[gpui_kit::test]
fn terminal_ime_commit_paste_and_release_preserve_transport_contract(cx: &mut TestAppContext) {
    let fixture = mount(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.render_frame(cx);
        click_terminal(window, cx);
        let mut handler = ElementInputHandler::new(
            Bounds::new(point(px(0.), px(0.)), size(px(800.), px(480.))),
            fixture.terminal.clone(),
        );
        handler.replace_and_mark_text_in_range(None, "に🦀", Some(1..3), window, cx);
        window.render_frame(cx);
        assert_eq!(handler.marked_text_range(window, cx), Some(0..3));
        assert_eq!(
            handler
                .selected_text_range(false, window, cx)
                .map(|selection| selection.range),
            Some(1..3)
        );
        assert!(
            written(&fixture.commands).is_empty(),
            "preedit must not reach the shell"
        );
        let mut actual = None;
        assert_eq!(
            handler.text_for_range(1..2, &mut actual, window, cx),
            Some("🦀".into())
        );
        assert_eq!(actual, Some(1..3));
        handler.replace_text_in_range(None, "日本語", window, cx);
        assert_eq!(handler.marked_text_range(window, cx), None);
        fixture.terminal.update(cx, |terminal, cx| {
            terminal.emulator.feed(b"\x1b[?2004h");
            terminal.paste("one\ntwo\x1b[201~");
            cx.notify();
        });
        window.render_frame(cx);
    })
    .checked("test operation failed");
    assert_eq!(
        written(&fixture.commands),
        "日本語\x1b[200~one\ntwo[201~\x1b[201~".as_bytes()
    );
    cx.update_window(fixture.window, |_, window, _| window.remove_window())
        .checked("test operation failed");
    let stop = fixture.cancelled.clone();
    drop(fixture);
    // Entity handles dropped outside an App transaction are released by the
    // next effect flush; advancing only runnable tasks does not flush the App.
    cx.update(|_| ());
    cx.run_until_parked();
    assert!(
        stop.load(Ordering::Acquire),
        "releasing the rendered tab cancels its transport"
    );
}

#[gpui_kit::test]
fn terminal_query_flood_yields_between_frames_and_preserves_suffix_before_exit(
    cx: &mut TestAppContext,
) {
    let fixture = mount(cx);
    let mut bytes = vec![7; 4096]; // Each BEL produces an ANSI event.
    bytes.extend_from_slice(b"after-flood");
    fixture
        .output
        .send(SessionEvent::Data(bytes))
        .checked("test operation failed");
    fixture
        .output
        .send(SessionEvent::Exited {
            code: 0,
            success: true,
        })
        .checked("test operation failed");
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(16));
    cx.run_until_parked();
    assert!(
        !fixture
            .terminal
            .read_with(cx, |view, _| view.visible_text())
            .contains("after-flood")
    );
    assert_ne!(
        fixture
            .terminal
            .read_with(cx, |view, _| view.status.clone()),
        Message::new("会话已退出（0）", "Exited (0)")
    );
    for _ in 0..40 {
        cx.background_executor
            .advance_clock(Duration::from_millis(16));
        cx.run_until_parked();
        if fixture.terminal.read_with(cx, |view, _| {
            view.status == Message::new("会话已退出（0）", "Exited (0)")
        }) {
            break;
        }
    }
    assert!(
        fixture
            .terminal
            .read_with(cx, |view, _| view.visible_text())
            .contains("after-flood")
    );
    assert_eq!(
        fixture
            .terminal
            .read_with(cx, |view, _| view.status.clone()),
        Message::new("会话已退出（0）", "Exited (0)")
    );
}

struct OverlayFixture {
    terminal: Entity<TerminalView>,
    field: Entity<InputState>,
    open: bool,
}

impl Render for OverlayFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            .child(self.terminal.clone())
            .when(self.open, |view| {
                view.child(
                    // Match the production Connection/Login modal's actual input
                    // occlusion boundary, including its full-window backdrop.
                    div()
                        .absolute()
                        .inset_0()
                        .occlude()
                        .bg(rgba(0x00000099))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(px(300.))
                                .p_4()
                                .child(Input::new(&self.field).id("overlay-input")),
                        ),
                )
            })
    }
}

#[gpui_kit::test]
fn occluding_modal_input_never_sends_text_paste_or_mouse_events_to_terminal(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (_output, incoming) = mpsc::sync_channel(8);
    let (outgoing, commands) = mpsc::sync_channel(128);
    let (handle, overlay) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(800.), px(480.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| OverlayFixture {
                    terminal: cx.new(|cx| {
                        TerminalView::from_transport(
                            "behind modal".into(),
                            14.,
                            100,
                            incoming,
                            outgoing,
                            Arc::new(AtomicBool::new(false)),
                            cx,
                        )
                    }),
                    field: cx.new(|cx| InputState::new(window, cx)),
                    open: false,
                })
            },
        )
        .checked("test operation failed")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        click_terminal(window, cx);
        overlay.update(cx, |view, cx| {
            view.terminal.update(cx, |terminal, _| {
                terminal.emulator.feed(b"\x1b[?1003h\x1b[?1006h")
            });
            view.open = true;
            cx.notify();
        });
        window.render_frame(cx);
        written(&commands);
        window.click("overlay-input", cx);
        window.input("host", cx);
        cx.write_to_clipboard(ClipboardItem::new_string(".example".into()));
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-v"
            } else {
                "ctrl-v"
            },
            cx,
        );
        assert_eq!(
            overlay.read(cx).field.read(cx).value().as_ref(),
            "host.example"
        );
        assert!(
            written(&commands).is_empty(),
            "modal interaction leaked into the terminal transport"
        );
    })
    .checked("test operation failed");
}
