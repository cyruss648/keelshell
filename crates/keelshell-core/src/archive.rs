//! Bounded, in-memory archive codecs with portable relative member names.
//!
//! Decoding never extracts files or retains ownership, timestamps, or permissions.
//! ZIP accepts single-disk ZIP32 with stored or deflated members. TAR.GZ accepts
//! regular files, directories, bounded GNU long names, and a strict POSIX PAX
//! metadata subset in one gzip stream; links, sparse files, and other extensions
//! are deliberately unsupported.

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

use flate2::{Compression, Crc, Decompress, FlushDecompress, Status};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, System, ZipArchive, ZipWriter};

/// Maximum encoded archive length: 64 MiB.
pub const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum cumulative regular-file payload: 128 MiB.
pub const MAX_ARCHIVE_EXPANDED_BYTES: u64 = 128 * 1024 * 1024;
/// Maximum explicit members in an archive.
pub const MAX_ARCHIVE_ENTRIES: usize = 2048;
/// Maximum components in a member's relative path.
pub const MAX_ARCHIVE_DEPTH: usize = 32;
/// Maximum UTF-8 bytes in a member's relative path, without a directory suffix.
pub const MAX_ARCHIVE_PATH_BYTES: usize = 4096;
/// Maximum cumulative full encoded member-path bytes, including directory suffixes.
pub const MAX_ARCHIVE_MANIFEST_BYTES: usize = 48 * 1024;

// Bound headers, padding, long-name metadata, and trailing zero blocks as well
// as file payloads, so an empty-file or padding bomb still has a streaming bound.
const MAX_TAR_STREAM_BYTES: u64 = MAX_ARCHIVE_EXPANDED_BYTES
    + MAX_ARCHIVE_ENTRIES as u64 * 2048
    + MAX_ARCHIVE_MANIFEST_BYTES as u64 * 2
    + 1024;
const BLOCK_BYTES: usize = 512;
// PAX is metadata, not file payload. Bound its allocation and aggregate work
// separately without increasing the existing whole-stream allowance.
const MAX_PAX_HEADER_BYTES: u64 = MAX_ARCHIVE_PATH_BYTES as u64 * 2;
const MAX_PAX_METADATA_BYTES: u64 =
    MAX_ARCHIVE_MANIFEST_BYTES as u64 + MAX_ARCHIVE_ENTRIES as u64 * 1024;
const MAX_PAX_HEADERS: usize = MAX_ARCHIVE_ENTRIES * 2;

/// An archive representation supported by the bounded codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    /// A single-disk ZIP32 archive using stored or deflated members.
    Zip,
    /// A TAR archive in one integrity-checked gzip stream.
    TarGz,
}

/// A filesystem object representable by the codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveEntryKind {
    /// A regular file whose bytes are carried by the member.
    File,
    /// An explicit directory with no payload.
    Directory,
}

/// A bounded archive validation or encoding failure.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    /// The encoded archive exceeds [`MAX_ARCHIVE_BYTES`].
    #[error("archive exceeds the encoded byte limit")]
    ArchiveTooLarge,
    /// Declared or actual file payloads exceed [`MAX_ARCHIVE_EXPANDED_BYTES`].
    #[error("archive exceeds the expanded byte limit")]
    ExpandedTooLarge,
    /// The archive exceeds [`MAX_ARCHIVE_ENTRIES`].
    #[error("archive exceeds the entry limit")]
    TooManyEntries,
    /// Full encoded member paths exceed [`MAX_ARCHIVE_MANIFEST_BYTES`].
    #[error("archive exceeds the cumulative manifest byte limit")]
    ManifestTooLarge,
    /// A member name is not valid UTF-8.
    #[error("archive member name is not UTF-8")]
    InvalidUtf8,
    /// A path fails portable relative-path validation.
    #[error("invalid archive member path: {0}")]
    InvalidPath(&'static str),
    /// Names alias, repeat, or place a member below a regular file.
    #[error("archive member paths conflict at {0:?}")]
    ConflictingPath(String),
    /// A member is a link, special object, or unsupported TAR extension.
    #[error("archive contains an unsupported entry type")]
    UnsupportedEntry,
    /// ZIP encryption is never accepted.
    #[error("encrypted ZIP members are unsupported")]
    EncryptedZip,
    /// A compression method or archive extension is unsupported.
    #[error("archive uses an unsupported compression method or extension")]
    UnsupportedFormat,
    /// Headers, payload sizes, checksums, or stream termination are inconsistent.
    #[error("archive integrity check failed: {0}")]
    Corrupt(&'static str),
    /// A memory-backed compressor or reader reported a failure.
    #[error("archive stream failed: {0}")]
    Io(#[from] io::Error),
    /// The ZIP implementation rejected its metadata or output.
    #[error("ZIP codec failed: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// A validated relative path and its regular-file bytes or empty directory.
///
/// Paths use `/` between components and never include a trailing slash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveMember {
    path: String,
    kind: ArchiveEntryKind,
    bytes: Vec<u8>,
}

impl ArchiveMember {
    /// Construct a regular file with a portable relative path and bounded bytes.
    ///
    /// Returns an error for invalid paths or payloads exceeding the expanded bound.
    pub fn new_file(path: impl Into<String>, bytes: Vec<u8>) -> Result<Self, ArchiveError> {
        let path = path.into();
        validate_path(&path)?;
        if bytes.len() as u64 > MAX_ARCHIVE_EXPANDED_BYTES {
            return Err(ArchiveError::ExpandedTooLarge);
        }
        Ok(Self {
            path,
            kind: ArchiveEntryKind::File,
            bytes,
        })
    }

    /// Construct an explicit empty directory with a portable relative path.
    ///
    /// Returns an error for invalid paths, including a trailing `/`. Decoders
    /// remove the single directory suffix used by archive container formats.
    pub fn new_directory(path: impl Into<String>) -> Result<Self, ArchiveError> {
        let path = path.into();
        validate_path(&path)?;
        Ok(Self {
            path,
            kind: ArchiveEntryKind::Directory,
            bytes: Vec::new(),
        })
    }

    /// Return the validated relative path, without a trailing directory slash.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Return whether this member is a regular file or directory.
    pub fn kind(&self) -> ArchiveEntryKind {
        self.kind
    }

    /// Return the exact file payload; directories always return an empty slice.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// An ordered, bounded archive manifest with no conflicting explicit or implicit paths.
///
/// Construction and decoding validate the complete manifest before returning a
/// bundle. This type performs no filesystem I/O and grants no extraction authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveBundle {
    members: Vec<ArchiveMember>,
    expanded_bytes: u64,
}

impl ArchiveBundle {
    /// Validate a complete manifest and its cumulative payload and path bounds.
    ///
    /// Duplicate paths, case aliases (including parents), and file/ancestor
    /// conflicts are rejected regardless of member order. Explicit directories
    /// may name an already implied parent with exactly the same spelling.
    pub fn new(members: Vec<ArchiveMember>) -> Result<Self, ArchiveError> {
        let mut manifest = Manifest::default();
        for member in &members {
            manifest.add(&member.path, member.kind, member.bytes.len() as u64)?;
        }
        Ok(Self {
            members,
            expanded_bytes: manifest.expanded_bytes,
        })
    }

    /// Return explicit members in their original manifest order.
    pub fn members(&self) -> &[ArchiveMember] {
        &self.members
    }

    /// Return the cumulative regular-file payload length.
    pub fn expanded_bytes(&self) -> u64 {
        self.expanded_bytes
    }

    /// Encode the bundle without preserving incoming permissions or ownership.
    ///
    /// Files use mode `0600`, directories `0700`. The output writer enforces the
    /// encoded byte bound during compression, including container metadata.
    pub fn encode(&self, format: ArchiveFormat) -> Result<Vec<u8>, ArchiveError> {
        match format {
            ArchiveFormat::Zip => self.encode_zip(),
            ArchiveFormat::TarGz => self.encode_tar_gz(),
        }
    }

    /// Decode and fully validate an archive without accessing the filesystem.
    ///
    /// Both declared and actual payload sizes are bounded. ZIP validates raw
    /// UTF-8 names, header consistency, CRCs, and exact deflate termination.
    /// TAR validates headers, payloads, padding, two end blocks, and the gzip
    /// trailer. Trailing compressed streams or nonzero TAR tails are rejected.
    /// PAX supports `path`, `size`, and validated but discarded time/owner fields.
    /// Local metadata overrides global defaults, then the ordinary header.
    /// Duplicate keys, deletions, unknown keys, and mixed local extensions fail
    /// closed; overridden ordinary paths and sizes retain their safety checks.
    pub fn decode(format: ArchiveFormat, bytes: &[u8]) -> Result<Self, ArchiveError> {
        if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(ArchiveError::ArchiveTooLarge);
        }
        match format {
            ArchiveFormat::Zip => decode_zip(bytes),
            ArchiveFormat::TarGz => decode_tar_gz(bytes),
        }
    }

    fn encode_zip(&self) -> Result<Vec<u8>, ArchiveError> {
        let mut writer = ZipWriter::new(BoundedWriter::default());
        // Permission bits require a Unix metadata namespace on every creator OS.
        let options = SimpleFileOptions::default().system(System::Unix);
        for member in &self.members {
            match member.kind {
                ArchiveEntryKind::File => {
                    writer.start_file(
                        &member.path,
                        options
                            .compression_method(CompressionMethod::Deflated)
                            .unix_permissions(0o600),
                    )?;
                    writer.write_all(&member.bytes)?;
                }
                ArchiveEntryKind::Directory => writer.add_directory(
                    format!("{}/", member.path),
                    options
                        .compression_method(CompressionMethod::Stored)
                        .unix_permissions(0o700),
                )?,
            }
        }
        Ok(writer.finish()?.0.into_inner())
    }

    fn encode_tar_gz(&self) -> Result<Vec<u8>, ArchiveError> {
        let gzip = flate2::write::GzEncoder::new(BoundedWriter::default(), Compression::default());
        let mut writer = tar::Builder::new(gzip);
        for member in &self.members {
            let mut header = tar::Header::new_gnu();
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_size(member.bytes.len() as u64);
            let path = match member.kind {
                ArchiveEntryKind::File => {
                    header.set_mode(0o600);
                    header.set_entry_type(tar::EntryType::Regular);
                    member.path.clone()
                }
                ArchiveEntryKind::Directory => {
                    header.set_mode(0o700);
                    header.set_entry_type(tar::EntryType::Directory);
                    format!("{}/", member.path)
                }
            };
            if path.len() > 100 {
                let mut long_header = tar::Header::new_gnu();
                long_header.set_entry_type(tar::EntryType::GNULongName);
                long_header.set_mode(0o600);
                long_header.set_size(path.len() as u64 + 1);
                let metadata_name = b"././@LongLink";
                long_header.as_mut_bytes()[..metadata_name.len()].copy_from_slice(metadata_name);
                long_header.set_cksum();
                writer.append(&long_header, path.as_bytes().chain(&b"\0"[..]))?;
                // A truncated original name can end in a separator or dot.
                // Use an independently valid short name for older TAR readers.
                header.set_path(if member.kind == ArchiveEntryKind::Directory {
                    "member/"
                } else {
                    "member"
                })?;
                header.set_cksum();
                writer.append(&header, member.bytes.as_slice())?;
            } else {
                writer.append_data(&mut header, path, member.bytes.as_slice())?;
            }
        }
        Ok(writer.into_inner()?.finish()?.0.into_inner())
    }
}

fn validate_path(path: &str) -> Result<(), ArchiveError> {
    if path.is_empty() || path.len() > MAX_ARCHIVE_PATH_BYTES {
        return Err(ArchiveError::InvalidPath("empty or overlong path"));
    }
    if path.chars().any(|character| {
        character.is_control()
            || matches!(character, '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
    }) {
        return Err(ArchiveError::InvalidPath("nonportable character"));
    }
    let mut depth = 0;
    for component in path.split('/') {
        depth += 1;
        if depth > MAX_ARCHIVE_DEPTH {
            return Err(ArchiveError::InvalidPath("path is too deep"));
        }
        if component.is_empty() || matches!(component, "." | "..") {
            return Err(ArchiveError::InvalidPath("empty or dot component"));
        }
        if component.ends_with(['.', ' ']) {
            return Err(ArchiveError::InvalidPath("trailing dot or space"));
        }
        let stem = component
            .split('.')
            .next()
            .unwrap_or_default()
            .trim_end_matches(['.', ' '])
            .to_uppercase();
        let reserved = matches!(
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
            });
        if reserved {
            return Err(ArchiveError::InvalidPath("reserved Windows device name"));
        }
    }
    Ok(())
}

struct ManifestNode {
    path: String,
    kind: ArchiveEntryKind,
    explicit: bool,
}

#[derive(Default)]
struct Manifest {
    nodes: BTreeMap<String, ManifestNode>,
    entries: usize,
    path_bytes: usize,
    expanded_bytes: u64,
}

impl Manifest {
    fn add(&mut self, path: &str, kind: ArchiveEntryKind, size: u64) -> Result<(), ArchiveError> {
        validate_path(path)?;
        self.entries += 1;
        if self.entries > MAX_ARCHIVE_ENTRIES {
            return Err(ArchiveError::TooManyEntries);
        }
        self.path_bytes += path.len() + usize::from(kind == ArchiveEntryKind::Directory);
        if self.path_bytes > MAX_ARCHIVE_MANIFEST_BYTES {
            return Err(ArchiveError::ManifestTooLarge);
        }
        if kind == ArchiveEntryKind::Directory && size != 0 {
            return Err(ArchiveError::Corrupt("directory has a payload"));
        }
        self.expanded_bytes = self
            .expanded_bytes
            .checked_add(size)
            .filter(|total| *total <= MAX_ARCHIVE_EXPANDED_BYTES)
            .ok_or(ArchiveError::ExpandedTooLarge)?;

        for (index, _) in path
            .match_indices('/')
            .chain(std::iter::once((path.len(), "/")))
        {
            let prefix = &path[..index];
            let terminal = index == path.len();
            // Both directions catch conservative Unicode aliases such as
            // sharp-S/SS, Kelvin-sign/K, and final-sigma/sigma.
            let key: String = prefix
                .chars()
                .flat_map(char::to_uppercase)
                .flat_map(char::to_lowercase)
                .collect();
            let node_kind = if terminal {
                kind
            } else {
                ArchiveEntryKind::Directory
            };
            if let Some(node) = self.nodes.get_mut(&key) {
                if node.path != prefix || node.kind != node_kind || (terminal && node.explicit) {
                    return Err(ArchiveError::ConflictingPath(prefix.to_owned()));
                }
                node.explicit |= terminal;
            } else {
                self.nodes.insert(
                    key,
                    ManifestNode {
                        path: prefix.to_owned(),
                        kind: node_kind,
                        explicit: terminal,
                    },
                );
            }
        }
        Ok(())
    }
}

fn member_path(raw: &[u8], kind: ArchiveEntryKind) -> Result<&str, ArchiveError> {
    let path = std::str::from_utf8(raw).map_err(|_| ArchiveError::InvalidUtf8)?;
    let path = if kind == ArchiveEntryKind::Directory {
        path.strip_suffix('/').unwrap_or(path)
    } else {
        path
    };
    validate_path(path)?;
    Ok(path)
}

// Preflight the physical central directory before ZipArchive can allocate from
// its entry count or collapse duplicate raw names into an indexed lookup.
fn preflight_zip(bytes: &[u8]) -> Result<usize, ArchiveError> {
    let search_start = bytes.len().saturating_sub(22 + u16::MAX as usize);
    let end = (search_start..bytes.len().saturating_sub(21))
        .rev()
        .find(|&offset| {
            bytes.get(offset..offset + 4) == Some(b"PK\x05\x06")
                && le16(bytes, offset + 20)
                    .is_ok_and(|length| offset + 22 + usize::from(length) == bytes.len())
        })
        .ok_or(ArchiveError::Corrupt("missing ZIP end record"))?;
    let entries = usize::from(le16(bytes, end + 10)?);
    if le16(bytes, end + 4)? != 0
        || le16(bytes, end + 6)? != 0
        || usize::from(le16(bytes, end + 8)?) != entries
        || entries == u16::MAX as usize
    {
        return Err(ArchiveError::UnsupportedFormat);
    }
    if entries > MAX_ARCHIVE_ENTRIES {
        return Err(ArchiveError::TooManyEntries);
    }
    let mut offset = le32(bytes, end + 16)? as usize;
    let size = le32(bytes, end + 12)? as usize;
    if offset.checked_add(size) != Some(end) {
        return Err(ArchiveError::Corrupt("ZIP central directory length"));
    }
    let mut manifest = Manifest::default();
    for _ in 0..entries {
        if bytes.get(offset..offset + 4) != Some(b"PK\x01\x02") {
            return Err(ArchiveError::Corrupt("ZIP central header"));
        }
        let flags = le16(bytes, offset + 8)?;
        if flags & (1 | 0x40 | 0x2000) != 0 {
            return Err(ArchiveError::EncryptedZip);
        }
        if flags & !(0x800 | 0x08 | 0x06) != 0
            || !matches!(le16(bytes, offset + 10)?, 0 | 8)
            || le16(bytes, offset + 34)? != 0
        {
            return Err(ArchiveError::UnsupportedFormat);
        }
        let expanded = le32(bytes, offset + 24)?;
        let compressed = le32(bytes, offset + 20)?;
        if compressed as u64 > MAX_ARCHIVE_BYTES {
            return Err(ArchiveError::ArchiveTooLarge);
        }
        let name_length = usize::from(le16(bytes, offset + 28)?);
        let extra_length = usize::from(le16(bytes, offset + 30)?);
        let comment_length = usize::from(le16(bytes, offset + 32)?);
        let name_start = offset + 46;
        let next = name_start + name_length + extra_length + comment_length;
        if next > end {
            return Err(ArchiveError::Corrupt("truncated ZIP central header"));
        }
        let raw = &bytes[name_start..name_start + name_length];
        let attributes = le32(bytes, offset + 38)?;
        let mode_type = (attributes >> 16) & 0o170000;
        if !matches!(mode_type, 0 | 0o100000 | 0o040000) {
            return Err(ArchiveError::UnsupportedEntry);
        }
        let is_directory = raw.ends_with(b"/") || attributes & 0x10 != 0 || mode_type == 0o040000;
        if (mode_type == 0o100000 && is_directory)
            || (mode_type == 0o040000 && !raw.ends_with(b"/"))
        {
            return Err(ArchiveError::Corrupt("inconsistent ZIP directory metadata"));
        }
        let kind = if is_directory {
            ArchiveEntryKind::Directory
        } else {
            ArchiveEntryKind::File
        };
        manifest.add(member_path(raw, kind)?, kind, expanded.into())?;
        let mut extra = name_start + name_length;
        let extra_end = extra + extra_length;
        while extra < extra_end {
            if extra_end - extra < 4 {
                return Err(ArchiveError::Corrupt("ZIP extra field"));
            }
            let tag = le16(bytes, extra)?;
            let length = usize::from(le16(bytes, extra + 2)?);
            extra += 4;
            if extra + length > extra_end {
                return Err(ArchiveError::Corrupt("truncated ZIP extra field"));
            }
            if matches!(tag, 0x0001 | 0x9901) {
                return Err(ArchiveError::UnsupportedFormat);
            }
            extra += length;
        }
        offset = next;
    }
    if offset != end {
        return Err(ArchiveError::Corrupt("unlisted ZIP central records"));
    }
    Ok(entries)
}

fn decode_zip(bytes: &[u8]) -> Result<ArchiveBundle, ArchiveError> {
    let count = preflight_zip(bytes)?;
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() != count {
        return Err(ArchiveError::Corrupt("duplicate ZIP names"));
    }
    archive.validate_header_consistency()?;
    let mut members = Vec::with_capacity(count);
    let mut manifest = Manifest::default();
    for index in 0..count {
        let entry = archive.by_index_raw(index)?;
        if entry.encrypted() {
            return Err(ArchiveError::EncryptedZip);
        }
        let mode = entry.unix_mode().unwrap_or(0) & 0o170000;
        if !matches!(mode, 0 | 0o100000 | 0o040000) {
            return Err(ArchiveError::UnsupportedEntry);
        }
        let kind = if entry.is_dir() || mode == 0o040000 {
            ArchiveEntryKind::Directory
        } else {
            ArchiveEntryKind::File
        };
        let path = member_path(entry.name_raw(), kind)?.to_owned();
        manifest.add(&path, kind, entry.size())?;
        let start = entry
            .data_start()
            .ok_or(ArchiveError::Corrupt("ZIP data offset"))?;
        let end = start
            .checked_add(entry.compressed_size())
            .ok_or(ArchiveError::Corrupt("ZIP compressed length"))?;
        let compressed = bytes
            .get(start as usize..end as usize)
            .ok_or(ArchiveError::Corrupt("truncated ZIP data"))?;
        let payload = match entry.compression() {
            CompressionMethod::Stored => {
                if compressed.len() as u64 != entry.size() {
                    return Err(ArchiveError::Corrupt("stored ZIP length"));
                }
                compressed.to_vec()
            }
            CompressionMethod::Deflated => inflate_zip(compressed, entry.size())?,
            _ => return Err(ArchiveError::UnsupportedFormat),
        };
        let mut crc = Crc::new();
        crc.update(&payload);
        if crc.sum() != entry.crc32() {
            return Err(ArchiveError::Corrupt("ZIP CRC"));
        }
        members.push(ArchiveMember {
            path,
            kind,
            bytes: payload,
        });
    }
    ArchiveBundle::new(members)
}

fn inflate_zip(compressed: &[u8], declared: u64) -> Result<Vec<u8>, ArchiveError> {
    let mut inflater = Decompress::new(false);
    let mut payload = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let before_in = inflater.total_in();
        let before_out = inflater.total_out();
        let status = inflater
            .decompress(
                &compressed[before_in as usize..],
                &mut buffer,
                FlushDecompress::None,
            )
            .map_err(|_| ArchiveError::Corrupt("invalid ZIP deflate"))?;
        let produced = (inflater.total_out() - before_out) as usize;
        if inflater.total_out() > declared || inflater.total_out() > MAX_ARCHIVE_EXPANDED_BYTES {
            return Err(ArchiveError::Corrupt("ZIP exceeds its declared size"));
        }
        payload.extend_from_slice(&buffer[..produced]);
        if status == Status::StreamEnd {
            if inflater.total_in() != compressed.len() as u64 || inflater.total_out() != declared {
                return Err(ArchiveError::Corrupt("ZIP deflate length or trailing data"));
            }
            return Ok(payload);
        }
        if inflater.total_in() == before_in && produced == 0 {
            return Err(ArchiveError::Corrupt("truncated ZIP deflate"));
        }
    }
}

fn decode_tar_gz(bytes: &[u8]) -> Result<ArchiveBundle, ArchiveError> {
    let gzip = flate2::bufread::GzDecoder::new(bytes);
    let mut reader = TarReader {
        inner: gzip,
        remaining: MAX_TAR_STREAM_BYTES,
    };
    let mut members = Vec::new();
    let mut manifest = Manifest::default();
    let mut long_name: Option<Vec<u8>> = None;
    let mut pax_global = PaxMetadata::default();
    let mut pax_local: Option<PaxMetadata> = None;
    let mut pax_budget = PaxBudget::default();
    let mut pending_global = false;
    let mut raw_expanded_bytes = 0u64;
    loop {
        let mut block = [0; BLOCK_BYTES];
        reader.read_exact(&mut block)?;
        if block.iter().all(|byte| *byte == 0) {
            reader.read_exact(&mut block)?;
            if block.iter().any(|byte| *byte != 0)
                || long_name.is_some()
                || pax_local.is_some()
                || pending_global
            {
                return Err(ArchiveError::Corrupt("TAR end blocks or orphan metadata"));
            }
            // Reading through EOF verifies the gzip CRC and size trailer.
            // Remaining TAR record padding is allowed only when it is zero.
            let mut tail_bytes = 0;
            let mut tail = [0; 8192];
            loop {
                let length = reader.read(&mut tail)?;
                if length == 0 {
                    break;
                }
                tail_bytes += length;
                if tail[..length].iter().any(|byte| *byte != 0) {
                    return Err(ArchiveError::Corrupt("nonzero trailing TAR data"));
                }
            }
            if tail_bytes % BLOCK_BYTES != 0 || !reader.inner.into_inner().is_empty() {
                return Err(ArchiveError::Corrupt("trailing or partial TAR.GZ stream"));
            }
            return ArchiveBundle::new(members);
        }
        let mut header = tar::Header::new_old();
        header.as_mut_bytes().copy_from_slice(&block);
        let checksum: u32 = block
            .iter()
            .enumerate()
            .map(|(index, byte)| {
                if (148..156).contains(&index) {
                    32
                } else {
                    u32::from(*byte)
                }
            })
            .sum();
        if header.cksum()? != checksum {
            return Err(ArchiveError::Corrupt("TAR header checksum"));
        }
        if header.as_ustar().is_none()
            && header.as_gnu().is_none()
            && block[257..265].iter().any(|byte| *byte != 0)
        {
            return Err(ArchiveError::UnsupportedFormat);
        }
        if !matches!(
            header.entry_type(),
            tar::EntryType::Regular
                | tar::EntryType::Directory
                | tar::EntryType::GNULongName
                | tar::EntryType::XHeader
                | tar::EntryType::XGlobalHeader
        ) {
            // Reject special semantics before tar::Header::size attempts to
            // interpret GNU sparse layout or another unsupported extension.
            return Err(ArchiveError::UnsupportedEntry);
        }
        let name = tar_path(&header)?;
        let size = tar_size(&block)?;
        if block[157..257].iter().any(|byte| *byte != 0) {
            return Err(ArchiveError::UnsupportedEntry);
        }
        if header.entry_type().is_pax_local_extensions()
            || header.entry_type().is_pax_global_extensions()
        {
            if long_name.is_some() || pax_local.is_some() {
                return Err(ArchiveError::Corrupt("stacked TAR local extensions"));
            }
            // Python and POSIX writers use leading ./ in metadata-only names.
            // These headers never become members; traversal is still forbidden.
            let metadata_name =
                std::str::from_utf8(&name).map_err(|_| ArchiveError::InvalidUtf8)?;
            validate_path(metadata_name.trim_start_matches("./"))?;
            let metadata = pax_budget.read(&mut reader, size)?;
            if header.entry_type().is_pax_global_extensions() {
                pax_global.update(metadata);
                pending_global = true;
            } else {
                pax_local = Some(metadata);
            }
            continue;
        }
        if header.entry_type() == tar::EntryType::GNULongName {
            if long_name.is_some()
                || pax_local.is_some()
                || pax_global.path.is_some()
                || size == 0
                || size > MAX_ARCHIVE_PATH_BYTES as u64 + 2
            {
                return Err(ArchiveError::Corrupt("invalid TAR long-name metadata"));
            }
            if name != b"././@LongLink" {
                return Err(ArchiveError::Corrupt("unexpected TAR long-name header"));
            }
            let mut raw = read_tar_payload(&mut reader, size)?;
            if raw.pop() != Some(0) || raw.contains(&0) {
                return Err(ArchiveError::Corrupt("TAR long-name terminator"));
            }
            std::str::from_utf8(&raw).map_err(|_| ArchiveError::InvalidUtf8)?;
            long_name = Some(raw);
            continue;
        }
        let kind = match header.entry_type() {
            tar::EntryType::Regular => ArchiveEntryKind::File,
            tar::EntryType::Directory => ArchiveEntryKind::Directory,
            _ => return Err(ArchiveError::UnsupportedEntry),
        };
        // Even an overridden short name must not carry a dangerous alternate
        // path for consumers that do not understand extended metadata.
        member_path(&name, kind)?;
        if kind == ArchiveEntryKind::Directory && size != 0 {
            return Err(ArchiveError::Corrupt("directory has a payload"));
        }
        raw_expanded_bytes = raw_expanded_bytes
            .checked_add(size)
            .filter(|total| *total <= MAX_ARCHIVE_EXPANDED_BYTES)
            .ok_or(ArchiveError::ExpandedTooLarge)?;
        let local = pax_local.take().unwrap_or_default();
        let size = local.size.or(pax_global.size).unwrap_or(size);
        let raw = local
            .path
            .or_else(|| pax_global.path.clone())
            .or_else(|| long_name.take())
            .unwrap_or(name);
        let path = member_path(&raw, kind)?.to_owned();
        manifest.add(&path, kind, size)?;
        let payload = read_tar_payload(&mut reader, size)?;
        pending_global = false;
        members.push(ArchiveMember {
            path,
            kind,
            bytes: payload,
        });
    }
}

#[derive(Default)]
struct PaxBudget {
    headers: usize,
    bytes: u64,
}

impl PaxBudget {
    fn read(&mut self, reader: &mut impl Read, size: u64) -> Result<PaxMetadata, ArchiveError> {
        if size == 0 || size > MAX_PAX_HEADER_BYTES {
            return Err(ArchiveError::Corrupt("invalid PAX metadata size"));
        }
        self.headers += 1;
        self.bytes += size;
        if self.headers > MAX_PAX_HEADERS || self.bytes > MAX_PAX_METADATA_BYTES {
            return Err(ArchiveError::Corrupt("PAX metadata exceeds its allowance"));
        }
        PaxMetadata::parse(&read_tar_payload(reader, size)?)
    }
}

#[derive(Default)]
struct PaxMetadata {
    path: Option<Vec<u8>>,
    size: Option<u64>,
}

impl PaxMetadata {
    fn parse(mut bytes: &[u8]) -> Result<Self, ArchiveError> {
        let mut metadata = Self::default();
        let mut seen = 0u16;
        while !bytes.is_empty() {
            let space = bytes
                .iter()
                .position(|byte| *byte == b' ')
                .ok_or(ArchiveError::Corrupt("missing PAX record length"))?;
            let digits = &bytes[..space];
            if digits.first() == Some(&b'0') {
                return Err(ArchiveError::Corrupt("noncanonical PAX record length"));
            }
            let length = usize::try_from(pax_unsigned(digits)?)
                .map_err(|_| ArchiveError::Corrupt("PAX record length overflow"))?;
            let record = bytes
                .get(..length)
                .filter(|record| record.len() >= 5 && record.last() == Some(&b'\n'))
                .ok_or(ArchiveError::Corrupt(
                    "invalid PAX record length or newline",
                ))?;
            let body = record
                .get(space + 1..length - 1)
                .ok_or(ArchiveError::Corrupt("invalid PAX record framing"))?;
            let equals = body
                .iter()
                .position(|byte| *byte == b'=')
                .ok_or(ArchiveError::Corrupt("missing PAX record assignment"))?;
            let key = &body[..equals];
            let bit = match key {
                b"path" => 1,
                b"size" => 2,
                b"mtime" => 4,
                b"atime" => 8,
                b"ctime" => 16,
                b"uid" => 32,
                b"gid" => 64,
                b"uname" => 128,
                b"gname" => 256,
                // Unknown records can redefine encoding or content (notably
                // sparse maps). Ignoring them would change archive semantics.
                _ => return Err(ArchiveError::UnsupportedEntry),
            };
            if seen & bit != 0 {
                return Err(ArchiveError::Corrupt("duplicate PAX keyword"));
            }
            seen |= bit;
            let value = &body[equals + 1..];
            let text = std::str::from_utf8(value).map_err(|_| ArchiveError::InvalidUtf8)?;
            if text.is_empty() {
                // POSIX empty values delete attributes, including inherited
                // ones. This slice deliberately does not implement deletion.
                return Err(ArchiveError::UnsupportedEntry);
            }
            match key {
                b"path" => {
                    // The effective kind is only known at the ordinary header.
                    // Allow one directory suffix here, then revalidate by kind.
                    member_path(value, ArchiveEntryKind::Directory)?;
                    metadata.path = Some(value.to_vec());
                }
                b"size" => {
                    let size = pax_unsigned(value)?;
                    if size > MAX_ARCHIVE_EXPANDED_BYTES {
                        return Err(ArchiveError::ExpandedTooLarge);
                    }
                    metadata.size = Some(size);
                }
                b"uid" | b"gid" => {
                    pax_unsigned(value)?;
                }
                b"mtime" | b"atime" | b"ctime" => validate_pax_time(text)?,
                _ => {
                    if text.chars().any(char::is_control) {
                        return Err(ArchiveError::Corrupt("invalid PAX owner name"));
                    }
                }
            }
            bytes = &bytes[length..];
        }
        Ok(metadata)
    }

    fn update(&mut self, newer: Self) {
        // Global updates persist; missing keys retain their previous defaults.
        // Duplicate keys inside any one header are rejected before this merge.
        if newer.path.is_some() {
            self.path = newer.path;
        }
        if newer.size.is_some() {
            self.size = newer.size;
        }
    }
}

fn pax_unsigned(bytes: &[u8]) -> Result<u64, ArchiveError> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return Err(ArchiveError::Corrupt("invalid PAX unsigned decimal"));
    }
    bytes.iter().try_fold(0u64, |number, digit| {
        number
            .checked_mul(10)
            .and_then(|number| number.checked_add(u64::from(*digit - b'0')))
            .ok_or(ArchiveError::Corrupt("PAX unsigned decimal overflow"))
    })
}

fn validate_pax_time(value: &str) -> Result<(), ArchiveError> {
    let (seconds, fraction) = value
        .split_once('.')
        .map_or((value, None), |(seconds, fraction)| {
            (seconds, Some(fraction))
        });
    let digits = seconds.strip_prefix('-').unwrap_or(seconds).as_bytes();
    if digits.is_empty()
        || !digits.iter().all(u8::is_ascii_digit)
        || seconds.parse::<i64>().is_err()
        || fraction.is_some_and(|fraction| {
            fraction.is_empty() || !fraction.as_bytes().iter().all(u8::is_ascii_digit)
        })
    {
        return Err(ArchiveError::Corrupt("invalid PAX timestamp"));
    }
    Ok(())
}

fn tar_path(header: &tar::Header) -> Result<Vec<u8>, ArchiveError> {
    let block = header.as_bytes();
    let name = tar_string(&block[..100])?;
    let mut result = Vec::new();
    if header.as_ustar().is_some() {
        let prefix = tar_string(&block[345..500])?;
        if !prefix.is_empty() {
            result.extend_from_slice(prefix);
            result.push(b'/');
        }
    }
    result.extend_from_slice(name);
    Ok(result)
}

fn tar_string(field: &[u8]) -> Result<&[u8], ArchiveError> {
    let length = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    if field[length..].iter().any(|byte| *byte != 0) {
        return Err(ArchiveError::Corrupt(
            "nonzero bytes after TAR path terminator",
        ));
    }
    Ok(&field[..length])
}

fn tar_size(block: &[u8; BLOCK_BYTES]) -> Result<u64, ArchiveError> {
    let field = &block[124..136];
    let invalid = || ArchiveError::Corrupt("invalid TAR size encoding");
    if field[0] & 0x80 != 0 {
        // The dependency retains only the low 64 bits of a 12-byte binary
        // field. Validate its full width first: positive u64 values have just
        // the marker bit in the first four bytes; negatives and overflow fail.
        if field[..4] != [0x80, 0, 0, 0] {
            return Err(invalid());
        }
        return Ok(field[4..]
            .iter()
            .fold(0u64, |value, byte| (value << 8) | u64::from(*byte)));
    }

    let mut value = 0u64;
    let mut seen_digit = false;
    let mut padding = false;
    for byte in field {
        match byte {
            b'0'..=b'7' if !padding => {
                seen_digit = true;
                value = value
                    .checked_mul(8)
                    .and_then(|value| value.checked_add(u64::from(*byte - b'0')))
                    .ok_or_else(invalid)?;
            }
            b' ' if !seen_digit => {}
            0 | b' ' if seen_digit => padding = true,
            _ => return Err(invalid()),
        }
    }
    if !seen_digit {
        return Err(invalid());
    }
    Ok(value)
}

fn read_tar_payload(reader: &mut impl Read, size: u64) -> Result<Vec<u8>, ArchiveError> {
    let mut bytes = Vec::new();
    let mut remaining = size;
    let mut buffer = [0; 8192];
    while remaining != 0 {
        let length = remaining.min(buffer.len() as u64) as usize;
        reader.read_exact(&mut buffer[..length])?;
        bytes.extend_from_slice(&buffer[..length]);
        remaining -= length as u64;
    }
    let padding = ((BLOCK_BYTES as u64 - size % BLOCK_BYTES as u64) % BLOCK_BYTES as u64) as usize;
    let mut block = [0; BLOCK_BYTES];
    reader.read_exact(&mut block[..padding])?;
    if block[..padding].iter().any(|byte| *byte != 0) {
        return Err(ArchiveError::Corrupt("nonzero TAR member padding"));
    }
    Ok(bytes)
}

struct TarReader<R> {
    inner: R,
    remaining: u64,
}

impl<R: Read> Read for TarReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut probe = [0];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => Err(io::Error::other(
                    "TAR stream exceeds its bounded metadata allowance",
                )),
            };
        }
        let length = self.remaining.min(buffer.len() as u64) as usize;
        let count = self.inner.read(&mut buffer[..length])?;
        self.remaining -= count as u64;
        Ok(count)
    }
}

#[derive(Default)]
struct BoundedWriter(Cursor<Vec<u8>>);

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.position().saturating_add(bytes.len() as u64) > MAX_ARCHIVE_BYTES {
            return Err(io::Error::other(ArchiveError::ArchiveTooLarge));
        }
        self.0.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl Seek for BoundedWriter {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let before = self.0.position();
        let after = self.0.seek(position)?;
        if after > MAX_ARCHIVE_BYTES {
            self.0.set_position(before);
            return Err(io::Error::other(ArchiveError::ArchiveTooLarge));
        }
        Ok(after)
    }
}

fn le16(bytes: &[u8], offset: usize) -> Result<u16, ArchiveError> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or(ArchiveError::Corrupt("truncated ZIP integer"))?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn le32(bytes: &[u8], offset: usize) -> Result<u32, ArchiveError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(ArchiveError::Corrupt("truncated ZIP integer"))?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}
