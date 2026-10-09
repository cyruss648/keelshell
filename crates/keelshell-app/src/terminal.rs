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

pub struct TerminalView {
    lifecycle: lifecycle::Lifecycle,
    pub title: String,
    pub status: Message,
    pub emulator: Emulator,
    focus: FocusHandle,
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
    search_input: Option<Entity<InputState>>,
    search_observer: Option<Subscription>,
    search_open: bool,
    search_query: String,
    search_feedback: Message,
    search_found: bool,
    _poll: Task<()>,
}

impl TerminalView {
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
        Self {
            lifecycle: lifecycle::Lifecycle::default(),
            title,
            status: Message::new("正在连接…", "Starting…"),
            emulator: Emulator::new(30, 100, history),
            focus: cx.focus_handle(),
            incoming,
            output: OutputQueue::default(),
            outgoing,
            cancelled,
            completion,
            completion_seen: false,
            pending: InputQueue::default(),
            exited: false,
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
            search_input: None,
            search_observer: None,
            search_open: false,
            search_query: String::new(),
            search_feedback: Message::empty(),
            search_found: false,
            _poll: poll,
        }
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
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
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
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
            color: rgb(0xd9e3ed).into(),
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
                        0x254d63
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
                    background_color: Some(rgb(0x1a2838).into()),
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
                .track_focus(&input.read(cx).focus_handle(cx))
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
            .on_scroll_wheel(cx.listener(|view, event: &ScrollWheelEvent, _, cx| {
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
