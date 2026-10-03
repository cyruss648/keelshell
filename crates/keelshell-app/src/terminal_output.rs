//! Slice transport output without losing the suffix or reordering final events.

use std::sync::mpsc::{Receiver, TryRecvError};

use keelshell_session::SessionEvent;

pub const FRAME_BYTES: usize = 64 * 1024;
pub const PARSER_CHUNK: usize = 1024;
pub const FRAME_ANSI_EVENTS: usize = 256;
pub const FRAME_TRANSPORT_EVENTS: usize = 128;

#[derive(Default)]
pub struct OutputQueue {
    pending: Option<(Vec<u8>, usize)>,
    disconnected: bool,
}

impl OutputQueue {
    /// All transport senders have gone and every received byte was consumed.
    pub fn is_drained(&self) -> bool {
        self.disconnected && self.pending.is_none()
    }
    /// A zero allowance does not dequeue anything, including exit notifications.
    /// Only one transport packet is retained; unread packets stay in its bounded
    /// channel. Parser state, rather than these byte chunks, owns UTF-8 framing.
    pub fn next(
        &mut self,
        source: &Receiver<SessionEvent>,
        allowance: usize,
    ) -> Option<SessionEvent> {
        if allowance == 0 {
            return None;
        }
        if self.pending.is_none() {
            let event = match source.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    self.disconnected = true;
                    return None;
                }
            };
            match event {
                SessionEvent::Data(bytes) => self.pending = Some((bytes, 0)),
                event => return Some(event),
            }
        }
        let (bytes, offset) = self.pending.as_mut()?;
        let end = offset.saturating_add(allowance).min(bytes.len());
        let chunk = bytes[*offset..end].to_vec();
        *offset = end;
        if end == bytes.len() {
            self.pending = None;
        }
        Some(SessionEvent::Data(chunk))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use crate::emulator::Emulator;

    #[test]
    fn frame_overflow_preserves_every_byte_before_error_and_exit()
    -> Result<(), mpsc::SendError<SessionEvent>> {
        let (sender, receiver) = mpsc::sync_channel(4);
        let bytes: Vec<_> = (0..FRAME_BYTES + 19)
            .map(|index| (index % 251) as u8)
            .collect();
        sender.send(SessionEvent::Data(bytes.clone()))?;
        sender.send(SessionEvent::Data(b"tail".to_vec()))?;
        sender.send(SessionEvent::Error("after output".into()))?;
        sender.send(SessionEvent::Exited {
            code: 7,
            success: false,
        })?;
        let mut queue = OutputQueue::default();
        let mut output = Vec::new();
        while output.len() < FRAME_BYTES {
            let Some(SessionEvent::Data(chunk)) = queue.next(&receiver, PARSER_CHUNK) else {
                panic!("data must precede final events");
            };
            assert!(chunk.len() <= PARSER_CHUNK);
            output.extend(chunk);
        }
        assert_eq!(output, bytes[..FRAME_BYTES]);
        assert!(queue.next(&receiver, 0).is_none());
        for _ in 0..2 {
            let Some(SessionEvent::Data(chunk)) = queue.next(&receiver, PARSER_CHUNK) else {
                panic!("remaining suffix must precede exit");
            };
            output.extend(chunk);
        }
        assert_eq!(output, [bytes, b"tail".to_vec()].concat());
        assert!(
            matches!(queue.next(&receiver, 1), Some(SessionEvent::Error(message)) if message == "after output")
        );
        assert!(matches!(
            queue.next(&receiver, 1),
            Some(SessionEvent::Exited {
                code: 7,
                success: false
            })
        ));
        Ok(())
    }

    #[test]
    fn parser_survives_utf8_and_escape_sequences_split_at_every_byte()
    -> Result<(), mpsc::SendError<SessionEvent>> {
        let (sender, receiver) = mpsc::sync_channel(1);
        sender.send(SessionEvent::Data(
            "\x1b[31m中文🦀\x1b[0m!".as_bytes().to_vec(),
        ))?;
        let mut queue = OutputQueue::default();
        let mut emulator = Emulator::new(4, 30, 100);
        while let Some(SessionEvent::Data(chunk)) = queue.next(&receiver, 1) {
            emulator.feed(&chunk);
        }
        assert!(emulator.visible_text().contains("中文🦀!"));
        assert_eq!(emulator.cells()[0].foreground, emulator.palette(1));
        Ok(())
    }
}
