//! Pure input state: bounded lossless queues and UTF-16 IME edits.

use std::{collections::VecDeque, ops::Range};

pub const MAX_PENDING_BYTES: usize = 1024 * 1024;
pub const WRITE_CHUNK: usize = 16 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum TerminalCommand {
    Write(Vec<u8>),
    Resize(u16, u16),
}

/// Retains accepted bytes until the next layer accepts them. A whole paste is
/// admitted before any chunk is sent, including its bracketed-paste terminator.
#[derive(Default)]
pub struct InputQueue {
    writes: VecDeque<Vec<u8>>,
    bytes: usize,
    resize: Option<(u16, u16)>,
}

impl InputQueue {
    pub fn write(&mut self, bytes: Vec<u8>) -> Result<(), &'static str> {
        if bytes.len() > MAX_PENDING_BYTES.saturating_sub(self.bytes) {
            return Err("Input buffer is full (1 MiB); this input was not accepted");
        }
        self.bytes += bytes.len();
        self.writes
            .extend(bytes.chunks(WRITE_CHUNK).map(<[u8]>::to_vec));
        Ok(())
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.resize = Some((rows, cols));
    }
    pub fn is_empty(&self) -> bool {
        self.writes.is_empty() && self.resize.is_none()
    }

    pub fn pop(&mut self) -> Option<TerminalCommand> {
        if let Some((rows, cols)) = self.resize.take() {
            return Some(TerminalCommand::Resize(rows, cols));
        }
        let bytes = self.writes.pop_front()?;
        self.bytes -= bytes.len();
        Some(TerminalCommand::Write(bytes))
    }

    pub fn retry(&mut self, command: TerminalCommand) {
        match command {
            TerminalCommand::Write(bytes) => {
                self.bytes += bytes.len();
                self.writes.push_front(bytes);
            }
            // Preserve a more recent resize if one arrived while this was pending.
            TerminalCommand::Resize(rows, cols) => {
                self.resize.get_or_insert((rows, cols));
            }
        }
    }
}

/// Preedit text uses UTF-16 ranges at the platform boundary, UTF-8 internally.
#[derive(Default)]
pub struct Composition {
    pub text: String,
    pub selected: Range<usize>,
    pub marked: Option<Range<usize>>,
}

impl Composition {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn range(&self, range: Range<usize>) -> (Range<usize>, Range<usize>) {
        let len = self.text.encode_utf16().count();
        let start = range.start.min(len);
        let end = range.end.max(start).min(len);
        let mut utf16 = 0;
        let mut start_byte = self.text.len();
        let mut end_byte = self.text.len();
        let mut actual_start = len;
        let mut actual_end = len;
        for (byte, character) in self.text.char_indices() {
            let next = utf16 + character.len_utf16();
            if start >= utf16 && start < next {
                start_byte = byte;
                actual_start = utf16;
            }
            if end >= utf16 && end < next {
                end_byte = if end == utf16 {
                    byte
                } else {
                    byte + character.len_utf8()
                };
                actual_end = if end == utf16 { utf16 } else { next };
            }
            utf16 = next;
        }
        (start_byte..end_byte, actual_start..actual_end)
    }

    pub fn text_for(&self, range: Range<usize>) -> (String, Range<usize>) {
        let (bytes, actual) = self.range(range);
        (self.text[bytes].to_owned(), actual)
    }

    fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> Range<usize> {
        let range = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selected.clone());
        let (bytes, actual) = self.range(range);
        self.text.replace_range(bytes, text);
        actual.start..actual.start + text.encode_utf16().count()
    }

    pub fn mark(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) {
        let inserted = self.replace(range, text);
        let relative = selected.unwrap_or_else(|| {
            let len = text.encode_utf16().count();
            len..len
        });
        let len = inserted.end - inserted.start;
        let requested =
            inserted.start + relative.start.min(len)..inserted.start + relative.end.min(len);
        self.selected = self.range(requested).1;
        self.marked = Some(inserted);
    }

    pub fn commit(&mut self, range: Option<Range<usize>>, text: &str) -> String {
        self.replace(range, text);
        let committed = std::mem::take(&mut self.text);
        self.clear();
        committed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backpressure_retains_order_and_complete_large_paste() {
        let mut queue = InputQueue::default();
        let bytes = [
            b"\x1b[200~".as_slice(),
            vec![b'x'; 100_000].as_slice(),
            b"\x1b[201~".as_slice(),
        ]
        .concat();
        assert!(queue.write(bytes.clone()).is_ok());
        let first = queue.pop();
        if let Some(first) = first {
            queue.retry(first);
        }
        let mut output = Vec::new();
        while let Some(TerminalCommand::Write(chunk)) = queue.pop() {
            assert!(chunk.len() <= WRITE_CHUNK);
            output.extend(chunk);
        }
        assert_eq!(output, bytes);
        assert!(queue.is_empty());
    }

    #[test]
    fn oversized_input_is_rejected_without_partial_admission() {
        let mut queue = InputQueue::default();
        assert!(queue.write(b"already accepted".to_vec()).is_ok());
        assert!(queue.write(vec![b'x'; MAX_PENDING_BYTES]).is_err());
        assert_eq!(
            queue.pop(),
            Some(TerminalCommand::Write(b"already accepted".to_vec()))
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn resize_retries_and_coalesces_to_latest_dimensions() {
        let mut queue = InputQueue::default();
        queue.resize(20, 80);
        if let Some(command) = queue.pop() {
            queue.resize(30, 100);
            queue.retry(command);
        }
        assert_eq!(queue.pop(), Some(TerminalCommand::Resize(30, 100)));
    }

    #[test]
    fn ime_replaces_only_requested_range_and_retains_selected_segment() {
        let mut composition = Composition::default();
        composition.mark(None, "你好世界", Some(0..2));
        assert_eq!(composition.selected, 0..2);
        composition.mark(Some(2..4), "中国", Some(0..1));
        assert_eq!(composition.text, "你好中国");
        assert_eq!(composition.selected, 2..3);
        assert_eq!(composition.commit(Some(2..4), "中华"), "你好中华");
        assert!(composition.text.is_empty());
    }

    #[test]
    fn ime_range_cannot_split_a_surrogate_pair() {
        let mut composition = Composition::default();
        composition.mark(None, "a🦀舟", None);
        assert_eq!(composition.text_for(2..3), ("🦀".into(), 1..3));
        assert_eq!(composition.commit(Some(2..3), "海"), "a海舟");
    }
}
