//! Synchronous NT operations with owned handles and borrowed, bounded buffers.

use std::fs::File;
use std::io;
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::ptr;

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_CREATE, FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
    FILE_RENAME_INFORMATION, FILE_SYNCHRONOUS_IO_NONALERT, FileFsDeviceInformation,
    FileNamesInformation, FileRenameInformation, NtCreateFile, NtQueryDirectoryFile,
    NtQueryVolumeInformationFile, NtSetInformationFile,
};
use windows_sys::Wdk::System::SystemServices::{
    FILE_FS_DEVICE_INFORMATION, FILE_REMOTE_DEVICE, FILE_REMOTE_DEVICE_VSMB,
};
use windows_sys::Win32::Foundation::{
    HANDLE, INVALID_HANDLE_VALUE, NTSTATUS, OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE,
    RtlNtStatusToDosError, UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_BASIC_INFO, FILE_DEVICE_DISK, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_ID_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, FILE_STANDARD_INFO, FILE_TYPE_DISK, FileBasicInfo, FileIdInfo,
    FileStandardInfo, GetDriveTypeW, GetFileInformationByHandleEx, GetFileType, ReOpenFile,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

use super::security::PrivateSecurity;
use super::{FileInfo, Identity, invalid, security};
use crate::directory_records;
use crate::native_diagnostics::{Operation, observe};

const STATUS_SUCCESS: NTSTATUS = 0;
const STATUS_NO_MORE_FILES: NTSTATUS = 0x8000_0006_u32 as i32;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const SHARE_ALL: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;

pub(super) fn open_root(drive: u8) -> io::Result<File> {
    let spelling = [drive as u16, b':' as u16, b'\\' as u16, 0];
    // SAFETY: spelling is a live NUL-terminated three-character drive root.
    let kind = unsafe { GetDriveTypeW(spelling.as_ptr()) };
    if !matches!(kind, DRIVE_FIXED | DRIVE_REMOVABLE) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "local publication requires a local drive",
        ));
    }
    let name = format!(r"\??\{}:\", drive as char);
    open(ptr::null_mut(), &name, true, false, FILE_OPEN, None)
}

pub(super) fn open_child(parent: &File, name: &str, directory: bool) -> io::Result<File> {
    open(
        parent.as_raw_handle(),
        name,
        directory,
        false,
        FILE_OPEN,
        None,
    )
}

pub(super) fn open_private(parent: &File, name: &str, directory: bool) -> io::Result<File> {
    open(
        parent.as_raw_handle(),
        name,
        directory,
        true,
        FILE_OPEN,
        None,
    )
}

pub(super) fn create_private(
    parent: &File,
    name: &str,
    directory: bool,
    security: &PrivateSecurity,
) -> io::Result<File> {
    open(
        parent.as_raw_handle(),
        name,
        directory,
        true,
        FILE_CREATE,
        Some(security),
    )
}

fn open(
    parent: HANDLE,
    name: &str,
    directory: bool,
    writable: bool,
    disposition: u32,
    security: Option<&PrivateSecurity>,
) -> io::Result<File> {
    let mut encoded: Vec<u16> = name.encode_utf16().collect();
    let length = encoded
        .len()
        .checked_mul(2)
        .and_then(|n| u16::try_from(n).ok())
        .ok_or_else(|| invalid("native object name exceeds its bound"))?;
    let object_name = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: encoded.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent,
        ObjectName: &object_name,
        // The root uses the selected DOS drive mapping. Descendants are single
        // names relative to an actual filesystem handle, with reparsing disabled.
        Attributes: OBJ_CASE_INSENSITIVE
            | if parent.is_null() {
                0
            } else {
                OBJ_DONT_REPARSE
            },
        SecurityDescriptor: security
            .map(|value| value.descriptor().cast_const().cast())
            .unwrap_or(ptr::null()),
        SecurityQualityOfService: ptr::null(),
    };
    let mut handle = ptr::null_mut();
    let mut status_block = IO_STATUS_BLOCK::default();
    let access = FILE_GENERIC_READ
        | if writable && !directory {
            FILE_GENERIC_WRITE | DELETE
        } else {
            0
        };
    // SAFETY: every pointer refers to live, correctly aligned storage through
    // this synchronous call. RootDirectory is null or a borrowed live handle.
    // FILE_SYNCHRONOUS_IO_NONALERT and SYNCHRONIZE (in FILE_GENERIC_READ) prevent
    // deferred completion from retaining our stack IOSB or name buffers.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            access,
            &attributes,
            &mut status_block,
            ptr::null(),
            if directory {
                FILE_ATTRIBUTE_DIRECTORY
            } else {
                FILE_ATTRIBUTE_NORMAL
            },
            if disposition == FILE_CREATE && !directory {
                FILE_SHARE_READ
            } else {
                SHARE_ALL
            },
            disposition,
            FILE_SYNCHRONOUS_IO_NONALERT
                | FILE_OPEN_REPARSE_POINT
                | if directory {
                    FILE_DIRECTORY_FILE
                } else {
                    FILE_NON_DIRECTORY_FILE
                },
            ptr::null(),
            0,
        )
    };
    if status < 0 {
        return Err(nt_error(Operation::NtCreateFile, status));
    }
    if handle.is_null() {
        return Err(invalid("native open returned no owned handle"));
    }
    // SAFETY: a successful NtCreateFile transfers this newly owned handle once.
    let file = unsafe { File::from_raw_handle(handle) };
    if status != STATUS_SUCCESS {
        return Err(invalid("native open returned an unconfirmed outcome"));
    }
    if metadata(&file)?.directory != directory {
        return Err(invalid("native local object changed kind"));
    }
    local_device(&file)?;
    Ok(file)
}

fn local_device(file: &File) -> io::Result<()> {
    let mut device = FILE_FS_DEVICE_INFORMATION::default();
    let mut status_block = IO_STATUS_BLOCK::default();
    // SAFETY: this private helper receives only a handle from open(), which sets
    // FILE_SYNCHRONOUS_IO_NONALERT and SYNCHRONIZE. Both initialized buffers
    // remain live through the synchronous query. Check the actual held volume:
    // a prior GetDriveTypeW pathname check cannot exclude drive-mapping races.
    let status = unsafe {
        NtQueryVolumeInformationFile(
            file.as_raw_handle(),
            &mut status_block,
            (&mut device as *mut FILE_FS_DEVICE_INFORMATION).cast(),
            size_of::<FILE_FS_DEVICE_INFORMATION>() as u32,
            FileFsDeviceInformation,
        )
    };
    if status != STATUS_SUCCESS {
        return Err(nt_error(Operation::NtQueryVolume, status));
    }
    if device.DeviceType != FILE_DEVICE_DISK
        || device.Characteristics & (FILE_REMOTE_DEVICE | FILE_REMOTE_DEVICE_VSMB) != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "local publication requires a verified local disk volume",
        ));
    }
    Ok(())
}

fn query<T: Default>(file: &File, class: i32, operation: Operation) -> io::Result<T> {
    let mut output = T::default();
    // SAFETY: private callers pair each information class with its documented
    // repr(C) output structure. The API receives its exact writable byte count.
    let result = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            class,
            (&mut output as *mut T).cast(),
            size_of::<T>() as u32,
        )
    };
    if result == 0 {
        Err(observe(operation, io::Error::last_os_error(), None))
    } else {
        Ok(output)
    }
}

pub(super) fn metadata(file: &File) -> io::Result<FileInfo> {
    // SAFETY: the borrowed File keeps this handle valid for the synchronous call.
    if unsafe { GetFileType(file.as_raw_handle()) } != FILE_TYPE_DISK {
        return Err(observe(
            Operation::CheckDiskFile,
            invalid("local object is not a disk file"),
            None,
        ));
    }
    let basic: FILE_BASIC_INFO = query(file, FileBasicInfo, Operation::QueryBasicInfo)?;
    let standard: FILE_STANDARD_INFO = query(file, FileStandardInfo, Operation::QueryStandardInfo)?;
    let identity: FILE_ID_INFO = query(file, FileIdInfo, Operation::QueryFileId)?;
    if basic.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || standard.DeletePending
        || (!standard.Directory && standard.NumberOfLinks != 1)
        || standard.EndOfFile < 0
    {
        return Err(invalid(
            "local object is a reparse point, link alias or pending deletion",
        ));
    }
    Ok(FileInfo {
        identity: Identity {
            volume: identity.VolumeSerialNumber,
            file: identity.FileId.Identifier,
        },
        bytes: if standard.Directory {
            0
        } else {
            standard.EndOfFile as u64
        },
        directory: standard.Directory,
        links: standard.NumberOfLinks,
        attributes: basic.FileAttributes,
        created: basic.CreationTime,
        modified: basic.LastWriteTime,
        changed: basic.ChangeTime,
        security: security::observation(file)
            .map_err(|error| observe(Operation::ObserveSecurity, error, None))?,
    })
}

pub(super) fn entries(parent: &File) -> io::Result<Vec<String>> {
    // A fresh synchronous file object owns this enumeration cursor. Duplicate
    // handles share directory scan state and would race independent observers.
    let cursor = open_child(parent, ".", true)
        .map_err(|error| observe(Operation::OpenDirectoryCursor, error, None))?;
    let mut storage = vec![0u64; 8192];
    let mut names = Vec::new();
    let mut restart = true;
    loop {
        let mut status_block = IO_STATUS_BLOCK::default();
        // SAFETY: cursor was opened synchronously, storage is aligned, initialized
        // and exactly 64 KiB, and all arguments remain live until completion.
        let status = unsafe {
            NtQueryDirectoryFile(
                cursor.as_raw_handle(),
                ptr::null_mut(),
                None,
                ptr::null(),
                &mut status_block,
                storage.as_mut_ptr().cast(),
                (storage.len() * size_of::<u64>()) as u32,
                FileNamesInformation,
                false,
                ptr::null(),
                restart,
            )
        };
        if status == STATUS_NO_MORE_FILES {
            return Ok(names);
        }
        if status != STATUS_SUCCESS {
            return Err(nt_error(Operation::NtQueryDirectory, status));
        }
        let length = status_block.Information;
        if length == 0 || length > storage.len() * size_of::<u64>() {
            return Err(invalid("native directory response exceeds its buffer"));
        }
        // SAFETY: length is within the initialized storage allocation. Decoding
        // uses checked byte slices rather than casting variable-length records.
        let bytes = unsafe { std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), length) };
        directory_records::decode(bytes, &mut names)?;
        restart = false;
    }
}

pub(super) fn directory_rename_handle(source: &File) -> io::Result<File> {
    let original = metadata(source)?;
    if !original.directory {
        return Err(invalid("directory publication requires a directory"));
    }
    // SAFETY: ReOpenFile addresses the actual retained object, not a pathname.
    // Omitting OVERLAPPED creates a synchronous file object; the borrowed
    // original stays live for the call and the returned handle is adopted once.
    // Only this short-lived source lease requests DELETE. Long-lived directory
    // capabilities must not conflict with Windows' target-directory open, which
    // on supported Windows versions shares READ/WRITE but does not share DELETE.
    let handle = unsafe {
        ReOpenFile(
            source.as_raw_handle(),
            FILE_GENERIC_READ | DELETE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
        )
    };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(observe(
            Operation::ReopenDirectoryRename,
            io::Error::last_os_error(),
            None,
        ));
    }
    // SAFETY: ReOpenFile succeeded with a new, exclusively owned handle.
    let file = unsafe { File::from_raw_handle(handle) };
    local_device(&file)?;
    if metadata(&file)? != original {
        return Err(invalid(
            "directory publication object changed while reopening",
        ));
    }
    security::verify_private(&file, true)?;
    Ok(file)
}

pub(super) fn rename_no_replace(source: &File, parent: &File, name: &str) -> io::Result<()> {
    let name: Vec<u16> = name.encode_utf16().collect();
    // The documented minimum includes the complete fixed structure plus the
    // name bytes, even though FileName starts before the structure's tail pad.
    let required = size_of::<FILE_RENAME_INFORMATION>()
        .checked_add(name.len() * size_of::<u16>())
        .ok_or_else(|| invalid("native rename name exceeds its bound"))?;
    let mut storage = vec![0usize; required.div_ceil(size_of::<usize>())];
    let length = storage.len() * size_of::<usize>();
    let information = storage.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
    // SAFETY: storage has pointer alignment and enough initialized space for the
    // header and exact UTF-16 tail. ReplaceIfExists remains false; RootDirectory
    // is a borrowed actual directory, and name is one previously checked child.
    unsafe {
        (*information).Anonymous.ReplaceIfExists = false;
        (*information).RootDirectory = parent.as_raw_handle();
        (*information).FileNameLength = (name.len() * size_of::<u16>()) as u32;
        ptr::copy_nonoverlapping(
            name.as_ptr(),
            ptr::addr_of_mut!((*information).FileName).cast::<u16>(),
            name.len(),
        );
    }
    let mut status_block = IO_STATUS_BLOCK::default();
    // SAFETY: source is a PrivateEntry created/opened with synchronous native
    // options and DELETE access. The entire IOSB and rename buffer remain live
    // throughout the synchronous namespace operation.
    let status = unsafe {
        NtSetInformationFile(
            source.as_raw_handle(),
            &mut status_block,
            information.cast(),
            length as u32,
            FileRenameInformation,
        )
    };
    if status == STATUS_SUCCESS {
        Ok(())
    } else {
        Err(nt_error(Operation::NtRename, status))
    }
}

fn nt_error(operation: Operation, status: NTSTATUS) -> io::Error {
    // SAFETY: this pure native conversion accepts any NTSTATUS value.
    let code = unsafe { RtlNtStatusToDosError(status) };
    observe(
        operation,
        io::Error::from_raw_os_error(code as i32),
        Some(status as u32),
    )
}
