//! Bounded, synchronous ZMODEM over an already established raw byte stream.
//!
//! This module has no filesystem, shell, timer, or transport authority. Feed
//! received PTY bytes to [`Receiver`] or [`Sender`] and write only [`Event::Send`]
//! back to that same leased stream, in event order. The caller owns human
//! review, transport exclusivity, deadlines, cancellation and local publication.
//! A receive offer never authorizes its data: call [`Receiver::accept`] first.
//! After [`Event::FileReceived`], publish and verify the bytes before calling
//! [`Receiver::acknowledge_saved`]. No subsequent file is authorized by either
//! operation.
//!
//! Implements the 1988 public protocol, not a port of an implementation:
//! <https://pauillac.inria.fr/~doligez/zmodem/zmodem.txt>
//! (sections 7–12, 16-bit and 32-bit CRC, binary/hex framing and transactions).
//! A second rendering of the same primary specification is
//! <https://techheap.packetizer.com/communications/modems/zmodem8.html>.
//! No GPL implementation source was used.
//!
//! Deliberate negotiated limits: ordinary binary files, no compression,
//! encryption, sparse writes, text conversion, remote commands or attention
//! string execution. The sender uses bounded stop-and-wait frames; the receiver
//! advertises a 1 KiB buffer and supports bounded streaming subpackets too.
//! CRC detects link corruption; it does not authenticate a hostile peer.
//! Receiving lengths are mandatory and checked at ZEOF. The sender cannot
//! establish the remote peer's destination, overwrite policy or saved contents.
//!
//! Call `timeout()` on an outer, bounded idle timer (normally ten seconds).
//! Human-review and saved-file pauses do not advance on timeout. The caller must
//! impose a separate overall/review deadline, including when no bytes arrive.

mod codec;
mod detector;
mod receiver;
mod sender;

pub use codec::{FrameEnd, FrameType, Header, HeaderFormat, encode_data, encode_header};
pub use detector::{DetectEvent, Detector, Role};
pub use receiver::Receiver;
pub use sender::Sender;

use std::collections::BTreeSet;
use std::fmt;

/// Maximum number of offered files, including explicitly rejected files.
pub const MAX_FILES: usize = 32;
/// Maximum declared and actual bytes in an individual file (64 MiB).
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum sum of declared file sizes, including rejected offers (128 MiB).
pub const MAX_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
/// Maximum bytes in one `feed` call; split larger transport chunks first.
pub const MAX_FEED_BYTES: usize = 64 * 1024;
/// Maximum decoded subpacket; the standard 1 KiB and common 8 KiB peers fit.
pub const MAX_SUBPACKET_BYTES: usize = 8192;
/// Maximum UTF-8 basename bytes.
pub const MAX_NAME_BYTES: usize = 255;
/// Maximum metadata subpacket bytes.
pub const MAX_METADATA_BYTES: usize = 1024;
/// Maximum cumulative malformed packets/timeouts, and maximum position retries
/// without a newly acknowledged byte offset. Wire/packet/elapsed bounds still
/// apply across all forward progress.
pub const MAX_RETRIES: u32 = 64;
/// Maximum bytes in either direction during the whole transfer, including retries.
pub const MAX_WIRE_BYTES: u64 = 512 * 1024 * 1024;
/// Maximum protocol events processed in a transfer, including empty packets.
pub const MAX_PACKETS: u64 = 1_000_000;

const MAX_NOISE_BYTES: usize = 64 * 1024;
const MAX_EVENTS_PER_CALL: usize = 1024;
const TX_PACKET_BYTES: usize = 1024;
const CANCEL: [u8; 18] = [24, 24, 24, 24, 24, 24, 24, 24, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8];

/// A checked, exact offered basename and declared byte length.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileOffer {
    /// Portable UTF-8 basename, never a path.
    pub name: String,
    /// Required declared byte length; verified before `FileReceived`.
    pub size: u64,
}

/// Immutable file contents supplied only after the caller's human review.
///
/// Construction checks the basename and per-file budget. [`Sender::new`]
/// additionally checks batch count, total size and case aliases. No mutable
/// access to the stored contents is exposed.
#[derive(Clone, Debug)]
pub struct SendFile {
    offer: FileOffer,
    bytes: Vec<u8>,
}

impl SendFile {
    /// Validate and take ownership of a reviewed ordinary file's bytes.
    ///
    /// The caller must establish that the source is an ordinary file and that
    /// these are precisely the bytes approved for the currently leased peer.
    pub fn new(name: impl Into<String>, bytes: Vec<u8>) -> Result<Self, Error> {
        let name = name.into();
        validate_basename(&name)?;
        let size = bytes.len() as u64;
        if size > MAX_FILE_BYTES {
            return Err(Error::Limit("file bytes"));
        }
        Ok(Self {
            offer: FileOffer { name, size },
            bytes,
        })
    }

    /// The exact name and size to include in the caller's review.
    pub fn offer(&self) -> &FileOffer {
        &self.offer
    }

    /// The exact immutable, reviewed contents.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Ordered effects produced by a protocol state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    /// Write these protocol bytes to the leased PTY, without interleaving.
    Send(Vec<u8>),
    /// Review this metadata; the receiver is paused before file bytes.
    Offer(FileOffer),
    /// CRC-checked, length-verified contents for reviewed local publication.
    ///
    /// This is not a filesystem success. The receiver stays paused until the
    /// caller verifies publication and calls `acknowledge_saved`.
    FileReceived {
        /// The unchanged accepted offer.
        offer: FileOffer,
        /// Complete file bytes; ownership transfers to the caller.
        bytes: Vec<u8>,
    },
    /// Peer acknowledged ZEOF; this does not verify remote storage or overwrite.
    FileSent(FileOffer),
    /// The receiver or caller explicitly skipped a file.
    FileSkipped(FileOffer),
    /// CRC-checked received or peer-acknowledged sent bytes of the current file.
    Progress {
        /// Exact current metadata.
        offer: FileOffer,
        /// Contiguous byte offset, not a remote publication claim.
        bytes: u64,
    },
    /// A corrupt packet, retry request or idle timeout caused bounded recovery.
    Retry {
        /// Last verified receive offset or the sender's peer-requested offset.
        position: Option<u64>,
    },
    /// Completed the ZFIN/OO handshake.
    Finished,
    /// Local cancellation or peer cancellation; no remote cleanup is inferred.
    Cancelled,
    /// Bytes following session completion, to return to the terminal unchanged.
    Terminal(Vec<u8>),
}

/// Typed protocol or caller-state failure.
///
/// A failed `feed`/`timeout` makes that machine unusable except for `cancel`.
/// Since a `Result::Err` has no events, the caller should call `cancel` to obtain
/// the bounded wire cancellation and then release the PTY lease. Do not replay
/// the failed input or infer that remote files were removed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// A fixed resource budget was exceeded.
    Limit(&'static str),
    /// A name is not a portable ordinary-file basename.
    InvalidName(&'static str),
    /// Two batch names repeat or alias under conservative Unicode case folding.
    NameAlias,
    /// Missing/invalid size, malformed metadata, or nonordinary file mode.
    InvalidMetadata,
    /// Invalid frame CRC.
    CorruptHeader,
    /// Invalid data-subpacket CRC.
    CorruptData,
    /// An invalid ZDLE escape was received.
    InvalidEscape,
    /// A peer requested a remote command; this is always forbidden.
    RemoteCommand,
    /// A protocol option is intentionally unsupported.
    Unsupported(&'static str),
    /// The peer sent a frame incompatible with the current state.
    UnexpectedFrame(FrameType),
    /// File data or a new file was sent across a required human/save pause.
    ApprovalRequired,
    /// A position does not match a contiguous, verified file prefix.
    InvalidPosition,
    /// Too many cumulative retries or corruptions.
    RetryLimit,
    /// The caller used an operation in an incompatible state.
    WrongState(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(limit) => write!(f, "ZMODEM {limit} budget exceeded"),
            Self::InvalidName(reason) => write!(f, "invalid ZMODEM basename: {reason}"),
            Self::NameAlias => f.write_str("ZMODEM filenames repeat or alias"),
            Self::InvalidMetadata => f.write_str("invalid ZMODEM ordinary-file metadata"),
            Self::CorruptHeader => f.write_str("corrupt ZMODEM header"),
            Self::CorruptData => f.write_str("corrupt ZMODEM data subpacket"),
            Self::InvalidEscape => f.write_str("invalid ZMODEM escape"),
            Self::RemoteCommand => f.write_str("ZMODEM remote command execution is forbidden"),
            Self::Unsupported(option) => write!(f, "unsupported ZMODEM {option}"),
            Self::UnexpectedFrame(kind) => write!(f, "unexpected ZMODEM {kind:?} frame"),
            Self::ApprovalRequired => f.write_str("ZMODEM requires explicit caller approval"),
            Self::InvalidPosition => f.write_str("invalid ZMODEM file position"),
            Self::RetryLimit => f.write_str("ZMODEM retry budget exhausted"),
            Self::WrongState(reason) => write!(f, "invalid ZMODEM state: {reason}"),
        }
    }
}

impl std::error::Error for Error {}

/// Check a UTF-8 basename without performing filesystem operations.
///
/// Rejects separators, dot traversal, ADS/metacharacters, controls, invisible
/// directional controls, trailing dots/spaces and Windows devices (including
/// superscript device digits). Combining marks, compatibility ideographs and
/// decomposed Hangul Jamo are rejected conservatively to avoid common filesystem
/// normalization aliases. Normal precomposed Unicode and CJK names are allowed.
/// Batch alias checking also folds upper- then lowercase (including sharp-S,
/// final sigma and Kelvin sign). The caller still must publish with no-replace
/// semantics; this validator cannot establish whether an existing name aliases
/// a name on a particular filesystem.
pub fn validate_basename(name: &str) -> Result<(), Error> {
    if name.is_empty() || name.len() > MAX_NAME_BYTES || matches!(name, "." | "..") {
        return Err(Error::InvalidName("empty, dot, or overlong name"));
    }
    if name.starts_with(' ') || name.ends_with([' ', '.']) {
        return Err(Error::InvalidName("leading space or trailing dot/space"));
    }
    if name.chars().any(|c| {
        c.is_control()
            || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
            || matches!(c as u32,
                0x0300..=0x036f | 0x1100..=0x11ff | 0x1ab0..=0x1aff |
                0x1dc0..=0x1dff | 0x200b..=0x200f | 0x202a..=0x202e |
                0x2060..=0x206f | 0x20d0..=0x20ff | 0xa960..=0xa97f |
                0xd7b0..=0xd7ff | 0xf900..=0xfaff | 0xfe00..=0xfe0f |
                0xfe20..=0xfe2f | 0xfeff | 0x2f800..=0x2fa1f)
    }) {
        return Err(Error::InvalidName("nonportable character"));
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.'])
        .to_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) || stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    {
        return Err(Error::InvalidName("reserved device"));
    }
    Ok(())
}

#[derive(Default)]
struct Batch {
    names: BTreeSet<String>,
    total: u64,
}

impl Batch {
    fn add(&mut self, offer: &FileOffer) -> Result<(), Error> {
        validate_basename(&offer.name)?;
        if offer.size > MAX_FILE_BYTES {
            return Err(Error::Limit("file bytes"));
        }
        if self.names.len() >= MAX_FILES {
            return Err(Error::Limit("file count"));
        }
        let total = self
            .total
            .checked_add(offer.size)
            .filter(|size| *size <= MAX_TOTAL_BYTES)
            .ok_or(Error::Limit("total file bytes"))?;
        let key = offer
            .name
            .chars()
            .flat_map(char::to_uppercase)
            .flat_map(char::to_lowercase)
            .collect();
        if !self.names.insert(key) {
            return Err(Error::NameAlias);
        }
        self.total = total;
        Ok(())
    }
}

#[derive(Default)]
struct Budget {
    incoming: u64,
    outgoing: u64,
    packets: u64,
    retries: u32,
}

impl Budget {
    fn input(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > MAX_FEED_BYTES {
            return Err(Error::Limit("feed bytes"));
        }
        self.incoming = self
            .incoming
            .checked_add(bytes.len() as u64)
            .filter(|size| *size <= MAX_WIRE_BYTES)
            .ok_or(Error::Limit("incoming wire bytes"))?;
        Ok(())
    }

    fn packet(&mut self) -> Result<(), Error> {
        self.packets += 1;
        if self.packets > MAX_PACKETS {
            return Err(Error::Limit("packets"));
        }
        Ok(())
    }

    fn retry(&mut self) -> Result<(), Error> {
        self.retries += 1;
        if self.retries > MAX_RETRIES {
            return Err(Error::RetryLimit);
        }
        Ok(())
    }

    fn send(&mut self, bytes: Vec<u8>, events: &mut Vec<Event>) -> Result<(), Error> {
        self.outgoing = self
            .outgoing
            .checked_add(bytes.len() as u64)
            .filter(|size| *size <= MAX_WIRE_BYTES)
            .ok_or(Error::Limit("outgoing wire bytes"))?;
        events.push(Event::Send(bytes));
        check_events(events)
    }
}

fn check_events(events: &[Event]) -> Result<(), Error> {
    if events.len() > MAX_EVENTS_PER_CALL {
        Err(Error::Limit("events per call"))
    } else {
        Ok(())
    }
}

fn metadata(bytes: &[u8]) -> Result<FileOffer, Error> {
    if bytes.len() > MAX_METADATA_BYTES {
        return Err(Error::Limit("metadata bytes"));
    }
    let nul = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::InvalidMetadata)?;
    let name = std::str::from_utf8(&bytes[..nul]).map_err(|_| Error::InvalidMetadata)?;
    validate_basename(name)?;
    let rest = &bytes[nul + 1..];
    let end = rest
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(rest.len());
    if rest[end..].iter().any(|byte| *byte != 0) {
        return Err(Error::InvalidMetadata);
    }
    let text = std::str::from_utf8(&rest[..end]).map_err(|_| Error::InvalidMetadata)?;
    if !text.is_ascii() || text.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(Error::InvalidMetadata);
    }
    let fields: Vec<_> = text.split(' ').filter(|field| !field.is_empty()).collect();
    let size_text = fields.first().ok_or(Error::InvalidMetadata)?;
    if !size_text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::InvalidMetadata);
    }
    let size = size_text
        .parse::<u64>()
        .map_err(|_| Error::InvalidMetadata)?;
    if size > MAX_FILE_BYTES {
        return Err(Error::Limit("file bytes"));
    }
    if fields.len() > 6 {
        return Err(Error::InvalidMetadata);
    }
    for (index, field) in fields.iter().enumerate().skip(1) {
        let radix = if index <= 2 { 8 } else { 10 };
        let value = u64::from_str_radix(field, radix).map_err(|_| Error::InvalidMetadata)?;
        if index == 2 {
            let kind = value & 0o170000;
            if value > 0o177777 || (kind != 0 && kind != 0o100000) {
                return Err(Error::InvalidMetadata);
            }
        }
    }
    Ok(FileOffer {
        name: name.to_owned(),
        size,
    })
}
