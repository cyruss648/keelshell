//! Rendered text dispatch and explicit composition commits; not native IME acceptance.

use super::*;
use ::core::prelude::v1::test;
use gpui_kit::test::{TestInput, TestWindowExt};

struct Fixture {
    window: AnyWindowHandle,
    terminal: Entity<TerminalView>,
    _output: mpsc::SyncSender<SessionEvent>,
    commands: mpsc::Receiver<TerminalCommand>,
}

fn fixture(cx: &mut TestAppContext) -> Fixture {
    cx.update(gpui_kit::init);
    let (output, incoming) = mpsc::sync_channel(4);
    let (outgoing, commands) = mpsc::sync_channel(32);
    let opened = cx.update(|cx| {
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
                        "Text protocol fixture".into(),
                        14.,
                        100,
                        incoming,
                        outgoing,
                        Arc::new(AtomicBool::new(false)),
                        cx,
                    )
                })
            },
        )
    });
    let (window, terminal) =
        opened.unwrap_or_else(|error| panic!("open rendered terminal text fixture: {error:?}"));
    cx.update_window(window, |_, window, cx| {
        terminal.update(cx, |view, cx| view.focus(window, cx));
        window.render_frame(cx);
    })
    .unwrap_or_else(|error| panic!("focus rendered terminal text fixture: {error:?}"));
    // Initial geometry may resize the PTY. It is not text input.
    assert!(
        commands
            .try_iter()
            .all(|command| matches!(command, TerminalCommand::Resize(_, _)))
    );
    Fixture {
        window,
        terminal,
        _output: output,
        commands,
    }
}

fn writes(commands: &mpsc::Receiver<TerminalCommand>) -> Vec<u8> {
    commands
        .try_iter()
        .filter_map(|command| match command {
            TerminalCommand::Write(bytes) => Some(bytes),
            TerminalCommand::Resize(_, _) => None,
        })
        .flatten()
        .collect()
}

#[gpui_kit::test]
fn rendered_terminal_dispatches_printable_text_and_subsequent_keys_once(cx: &mut TestAppContext) {
    let fixture = fixture(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        window.input("x中文🙂", cx);
        assert_eq!(writes(&fixture.commands), "x中文🙂".as_bytes());
        window.press("enter", cx);
        window.press("right", cx);
        assert_eq!(writes(&fixture.commands), b"\r\x1b[C");
        assert!(fixture.terminal.read(cx).composition.marked.is_none());
    })
    .unwrap_or_else(|error| panic!("dispatch rendered terminal text and keys: {error:?}"));
}

#[gpui_kit::test]
fn composition_does_not_send_preedit_or_confirmation_key_before_explicit_commit(
    cx: &mut TestAppContext,
) {
    let fixture = fixture(cx);
    cx.update_window(fixture.window, |_, window, cx| {
        let bounds = fixture
            .terminal
            .read(cx)
            .bounds
            .unwrap_or_else(|| panic!("rendered terminal must have text bounds"));
        // This bridge exercises the production EntityInputHandler contract.
        // Native input methods and candidate-window translation remain separate.
        {
            let handler = ElementInputHandler::new(bounds, fixture.terminal.clone());
            let mut input = TestInput::new(handler, window);
            input.compose_text(None, "ni", Some(2..2), cx);
        }
        window.press("enter", cx);
        window.press("right", cx);
        assert!(writes(&fixture.commands).is_empty());
        {
            let handler = ElementInputHandler::new(bounds, fixture.terminal.clone());
            TestInput::new(handler, window).commit_text("你", cx);
        }
        assert_eq!(writes(&fixture.commands), "你".as_bytes());
        window.press("enter", cx);
        assert_eq!(writes(&fixture.commands), b"\r");
        let terminal = fixture.terminal.read(cx);
        assert!(terminal.composition.marked.is_none());
        assert!(terminal.composition.text.is_empty());
    })
    .unwrap_or_else(|error| panic!("commit terminal composition exactly once: {error:?}"));
}
