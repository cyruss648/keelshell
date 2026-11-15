use super::codec::{Decoder, Token};
use super::{
    Batch, Budget, CANCEL, Error, Event, FileOffer, FrameEnd, FrameType, Header, HeaderFormat,
    MAX_METADATA_BYTES, TX_PACKET_BYTES, check_events, encode_header, metadata,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    New,
    Ready,
    Review,
    Receiving,
    Saving,
    Finishing,
    Done,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy)]
enum Payload {
    Metadata,
    SenderInit,
    File,
}

/// Bounded in-memory receiver with independent offer and saved-file pauses.
///
/// `start` advertises CRC32 and a 1 KiB receive buffer, without overlapped I/O or
/// full-duplex streaming. An offered file receives no ZRPOS until `accept`.
/// After ZEOF verification, no ZRINIT is sent until `acknowledge_saved`.
/// The caller must hold the raw PTY lease throughout both pauses.
pub struct Receiver {
    state: State,
    decoder: Decoder,
    payload: Option<Payload>,
    offer: Option<FileOffer>,
    bytes: Vec<u8>,
    last_saved: Option<FileOffer>,
    last_skipped: Option<FileOffer>,
    batch: Batch,
    budget: Budget,
}

impl Default for Receiver {
    fn default() -> Self {
        Self::new()
    }
}

impl Receiver {
    /// Construct an inactive receiver with no transport or file authority.
    pub fn new() -> Self {
        Self {
            state: State::New,
            decoder: Decoder::default(),
            payload: None,
            offer: None,
            bytes: Vec::new(),
            last_saved: None,
            last_skipped: None,
            batch: Batch::default(),
            budget: Budget::default(),
        }
    }

    /// Begin negotiation; write the returned ZRINIT before feeding peer input.
    pub fn start(&mut self) -> Result<Vec<Event>, Error> {
        if self.state != State::New {
            return Err(Error::WrongState("receiver already started"));
        }
        self.state = State::Ready;
        let mut events = Vec::new();
        self.init(&mut events)?;
        Ok(events)
    }

    /// Feed one bounded raw chunk; partial headers and subpackets are retained.
    ///
    /// Corrupt headers/data cause a bounded NAK/ZRPOS recovery event, never a
    /// completed file. Unauthorized data or budget/semantic failures fail closed.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Event>, Error> {
        match self.state {
            State::New | State::Failed => return Err(Error::WrongState("receiver inactive")),
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
                Err(Error::CorruptHeader | Error::CorruptData | Error::InvalidEscape) => {
                    self.recover(&mut events)?;
                    continue;
                }
                Err(error) => return Err(error),
            };
            if let Some(token) = token {
                self.budget.packet()?;
                match token {
                    Token::Header(header) => self.header(header, &mut events)?,
                    Token::Data(bytes, end) => self.data(bytes, end, &mut events)?,
                    Token::Cancelled => {
                        self.state = State::Cancelled;
                        self.bytes.clear();
                        events.push(Event::Cancelled);
                    }
                    Token::OverAndOut => {
                        if self.state != State::Finishing {
                            return Err(Error::WrongState("unsolicited OO"));
                        }
                        self.state = State::Done;
                        events.push(Event::Finished);
                    }
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

    /// Accept only the currently pending, externally reviewed offer.
    ///
    /// Allocates no declared-size buffer up front. Receiving begins at zero;
    /// existing filesystem contents are never used as a verified prefix.
    pub fn accept(&mut self) -> Result<Vec<Event>, Error> {
        if self.state != State::Review || self.offer.is_none() {
            return Err(Error::WrongState("no offer to accept"));
        }
        self.state = State::Receiving;
        self.bytes.clear();
        let mut events = Vec::new();
        self.control(FrameType::Position, 0, &mut events)?;
        Ok(events)
    }

    /// Reject only the pending offer and ask the sender to skip that file.
    ///
    /// The offer still counts against batch budgets, preventing unbounded review
    /// floods. The next distinct offer requires a new independent approval.
    pub fn reject(&mut self) -> Result<Vec<Event>, Error> {
        if self.state != State::Review {
            return Err(Error::WrongState("no offer to reject"));
        }
        let offer = self
            .offer
            .take()
            .ok_or(Error::WrongState("missing receive offer"))?;
        self.state = State::Ready;
        self.decoder.expect_header();
        self.payload = None;
        self.last_skipped = Some(offer.clone());
        let mut events = vec![Event::FileSkipped(offer)];
        self.control(FrameType::Skip, 0, &mut events)?;
        Ok(events)
    }

    /// Acknowledge verified local publication of the last `FileReceived`.
    ///
    /// This is the only path that emits ZRINIT after a received file. If local
    /// publication failed or is uncertain, call `cancel` instead and preserve
    /// the caller's local recovery/uncertainty records.
    pub fn acknowledge_saved(&mut self) -> Result<Vec<Event>, Error> {
        if self.state != State::Saving {
            return Err(Error::WrongState("no verified receive awaiting save"));
        }
        self.last_saved = self.offer.take();
        self.state = State::Ready;
        self.payload = None;
        self.decoder.expect_header();
        let mut events = Vec::new();
        self.init(&mut events)?;
        Ok(events)
    }

    /// Recover from a bounded outer idle timer, without advancing human pauses.
    ///
    /// Each actual retry consumes the cumulative retry budget. The caller owns
    /// real elapsed-time and review deadlines; no time is read in this module.
    pub fn timeout(&mut self) -> Result<Vec<Event>, Error> {
        if matches!(self.state, State::Review | State::Saving) {
            return Ok(Vec::new());
        }
        if matches!(self.state, State::New | State::Failed) {
            return Err(Error::WrongState("receiver inactive"));
        }
        if matches!(self.state, State::Done | State::Cancelled) {
            return Ok(Vec::new());
        }
        let mut events = Vec::new();
        let result = if self.state == State::Finishing {
            self.budget.retry().and_then(|()| {
                events.push(Event::Retry { position: None });
                self.control(FrameType::Finish, 0, &mut events)
            })
        } else {
            self.recover(&mut events)
        };
        if let Err(error) = result {
            self.state = State::Failed;
            return Err(error);
        }
        Ok(events)
    }

    /// Cancel once, emitting the fixed standard CAN/backspace sequence.
    ///
    /// Cancellation is available after an error and never performs cleanup or
    /// claims the peer removed any partial file.
    pub fn cancel(&mut self) -> Result<Vec<Event>, Error> {
        if matches!(self.state, State::Done | State::Cancelled) {
            return Ok(Vec::new());
        }
        self.state = State::Cancelled;
        self.bytes.clear();
        self.offer = None;
        self.payload = None;
        self.decoder.expect_header();
        Ok(vec![Event::Send(CANCEL.to_vec()), Event::Cancelled])
    }

    /// Metadata requiring acceptance, or awaiting local-save acknowledgement.
    pub fn pending_offer(&self) -> Option<&FileOffer> {
        self.offer.as_ref()
    }

    /// Whether only an explicit `accept`/`reject` may authorize progress.
    pub fn is_waiting_for_accept(&self) -> bool {
        self.state == State::Review
    }

    /// Whether complete bytes have been emitted and local save is unacknowledged.
    pub fn is_waiting_for_saved(&self) -> bool {
        self.state == State::Saving
    }

    /// Whether the ZFIN/OO handshake completed successfully.
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
            FrameType::Cancel | FrameType::Abort | FrameType::FileError => {
                self.state = State::Cancelled;
                self.bytes.clear();
                events.push(Event::Cancelled);
            }
            FrameType::RequestInit => {
                if self.state == State::Ready {
                    self.init(events)?;
                }
                // A repeated invitation cannot revoke review/save pauses or
                // discard accepted bytes, even if it was queued before ZRPOS.
            }
            FrameType::Challenge => self.control(FrameType::Ack, header.position, events)?,
            FrameType::File => {
                if self.state == State::Saving {
                    return Err(Error::ApprovalRequired);
                }
                if !matches!(self.state, State::Ready | State::Review | State::Receiving) {
                    return Err(Error::UnexpectedFrame(kind));
                }
                let flags = header.flags();
                if flags[0] != 0 || flags[1] != 0 || !matches!(flags[3], 0 | 1) {
                    return Err(Error::Unsupported(
                        "file conversion/transport/sparse option",
                    ));
                }
                // Management flags cannot grant overwrite/append authority.
                // Every accepted file is a new in-memory object and outer
                // publication is reviewed independently with no-replace.
                self.payload = Some(Payload::Metadata);
                self.decoder.expect_data(header.format);
            }
            FrameType::SenderInit => {
                if self.state != State::Ready {
                    return Err(Error::UnexpectedFrame(kind));
                }
                if header.flags()[3] & 0x80 != 0 {
                    return Err(Error::Unsupported("8-bit escaping"));
                }
                self.payload = Some(Payload::SenderInit);
                self.decoder.expect_data(header.format);
            }
            FrameType::Data => {
                if matches!(self.state, State::Review | State::Saving | State::Ready) {
                    return Err(Error::ApprovalRequired);
                }
                if self.state != State::Receiving {
                    return Err(Error::UnexpectedFrame(kind));
                }
                if header.position as usize != self.bytes.len() {
                    self.recover(events)?;
                } else {
                    self.payload = Some(Payload::File);
                    self.decoder.expect_data(header.format);
                }
            }
            FrameType::EndOfFile => {
                if self.state == State::Saving {
                    let offer = self
                        .offer
                        .as_ref()
                        .ok_or(Error::WrongState("missing save offer"))?;
                    if u64::from(header.position) != offer.size {
                        return Err(Error::InvalidPosition);
                    }
                    // Lost/late EOF cannot emit the file twice or acknowledge
                    // publication before the caller has actually saved it.
                    return Ok(());
                }
                if self.state == State::Ready {
                    if self
                        .last_saved
                        .as_ref()
                        .is_some_and(|offer| offer.size == u64::from(header.position))
                    {
                        self.init(events)?;
                        return Ok(());
                    }
                    return Err(Error::UnexpectedFrame(kind));
                }
                if self.state != State::Receiving {
                    return Err(Error::ApprovalRequired);
                }
                let offer = self
                    .offer
                    .as_ref()
                    .ok_or(Error::WrongState("missing receive offer"))?;
                if u64::from(header.position) != offer.size || self.bytes.len() as u64 != offer.size
                {
                    self.recover(events)?;
                } else {
                    self.state = State::Saving;
                    self.payload = None;
                    events.push(Event::FileReceived {
                        offer: offer.clone(),
                        bytes: std::mem::take(&mut self.bytes),
                    });
                }
            }
            FrameType::Finish => {
                if !matches!(self.state, State::Ready | State::Finishing) {
                    return Err(Error::UnexpectedFrame(kind));
                }
                self.state = State::Finishing;
                self.control(FrameType::Finish, 0, events)?;
                self.decoder.expect_finish();
            }
            FrameType::Nak => self.recover(events)?,
            FrameType::FreeCount if self.state == State::Ready => {
                self.control(FrameType::Ack, 0, events)?;
            }
            _ => return Err(Error::UnexpectedFrame(kind)),
        }
        Ok(())
    }

    fn data(
        &mut self,
        bytes: Vec<u8>,
        end: FrameEnd,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        match self.payload {
            Some(Payload::Metadata) => {
                if end != FrameEnd::EndAck {
                    return Err(Error::InvalidMetadata);
                }
                self.payload = None;
                if bytes.len() > MAX_METADATA_BYTES {
                    return Err(Error::Limit("metadata bytes"));
                }
                let offer = metadata(&bytes)?;
                if matches!(self.state, State::Review | State::Receiving) {
                    if self.offer.as_ref() != Some(&offer) {
                        return Err(Error::ApprovalRequired);
                    }
                    self.budget.retry()?;
                    if self.state == State::Receiving {
                        self.control(FrameType::Position, self.bytes.len() as u32, events)?;
                    }
                } else if self.state == State::Ready {
                    if self.last_skipped.as_ref() == Some(&offer) {
                        self.budget.retry()?;
                        return self.control(FrameType::Skip, 0, events);
                    }
                    self.batch.add(&offer)?;
                    self.last_skipped = None;
                    self.state = State::Review;
                    self.offer = Some(offer.clone());
                    events.push(Event::Offer(offer));
                } else {
                    return Err(Error::ApprovalRequired);
                }
            }
            Some(Payload::SenderInit) => {
                self.payload = None;
                if end != FrameEnd::EndAck
                    || bytes.len() > 32
                    || bytes.last() != Some(&0)
                    || bytes[..bytes.len().saturating_sub(1)].contains(&0)
                {
                    return Err(Error::InvalidMetadata);
                }
                // Attention strings are inert bytes. In particular no Ctrl-C,
                // delay, break, terminal write or shell command is executed.
                self.control(FrameType::Ack, 0, events)?;
            }
            Some(Payload::File) => {
                if self.state != State::Receiving {
                    return Err(Error::ApprovalRequired);
                }
                let offer = self
                    .offer
                    .as_ref()
                    .ok_or(Error::WrongState("missing receive offer"))?;
                let length = self.bytes.len() as u64 + bytes.len() as u64;
                if length > offer.size {
                    return Err(Error::InvalidPosition);
                }
                self.bytes.extend_from_slice(&bytes);
                events.push(Event::Progress {
                    offer: offer.clone(),
                    bytes: length,
                });
                if end.ends_frame() {
                    self.payload = None;
                }
                if end.needs_ack() {
                    self.control(FrameType::Ack, length as u32, events)?;
                }
            }
            None => return Err(Error::WrongState("data without frame")),
        }
        Ok(())
    }

    fn recover(&mut self, events: &mut Vec<Event>) -> Result<(), Error> {
        self.budget.retry()?;
        self.decoder.expect_header();
        self.payload = None;
        let position = (self.state == State::Receiving).then_some(self.bytes.len() as u64);
        events.push(Event::Retry { position });
        if self.state == State::Finishing {
            self.decoder.expect_finish();
            self.control(FrameType::Finish, 0, events)
        } else if let Some(position) = position {
            self.control(FrameType::Position, position as u32, events)
        } else if matches!(self.state, State::Review | State::Saving) {
            // Retransmit requests must not accidentally authorize file data or
            // indicate that outer publication succeeded.
            self.control(FrameType::Nak, 0, events)
        } else {
            self.init(events)
        }
    }

    fn init(&mut self, events: &mut Vec<Event>) -> Result<(), Error> {
        // ZF0 CANFC32|ESCCTL, no CANFDX/CANOVIO; ZP0/1 bound the peer's frame.
        self.control(
            FrameType::ReceiverInit,
            (0x60_u32 << 24) | TX_PACKET_BYTES as u32,
            events,
        )
    }

    fn control(
        &mut self,
        kind: FrameType,
        position: u32,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        self.budget.send(
            encode_header(Header::new(kind, position, HeaderFormat::Hex)),
            events,
        )
    }
}
