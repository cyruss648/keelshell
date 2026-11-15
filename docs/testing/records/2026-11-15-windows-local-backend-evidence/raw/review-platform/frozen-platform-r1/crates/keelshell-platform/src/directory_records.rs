//! Decode the bounded FILE_NAMES_INFORMATION chain without typed pointer casts.

use std::io;

pub(crate) const MAX_DIRECTORY_ENTRIES: usize = 16_384;

pub(crate) fn decode(bytes: &[u8], names: &mut Vec<String>) -> io::Result<()> {
    let mut offset = 0usize;
    loop {
        let header = bytes
            .get(offset..offset.saturating_add(12))
            .ok_or_else(invalid)?;
        let next = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let length = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;
        if length == 0 || !length.is_multiple_of(2) || length > 510 {
            return Err(invalid());
        }
        let start = offset.checked_add(12).ok_or_else(invalid)?;
        let end = start.checked_add(length).ok_or_else(invalid)?;
        let encoded = bytes.get(start..end).ok_or_else(invalid)?;
        let encoded: Vec<u16> = encoded
            .as_chunks::<2>()
            .0
            .iter()
            .map(|unit| u16::from_le_bytes(*unit))
            .collect();
        let name = String::from_utf16(&encoded).map_err(|_| invalid())?;
        if name.contains(['\0', '\\', '/']) {
            return Err(invalid());
        }
        if !matches!(name.as_str(), "." | "..") {
            if names.len() == MAX_DIRECTORY_ENTRIES {
                return Err(io::Error::other("directory exceeds its observation bound"));
            }
            names.push(name);
        }
        if next == 0 {
            return Ok(());
        }
        let following = offset.checked_add(next).ok_or_else(invalid)?;
        if following < end || following >= bytes.len() {
            return Err(invalid());
        }
        offset = following;
    }
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid native directory-name response",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(name: &[u16], next: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&next.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u32 * 2).to_le_bytes());
        for unit in name {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn fragmented_query_batches_preserve_unicode_names_and_exclude_dot_entries() -> io::Result<()> {
        let mut names = Vec::new();
        decode(&record(&[46], 0), &mut names)?;
        decode(&record(&[46, 46], 0), &mut names)?;
        decode(
            &record(&"文件🦀.txt".encode_utf16().collect::<Vec<_>>(), 0),
            &mut names,
        )?;
        assert_eq!(names, ["文件🦀.txt"]);
        Ok(())
    }

    #[test]
    fn next_entry_offset_can_include_native_alignment_padding() -> io::Result<()> {
        let mut bytes = record(&[65], 16);
        bytes.resize(16, 0);
        bytes.extend(record(&[66], 0));
        let mut names = Vec::new();
        decode(&bytes, &mut names)?;
        assert_eq!(names, ["A", "B"]);
        Ok(())
    }

    #[test]
    fn truncated_header_and_name_never_yield_partial_success() {
        for bytes in [vec![0; 11], record(&[65], 0)[..13].to_vec()] {
            assert!(decode(&bytes, &mut Vec::new()).is_err());
        }
    }

    #[test]
    fn overlapping_or_out_of_bounds_entry_offsets_are_rejected() {
        for offset in [1, 12, 13, 14, u32::MAX] {
            assert!(decode(&record(&[65], offset), &mut Vec::new()).is_err());
        }
    }

    #[test]
    fn odd_or_oversized_utf16_name_lengths_are_rejected() {
        for length in [0u32, 1, 511, u32::MAX] {
            let mut bytes = record(&[65], 0);
            bytes[8..12].copy_from_slice(&length.to_le_bytes());
            assert!(decode(&bytes, &mut Vec::new()).is_err());
        }
    }

    #[test]
    fn lone_surrogates_and_embedded_path_separators_are_rejected() {
        for units in [vec![0xd800], vec![0], vec![47], vec![92]] {
            assert!(decode(&record(&units, 0), &mut Vec::new()).is_err());
        }
    }

    #[test]
    fn name_budget_is_shared_across_all_native_query_batches() {
        let mut names = vec!["existing".to_owned(); MAX_DIRECTORY_ENTRIES];
        assert!(decode(&record(&[65], 0), &mut names).is_err());
        assert_eq!(names.len(), MAX_DIRECTORY_ENTRIES);
    }
}
