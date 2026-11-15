//! Creation-time private security descriptors and handle-bound observations.
//!
//! The absolute descriptor owns every referenced allocation. Kernel readback is
//! copied into bounded values before its LocalAlloc buffer is released; neither
//! observations nor errors contain native addresses or printable account SIDs.

use std::ffi::c_void;
use std::fmt;
use std::fs::File;
use std::io;
use std::mem::{offset_of, size_of};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::{self, NonNull};
use std::slice;

use windows_sys::Win32::Foundation::{
    ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_REVISION, ACL_REVISION_DS, ACL_SIZE_INFORMATION,
    AclSizeInformation, AddAccessAllowedAceEx, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION,
    GROUP_SECURITY_INFORMATION, GetAce, GetAclInformation, GetSecurityDescriptorControl,
    GetSecurityDescriptorLength, GetTokenInformation, InitializeAcl, InitializeSecurityDescriptor,
    IsValidAcl, IsValidSid, OBJECT_INHERIT_ACE, OWNER_SECURITY_INFORMATION, PSID, SE_DACL_PRESENT,
    SE_DACL_PROTECTED, SE_RM_CONTROL_VALID, SE_SACL_AUTO_INHERIT_REQ, SE_SACL_AUTO_INHERITED,
    SE_SACL_DEFAULTED, SE_SACL_PRESENT, SE_SACL_PROTECTED, SE_SELF_RELATIVE, SECURITY_DESCRIPTOR,
    SetSecurityDescriptorControl, SetSecurityDescriptorDacl, SetSecurityDescriptorOwner,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows_sys::Win32::System::Memory::{LMEM_FIXED, LMEM_ZEROINIT, LocalAlloc};
use windows_sys::Win32::System::SystemServices::{
    ACCESS_ALLOWED_ACE_TYPE, SECURITY_DESCRIPTOR_REVISION,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const SID_HEADER_BYTES: usize = 8;
const MAX_SID_SUBAUTHORITIES: usize = 15;
const MAX_TOKEN_USER_BYTES: usize = 64 * 1024;
const MAX_DESCRIPTOR_BYTES: usize = 128 * 1024;
const MAX_OBSERVATION_BYTES: usize = 128 * 1024;
// Preserve even unrecognized owner/group/DACL control bits. Only SACL state,
// its resource-manager byte marker, and pointer representation are excluded.
const UNOBSERVED_CONTROL: u16 = SE_SACL_PRESENT
    | SE_SACL_DEFAULTED
    | SE_SACL_AUTO_INHERIT_REQ
    | SE_SACL_AUTO_INHERITED
    | SE_SACL_PROTECTED
    | SE_RM_CONTROL_VALID
    | SE_SELF_RELATIVE;

/// An absolute descriptor whose owner SID and DACL remain live across moves.
///
/// Construct and consume this value on the same blocking worker. No thread
/// safety is asserted for the native pointers.
pub(super) struct PrivateSecurity {
    descriptor: LocalAllocation,
    _owner: CurrentUser,
    _dacl: LocalAllocation,
}

impl PrivateSecurity {
    /// Builds a protected, one-ACE DACL for the current process's TokenUser.
    ///
    /// Directories grant object/container inheritance; regular files grant no
    /// inheritance flags. No ambient token-default or parent DACL is copied.
    ///
    /// # Errors
    ///
    /// Returns native query/allocation errors or invalid-data errors for a
    /// malformed token. No filesystem object is created or modified here.
    pub(super) fn new(directory: bool) -> io::Result<Self> {
        let owner = CurrentUser::read()?;
        let acl_bytes =
            size_of::<ACL>() + offset_of!(ACCESS_ALLOWED_ACE, SidStart) + owner.sid_length;
        let dacl = LocalAllocation::zeroed(acl_bytes)?;
        let dacl_ptr = dacl.as_ptr().cast::<ACL>();
        // SAFETY: LocalAlloc supplies aligned, zeroed storage of acl_bytes;
        // this unique allocation remains owned until after descriptor use.
        check_bool(unsafe { InitializeAcl(dacl_ptr, acl_bytes as u32, ACL_REVISION) })?;
        // SAFETY: The ACL has room for this complete SID and ACE. CurrentUser
        // owns the validated SID buffer; the API copies it into the ACL.
        check_bool(unsafe {
            AddAccessAllowedAceEx(
                dacl_ptr,
                ACL_REVISION,
                inheritance_flags(directory),
                FILE_ALL_ACCESS,
                owner.sid_ptr(),
            )
        })?;

        let descriptor = LocalAllocation::zeroed(size_of::<SECURITY_DESCRIPTOR>())?;
        // SAFETY: This aligned allocation holds a complete absolute descriptor
        // and is exclusively initialized before exposing its borrowed pointer.
        check_bool(unsafe {
            InitializeSecurityDescriptor(descriptor.as_ptr(), SECURITY_DESCRIPTOR_REVISION)
        })?;
        // SAFETY: The initialized absolute descriptor and its owner allocation
        // are retained together in Self. The API borrows, rather than owns, SID.
        check_bool(unsafe { SetSecurityDescriptorOwner(descriptor.as_ptr(), owner.sid_ptr(), 0) })?;
        // SAFETY: Both allocations stay live in Self; the ACL is initialized
        // and is attached as an explicit non-defaulted, non-null DACL.
        check_bool(unsafe { SetSecurityDescriptorDacl(descriptor.as_ptr(), 1, dacl_ptr, 0) })?;
        // SAFETY: The descriptor is initialized and exclusively constructed.
        // Protection applies during creation, before any content can be written.
        check_bool(unsafe {
            SetSecurityDescriptorControl(descriptor.as_ptr(), SE_DACL_PROTECTED, SE_DACL_PROTECTED)
        })?;
        Ok(Self {
            descriptor,
            _owner: owner,
            _dacl: dacl,
        })
    }

    /// Borrows the owned SECURITY_DESCRIPTOR for a synchronous creation call.
    ///
    /// Pass this pointer as `OBJECT_ATTRIBUTES.SecurityDescriptor` and retain
    /// `self` through that call. Do not mutate it, free it, cache it, or retain
    /// any referenced SID/ACL pointer after `self` is dropped.
    pub(super) fn descriptor(&self) -> *mut c_void {
        self.descriptor.as_ptr()
    }
}

/// Verifies the actual opened object's owner and exact protected private DACL.
///
/// The handle must grant READ_CONTROL. A successful check observes security at
/// this instant; callers still bind identity and subsequent mutation checks to
/// the same handle. No path lookup, permission repair or content read occurs.
///
/// # Errors
///
/// Returns permission denied for a non-owner, unprotected/missing/null DACL, or
/// anything except one owner full-control ACE with exactly the expected flags.
/// Native query errors retain their Windows error code; malformed data fails
/// with invalid data.
pub(super) fn verify_private(file: &File, directory: bool) -> io::Result<()> {
    let owner = CurrentUser::read()?;
    let security = SecuritySnapshot::read(file)?;
    security.verify(owner.sid_bytes(), directory)
}

/// Returns a bounded, deterministic owner/group/DACL observation of this handle.
///
/// Encoding version 1 uses little-endian lengths and control bits, complete SID
/// bytes, a DACL-presence tag, its revision/count, and each live ACE's bytes in
/// order. ACL free space, structure padding, absolute pointers, SACL and
/// representation-only control bits are omitted. These bytes are only for
/// equality checks: keep them internal and never print them or account SIDs.
///
/// # Errors
///
/// Requires READ_CONTROL and returns native query or bounded-data errors.
/// Broad/unprotected DACLs remain observable; only `verify_private` requires
/// private permissions.
pub(super) fn observation(file: &File) -> io::Result<Vec<u8>> {
    SecuritySnapshot::read(file)?.encode()
}

struct LocalAllocation(NonNull<c_void>);

impl LocalAllocation {
    fn zeroed(bytes: usize) -> io::Result<Self> {
        // SAFETY: Fixed LocalAlloc storage is suitably aligned; the caller
        // receives unique ownership and Drop always pairs it with LocalFree.
        let pointer = unsafe { LocalAlloc(LMEM_FIXED | LMEM_ZEROINIT, bytes) };
        NonNull::new(pointer)
            .map(Self)
            .ok_or_else(io::Error::last_os_error)
    }

    fn as_ptr(&self) -> *mut c_void {
        self.0.as_ptr()
    }
}

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // SAFETY: This is the unique, still-live allocation returned by
        // LocalAlloc or GetSecurityInfo; no borrowed pointer survives the owner.
        unsafe { LocalFree(self.as_ptr()) };
    }
}

struct CurrentUser {
    allocation: LocalAllocation,
    sid_offset: usize,
    sid_length: usize,
}

impl CurrentUser {
    fn read() -> io::Result<Self> {
        let mut token = ptr::null_mut();
        // SAFETY: The pseudo process handle needs no close; token is a writable
        // out-parameter. Only TOKEN_QUERY is requested, with no impersonation.
        check_bool(unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) })?;
        if token.is_null() || token == INVALID_HANDLE_VALUE {
            return Err(SecurityViolation::MalformedToken.error());
        }
        // SAFETY: OpenProcessToken succeeded with a non-null, valid owned
        // handle. OwnedHandle closes it on every return path exactly once.
        let token = unsafe { OwnedHandle::from_raw_handle(token) };
        let mut required = 0;
        // SAFETY: A null buffer with zero length is the documented size query.
        // The live token and writable length out-parameter outlive the call.
        let result = unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenUser,
                ptr::null_mut(),
                0,
                &mut required,
            )
        };
        let size_error = io::Error::last_os_error();
        if result != 0 {
            return Err(SecurityViolation::MalformedToken.error());
        }
        if size_error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
            return Err(size_error);
        }
        let capacity = required as usize;
        if !(size_of::<TOKEN_USER>()..=MAX_TOKEN_USER_BYTES).contains(&capacity) {
            return Err(SecurityViolation::MalformedToken.error());
        }
        let allocation = LocalAllocation::zeroed(capacity)?;
        let mut written = 0;
        // SAFETY: The fixed buffer has the requested capacity and correct
        // alignment; the handle and both writable outputs outlive this call.
        check_bool(unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenUser,
                allocation.as_ptr(),
                required,
                &mut written,
            )
        })?;
        if written as usize > capacity || (written as usize) < size_of::<TOKEN_USER>() {
            return Err(SecurityViolation::MalformedToken.error());
        }
        // SAFETY: A successful TokenUser query initialized this complete
        // structure in the live buffer. No reference to its padding is formed.
        let token_user = unsafe { ptr::read_unaligned(allocation.as_ptr().cast::<TOKEN_USER>()) };
        let offset = bounded_offset(
            allocation.as_ptr(),
            written as usize,
            token_user.User.Sid,
            SID_HEADER_BYTES,
        )?;
        // SAFETY: The entire allocation was zero-initialized, including any
        // unused bytes, and written was checked against its capacity.
        let bytes =
            unsafe { slice::from_raw_parts(allocation.as_ptr().cast::<u8>(), written as usize) };
        let sid_length = sid_length(&bytes[offset..])?;
        bounded_offset(
            allocation.as_ptr(),
            written as usize,
            token_user.User.Sid,
            sid_length,
        )?;
        // SAFETY: The SID header and full computed length fit inside the live
        // token buffer; IsValidSid cannot run beyond that validated extent.
        if unsafe { IsValidSid(token_user.User.Sid) } == 0 {
            return Err(SecurityViolation::MalformedSid.error());
        }
        Ok(Self {
            allocation,
            sid_offset: offset,
            sid_length,
        })
    }

    fn sid_ptr(&self) -> PSID {
        // SAFETY: read() validated this offset within the still-owned fixed
        // allocation, which remains stable when CurrentUser is moved.
        unsafe {
            self.allocation
                .as_ptr()
                .cast::<u8>()
                .add(self.sid_offset)
                .cast()
        }
    }

    fn sid_bytes(&self) -> &[u8] {
        // SAFETY: read() validated the complete initialized SID extent, and the
        // returned slice cannot outlive this allocation's shared borrow.
        unsafe { slice::from_raw_parts(self.sid_ptr().cast::<u8>(), self.sid_length) }
    }
}

struct SecuritySnapshot {
    control: u16,
    owner: Vec<u8>,
    group: Vec<u8>,
    dacl: Dacl,
}

enum Dacl {
    Absent,
    Null,
    Present { revision: u8, aces: Vec<Vec<u8>> },
}

impl SecuritySnapshot {
    fn read(file: &File) -> io::Result<Self> {
        let mut descriptor = ptr::null_mut();
        let mut owner = ptr::null_mut();
        let mut group = ptr::null_mut();
        let mut dacl = ptr::null_mut();
        // SAFETY: File owns the live handle for this call. All outputs are
        // writable locals. No SACL or privileged access is requested; the
        // returned component pointers borrow the returned descriptor buffer.
        let status = unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                &mut group,
                &mut dacl,
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        // Adopt any API-returned buffer before checking status so error paths
        // release it as well. No pointer is dereferenced on a failed query.
        let allocation = NonNull::new(descriptor).map(LocalAllocation);
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let allocation =
            allocation.ok_or_else(|| SecurityViolation::MalformedDescriptor.error())?;
        // SAFETY: Successful GetSecurityInfo returned an initialized native
        // descriptor whose allocation stays owned through all component reads.
        let bytes = unsafe { GetSecurityDescriptorLength(allocation.as_ptr()) } as usize;
        if !(20..=MAX_DESCRIPTOR_BYTES).contains(&bytes) {
            return Err(SecurityViolation::MalformedDescriptor.error());
        }
        let mut control = 0;
        let mut revision = 0;
        // SAFETY: The descriptor is the successful kernel readback and both
        // scalar out-parameters are initialized, writable local storage.
        check_bool(unsafe {
            GetSecurityDescriptorControl(allocation.as_ptr(), &mut control, &mut revision)
        })?;
        // GetSecurityInfo returns self-relative storage. Requiring that format
        // makes the single-allocation bounds used below an explicit invariant.
        if revision != SECURITY_DESCRIPTOR_REVISION || control & SE_SELF_RELATIVE == 0 {
            return Err(SecurityViolation::MalformedDescriptor.error());
        }
        let owner = read_sid(&allocation, bytes, owner)?;
        let group = read_sid(&allocation, bytes, group)?;
        let dacl = if control & SE_DACL_PRESENT == 0 {
            if !dacl.is_null() {
                return Err(SecurityViolation::MalformedDescriptor.error());
            }
            Dacl::Absent
        } else if dacl.is_null() {
            Dacl::Null
        } else {
            read_acl(&allocation, bytes, dacl)?
        };
        Ok(Self {
            control,
            owner,
            group,
            dacl,
        })
    }

    fn verify(&self, owner: &[u8], directory: bool) -> io::Result<()> {
        if self.owner != owner {
            return Err(SecurityViolation::OwnerMismatch.error());
        }
        if self.control & SE_DACL_PROTECTED == 0 {
            return Err(SecurityViolation::UnprotectedDacl.error());
        }
        let Dacl::Present { aces, .. } = &self.dacl else {
            return Err(SecurityViolation::NonExclusiveDacl.error());
        };
        let [ace] = aces.as_slice() else {
            return Err(SecurityViolation::NonExclusiveDacl.error());
        };
        let sid_offset = offset_of!(ACCESS_ALLOWED_ACE, SidStart);
        if ace.len() < sid_offset
            || u32::from(ace[0]) != ACCESS_ALLOWED_ACE_TYPE
            || u32::from(ace[1]) != inheritance_flags(directory)
            || ace[4..sid_offset] != FILE_ALL_ACCESS.to_le_bytes()
            || ace[sid_offset..] != *owner
        {
            return Err(SecurityViolation::UnexpectedAce.error());
        }
        Ok(())
    }

    fn encode(&self) -> io::Result<Vec<u8>> {
        let mut output = Vec::new();
        output.extend_from_slice(b"KSSD\x01");
        output.extend_from_slice(&(self.control & !UNOBSERVED_CONTROL).to_le_bytes());
        append_field(&mut output, &self.owner)?;
        append_field(&mut output, &self.group)?;
        match &self.dacl {
            Dacl::Absent => output.push(0),
            Dacl::Null => output.push(1),
            Dacl::Present { revision, aces } => {
                output.push(2);
                output.push(*revision);
                let count = u16::try_from(aces.len())
                    .map_err(|_| SecurityViolation::MalformedAcl.error())?;
                output.extend_from_slice(&count.to_le_bytes());
                for ace in aces {
                    append_field(&mut output, ace)?;
                }
            }
        }
        if output.len() > MAX_OBSERVATION_BYTES {
            return Err(SecurityViolation::MalformedDescriptor.error());
        }
        Ok(output)
    }
}

fn read_sid(allocation: &LocalAllocation, bytes: usize, sid: PSID) -> io::Result<Vec<u8>> {
    bounded_offset(allocation.as_ptr(), bytes, sid, SID_HEADER_BYTES)?;
    // SAFETY: This is an owner/group pointer returned by GetSecurityInfo,
    // borrowing initialized SID header bytes inside its still-live allocation.
    let header = unsafe { slice::from_raw_parts(sid.cast::<u8>(), SID_HEADER_BYTES) };
    let length = sid_size_from_header(header)?;
    bounded_offset(allocation.as_ptr(), bytes, sid, length)?;
    // SAFETY: The entire initialized native SID fits within the descriptor.
    if unsafe { IsValidSid(sid) } == 0 {
        return Err(SecurityViolation::MalformedSid.error());
    }
    // SAFETY: This component is initialized by GetSecurityInfo, the complete
    // SID was validated, and bytes are copied before LocalFree.
    Ok(unsafe { slice::from_raw_parts(sid.cast::<u8>(), length) }.to_vec())
}

fn read_acl(allocation: &LocalAllocation, bytes: usize, acl: *mut ACL) -> io::Result<Dacl> {
    bounded_offset(allocation.as_ptr(), bytes, acl.cast(), size_of::<ACL>())?;
    // SAFETY: This complete initialized ACL header is a kernel-returned
    // component within the retained descriptor; unaligned reads avoid aliases.
    let header = unsafe { ptr::read_unaligned(acl) };
    let acl_bytes = usize::from(header.AclSize);
    if acl_bytes < size_of::<ACL>()
        || (u32::from(header.AclRevision) != ACL_REVISION
            && u32::from(header.AclRevision) != ACL_REVISION_DS)
    {
        return Err(SecurityViolation::MalformedAcl.error());
    }
    bounded_offset(allocation.as_ptr(), bytes, acl.cast(), acl_bytes)?;
    // SAFETY: The complete ACL extent is within the descriptor allocation.
    if unsafe { IsValidAcl(acl) } == 0 {
        return Err(SecurityViolation::MalformedAcl.error());
    }
    let mut info = ACL_SIZE_INFORMATION::default();
    // SAFETY: The ACL was validated. info is a correctly sized, writable
    // structure; this API reads ACL metadata without retaining either pointer.
    check_bool(unsafe {
        GetAclInformation(
            acl,
            (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    })?;
    let used = info.AclBytesInUse as usize;
    if info.AceCount != u32::from(header.AceCount)
        || used < size_of::<ACL>()
        || used > acl_bytes
        || info.AclBytesFree as usize != acl_bytes - used
        || info.AceCount as usize > (used - size_of::<ACL>()) / size_of::<ACE_HEADER>()
    {
        return Err(SecurityViolation::MalformedAcl.error());
    }
    let mut aces = Vec::new();
    let mut cursor = size_of::<ACL>();
    for index in 0..info.AceCount {
        let mut ace = ptr::null_mut();
        // SAFETY: This index is below the validated ACL count. ace receives a
        // borrowed component pointer that cannot outlive the owned descriptor.
        check_bool(unsafe { GetAce(acl, index, &mut ace) })?;
        let offset = bounded_offset(acl.cast(), used, ace, size_of::<ACE_HEADER>())?;
        if offset != cursor {
            return Err(SecurityViolation::MalformedAcl.error());
        }
        // SAFETY: The initialized ACE header fits within the live ACL, as
        // checked above. No reference to native structure padding is formed.
        let ace_header = unsafe { ptr::read_unaligned(ace.cast::<ACE_HEADER>()) };
        let length = usize::from(ace_header.AceSize);
        if length < size_of::<ACE_HEADER>() || !length.is_multiple_of(4) {
            return Err(SecurityViolation::MalformedAce.error());
        }
        bounded_offset(acl.cast(), used, ace, length)?;
        // SAFETY: The full live ACE is kernel-returned initialized data within
        // this ACL. Only active ACE bytes are copied, never ACL free space.
        aces.push(unsafe { slice::from_raw_parts(ace.cast::<u8>(), length) }.to_vec());
        cursor += length;
    }
    if cursor != used {
        return Err(SecurityViolation::MalformedAcl.error());
    }
    Ok(Dacl::Present {
        revision: header.AclRevision,
        aces,
    })
}

fn bounded_offset(
    base: *const c_void,
    bytes: usize,
    component: *const c_void,
    length: usize,
) -> io::Result<usize> {
    if component.is_null() {
        return Err(SecurityViolation::MalformedDescriptor.error());
    }
    let offset = (component as usize)
        .checked_sub(base as usize)
        .ok_or_else(|| SecurityViolation::MalformedDescriptor.error())?;
    if offset.checked_add(length).is_none_or(|end| end > bytes) {
        return Err(SecurityViolation::MalformedDescriptor.error());
    }
    Ok(offset)
}

fn sid_size_from_header(bytes: &[u8]) -> io::Result<usize> {
    if bytes.len() < SID_HEADER_BYTES
        || bytes[0] != 1
        || usize::from(bytes[1]) > MAX_SID_SUBAUTHORITIES
    {
        return Err(SecurityViolation::MalformedSid.error());
    }
    Ok(SID_HEADER_BYTES + usize::from(bytes[1]) * size_of::<u32>())
}

fn sid_length(bytes: &[u8]) -> io::Result<usize> {
    let length = sid_size_from_header(bytes)?;
    if length > bytes.len() {
        return Err(SecurityViolation::MalformedSid.error());
    }
    Ok(length)
}

fn append_field(output: &mut Vec<u8>, bytes: &[u8]) -> io::Result<()> {
    let length =
        u16::try_from(bytes.len()).map_err(|_| SecurityViolation::MalformedDescriptor.error())?;
    if output
        .len()
        .checked_add(size_of::<u16>() + bytes.len())
        .is_none_or(|end| end > MAX_OBSERVATION_BYTES)
    {
        return Err(SecurityViolation::MalformedDescriptor.error());
    }
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

fn inheritance_flags(directory: bool) -> u32 {
    if directory {
        OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
    } else {
        0
    }
}

fn check_bool(result: i32) -> io::Result<()> {
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SecurityViolation {
    MalformedToken,
    MalformedDescriptor,
    MalformedSid,
    MalformedAcl,
    MalformedAce,
    OwnerMismatch,
    UnprotectedDacl,
    NonExclusiveDacl,
    UnexpectedAce,
}

impl SecurityViolation {
    fn error(self) -> io::Error {
        let kind = match self {
            Self::OwnerMismatch
            | Self::UnprotectedDacl
            | Self::NonExclusiveDacl
            | Self::UnexpectedAce => io::ErrorKind::PermissionDenied,
            _ => io::ErrorKind::InvalidData,
        };
        io::Error::new(kind, self)
    }
}

impl fmt::Display for SecurityViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MalformedToken => "invalid process TokenUser data",
            Self::MalformedDescriptor => "invalid or oversized file security descriptor",
            Self::MalformedSid => "invalid bounded security identifier",
            Self::MalformedAcl => "invalid bounded file access control list",
            Self::MalformedAce => "invalid bounded access control entry",
            Self::OwnerMismatch => "file owner differs from the current process user",
            Self::UnprotectedDacl => "file DACL is not protected against inheritance",
            Self::NonExclusiveDacl => "file DACL must contain exactly one owner allow entry",
            Self::UnexpectedAce => "file allow entry has unexpected trustee, rights, type or flags",
        })
    }
}

impl std::error::Error for SecurityViolation {}

#[cfg(test)]
mod tests;
