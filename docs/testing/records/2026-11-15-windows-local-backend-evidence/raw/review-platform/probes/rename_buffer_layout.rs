use std::mem::{offset_of, size_of};
use windows_sys::Wdk::Storage::FileSystem::FILE_RENAME_INFORMATION;

pub const PREFIX: usize = offset_of!(FILE_RENAME_INFORMATION, FileName);
pub const HEADER: usize = size_of::<FILE_RENAME_INFORMATION>();
pub const CURRENT_ONE_UNIT_LENGTH: usize = PREFIX + size_of::<u16>();
pub const DOCUMENTED_ONE_UNIT_MINIMUM: usize = HEADER + size_of::<u16>();
const _: [(); 20] = [(); PREFIX];
const _: [(); 24] = [(); HEADER];
const _: [(); 22] = [(); CURRENT_ONE_UNIT_LENGTH];
const _: [(); 26] = [(); DOCUMENTED_ONE_UNIT_MINIMUM];
const _: () = assert!(CURRENT_ONE_UNIT_LENGTH < HEADER);
const _: () = assert!(CURRENT_ONE_UNIT_LENGTH < DOCUMENTED_ONE_UNIT_MINIMUM);
