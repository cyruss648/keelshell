use super::codec::{Probe, probe_header};
use super::{Error, FrameType, MAX_FEED_BYTES};

/// Local protocol role inferred from a checksum-valid remote initiation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// Remote sender sent ZRQINIT or ZFILE.
    Receive,
    /// Remote receiver sent ZRINIT; choose and review local files first.
    Send,
}

/// Ordered detector effects; detection itself confers no file authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DetectEvent {
    /// Ordinary output, byte-for-byte and in its original order.
    Terminal(Vec<u8>),
    /// Complete original valid prefix plus every following byte in this feed.
    ///
    /// Feed `bytes` to the appropriate state machine after leasing the PTY.
    /// It includes optional trailing XON and any already arrived metadata/data.
    /// The detector does not decode, discard or consume this protocol content.
    Start {
        /// Role for the local side.
        role: Role,
        /// Exact original protocol prefix and unconsumed suffix.
        bytes: Vec<u8>,
    },
}

/// Small bounded initiation detector for an ordinary terminal stream.
///
/// Retains at most 96 bytes between calls and triggers only on a complete,
/// checksum-valid ZRQINIT, ZRINIT or ZFILE (including required HEX line ending).
/// Textual `B00`/`rz` lookalikes and corrupt/incomplete signatures remain terminal
/// output. Call [`Self::flush`] when detection is disabled or the stream ends.
/// After `Start`, stop feeding this detector until the leased transfer finishes;
/// use [`Self::reset`] before returning it to ordinary terminal monitoring.
#[derive(Default)]
pub struct Detector {
    pending: Vec<u8>,
    detected: bool,
}

impl Detector {
    /// Construct a detector with no buffered terminal bytes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Consume a bounded terminal chunk, preserving every nonprotocol byte.
    ///
    /// A fragmented possible prefix is retained until validated, disproved, or
    /// explicitly flushed. A `Start` contains its entire original prefix and
    /// the rest of this chunk, so the first protocol frame is never lost.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<DetectEvent>, Error> {
        if self.detected {
            return Err(Error::WrongState("detector already transferred ownership"));
        }
        if bytes.len() > MAX_FEED_BYTES {
            return Err(Error::Limit("detector feed bytes"));
        }
        let mut input = std::mem::take(&mut self.pending);
        input.extend_from_slice(bytes);
        let mut cursor = 0;
        while cursor < input.len() {
            if input[cursor] & 0x7f != b'*' {
                cursor += 1;
                continue;
            }
            match probe_header(&input[cursor..]) {
                Probe::Header(header, consumed) => {
                    let role = match header.kind {
                        FrameType::RequestInit | FrameType::File => Some(Role::Receive),
                        FrameType::ReceiverInit => Some(Role::Send),
                        _ => None,
                    };
                    if let Some(role) = role {
                        let mut events = Vec::with_capacity(2);
                        if cursor > 0 {
                            events.push(DetectEvent::Terminal(input[..cursor].to_vec()));
                        }
                        events.push(DetectEvent::Start {
                            role,
                            bytes: input[cursor..].to_vec(),
                        });
                        self.detected = true;
                        return Ok(events);
                    }
                    cursor += consumed;
                }
                Probe::NeedMore if input.len() - cursor <= 96 => {
                    self.pending.extend_from_slice(&input[cursor..]);
                    input.truncate(cursor);
                    break;
                }
                Probe::NeedMore | Probe::NotHeader | Probe::Corrupt(_) => cursor += 1,
            }
        }
        if input.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(vec![DetectEvent::Terminal(input)])
        }
    }

    /// Return an incomplete possible prefix to the terminal without alteration.
    pub fn flush(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending)
    }

    /// Re-enable detection after a transfer, returning any old pending prefix.
    ///
    /// The caller must return these bytes to the terminal, if nonempty.
    pub fn reset(&mut self) -> Vec<u8> {
        self.detected = false;
        self.flush()
    }
}
