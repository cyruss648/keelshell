//! Native terminal rendering, owned transport workers, and platform IME input.
use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant},
};

use alacritty_terminal::{event::Event, term::TermMode};
use gpui_kit::*;
use keelshell_session::SessionEvent;

use crate::emulator::{
    Emulator, MouseAction, SearchDirection, SearchOutcome, encode_key, encode_mouse,
};
use crate::i18n::{LocalizedTooltipExt, Message, t};
use gpui_kit::component::input::{Enter, Escape, Input, InputEvent as FieldEvent, InputState};
use gpui_kit::component::{
    Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::prelude::FluentBuilder;
#[path = "terminal_input.rs"]
mod input;
#[path = "terminal_lifecycle.rs"]
mod lifecycle;
#[path = "terminal_output.rs"]
mod output;
pub(crate) use lifecycle::TransportState;
#[cfg(test)]
#[path = "terminal_text_input_tests.rs"]
mod text_input_tests;
#[path = "terminal_workers.rs"]
mod workers;
pub use input::TerminalCommand;
use input::{Composition, InputQueue};
use output::{FRAME_ANSI_EVENTS, FRAME_BYTES, FRAME_TRANSPORT_EVENTS, OutputQueue, PARSER_CHUNK};

actions!(terminal, [QuitTerminalApp]);

/// Register an owned transport thread; the closure must close its session before returning.
pub fn spawn_transport_worker(
    name: &str,
    cancelled: Arc<AtomicBool>,
    worker: impl FnOnce() -> Result<(), String> + Send + 'static,
) -> std::io::Result<()> {
    workers::registry().spawn(name, cancelled, worker)
}

#[cfg(test)]
pub(crate) fn transport_completion_for_test(
    cancelled: &Arc<AtomicBool>,
) -> Option<workers::Completion> {
    workers::registry().completion(cancelled)
}

/// Install explicit quit actions and the platform's bounded last-resort quit hook.
pub fn install_shutdown(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", QuitTerminalApp, None),
        KeyBinding::new("ctrl-shift-q", QuitTerminalApp, None),
    ]);
    cx.on_action(|_: &QuitTerminalApp, cx| shutdown_and_quit(cx));
    cx.on_app_quit(|cx| {
        workers::registry().request_stop();
        cx.background_executor().spawn(async {
            for error in workers::registry().shutdown() {
                eprintln!("Terminal shutdown: {error}");
            }
        })
    })
    .detach();
}

/// Quit only after transport workers finish. Call from window-close interception too.
/// The platform `on_app_quit` hook alone has a 200 ms timeout, so it is only a fallback.
pub fn shutdown_and_quit(cx: &mut App) {
    if workers::registry().quitting.swap(true, Ordering::AcqRel) {
        return;
    }
    workers::registry().request_stop();
    let shutdown = cx
        .background_executor()
        .spawn(async { workers::registry().shutdown() });
    cx.spawn(async move |cx| {
        let failures = shutdown.await;
        for error in failures {
            eprintln!("Terminal shutdown: {error}");
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}

type InputSelectionHandler = Box<dyn Fn(&mut App)>;

pub struct TerminalView {
    lifecycle: lifecycle::Lifecycle,
    pub title: String,
    pub status: Message,
    pub emulator: Emulator,
    focus: FocusHandle,
    input_selection: Option<InputSelectionHandler>,
    incoming: Receiver<SessionEvent>,
    output: OutputQueue,
    outgoing: SyncSender<TerminalCommand>,
    cancelled: Arc<AtomicBool>,
    completion: Option<workers::Completion>,
    completion_seen: bool,
    pending: InputQueue,
    exited: bool,
    started: bool,
    input_error: Option<String>,
    bounds: Option<Bounds<Pixels>>,
    cell_width: f32,
    line_height: f32,
    font_size: f32,
    composition: Composition,
    selecting: bool,
    mouse_pressed: Option<u8>,
    scroll_remainder: f32,
    search_focus: FocusHandle,
    search_input: Option<Entity<InputState>>,
    search_observer: Option<Subscription>,
    search_open: bool,
    search_query: String,
    search_feedback: Message,
    search_found: bool,
    _poll: Option<Task<()>>,
}

impl TerminalView {
    /// Create an inert entity identity for a restored tab without a transport.
    ///
    /// Starts closed with blank output and no history, pending input, lifecycle
    /// source, worker or polling task. Both channel peers are dropped immediately,
    /// so no outgoing receiver exists. Render the workspace's inactive placeholder
    /// for this entity; an explicit connection must create a new transport entity.
    pub fn inactive(title: String, font_size: f32, cx: &mut Context<Self>) -> Self {
        let (output, incoming) = mpsc::sync_channel(1);
        drop(output);
        let (outgoing, commands) = mpsc::sync_channel(1);
        drop(commands);
        Self::initialize(
            title,
            font_size,
            0,
            incoming,
            outgoing,
            Arc::new(AtomicBool::new(true)),
            cx,
        )
    }

    pub fn from_transport(
        title: String,
        font_size: f32,
        history: usize,
        incoming: Receiver<SessionEvent>,
        outgoing: SyncSender<TerminalCommand>,
        cancelled: Arc<AtomicBool>,
        cx: &mut Context<Self>,
    ) -> Self {
        let executor = cx.background_executor().clone();
        let poll = cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_millis(16)).await;
                if this.update(cx, |view, cx| view.poll(cx)).is_err() {
                    break;
                }
            }
        });
        let completion = workers::registry().completion(&cancelled);
        let mut view =
            Self::initialize(title, font_size, history, incoming, outgoing, cancelled, cx);
        view.status = Message::new("正在连接…", "Starting…");
        view.exited = false;
        view.completion = completion;
        view._poll = Some(poll);
        view
    }

    fn initialize(
        title: String,
        font_size: f32,
        history: usize,
        incoming: Receiver<SessionEvent>,
        outgoing: SyncSender<TerminalCommand>,
        cancelled: Arc<AtomicBool>,
        cx: &mut Context<Self>,
    ) -> Self {
        let palette = crate::design::terminal_palette(cx);
        let mut emulator = Emulator::new(30, 100, history);
        emulator.set_default_colors(palette.foreground, palette.background, palette.cursor);
        Self {
            lifecycle: lifecycle::Lifecycle::default(),
            title,
            status: Message::new("未连接", "Not connected"),
            emulator,
            focus: cx.focus_handle(),
            input_selection: None,
            incoming,
            output: OutputQueue::default(),
            outgoing,
            cancelled,
            completion: None,
            completion_seen: false,
            pending: InputQueue::default(),
            exited: true,
            started: false,
            input_error: None,
            bounds: None,
            cell_width: font_size * 0.61,
            line_height: font_size * 1.5,
            font_size,
            composition: Composition::default(),
            selecting: false,
            mouse_pressed: None,
            scroll_remainder: 0.,
            search_focus: cx.focus_handle().tab_stop(false),
            search_input: None,
            search_observer: None,
            search_open: false,
            search_query: String::new(),
            search_feedback: Message::empty(),
            search_found: false,
            _poll: None,
        }
    }

    fn sync_default_colors(&mut self, cx: &App) -> crate::design::TerminalPalette {
        let palette = crate::design::terminal_palette(cx);
        self.emulator
            .set_default_colors(palette.foreground, palette.background, palette.cursor);
        palette
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        // Hidden terminals can receive OSC queries before their next render.
        self.sync_default_colors(cx);
        let started = Instant::now();
        let mut changed = self.poll_lifecycle();
        changed |= !self.exited && !self.pending.is_empty();
        if self.search_found && !self.emulator.has_search_match() {
            self.invalidate_search_feedback();
            changed = true;
        }
        self.flush_input();
        let mut bytes_left = FRAME_BYTES;
        let mut ansi_left = FRAME_ANSI_EVENTS;
        let mut transport_left = FRAME_TRANSPORT_EVENTS;
        loop {
            // Drain the existing parser events before admitting more bytes. A
            // query/bell flood can therefore retain at most one parser chunk's
            // events instead of growing an unbounded queue across UI frames.
            while ansi_left > 0 && started.elapsed() < Duration::from_millis(4) {
                let Some(event) = self.emulator.event() else {
                    break;
                };
                ansi_left -= 1;
                changed = true;
                match event {
                    Event::PtyWrite(text) if self.is_open() => {
                        self.enqueue(text.into_bytes(), false)
                    }
                    Event::Title(title) => {
                        self.title = title.chars().filter(|c| !c.is_control()).take(80).collect();
                    }
                    Event::TextAreaSizeRequest(reply) if self.is_open() => self.enqueue(
                        reply(alacritty_terminal::event::WindowSize {
                            num_lines: self.emulator.size.rows as u16,
                            num_cols: self.emulator.size.cols as u16,
                            cell_width: self.cell_width as u16,
                            cell_height: self.line_height as u16,
                        })
                        .into_bytes(),
                        false,
                    ),
                    Event::ColorRequest(index, reply) if self.is_open() => {
                        self.enqueue(reply(self.emulator.color_reply(index)).into_bytes(), false);
                    }
                    // OSC52 never reads or changes the desktop clipboard implicitly.
                    _ => {}
                }
            }
            if ansi_left == 0
                || bytes_left == 0
                || transport_left == 0
                || started.elapsed() >= Duration::from_millis(4)
            {
                break;
            }
            let Some(event) = self
                .output
                .next(&self.incoming, PARSER_CHUNK.min(bytes_left))
            else {
                break;
            };
            transport_left -= 1;
            match event {
                SessionEvent::Data(bytes) => {
                    bytes_left -= bytes.len();
                    self.emulator.feed(&bytes);
                    if !bytes.is_empty() {
                        self.invalidate_search_feedback();
                    }
                    if !self.started {
                        self.started = true;
                        self.status = Message::new("已连接", "Connected");
                    }
                }
                SessionEvent::Exited { code, .. } if !self.lifecycle_managed() => {
                    self.status =
                        Message::new(format!("会话已退出（{code}）"), format!("Exited ({code})"));
                    self.exited = true;
                }
                SessionEvent::Error(error) if !self.lifecycle_managed() => {
                    self.input_error = Some(error.clone());
                    self.started = true;
                    self.status = Message::detail("远程终端错误", "Remote terminal error", error);
                }
                SessionEvent::Exited { .. } | SessionEvent::Error(_) => {}
            }
            changed = true;
        }
        if !self.completion_seen
            && let Some(completion) = &self.completion
        {
            let result = completion.lock().ok().and_then(|result| result.clone());
            if let Some(result) = result {
                self.completion_seen = true;
                if let Err(error) = result {
                    self.status = if error == "SSH_SHELL_CLEANUP_TIMEOUT" {
                        Message::new(
                            "关闭远程终端超时，远端状态尚未确认",
                            "SSH shell cleanup timed out; remote outcome is unknown",
                        )
                    } else {
                        Message::detail("远程终端已关闭", "Terminal closed", error)
                    };
                    self.exited = true;
                }
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
    }

    fn flush_input(&mut self) {
        if !self.is_open() {
            return;
        }
        for _ in 0..64 {
            let Some(command) = self.pending.pop() else {
                break;
            };
            match self.outgoing.try_send(command) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(command)) => {
                    self.pending.retry(command);
                    break;
                }
                Err(mpsc::TrySendError::Disconnected(command)) => {
                    self.pending.retry(command);
                    self.status = Message::new(
                        "远程终端连接已关闭，待发送的输入未发出",
                        "Terminal transport closed; pending input was not sent",
                    );
                    self.exited = true;
                    break;
                }
            }
        }
    }
    fn enqueue(&mut self, bytes: Vec<u8>, scroll: bool) {
        if !self.is_open() {
            self.status = Message::new(
                "终端已关闭，输入未发送",
                "Input not sent: terminal is closed",
            );
            return;
        }
        if let Err(error) = self.pending.write(bytes) {
            self.input_error = Some(error.into());
            self.status = input_capacity_message();
            return;
        }
        if self.input_error.take().is_some() {
            self.status = Message::new("已连接", "Connected");
        }
        if scroll {
            self.emulator.bottom();
        }
        self.flush_input();
    }
    /// Publish the input owner before moving focus, without waiting for a frame.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.publish_input_selection(cx);
        self.focus.focus(window, cx);
    }

    /// Bind a foreground owner notification for terminal and search input.
    ///
    /// The callback must not read or update this terminal or its search input:
    /// either entity may already be leased while an input action is handled.
    pub(crate) fn bind_input_selection(&mut self, handler: impl Fn(&mut App) + 'static) {
        self.input_selection = Some(Box::new(handler));
    }

    fn publish_input_selection(&self, cx: &mut App) {
        if let Some(handler) = &self.input_selection {
            handler(cx);
        }
    }

    /// Inspect focus only after the originating key action has returned.
    pub(crate) fn owns_input_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus.is_focused(window)
            || (self.search_open && self.search_focus.contains_focused(window, cx))
    }
    pub(crate) fn is_open(&self) -> bool {
        !self.exited && self.lifecycle_ready() && self.end_reason().is_none()
    }

    /// Admit a complete user command; success means queued, not remotely executed.
    pub fn try_write(&mut self, bytes: Vec<u8>) -> Result<(), Message> {
        if !self.is_open() {
            return Err(Message::new(
                "远程终端已关闭，命令未被接收",
                "Terminal is closed; command was not accepted",
            ));
        }
        self.pending
            .write(bytes)
            .map_err(|_| input_capacity_message())?;
        self.emulator.bottom();
        self.flush_input();
        if !self.is_open() {
            Err(Message::new(
                "输入入队时连接已关闭，请检查执行结果后再重试",
                "Transport closed while queueing input; inspect before retrying",
            ))
        } else {
            Ok(())
        }
    }
    pub fn write(&mut self, bytes: Vec<u8>) {
        self.enqueue(bytes, true);
    }
    pub fn paste(&mut self, text: &str) {
        let bytes = self.emulator.paste(text);
        self.write(bytes);
    }
    pub fn selected_text(&self) -> String {
        self.emulator.selected_text().unwrap_or_default()
    }
    pub fn visible_text(&self) -> String {
        self.emulator.visible_text()
    }
    fn ensure_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = &self.search_input {
            return input.clone();
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t(
                cx,
                "搜索远程终端输出…",
                "Search remote output…",
            ))
        });
        self.search_observer = Some(cx.subscribe(&input, |view, _, event: &FieldEvent, cx| {
            if view.search_open && matches!(event, FieldEvent::Change) {
                view.sync_search_query(cx);
            }
        }));
        self.search_input = Some(input.clone());
        input
    }
    fn sync_search_query(&mut self, cx: &mut Context<Self>) {
        let Some(input) = &self.search_input else {
            return;
        };
        let query = input.read(cx).value().to_string();
        if query == self.search_query {
            return;
        }
        self.search_query = query.clone();
        self.emulator.clear_search();
        let result = self.emulator.search(&query, SearchDirection::Next);
        self.apply_search_result(result, cx);
    }
    fn apply_search_result(&mut self, result: SearchOutcome, cx: &mut Context<Self>) {
        self.search_found = result == SearchOutcome::Found;
        self.search_feedback = match result {
            SearchOutcome::Found => Message::new("已定位到匹配内容", "Match found"),
            SearchOutcome::NotFound => Message::new("未找到匹配内容", "No match"),
            SearchOutcome::Empty => Message::new("请输入搜索关键词", "Enter a search term"),
            SearchOutcome::Invalid => Message::new(
                "请输入不超过 4 KiB 的单行关键词",
                "Use a single-line query up to 4 KiB",
            ),
        };
        cx.notify();
    }
    fn move_search(&mut self, direction: SearchDirection, cx: &mut Context<Self>) {
        self.publish_input_selection(cx);
        let Some(input) = &self.search_input else {
            return;
        };
        let query = input.read(cx).value().to_string();
        self.search_query = query.clone();
        let result = self.emulator.search(&query, direction);
        self.apply_search_result(result, cx);
    }
    fn invalidate_search_feedback(&mut self) {
        self.search_found = false;
        if self.search_open && !self.search_query.is_empty() {
            self.search_feedback = Message::new(
                "内容已更新，按回车重新查找",
                "Output changed; press Enter to search again",
            );
        }
    }
    /// Refresh only translated UI text; retain the user's query and match.
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.search_input {
            input.update(cx, |input, cx| {
                input.set_placeholder(
                    t(cx, "搜索远程终端输出…", "Search remote output…"),
                    window,
                    cx,
                );
            });
        }
        cx.notify();
    }
    fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.publish_input_selection(cx);
        let input = self.ensure_search_input(window, cx);
        self.search_open = true;
        self.refresh_locale(window, cx);
        self.emulator.clear_search();
        self.move_search(SearchDirection::Next, cx);
        input.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }
    fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = false;
        self.search_found = false;
        self.emulator.clear_search();
        self.focus(window, cx);
        cx.notify();
    }
    fn search_enter(&mut self, action: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        self.move_search(
            if action.shift {
                SearchDirection::Previous
            } else {
                SearchDirection::Next
            },
            cx,
        );
    }
    fn search_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        self.close_search(window, cx);
    }
    fn position(&self, position: Point<Pixels>) -> (usize, usize) {
        let Some(bounds) = self.bounds else {
            return (0, 0);
        };
        (
            (((f32::from(position.y - bounds.top()) / self.line_height).max(0.)) as usize)
                .min(self.emulator.size.rows - 1),
            (((f32::from(position.x - bounds.left()) / self.cell_width).max(0.)) as usize)
                .min(self.emulator.size.cols - 1),
        )
    }
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Inputs nested elsewhere in the workspace may bubble events. Only the
        // focused terminal surface is allowed to encode bytes for SSH.
        if !self.focus.is_focused(window) {
            return;
        }
        let key = &event.keystroke;
        let accelerator = (cfg!(target_os = "macos") && key.modifiers.platform)
            || (key.modifiers.control && key.modifiers.shift);
        if accelerator && key.key == "f" {
            self.open_search(window, cx);
            cx.stop_propagation();
            return;
        }
        if self.search_open && key.key == "escape" {
            self.close_search(window, cx);
            cx.stop_propagation();
            return;
        }
        if accelerator && key.key == "q" {
            shutdown_and_quit(cx);
            cx.stop_propagation();
            return;
        }
        if accelerator && key.key == "c" {
            let selection = self.selected_text();
            if !selection.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(selection));
            }
            cx.stop_propagation();
            return;
        }
        if accelerator && key.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.paste(&text);
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if self.composition.marked.is_some() {
            return;
        }
        if let Some(bytes) = encode_key(
            &key.key,
            key.modifiers.control,
            key.modifiers.alt,
            key.modifiers.shift,
            self.emulator.mode(),
        ) {
            self.write(bytes);
            cx.stop_propagation();
            cx.notify();
        }
    }
    fn mouse(
        &mut self,
        action: MouseAction,
        position: Point<Pixels>,
        modifiers: Modifiers,
    ) -> bool {
        if modifiers.shift {
            return false;
        }
        let (row, col) = self.position(position);
        if let Some(bytes) = encode_mouse(
            action,
            row,
            col,
            modifiers.control,
            modifiers.alt,
            false,
            self.emulator.mode(),
        ) {
            self.enqueue(bytes, false);
            true
        } else {
            false
        }
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let palette = self.sync_default_colors(cx);
        self.bounds = Some(bounds);
        let font = font(if cfg!(target_os = "windows") {
            "Consolas"
        } else if cfg!(target_os = "macos") {
            "Menlo"
        } else {
            "monospace"
        });
        let run = TextRun {
            len: 1,
            font: font.clone(),
            color: rgb(self.emulator.palette(256)).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        self.cell_width = f32::from(
            window
                .text_system()
                .shape_line("M".into(), px(self.font_size), &[run], None)
                .width,
        )
        .max(1.);
        let rows = (f32::from(bounds.size.height) / self.line_height)
            .floor()
            .clamp(2., 65535.) as usize;
        let cols = (f32::from(bounds.size.width) / self.cell_width)
            .floor()
            .clamp(2., 65535.) as usize;
        if rows != self.emulator.size.rows || cols != self.emulator.size.cols {
            self.emulator.resize(rows, cols);
            self.pending.resize(rows as u16, cols as u16);
            self.flush_input();
        }
        let background = self.emulator.palette(257);
        window.paint_quad(fill(bounds, rgb(background)));
        for cell in self.emulator.cells() {
            let position = point(
                bounds.left() + px(cell.col as f32 * self.cell_width),
                bounds.top() + px(cell.row as f32 * self.line_height),
            );
            let cell_bounds = Bounds::new(
                position,
                size(
                    px(self.cell_width * if cell.wide { 2. } else { 1. }),
                    px(self.line_height),
                ),
            );
            if cell.background != background || cell.selected {
                window.paint_quad(fill(
                    cell_bounds,
                    rgb(if cell.selected {
                        palette.selection
                    } else {
                        cell.background
                    }),
                ));
            }
            if cell.text != " " {
                let mut cell_font = font.clone();
                if cell.bold {
                    cell_font.weight = FontWeight::BOLD;
                }
                let run = TextRun {
                    len: cell.text.len(),
                    font: cell_font,
                    color: rgb(cell.foreground).into(),
                    background_color: None,
                    underline: cell.underline.then_some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(rgb(cell.foreground).into()),
                        wavy: false,
                    }),
                    strikethrough: None,
                };
                let line = window.text_system().shape_line(
                    cell.text.into(),
                    px(self.font_size),
                    &[run],
                    None,
                );
                if let Err(error) = line.paint(
                    position,
                    px(self.line_height),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                ) {
                    self.status =
                        Message::detail("终端文字渲染失败", "Text rendering failed", error);
                }
            }
        }
        if self.focus.is_focused(window)
            && let Some((row, col)) = self.emulator.cursor()
        {
            let position = point(
                bounds.left() + px(col as f32 * self.cell_width),
                bounds.top() + px(row as f32 * self.line_height),
            );
            window.paint_quad(fill(
                Bounds::new(position, size(px(2.), px(self.line_height))),
                rgb(self.emulator.palette(258)),
            ));
            if !self.composition.text.is_empty() {
                let run = TextRun {
                    len: self.composition.text.len(),
                    font,
                    color: rgb(self.emulator.palette(258)).into(),
                    background_color: Some(rgb(background).into()),
                    underline: None,
                    strikethrough: None,
                };
                let line = window.text_system().shape_line(
                    self.composition.text.clone().into(),
                    px(self.font_size),
                    &[run],
                    None,
                );
                let _ = line.paint(
                    position,
                    px(self.line_height),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        }
    }
}

impl TerminalView {
    fn search_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let visual = crate::design::palette(cx);
        let input = self.search_input.as_ref().filter(|_| self.search_open)?;
        Some(
            div()
                .id("terminal-search-bar")
                .test_support()
                .occlude()
                .capture_any_mouse_down(cx.listener(|view, _, _, cx| {
                    view.publish_input_selection(cx);
                }))
                // A distinct, non-tab-stop scope owns input and button focus.
                // Reusing Input's handle here would insert duplicate tab stops.
                .track_focus(&self.search_focus)
                .on_action(cx.listener(Self::search_enter))
                .on_action(cx.listener(Self::search_escape))
                .absolute()
                .top_2()
                .right_2()
                .w(px(380.))
                .max_w_full()
                .p_2()
                .rounded_md()
                .bg(rgb(visual.surface))
                .text_color(rgb(visual.text))
                .border_1()
                .border_color(rgb(visual.border))
                .shadow_lg()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div().flex_1().min_w_0().child(
                                Input::new(input)
                                    .id("terminal-search-input")
                                    .small()
                                    .aria_label(t(cx, "搜索当前终端", "Search current terminal")),
                            ),
                        )
                        .child(
                            Button::new("terminal-search-previous")
                                .ghost()
                                .compact()
                                .label("↑")
                                .accessibility_label(t(
                                    cx,
                                    "上一个搜索结果",
                                    "Previous search result",
                                ))
                                .localized_tooltip(
                                    "上一个（Shift+Enter）",
                                    "Previous (Shift+Enter)",
                                )
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.move_search(SearchDirection::Previous, cx)
                                })),
                        )
                        .child(
                            Button::new("terminal-search-next")
                                .ghost()
                                .compact()
                                .label("↓")
                                .accessibility_label(t(cx, "下一个搜索结果", "Next search result"))
                                .localized_tooltip("下一个（Enter）", "Next (Enter)")
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.move_search(SearchDirection::Next, cx)
                                })),
                        )
                        .child(
                            Button::new("terminal-search-close")
                                .ghost()
                                .compact()
                                .label("×")
                                .accessibility_label(t(cx, "关闭终端搜索", "Close terminal search"))
                                .localized_tooltip("关闭搜索（Esc）", "Close search (Esc)")
                                .on_click(
                                    cx.listener(|view, _, window, cx| {
                                        view.close_search(window, cx)
                                    }),
                                ),
                        ),
                )
                .child(
                    div()
                        .id("terminal-search-feedback")
                        .test_support()
                        .aria_label(self.search_feedback.render(cx))
                        .text_xs()
                        .text_color(rgb(visual.muted))
                        .child(self.search_feedback.render(cx)),
                )
                .into_any_element(),
        )
    }
}

fn input_capacity_message() -> Message {
    Message::new(
        "输入缓冲区已满（1 MiB），本次输入未被接收",
        "Input buffer is full (1 MiB); this input was not accepted",
    )
}

impl Drop for TerminalView {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn button_code(button: MouseButton) -> Option<u8> {
    match button {
        MouseButton::Left => Some(0),
        MouseButton::Middle => Some(1),
        MouseButton::Right => Some(2),
        _ => None,
    }
}
impl Render for TerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_default_colors(cx);
        let entity = cx.entity();
        let paint_entity = entity.clone();
        let mut element = div()
            .id("terminal-surface")
            .size_full()
            .relative()
            .overflow_hidden()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::key));
        for button in [MouseButton::Left, MouseButton::Middle, MouseButton::Right] {
            element = element
                .on_mouse_down(
                    button,
                    cx.listener(|view, event: &MouseDownEvent, window, cx| {
                        view.focus(window, cx);
                        let reported = button_code(event.button).is_some_and(|button| {
                            view.mouse(MouseAction::Press(button), event.position, event.modifiers)
                        });
                        if !reported && event.button == MouseButton::Left {
                            let (row, col) = view.position(event.position);
                            view.emulator.start_selection(row, col);
                            view.selecting = true;
                        }
                        if reported {
                            view.mouse_pressed = button_code(event.button);
                            cx.stop_propagation();
                        }
                        cx.notify();
                    }),
                )
                .on_mouse_up(
                    button,
                    cx.listener(|view, event: &MouseUpEvent, _, cx| {
                        if let Some(button) = button_code(event.button)
                            && view.mouse_pressed == Some(button)
                        {
                            view.mouse_pressed = None;
                            view.mouse(
                                MouseAction::Release(button),
                                event.position,
                                event.modifiers,
                            );
                        }
                        view.selecting = false;
                        cx.notify();
                    }),
                )
                .on_mouse_up_out(
                    button,
                    cx.listener(|view, event: &MouseUpEvent, _, cx| {
                        // An occluding dialog also counts as "up outside".
                        // Only finish a gesture which began in this terminal;
                        // clicking an overlay must not emit a stray SSH mouse-up.
                        if let Some(button) = button_code(event.button)
                            && view.mouse_pressed == Some(button)
                        {
                            view.mouse_pressed = None;
                            view.mouse(
                                MouseAction::Release(button),
                                event.position,
                                event.modifiers,
                            );
                        }
                        view.selecting = false;
                        cx.notify();
                    }),
                );
        }
        let surface = element
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, _, cx| {
                if view.selecting {
                    let (row, col) = view.position(event.position);
                    view.emulator.update_selection(row, col);
                    cx.notify();
                } else if view.mouse(
                    MouseAction::Motion(event.pressed_button.and_then(button_code)),
                    event.position,
                    event.modifiers,
                ) {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_scroll(cx.listener(|view, event: &ScrollEvent, _, cx| {
                let delta = event.delta.pixel_delta(px(view.line_height));
                view.scroll_remainder += f32::from(delta.y) / view.line_height;
                let lines = view.scroll_remainder.trunc() as i32;
                view.scroll_remainder -= lines as f32;
                if lines != 0 {
                    if !event.modifiers.shift
                        && view.emulator.mode().intersects(TermMode::MOUSE_MODE)
                    {
                        for _ in 0..lines.unsigned_abs().min(32) {
                            view.mouse(
                                MouseAction::Wheel(lines > 0),
                                event.position,
                                event.modifiers,
                            );
                        }
                    } else {
                        view.emulator.scroll(lines);
                    }
                }
                cx.notify();
                cx.stop_propagation();
            }))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, cx| {
                        let focus = entity.read(cx).focus.clone();
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, entity.clone()),
                            cx,
                        );
                        paint_entity.update(cx, |view, cx| view.paint(bounds, window, cx))
                    },
                )
                .size_full(),
            );
        // The search bar is a sibling of the transport surface, so pointer
        // events from its controls cannot bubble into the SSH mouse encoder.
        div()
            .id("terminal")
            .size_full()
            .relative()
            .overflow_hidden()
            .child(surface)
            .when_some(self.search_bar(cx), |element, bar| element.child(bar))
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let (text, adjusted) = self.composition.text_for(range);
        *actual = Some(adjusted);
        Some(text)
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.composition.selected.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.composition.marked.clone()
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.composition.clear();
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let committed = self.composition.commit(range, text);
        self.write(committed.into_bytes());
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composition.mark(range, text, selected);
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (row, col) = self.emulator.cursor().unwrap_or((0, 0));
        Some(Bounds::new(
            point(
                bounds.left() + px(col as f32 * self.cell_width),
                bounds.top() + px(row as f32 * self.line_height),
            ),
            size(px(self.cell_width), px(self.line_height)),
        ))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.composition.selected.start)
    }
}

#[cfg(test)]
mod inactive_tests {
    use super::*;
    use ::core::prelude::v1::test;

    #[gpui_kit::test]
    fn inactive_has_no_transport_lifecycle_or_polling_task(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let terminal = cx.new(|cx| TerminalView::inactive("Restored tab".into(), 14., cx));
        terminal.update(cx, |view, _| {
            assert_eq!(view.title, "Restored tab");
            assert_eq!(view.status, Message::new("未连接", "Not connected"));
            assert!(!view.is_open());
            assert!(!view.started);
            assert!(view._poll.is_none());
            assert!(view.completion.is_none());
            assert!(view.cancelled.load(Ordering::Acquire));
            assert!(view.subscribe_lifecycle().is_none());
            assert_eq!(view.end_reason(), None);
            assert!(!view.ready_to_archive());
            assert!(matches!(
                view.incoming.try_recv(),
                Err(mpsc::TryRecvError::Disconnected)
            ));
            assert_eq!(
                view.outgoing.try_send(TerminalCommand::Write(vec![b'x'])),
                Err(mpsc::TrySendError::Disconnected(TerminalCommand::Write(
                    vec![b'x']
                )))
            );
            assert!(view.pending.is_empty());
            assert!(view.composition.text.is_empty());
            assert!(view.visible_text().trim().is_empty());
            assert!(view.search_input.is_none());
            assert!(view.search_query.is_empty());
        });
    }

    #[gpui_kit::test]
    fn inactive_rejects_commands_writes_and_paste_without_admitting_input(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let terminal = cx.new(|cx| TerminalView::inactive("Restored tab".into(), 14., cx));
        terminal.update(cx, |view, cx| {
            assert!(view.try_write(b"command\n".to_vec()).is_err());
            view.write(b"typed text".to_vec());
            view.paste("pasted text\n");
            view.flush_input();
            view.poll(cx);
            assert!(!view.is_open());
            assert!(view.pending.is_empty());
            assert!(view.composition.text.is_empty());
            assert!(view.visible_text().trim().is_empty());
            assert!(view.subscribe_lifecycle().is_none());
            assert!(view._poll.is_none());
        });
    }
}

#[cfg(test)]
mod theme_tests {
    use super::*;
    use ::core::prelude::v1::test;
    use gpui_kit::test::TestWindowExt;

    fn apply_family(id: &str, window: Option<&mut Window>, cx: &mut App) {
        let family = crate::design::definition(id);
        let settings = keelshell_core::Settings {
            theme_family: keelshell_core::ThemeId::new(id)
                .unwrap_or_else(|error| panic!("registered family: {error}")),
            theme: family.preferred_appearance,
            ..Default::default()
        };
        crate::design::apply_settings(&settings, window, cx);
    }

    fn assert_theme_defaults(view: &TerminalView, cx: &App) {
        let palette = crate::design::terminal_palette(cx);
        assert_eq!(
            [
                view.emulator.palette(256),
                view.emulator.palette(257),
                view.emulator.palette(258),
            ],
            [palette.foreground, palette.background, palette.cursor]
        );
    }

    #[gpui_kit::test]
    fn construction_uses_current_theme_without_transport_commands(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        for family in crate::design::registry() {
            cx.update(|cx| apply_family(family.id, None, cx));
            let (_output, incoming) = mpsc::sync_channel(4);
            let (outgoing, commands) = mpsc::sync_channel(4);
            let terminal = cx.new(|cx| {
                TerminalView::from_transport(
                    "Theme defaults".into(),
                    14.,
                    100,
                    incoming,
                    outgoing,
                    Arc::new(AtomicBool::new(false)),
                    cx,
                )
            });
            terminal.update(cx, |view, cx| assert_theme_defaults(view, cx));
            assert_eq!(commands.try_recv(), Err(mpsc::TryRecvError::Empty));
        }
    }

    #[gpui_kit::test]
    fn hidden_terminal_queries_use_changed_theme_before_render(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| apply_family("frost", None, cx));
        let (_output, incoming) = mpsc::sync_channel(4);
        let (outgoing, commands) = mpsc::sync_channel(8);
        let terminal = cx.new(|cx| {
            TerminalView::from_transport(
                "Hidden terminal".into(),
                14.,
                100,
                incoming,
                outgoing,
                Arc::new(AtomicBool::new(false)),
                cx,
            )
        });
        cx.update(|cx| apply_family("graphite", None, cx));
        terminal.update(cx, |view, cx| {
            view.poll(cx);
            assert_theme_defaults(view, cx);
        });
        assert_eq!(commands.try_recv(), Err(mpsc::TryRecvError::Empty));

        // A later query must use the latest theme even if this tab never paints.
        terminal.update(cx, |view, _| {
            view.emulator
                .feed(b"\x1b]10;?\x07\x1b]11;?\x07\x1b]12;?\x07");
        });
        cx.update(|cx| apply_family("lavender", None, cx));
        let expected = terminal.update(cx, |view, cx| {
            for _ in 0..4 {
                view.poll(cx);
            }
            assert_theme_defaults(view, cx);
            let palette = crate::design::terminal_palette(cx);
            [palette.foreground, palette.background, palette.cursor]
        });
        let replies: Vec<_> = commands.try_iter().collect();
        assert_eq!(replies.len(), expected.len());
        for (command, color) in replies.iter().zip(expected) {
            let TerminalCommand::Write(bytes) = command else {
                panic!("a color query must produce a terminal write");
            };
            let expected = format!(
                "rgb:{:04x}/{:04x}/{:04x}",
                ((color >> 16) & 255) * 257,
                ((color >> 8) & 255) * 257,
                (color & 255) * 257,
            );
            assert!(String::from_utf8_lossy(bytes).contains(&expected));
        }
    }

    #[gpui_kit::test]
    fn theme_renders_preserve_terminal_state_and_transport(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| apply_family("frost", None, cx));
        let (_output, incoming) = mpsc::sync_channel(4);
        let (outgoing, commands) = mpsc::sync_channel(16);
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
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
                            "Theme rendering".into(),
                            14.,
                            100,
                            incoming,
                            outgoing,
                            stop,
                            cx,
                        )
                    })
                },
            )
        });
        let (window, terminal) = match opened {
            Ok(fixture) => fixture,
            Err(error) => panic!("open terminal theme fixture: {error:?}"),
        };
        let entity = terminal.entity_id();
        let result = cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(
                commands
                    .try_iter()
                    .all(|command| matches!(command, TerminalCommand::Resize(_, _)))
            );
            let before = terminal.update(cx, |view, cx| {
                view.focus(window, cx);
                for row in 0..view.emulator.size.rows + 3 {
                    view.emulator.feed(format!("row {row}\r\n").as_bytes());
                }
                view.emulator.scroll(100);
                view.emulator.start_selection(0, 0);
                view.emulator.update_selection(0, 4);
                view.composition.mark(None, "preedit", None);
                assert_eq!(view.selected_text(), "row 0");
                view.visible_text()
            });
            for family in crate::design::registry() {
                apply_family(family.id, Some(window), cx);
                window.render_frame(cx);
                terminal.update(cx, |view, cx| {
                    assert_theme_defaults(view, cx);
                    assert_eq!(view.visible_text(), before);
                    assert_eq!(view.selected_text(), "row 0");
                    assert_eq!(view.composition.text, "preedit");
                    assert!(view.focus.is_focused(window));
                    assert!(view.is_open());
                    assert!(Arc::ptr_eq(&view.cancelled, &cancelled));
                    view.poll(cx);
                });
                assert_eq!(terminal.entity_id(), entity);
                assert!(!cancelled.load(Ordering::Acquire));
                assert_eq!(commands.try_recv(), Err(mpsc::TryRecvError::Empty));
            }
            terminal.update(cx, |view, _| view.write(b"same transport".to_vec()));
            assert_eq!(
                commands.try_recv(),
                Ok(TerminalCommand::Write(b"same transport".to_vec()))
            );
        });
        assert!(result.is_ok(), "render terminal theme fixture: {result:?}");
    }
}
