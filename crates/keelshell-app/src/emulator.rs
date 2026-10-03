//! Terminal state and input encoding, independent of GPUI rendering.
use std::sync::mpsc::{self, Receiver, Sender};

use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    index::{Boundary, Column, Direction, Line, Point, Side},
    selection::{Selection, SelectionType},
    term::{
        Config, TermMode,
        cell::Flags,
        color::{COUNT, Colors},
        search::RegexSearch,
    },
    vte::ansi::{Color, Processor},
};

#[derive(Clone)]
struct Listener(Sender<Event>);
impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

#[derive(Clone, Copy)]
pub struct Size {
    pub rows: usize,
    pub cols: usize,
}

/// Direction used by the terminal's scrollback search controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchDirection {
    Next,
    Previous,
}

/// Result of searching the retained remote terminal scrollback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchOutcome {
    Empty,
    Found,
    NotFound,
    Invalid,
}
impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

pub struct Cell {
    pub row: usize,
    pub col: usize,
    pub text: String,
    pub foreground: u32,
    pub background: u32,
    pub wide: bool,
    pub bold: bool,
    pub underline: bool,
    pub selected: bool,
}

pub struct Emulator {
    term: Term<Listener>,
    parser: Processor,
    events: Receiver<Event>,
    pub size: Size,
    search_match: Option<(Point, Point)>,
    search_query: String,
    history_limit: usize,
}

impl Emulator {
    pub fn new(rows: usize, cols: usize, history: usize) -> Self {
        let (sender, events) = mpsc::channel();
        let size = Size {
            rows: rows.max(2),
            cols: cols.max(2),
        };
        let config = Config {
            scrolling_history: history,
            ..Config::default()
        };
        Self {
            term: Term::new(config, &size, Listener(sender)),
            parser: Processor::new(),
            events,
            size,
            search_match: None,
            search_query: String::new(),
            history_limit: history,
        }
    }

    /// Transfer rendered history without carrying parser, input or remote modes.
    pub fn take_for_next_session(&mut self) -> Self {
        let mut previous = std::mem::replace(
            self,
            Self::new(self.size.rows, self.size.cols, self.history_limit),
        );
        let alternate = if previous.mode().contains(TermMode::ALT_SCREEN) {
            previous.term.grid_mut().scroll_display(Scroll::Bottom);
            let mut lines = vec![String::new(); previous.size.rows];
            for cell in previous.cells() {
                // A rendered snapshot cannot inject terminal escape sequences.
                lines[cell.row].extend(cell.text.chars().filter(|ch| !ch.is_control()));
            }
            previous.term.swap_alt();
            Some(lines)
        } else {
            None
        };
        let mut next = Self::new(
            previous.size.rows,
            previous.size.cols,
            previous.history_limit,
        );
        std::mem::swap(next.term.grid_mut(), previous.term.grid_mut());
        let rows = next.size.rows;
        let grid = next.term.grid_mut();
        grid.cursor = Default::default();
        grid.saved_cursor = Default::default();
        grid.scroll_display(Scroll::Bottom);
        grid.scroll_up(&(Line(0)..Line(rows as i32)), rows);
        if let Some(lines) = alternate {
            next.feed("[上一会话的最后屏幕 / Previous session screen]\r\n".as_bytes());
            for line in lines {
                next.feed(line.trim_end().as_bytes());
                next.feed(b"\r\n");
            }
        }
        next.feed("[新会话 / New session]\r\n".as_bytes());
        next
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        // Output can scroll, erase or switch the active grid. Coordinates from
        // an earlier search must never be reused against the changed buffer.
        if !bytes.is_empty() {
            self.clear_search();
        }
        self.parser.advance(&mut self.term, bytes);
    }
    pub fn event(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }
    pub fn mode(&self) -> TermMode {
        *self.term.mode()
    }
    pub fn palette(&self, index: usize) -> u32 {
        palette(index, self.term.colors())
    }
    pub fn color_reply(&self, index: usize) -> alacritty_terminal::vte::ansi::Rgb {
        let value = self.palette(index);
        alacritty_terminal::vte::ansi::Rgb {
            r: (value >> 16) as u8,
            g: (value >> 8) as u8,
            b: value as u8,
        }
    }
    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.clear_search();
        self.size = Size {
            rows: rows.max(2),
            cols: cols.max(2),
        };
        self.term.resize(self.size);
    }
    pub fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
    }
    pub fn bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }
    /// Search plain text in the configured scrollback buffer and select the
    /// current match. The query is escaped before reaching alacritty's regex
    /// engine, so punctuation has literal search semantics.
    pub fn search(&mut self, query: &str, direction: SearchDirection) -> SearchOutcome {
        if query.is_empty() {
            self.clear_search();
            return SearchOutcome::Empty;
        }
        if query.len() > 4096 || query.contains(['\r', '\n']) {
            self.clear_search();
            return SearchOutcome::Invalid;
        }
        if query != self.search_query {
            self.clear_search();
            self.search_query = query.to_owned();
        }
        let pattern = escape_search_query(query);
        let Ok(mut regex) = RegexSearch::new(&pattern) else {
            self.clear_search();
            return SearchOutcome::Invalid;
        };
        let (direction, side, origin) = match (direction, self.search_match) {
            (SearchDirection::Next, Some((_, end))) => (
                Direction::Right,
                Side::Right,
                end.add(&self.term, Boundary::None, 1),
            ),
            (SearchDirection::Previous, Some((start, _))) => (
                Direction::Left,
                Side::Left,
                start.sub(&self.term, Boundary::None, 1),
            ),
            (SearchDirection::Next, None) => (
                Direction::Right,
                Side::Left,
                Point::new(
                    Line(-((self.term.total_lines().saturating_sub(self.size.rows)) as i32)),
                    Column(0),
                ),
            ),
            (SearchDirection::Previous, None) => (
                Direction::Left,
                Side::Right,
                Point::new(Line(self.size.rows as i32 - 1), Column(self.size.cols - 1)),
            ),
        };
        let Some(found) = self
            .term
            .search_next(&mut regex, origin, direction, side, None)
        else {
            self.clear_search();
            return SearchOutcome::NotFound;
        };
        let first = *found.start();
        let last = *found.end();
        let (start, end) = if first <= last {
            (first, last)
        } else {
            (last, first)
        };
        self.term.selection = Some(Selection::new(SelectionType::Simple, start, Side::Left));
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(end, Side::Right);
        }
        // Bring an off-screen scrollback match into view while preserving the
        // existing terminal viewport when it already contains the result.
        let max_offset = self.term.total_lines().saturating_sub(self.size.rows);
        let current_offset = self.term.grid().display_offset();
        let viewport_start = -(current_offset as i32);
        let viewport_end = viewport_start + self.size.rows as i32 - 1;
        let target_offset = if start.line.0 < viewport_start || end.line.0 > viewport_end {
            ((-start.line.0).max(0) as usize).min(max_offset)
        } else {
            current_offset
        };
        let delta = target_offset as i32 - current_offset as i32;
        if delta != 0 {
            self.term.scroll_display(Scroll::Delta(delta));
        }
        self.search_match = Some((start, end));
        SearchOutcome::Found
    }
    pub fn clear_search(&mut self) {
        if self.search_match.take().is_some() {
            self.term.selection = None;
        }
        self.search_query.clear();
    }
    pub fn has_search_match(&self) -> bool {
        self.search_match.is_some()
    }
    pub fn cursor(&self) -> Option<(usize, usize)> {
        if !self.mode().contains(TermMode::SHOW_CURSOR) {
            return None;
        }
        let p = self.term.grid().cursor.point;
        let row = p.line.0 + self.term.grid().display_offset() as i32;
        (row >= 0 && row < self.size.rows as i32).then_some((row as usize, p.column.0))
    }
    pub fn cells(&self) -> Vec<Cell> {
        let content = self.term.renderable_content();
        content
            .display_iter
            .filter_map(|indexed| {
                let row = indexed.point.line.0 + content.display_offset as i32;
                if row < 0
                    || row >= self.size.rows as i32
                    || indexed.cell.flags.contains(Flags::WIDE_CHAR_SPACER)
                {
                    return None;
                }
                let cell = indexed.cell;
                let mut foreground = color(cell.fg, content.colors);
                let mut background = color(cell.bg, content.colors);
                if cell.flags.contains(Flags::INVERSE) {
                    std::mem::swap(&mut foreground, &mut background);
                }
                let mut text = cell.c.to_string();
                if let Some(combining) = cell.zerowidth() {
                    text.extend(combining);
                }
                if cell.flags.contains(Flags::HIDDEN) {
                    text = " ".to_owned();
                }
                Some(Cell {
                    row: row as usize,
                    col: indexed.point.column.0,
                    text,
                    foreground,
                    background,
                    wide: cell.flags.contains(Flags::WIDE_CHAR),
                    bold: cell.flags.contains(Flags::BOLD),
                    underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
                    selected: content
                        .selection
                        .is_some_and(|selection| selection.contains(indexed.point)),
                })
            })
            .collect()
    }
    fn point(&self, row: usize, col: usize) -> Point {
        Point::new(
            Line(row.min(self.size.rows - 1) as i32 - self.term.grid().display_offset() as i32),
            Column(col.min(self.size.cols - 1)),
        )
    }
    pub fn start_selection(&mut self, row: usize, col: usize) {
        self.clear_search();
        self.term.selection = Some(Selection::new(
            SelectionType::Simple,
            self.point(row, col),
            Side::Left,
        ));
    }
    pub fn update_selection(&mut self, row: usize, col: usize) {
        let point = self.point(row, col);
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, Side::Right);
        }
    }
    pub fn selected_text(&self) -> Option<String> {
        self.term.selection_to_string()
    }
    pub fn visible_text(&self) -> String {
        self.term.bounds_to_string(
            self.point(0, 0),
            self.point(self.size.rows - 1, self.size.cols - 1),
        )
    }
    pub fn paste(&self, text: &str) -> Vec<u8> {
        // Escape is removed so pasted content cannot terminate bracketed paste early.
        let text = text.replace('\u{1b}', "");
        if self.mode().contains(TermMode::BRACKETED_PASTE) {
            format!("\x1b[200~{text}\x1b[201~").into_bytes()
        } else {
            text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
        }
    }
}

fn escape_search_query(query: &str) -> String {
    // Explicitly disable the engine's smart-case rule: this UI offers literal,
    // case-sensitive matching rather than an implicit uppercase heuristic.
    let mut pattern = String::with_capacity(query.len() + 6);
    pattern.push_str("(?-i:");
    for character in query.chars() {
        if matches!(
            character,
            '\\' | '.' | '^' | '$' | '|' | '(' | ')' | '[' | ']' | '{' | '}' | '*' | '+' | '?'
        ) {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push(')');
    pattern
}

const PALETTE: [u32; 16] = [
    0x17212f, 0xee7b84, 0x75d5aa, 0xe8c481, 0x7aaef7, 0xb599ed, 0x69cbd2, 0xd9e3ed, 0x526279,
    0xff9aa2, 0x9be4bf, 0xf2d8a0, 0xa0c7ff, 0xd0b9ff, 0x9ae5e7, 0xf4f7fa,
];

fn indexed_color(index: u8) -> u32 {
    match index {
        0..=15 => PALETTE[usize::from(index)],
        16..=231 => {
            let n = u32::from(index) - 16;
            let component = |x| if x == 0 { 0 } else { 55 + x * 40 };
            (component(n / 36) << 16) | (component((n / 6) % 6) << 8) | component(n % 6)
        }
        _ => {
            let value = 8 + (u32::from(index) - 232) * 10;
            (value << 16) | (value << 8) | value
        }
    }
}
fn rgb_number(rgb: alacritty_terminal::vte::ansi::Rgb) -> u32 {
    (u32::from(rgb.r) << 16) | (u32::from(rgb.g) << 8) | u32::from(rgb.b)
}
fn palette(index: usize, colors: &Colors) -> u32 {
    if index < COUNT
        && let Some(rgb) = colors[index]
    {
        return rgb_number(rgb);
    }
    match index {
        0..=255 => indexed_color(index as u8),
        257 => 0x0c121b,
        258 => 0x7bdec5,
        259..=266 => {
            let rgb = PALETTE[index - 259];
            let dim = |component: u32| component * 2 / 3;
            (dim((rgb >> 16) & 255) << 16) | (dim((rgb >> 8) & 255) << 8) | dim(rgb & 255)
        }
        267 => 0xf4f7fa,
        268 => 0x90979e,
        _ => 0xd9e3ed,
    }
}
fn color(value: Color, colors: &Colors) -> u32 {
    match value {
        Color::Spec(rgb) => rgb_number(rgb),
        Color::Indexed(index) => palette(usize::from(index), colors),
        Color::Named(named) => palette(named as usize, colors),
    }
}

/// Encode non-text keys; committed Unicode travels through the platform IME handler.
pub fn encode_key(
    key: &str,
    control: bool,
    alt: bool,
    shift: bool,
    mode: TermMode,
) -> Option<Vec<u8>> {
    let with_alt = |bytes: Vec<u8>| {
        if alt {
            [vec![0x1b], bytes].concat()
        } else {
            bytes
        }
    };
    if control {
        let byte = if key == "space" {
            Some(b' ')
        } else if key.len() == 1 {
            Some(key.as_bytes()[0])
        } else {
            None
        };
        if let Some(byte) = byte {
            let code = match byte {
                b'a'..=b'z' | b'A'..=b'Z' => Some(byte.to_ascii_uppercase() & 0x1f),
                b' ' | b'@' | b'2' => Some(0),
                b'['..=b'_' => Some(byte & 0x1f),
                b'3' => Some(0x1b),
                b'4' => Some(0x1c),
                b'5' => Some(0x1d),
                b'6' => Some(0x1e),
                b'7' | b'/' => Some(0x1f),
                b'8' | b'?' => Some(0x7f),
                _ => None,
            };
            if let Some(code) = code {
                return Some(with_alt(vec![code]));
            }
        }
    }
    let modifier = 1 + usize::from(shift) + 2 * usize::from(alt) + 4 * usize::from(control);
    let arrow = match key {
        "up" => Some('A'),
        "down" => Some('B'),
        "right" => Some('C'),
        "left" => Some('D'),
        "home" => Some('H'),
        "end" => Some('F'),
        _ => None,
    };
    if let Some(code) = arrow {
        let prefix = if mode.contains(TermMode::APP_CURSOR) {
            "\x1bO"
        } else {
            "\x1b["
        };
        return Some(
            if modifier == 1 {
                format!("{prefix}{code}")
            } else {
                format!("\x1b[1;{modifier}{code}")
            }
            .into_bytes(),
        );
    }
    let function = match key {
        "f1" => Some('P'),
        "f2" => Some('Q'),
        "f3" => Some('R'),
        "f4" => Some('S'),
        _ => None,
    };
    if let Some(code) = function {
        return Some(
            if modifier == 1 {
                format!("\x1bO{code}")
            } else {
                format!("\x1b[1;{modifier}{code}")
            }
            .into_bytes(),
        );
    }
    let numbered = match key {
        "insert" => Some(2),
        "delete" => Some(3),
        "pageup" => Some(5),
        "pagedown" => Some(6),
        "f5" => Some(15),
        "f6" => Some(17),
        "f7" => Some(18),
        "f8" => Some(19),
        "f9" => Some(20),
        "f10" => Some(21),
        "f11" => Some(23),
        "f12" => Some(24),
        _ => None,
    };
    if let Some(number) = numbered {
        return Some(
            if modifier == 1 {
                format!("\x1b[{number}~")
            } else {
                format!("\x1b[{number};{modifier}~")
            }
            .into_bytes(),
        );
    }
    let bytes = match key {
        "enter" => b"\r".to_vec(),
        "backspace" if control => vec![8],
        "backspace" => vec![127],
        "escape" => vec![27],
        "tab" if shift => b"\x1b[Z".to_vec(),
        "tab" => vec![9],
        "space" if alt => vec![b' '],
        _ if alt && key.chars().count() == 1 => key.as_bytes().to_vec(),
        _ => return None,
    };
    Some(with_alt(bytes))
}

#[derive(Clone, Copy)]
pub enum MouseAction {
    Press(u8),
    Release(u8),
    Motion(Option<u8>),
    Wheel(bool),
}

/// Xterm SGR and basic/UTF-8 mouse protocols. Shift override is handled by the UI.
pub fn encode_mouse(
    action: MouseAction,
    row: usize,
    col: usize,
    control: bool,
    alt: bool,
    shift: bool,
    mode: TermMode,
) -> Option<Vec<u8>> {
    if !mode.intersects(TermMode::MOUSE_MODE) {
        return None;
    }
    let mut code = match action {
        MouseAction::Press(button) | MouseAction::Release(button) => button,
        MouseAction::Wheel(up) => {
            if up {
                64
            } else {
                65
            }
        }
        MouseAction::Motion(button) => {
            if !mode.contains(TermMode::MOUSE_MOTION)
                && !(mode.contains(TermMode::MOUSE_DRAG) && button.is_some())
            {
                return None;
            }
            32 + button.unwrap_or(3)
        }
    };
    code += 4 * u8::from(shift) + 8 * u8::from(alt) + 16 * u8::from(control);
    let release = matches!(action, MouseAction::Release(_));
    if mode.contains(TermMode::SGR_MOUSE) {
        let suffix = if release { 'm' } else { 'M' };
        return Some(format!("\x1b[<{code};{};{}{suffix}", col + 1, row + 1).into_bytes());
    }
    if release {
        code = 3 + (code & 28);
    }
    let mut result = vec![27, b'[', b'M', code + 32];
    if mode.contains(TermMode::UTF8_MOUSE) {
        if row >= 2015 || col >= 2015 {
            return None;
        }
        for point in [col + 33, row + 33] {
            let character = char::from_u32(point as u32)?;
            let mut buffer = [0; 4];
            result.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
        }
    } else {
        if row >= 223 || col >= 223 {
            return None;
        }
        result.extend_from_slice(&[(col + 33) as u8, (row + 33) as u8]);
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_preserves_history_and_last_alternate_screen_without_remote_modes() {
        let mut old = Emulator::new(4, 80, 100);
        old.feed(b"old scrollback\r\nsecond\r\nthird\r\nfourth\r\nmain tail");
        old.feed(b"\x1b[?1049hfullscreen tail\x1b[?1h\x1b[?2004h\x1b[?1003h");
        old.feed(b"\x1b[8mconcealed\x1b[0m");
        let mut next = old.take_for_next_session();
        assert_eq!(next.mode(), Emulator::new(4, 80, 100).mode());
        for text in [
            "old scrollback",
            "main tail",
            "fullscreen tail",
            "New session",
        ] {
            assert_eq!(
                next.search(text, SearchDirection::Next),
                SearchOutcome::Found
            );
        }
        assert_eq!(
            next.search("concealed", SearchDirection::Next),
            SearchOutcome::NotFound
        );
        assert_eq!(
            old.search("old scrollback", SearchDirection::Next),
            SearchOutcome::NotFound
        );
    }

    #[test]
    fn reconnect_discards_pending_parser_replies_and_partial_escape_sequences() {
        let mut old = Emulator::new(8, 80, 100);
        old.feed(b"\x1b[31mold\x1b[6n\x1b[?200");
        old.search("old", SearchDirection::Next);
        let mut next = old.take_for_next_session();
        assert!(next.event().is_none());
        assert!(next.search_match.is_none());
        assert!(next.search_query.is_empty());
        assert!(next.selected_text().is_none());
        next.feed(b"4hnew");
        assert!(!next.mode().contains(TermMode::BRACKETED_PASTE));
        assert!(next.visible_text().contains("4hnew"));
        let new_cell = next.cells().into_iter().find(|cell| cell.text == "4");
        assert_eq!(
            new_cell.map(|cell| cell.foreground),
            Some(next.palette(256))
        );
    }

    #[test]
    fn reconnect_repeated_history_transfer_stays_bounded() {
        let mut terminal = Emulator::new(4, 80, 12);
        for _ in 0..30 {
            terminal.feed(b"before drop\r\n");
            terminal = terminal.take_for_next_session();
            assert!(terminal.term.grid().history_size() <= 12);
        }
        assert_eq!(
            terminal.search("before drop", SearchDirection::Previous),
            SearchOutcome::Found
        );
    }
    #[test]
    fn parses_unicode_truecolor_and_alt_screen() {
        let mut e = Emulator::new(10, 40, 100);
        e.feed("\x1b[38;2;12;34;56m你好\x1b[0m".as_bytes());
        assert!(e.visible_text().contains("你好"));
        assert_eq!(e.cells()[0].foreground, 0x0c2238);
        assert!(e.cells()[0].wide);
        e.feed(b"\x1b[?1049hTEMP\x1b[?1049l");
        assert!(e.visible_text().contains("你好"));
        assert!(!e.visible_text().contains("TEMP"));
    }
    #[test]
    fn handles_split_utf8_and_device_status_queries() {
        let mut e = Emulator::new(10, 40, 100);
        let bytes = "舟".as_bytes();
        e.feed(&bytes[..1]);
        e.feed(&bytes[1..]);
        e.feed(b"\x1b[6n");
        assert!(e.visible_text().starts_with('舟'));
        assert!(std::iter::from_fn(|| e.event()).any(|event| matches!(event, Event::PtyWrite(_))));
    }
    #[test]
    fn respects_application_cursor_and_bracketed_paste() {
        let mut e = Emulator::new(10, 40, 100);
        e.feed(b"\x1b[?1h\x1b[?2004h");
        assert_eq!(
            encode_key("up", false, false, false, e.mode()),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(e.paste("safe\x1b[201~"), b"\x1b[200~safe[201~\x1b[201~");
    }
    #[test]
    fn preserves_selection_and_resizes_grid() {
        let mut e = Emulator::new(10, 40, 100);
        e.feed(b"hello world");
        e.start_selection(0, 0);
        e.update_selection(0, 4);
        assert_eq!(e.selected_text().as_deref(), Some("hello"));
        e.resize(20, 80);
        assert_eq!(e.size.cols, 80);
    }

    #[test]
    fn search_cycles_matches_reveals_history_and_keeps_visible_viewport_stable() {
        let mut e = Emulator::new(3, 40, 32);
        e.feed(b"first hit\r\nsecond line\r\nthird hit\r\nlast line\r\n");
        assert!(!e.visible_text().contains("first hit"));
        assert_eq!(e.search("hit", SearchDirection::Next), SearchOutcome::Found);
        let first = e.search_match;
        assert_eq!(e.selected_text().as_deref(), Some("hit"));
        assert!(e.visible_text().contains("first hit"));
        let offset = e.term.grid().display_offset();
        assert_eq!(e.search("hit", SearchDirection::Next), SearchOutcome::Found);
        let second = e.search_match;
        assert_ne!(first, second);
        assert_eq!(
            e.term.grid().display_offset(),
            offset,
            "an already-visible match must not jump the viewport"
        );
        assert_eq!(e.search("hit", SearchDirection::Next), SearchOutcome::Found);
        assert_eq!(e.search_match, first, "next wraps to oldest match");
        assert_eq!(
            e.search("hit", SearchDirection::Previous),
            SearchOutcome::Found
        );
        assert_eq!(e.search_match, second, "previous wraps to newest match");
        assert_eq!(
            e.search("hit", SearchDirection::Previous),
            SearchOutcome::Found
        );
        assert_eq!(e.search_match, first);
    }

    #[test]
    fn search_matches_unicode_wrapped_text_and_literal_case_sensitive_punctuation() {
        let mut e = Emulator::new(4, 10, 10);
        e.feed("abcd中文[ok]\r\nABCD".as_bytes());
        assert_eq!(
            e.search("中文[ok]", SearchDirection::Next),
            SearchOutcome::Found
        );
        assert_eq!(e.selected_text().as_deref(), Some("中文[ok]"));
        assert!(
            e.cells()
                .iter()
                .filter(|cell| cell.selected)
                .any(|cell| cell.wide)
        );
        assert_eq!(
            e.search("abcd", SearchDirection::Next),
            SearchOutcome::Found
        );
        let first = e.search_match;
        assert_eq!(
            e.search("abcd", SearchDirection::Next),
            SearchOutcome::Found
        );
        assert_eq!(
            e.search_match, first,
            "lowercase must not match uppercase implicitly"
        );
        assert_eq!(
            e.search("[missing]", SearchDirection::Next),
            SearchOutcome::NotFound
        );
        assert!(e.selected_text().is_none());
    }

    #[test]
    fn search_clears_stale_coordinates_on_output_resize_and_alternate_screen() {
        let mut e = Emulator::new(2, 16, 2);
        e.feed(b"old hit\r\nline\r\n");
        assert_eq!(e.search("hit", SearchDirection::Next), SearchOutcome::Found);
        e.feed(b"1\r\n2\r\n3\r\n4\r\n");
        assert!(!e.has_search_match());
        assert!(e.selected_text().is_none());
        assert_eq!(
            e.search("hit", SearchDirection::Next),
            SearchOutcome::NotFound
        );
        e.feed(b"new hit");
        assert_eq!(e.search("hit", SearchDirection::Next), SearchOutcome::Found);
        e.resize(3, 8);
        assert!(!e.has_search_match());
        assert!(e.selected_text().is_none());
        assert_eq!(e.search("hit", SearchDirection::Next), SearchOutcome::Found);
        e.feed(b"\x1b[?1049hALT");
        assert!(!e.has_search_match());
        assert_eq!(
            e.search("hit", SearchDirection::Next),
            SearchOutcome::NotFound
        );
        e.feed(b"\x1b[?1049l");
        assert_eq!(e.search("hit", SearchDirection::Next), SearchOutcome::Found);
    }

    #[test]
    fn search_new_query_resets_origin_and_close_preserves_subsequent_manual_selection() {
        let mut e = Emulator::new(3, 20, 10);
        e.feed(b"one target\r\nsecond target");
        assert_eq!(
            e.search("target", SearchDirection::Next),
            SearchOutcome::Found
        );
        assert_eq!(
            e.search("target", SearchDirection::Next),
            SearchOutcome::Found
        );
        assert_eq!(e.search("one", SearchDirection::Next), SearchOutcome::Found);
        assert_eq!(
            e.search_match.map(|(start, _)| start),
            Some(Point::new(Line(0), Column(0)))
        );
        assert_eq!(e.search("", SearchDirection::Next), SearchOutcome::Empty);
        assert!(e.selected_text().is_none());
        assert!(!e.has_search_match());
        e.start_selection(0, 0);
        e.update_selection(0, 2);
        e.clear_search();
        assert_eq!(
            e.selected_text().as_deref(),
            Some("one"),
            "closing search must not clear a later manual selection"
        );
    }

    #[test]
    fn search_rejects_multiline_and_oversized_queries_before_building_a_pattern() {
        let mut e = Emulator::new(2, 10, 0);
        e.feed(b"ok");
        assert_eq!(e.search("ok", SearchDirection::Next), SearchOutcome::Found);
        for query in ["ok\r".to_owned(), "ok\n".to_owned(), "a".repeat(4097)] {
            assert_eq!(
                e.search(&query, SearchDirection::Next),
                SearchOutcome::Invalid
            );
            assert!(!e.has_search_match());
            assert!(e.selected_text().is_none());
        }
    }

    #[test]
    fn control_and_alt_modifiers_survive_key_encoding() {
        let cases = [
            ("space", true, false, false, b"\0".as_slice()),
            ("a", true, true, false, b"\x1b\x01".as_slice()),
            ("backspace", false, true, false, b"\x1b\x7f".as_slice()),
            ("backspace", true, false, false, b"\x08".as_slice()),
            ("delete", true, false, false, b"\x1b[3;5~".as_slice()),
            ("f5", false, false, true, b"\x1b[15;2~".as_slice()),
            ("f1", false, true, false, b"\x1b[1;3P".as_slice()),
            ("left", true, true, true, b"\x1b[1;8D".as_slice()),
        ];
        for (key, control, alt, shift, expected) in cases {
            assert_eq!(
                encode_key(key, control, alt, shift, TermMode::empty()),
                Some(expected.to_vec()),
                "{key}"
            );
        }
    }

    #[test]
    fn osc_palette_changes_affect_rendering_queries_and_reset() {
        let mut emulator = Emulator::new(10, 40, 100);
        emulator.feed(b"\x1b]4;1;rgb:12/34/56\x07\x1b[31mR\x1b]11;rgb:01/02/03\x07");
        assert_eq!(emulator.cells()[0].foreground, 0x123456);
        assert_eq!(emulator.cells()[0].background, 0x010203);
        emulator.feed(b"\x1b]4;1;?\x07");
        let mut replies = Vec::new();
        while let Some(event) = emulator.event() {
            if let Event::ColorRequest(index, reply) = event {
                replies.push(reply(emulator.color_reply(index)));
            }
        }
        assert!(
            replies
                .iter()
                .any(|reply| reply.contains("rgb:1212/3434/5656"))
        );
        emulator.feed(b"\x1b]104;1\x07\x1b]111\x07");
        assert_eq!(emulator.palette(1), PALETTE[1]);
        assert_eq!(emulator.palette(257), 0x0c121b);
    }

    #[test]
    fn sgr_mouse_reports_press_drag_release_and_wheel() {
        let mode = TermMode::MOUSE_DRAG | TermMode::SGR_MOUSE;
        let cases = [
            (MouseAction::Press(0), b"\x1b[<0;6;3M".as_slice()),
            (MouseAction::Motion(Some(0)), b"\x1b[<32;6;3M".as_slice()),
            (MouseAction::Release(0), b"\x1b[<0;6;3m".as_slice()),
            (MouseAction::Wheel(true), b"\x1b[<64;6;3M".as_slice()),
        ];
        for (action, expected) in cases {
            assert_eq!(
                encode_mouse(action, 2, 5, false, false, false, mode),
                Some(expected.to_vec())
            );
        }
        assert!(encode_mouse(MouseAction::Motion(None), 2, 5, false, false, false, mode).is_none());
    }

    #[test]
    fn basic_mouse_checks_coordinate_bounds_and_mode() {
        let mode = TermMode::MOUSE_REPORT_CLICK;
        assert_eq!(
            encode_mouse(MouseAction::Press(0), 0, 0, false, false, false, mode),
            Some(vec![27, b'[', b'M', 32, 33, 33])
        );
        assert_eq!(
            encode_mouse(MouseAction::Release(0), 0, 0, false, false, false, mode),
            Some(vec![27, b'[', b'M', 35, 33, 33])
        );
        assert!(encode_mouse(MouseAction::Press(0), 223, 0, false, false, false, mode).is_none());
        assert!(
            encode_mouse(
                MouseAction::Press(0),
                0,
                0,
                false,
                false,
                false,
                TermMode::empty()
            )
            .is_none()
        );
    }
}
