use super::codec::{Decoder, Token, crc32};
use super::{
    Batch, Budget, CANCEL, Error, Event, FileOffer, FrameEnd, FrameType, Header, HeaderFormat,
    MAX_TOTAL_BYTES, SendFile, TX_PACKET_BYTES, check_events, encode_data, encode_header,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    New,
    Init,
    Position,
    Ack,
    FileEnd,
    Finish,
    Done,
    Cancelled,
    Failed,
}

/// Immutable-file sender using bounded ZCRCW/ACK pacing and ZRPOS recovery.
///
/// Construct only after review of all names, bytes and the actual remote peer.
/// Start does not invoke `rz`, execute commands or choose a remote directory.
/// A ZMPROT hint asks a cooperating receiver to protect existing destinations;
/// the receiver controls its policy, so this is never an overwrite guarantee.
/// Initial ZRPOS can request resume; neither that prefix nor remote publication
/// is verified by this protocol. `FileSent` means ZEOF was acknowledged only.
pub struct Sender {
    state: State,
    files: Vec<SendFile>,
    index: usize,
    decoder: Decoder,
    budget: Budget,
    format: HeaderFormat,
    packet_bytes: usize,
    pending_end: u32,
    acknowledged_position: u32,
    resumed_position: u32,
    position_retries: u32,
    last_wire: Vec<u8>,
    crc_bytes: u64,
    aborting: bool,
}

impl Sender {
    /// Validate a nonempty reviewed batch and take immutable ownership.
    ///
    /// Rejects more than 32 files, per-file/total excess, and repeated/case-alias
    /// names before emitting any protocol bytes.
    pub fn new(files: Vec<SendFile>) -> Result<Self, Error> {
        if files.is_empty() {
            return Err(Error::WrongState("empty send batch"));
        }
        let mut batch = Batch::default();
        for file in &files {
            batch.add(&file.offer)?;
        }
        Ok(Self {
            state: State::New,
            files,
            index: 0,
            decoder: Decoder::default(),
            budget: Budget::default(),
            format: HeaderFormat::Binary16,
            packet_bytes: TX_PACKET_BYTES,
            pending_end: 0,
            acknowledged_position: 0,
            resumed_position: 0,
            position_retries: 0,
            last_wire: Vec::new(),
            crc_bytes: 0,
            aborting: false,
        })
    }

    /// Emit ZRQINIT, without a shell command or implicit file approval.
    pub fn start(&mut self) -> Result<Vec<Event>, Error> {
        if self.state != State::New {
            return Err(Error::WrongState("sender already started"));
        }
        self.state = State::Init;
        let mut events = Vec::new();
        self.control(FrameType::RequestInit, 0, true, &mut events)?;
        Ok(events)
    }

    /// Feed bounded raw response bytes, retaining fragmented headers.
    ///
    /// ZRINIT negotiates CRC and receive-window size. Each ZCRCW frame waits for
    /// the exact matching ZACK; ZRPOS can restart at any valid file offset.
    /// Errors make the sender inactive until explicit cancellation.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Event>, Error> {
        match self.state {
            State::New | State::Failed => return Err(Error::WrongState("sender inactive")),
            State::Done | State::Cancelled => {
                if bytes.len() > super::MAX_FEED_BYTES {
                    return Err(Error::Limit("feed bytes"));
                }
                return Ok(if bytes.is_empty() {
                    Vec::new()
                } else {
                    vec![Event::Terminal(bytes.to_vec())]
                });
            }
            _ => {}
        }
        let result = self.feed_inner(bytes);
        if result.is_err() {
            self.state = State::Failed;
        }
        result
    }

    fn feed_inner(&mut self, bytes: &[u8]) -> Result<Vec<Event>, Error> {
        self.budget.input(bytes)?;
        let mut events = Vec::new();
        for (index, &byte) in bytes.iter().enumerate() {
            let token = match self.decoder.push(byte) {
                Ok(token) => token,
                Err(Error::CorruptHeader | Error::InvalidEscape) => {
                    self.budget.retry()?;
                    events.push(Event::Retry { position: None });
                    self.control(FrameType::Nak, 0, false, &mut events)?;
                    continue;
                }
                Err(error) => return Err(error),
            };
            if let Some(token) = token {
                self.budget.packet()?;
                match token {
                    Token::Header(header) => self.header(header, &mut events)?,
                    Token::Cancelled => {
                        self.state = State::Cancelled;
                        events.push(Event::Cancelled);
                    }
                    _ => return Err(Error::WrongState("unsolicited sender data/OO")),
                }
                check_events(&events)?;
                if matches!(self.state, State::Done | State::Cancelled) {
                    if index + 1 < bytes.len() {
                        events.push(Event::Terminal(bytes[index + 1..].to_vec()));
                    }
                    break;
                }
            }
        }
        Ok(events)
    }

    /// Retry the last bounded frame on an outer idle timer.
    ///
    /// The timer/deadline belongs to the caller; each call consumes the
    /// cumulative retry budget. Replayed ZDATA still requires the receiver's
    /// contiguous-position check and cannot acknowledge a remote save.
    pub fn timeout(&mut self) -> Result<Vec<Event>, Error> {
        if matches!(self.state, State::New | State::Failed) {
            return Err(Error::WrongState("sender inactive"));
        }
        if matches!(self.state, State::Done | State::Cancelled) {
            return Ok(Vec::new());
        }
        let mut events = Vec::new();
        let result = self.retry(&mut events);
        if let Err(error) = result {
            self.state = State::Failed;
            return Err(error);
        }
        Ok(events)
    }

    /// Cancel once, including after an error; infer no remote cleanup.
    pub fn cancel(&mut self) -> Result<Vec<Event>, Error> {
        if matches!(self.state, State::Done | State::Cancelled) {
            return Ok(Vec::new());
        }
        self.state = State::Cancelled;
        self.last_wire.clear();
        Ok(vec![Event::Send(CANCEL.to_vec()), Event::Cancelled])
    }

    /// The current reviewed metadata, if any files remain.
    pub fn pending_offer(&self) -> Option<&FileOffer> {
        self.files.get(self.index).map(|file| &file.offer)
    }

    /// Whether the complete ZFIN/OO handshake finished without cancellation.
    pub fn is_finished(&self) -> bool {
        self.state == State::Done
    }

    fn header(&mut self, header: Header, events: &mut Vec<Event>) -> Result<(), Error> {
        let kind = header.kind;
        if kind == FrameType::Command
            || (kind == FrameType::RequestInit && header.flags()[3] == FrameType::Command as u8)
        {
            return Err(Error::RemoteCommand);
        }
        match kind {
            FrameType::ReceiverInit => match self.state {
                State::Init => {
                    self.negotiate(header)?;
                    self.offer_next(events)?;
                }
                State::Position => {
                    self.budget.retry()?;
                    self.negotiate(header)?;
                    self.offer_next(events)?;
                }
                State::FileEnd => {
                    let offer = self
                        .pending_offer()
                        .ok_or(Error::WrongState("missing sent offer"))?
                        .clone();
                    events.push(Event::FileSent(offer));
                    self.index += 1;
                    self.negotiate(header)?;
                    self.offer_next(events)?;
                }
                State::Finish => self.retry(events)?,
                _ => return Err(Error::UnexpectedFrame(kind)),
            },
            FrameType::Position => {
                if !matches!(self.state, State::Position | State::Ack | State::FileEnd) {
                    return Err(Error::UnexpectedFrame(kind));
                }
                let size = self
                    .pending_offer()
                    .ok_or(Error::WrongState("missing send offer"))?
                    .size;
                if u64::from(header.position) > size {
                    return Err(Error::InvalidPosition);
                }
                if self.state == State::Position {
                    self.acknowledged_position = header.position;
                    self.resumed_position = header.position;
                } else {
                    // Some independent receivers request the next acknowledged
                    // offset again after each stop-and-wait frame. Forward,
                    // already acknowledged progress is ordinary pacing. Repeated
                    // or backward recovery still consumes the retry budget.
                    if header.position != self.acknowledged_position
                        || header.position <= self.resumed_position
                    {
                        self.position_retries += 1;
                        if self.position_retries > super::MAX_RETRIES {
                            return Err(Error::RetryLimit);
                        }
                        events.push(Event::Retry {
                            position: Some(u64::from(header.position)),
                        });
                    }
                    self.resumed_position = self.resumed_position.max(header.position);
                }
                self.send_at(header.position, events)?;
            }
            FrameType::Ack => {
                if self.state == State::Ack && header.position == self.pending_end {
                    let offer = self
                        .pending_offer()
                        .ok_or(Error::WrongState("missing acknowledged offer"))?
                        .clone();
                    events.push(Event::Progress {
                        offer,
                        bytes: u64::from(header.position),
                    });
                    if header.position > self.acknowledged_position {
                        self.position_retries = 0;
                        self.acknowledged_position = header.position;
                    }
                    self.send_at(header.position, events)?;
                } else if header.position > self.acknowledged_position {
                    // Stale acknowledged positions are harmless duplicates.
                    // Future/mismatching positions cannot skip file bytes.
                    self.budget.retry()?;
                }
            }
            FrameType::Skip => {
                if !matches!(self.state, State::Position | State::Ack | State::FileEnd) {
                    return Err(Error::UnexpectedFrame(kind));
                }
                let offer = self
                    .pending_offer()
                    .ok_or(Error::WrongState("missing skipped offer"))?
                    .clone();
                events.push(Event::FileSkipped(offer));
                self.index += 1;
                self.offer_next(events)?;
            }
            FrameType::Crc => {
                if self.state != State::Position {
                    return Err(Error::UnexpectedFrame(kind));
                }
                let file = self
                    .files
                    .get(self.index)
                    .ok_or(Error::WrongState("missing CRC source"))?;
                let count = if header.position == 0 {
                    file.bytes.len()
                } else {
                    header.position as usize
                };
                if count > file.bytes.len() {
                    return Err(Error::InvalidPosition);
                }
                self.crc_bytes = self
                    .crc_bytes
                    .checked_add(count as u64)
                    .filter(|bytes| *bytes <= MAX_TOTAL_BYTES * 2)
                    .ok_or(Error::Limit("file-prefix CRC work"))?;
                self.budget.retry()?;
                let value = crc32(&file.bytes[..count]);
                self.control(FrameType::Crc, value, false, events)?;
            }
            FrameType::Challenge => {
                self.control(FrameType::Ack, header.position, false, events)?;
            }
            FrameType::RequestInit if header.position == 0 => {
                // Echo of our invitation has no authority and no effect.
            }
            FrameType::Nak => self.retry(events)?,
            FrameType::Abort | FrameType::FileError => {
                self.aborting = true;
                self.state = State::Finish;
                self.control(FrameType::Finish, 0, true, events)?;
            }
            FrameType::Cancel => {
                self.state = State::Cancelled;
                events.push(Event::Cancelled);
            }
            FrameType::Finish if self.state == State::Finish => {
                self.budget.send(b"OO".to_vec(), events)?;
                self.last_wire.clear();
                if self.aborting {
                    self.state = State::Cancelled;
                    events.push(Event::Cancelled);
                } else {
                    self.state = State::Done;
                    events.push(Event::Finished);
                }
            }
            _ => return Err(Error::UnexpectedFrame(kind)),
        }
        Ok(())
    }

    fn negotiate(&mut self, header: Header) -> Result<(), Error> {
        let flags = header.flags();
        if flags[3] & 0x80 != 0 {
            return Err(Error::Unsupported("8-bit escaping"));
        }
        self.format = if flags[3] & 0x20 != 0 {
            HeaderFormat::Binary32
        } else {
            HeaderFormat::Binary16
        };
        let buffer = usize::from(u16::from_le_bytes([flags[0], flags[1]]));
        self.packet_bytes = if buffer == 0 {
            TX_PACKET_BYTES
        } else {
            buffer.min(TX_PACKET_BYTES)
        };
        Ok(())
    }

    fn offer_next(&mut self, events: &mut Vec<Event>) -> Result<(), Error> {
        let Some(file) = self.files.get(self.index) else {
            self.state = State::Finish;
            return self.control(FrameType::Finish, 0, true, events);
        };
        let remaining_bytes: u64 = self.files[self.index + 1..]
            .iter()
            .map(|file| file.offer.size)
            .sum();
        let mut metadata = file.offer.name.as_bytes().to_vec();
        metadata.push(0);
        metadata.extend_from_slice(
            format!(
                "{} 0 100644 0 {} {}\0",
                file.offer.size,
                self.files.len() - self.index - 1,
                remaining_bytes
            )
            .as_bytes(),
        );
        // ZCBIN and ZMPROT request binary/no-clobber treatment from a
        // cooperating peer. They do not prove a receiver's actual behavior.
        let mut wire = encode_header(Header::new(
            FrameType::File,
            (1_u32 << 24) | (7_u32 << 16),
            self.format,
        ));
        wire.extend_from_slice(&encode_data(&metadata, FrameEnd::EndAck, self.format)?);
        self.state = State::Position;
        self.pending_end = 0;
        self.acknowledged_position = 0;
        self.resumed_position = 0;
        self.position_retries = 0;
        self.transmit(wire, true, events)
    }

    fn send_at(&mut self, position: u32, events: &mut Vec<Event>) -> Result<(), Error> {
        let file = self
            .files
            .get(self.index)
            .ok_or(Error::WrongState("missing data source"))?;
        let position_usize = position as usize;
        if position_usize > file.bytes.len() {
            return Err(Error::InvalidPosition);
        }
        if position_usize == file.bytes.len() {
            self.state = State::FileEnd;
            return self.control(FrameType::EndOfFile, position, true, events);
        }
        let end = (position_usize + self.packet_bytes).min(file.bytes.len());
        let mut wire = encode_header(Header::new(FrameType::Data, position, self.format));
        wire.extend_from_slice(&encode_data(
            &file.bytes[position_usize..end],
            FrameEnd::EndAck,
            self.format,
        )?);
        self.pending_end = end as u32;
        self.state = State::Ack;
        self.transmit(wire, true, events)
    }

    fn retry(&mut self, events: &mut Vec<Event>) -> Result<(), Error> {
        self.budget.retry()?;
        self.decoder.expect_header();
        events.push(Event::Retry { position: None });
        self.budget.send(self.last_wire.clone(), events)
    }

    fn control(
        &mut self,
        kind: FrameType,
        position: u32,
        remember: bool,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        self.transmit(
            encode_header(Header::new(kind, position, HeaderFormat::Hex)),
            remember,
            events,
        )
    }

    fn transmit(
        &mut self,
        wire: Vec<u8>,
        remember: bool,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        if remember {
            self.last_wire.clone_from(&wire);
        }
        self.budget.send(wire, events)
    }
}
