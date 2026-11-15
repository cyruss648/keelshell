#![cfg(test)]
#![cfg(windows)]

use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Security::Authorization::SetSecurityInfo;
use windows_sys::Win32::Security::{
    AddAccessDeniedAceEx, CreateWellKnownSid, GetLengthSid, INHERIT_ONLY_ACE, INHERITED_ACE,
    PROTECTED_DACL_SECURITY_INFORMATION, SECURITY_ATTRIBUTES,
    UNPROTECTED_DACL_SECURITY_INFORMATION, WELL_KNOWN_SID_TYPE, WinWorldSid,
};
use windows_sys::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateDirectoryW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_GENERIC_READ, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

use super::*;

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn attributes(security: &PrivateSecurity) -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.descriptor(),
        bInheritHandle: 0,
    }
}

fn create_file(path: &Path, security: &PrivateSecurity) -> io::Result<File> {
    let path = wide(path);
    let attributes = attributes(security);
    // SAFETY: The terminated path and attributes remain live through this
    // synchronous call; security owns every referenced descriptor component.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            FILE_ALL_ACCESS,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: CreateFileW returned a new, valid owned handle; File is its sole
    // owner and closes it on all test exit paths.
    Ok(unsafe { File::from_raw_handle(handle) })
}

fn create_directory(path: &Path, security: &PrivateSecurity) -> io::Result<File> {
    let path = wide(path);
    let attributes = attributes(security);
    // SAFETY: Both terminated path and owned creation descriptor outlive this
    // synchronous creation. No content is written before permissions exist.
    check_bool(unsafe { CreateDirectoryW(path.as_ptr(), &attributes) })?;
    // SAFETY: The path is terminated and retained, and opening with backup
    // semantics returns the newly created directory's owned handle.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            FILE_ALL_ACCESS,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: The successful open returned a new, valid owned directory
    // handle; this File takes sole responsibility for closing it.
    Ok(unsafe { File::from_raw_handle(handle) })
}

fn well_known_sid(kind: WELL_KNOWN_SID_TYPE) -> io::Result<LocalAllocation> {
    let size = SID_HEADER_BYTES + MAX_SID_SUBAUTHORITIES * size_of::<u32>();
    let allocation = LocalAllocation::zeroed(size)?;
    let mut length = size as u32;
    // SAFETY: The SID buffer has the documented maximum size and alignment;
    // the API initializes it without retaining either pointer.
    check_bool(unsafe {
        CreateWellKnownSid(kind, ptr::null_mut(), allocation.as_ptr(), &mut length)
    })?;
    Ok(allocation)
}

// Every SID argument comes from a retained CurrentUser or well_known_sid
// allocation. AddAccessAllowedAceEx copies it; the returned ACL owns its bytes.
fn allowed_acl(entries: &[(PSID, u32, u32)]) -> io::Result<LocalAllocation> {
    allowed_acl_with_slack(entries, 0)
}

fn allowed_acl_with_slack(
    entries: &[(PSID, u32, u32)],
    slack: usize,
) -> io::Result<LocalAllocation> {
    let mut bytes = size_of::<ACL>() + slack;
    for (sid, _, _) in entries {
        // SAFETY: Test callers retain valid SID allocations through this call.
        bytes += offset_of!(ACCESS_ALLOWED_ACE, SidStart) + unsafe { GetLengthSid(*sid) } as usize;
    }
    let allocation = LocalAllocation::zeroed(bytes)?;
    let acl = allocation.as_ptr().cast::<ACL>();
    // SAFETY: This aligned allocation has enough space for the header and
    // all complete entries, and remains uniquely owned while initialized.
    check_bool(unsafe { InitializeAcl(acl, bytes as u32, ACL_REVISION) })?;
    for (sid, mask, flags) in entries {
        // SAFETY: The ACL has sufficient remaining space; each SID is valid
        // and live, and its bytes are copied into the uniquely owned ACL.
        check_bool(unsafe { AddAccessAllowedAceEx(acl, ACL_REVISION, *flags, *mask, *sid) })?;
    }
    Ok(allocation)
}

fn set_dacl(file: &File, acl: Option<&LocalAllocation>, protected: bool) -> io::Result<()> {
    let protection = if protected {
        PROTECTED_DACL_SECURITY_INFORMATION
    } else {
        UNPROTECTED_DACL_SECURITY_INFORMATION
    };
    // SAFETY: File owns a handle opened with WRITE_DAC. Any provided ACL is
    // initialized by allowed_acl and remains live until SetSecurityInfo copies
    // it. Null is intentional for the null-DACL rejection test.
    let status = unsafe {
        SetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | protection,
            ptr::null_mut(),
            ptr::null_mut(),
            acl.map_or(ptr::null(), |value| value.as_ptr().cast::<ACL>()),
            ptr::null(),
        )
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

fn assert_violation(result: io::Result<()>, expected: SecurityViolation) {
    let error = match result {
        Ok(()) => panic!("unsafe security configuration was accepted"),
        Err(error) => error,
    };
    assert_eq!(
        error
            .get_ref()
            .and_then(|error| error.downcast_ref::<SecurityViolation>()),
        Some(&expected),
        "security rejection must retain its typed reason"
    );
}

#[test]
fn private_file_is_verified_before_the_first_content_write() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let mut file = create_file(&temporary.path().join("private.bin"), &security)?;
    verify_private(&file, false)?;
    drop(security);
    verify_private(&file, false)?;
    file.write_all(b"public test fixture")?;
    file.sync_all()?;
    verify_private(&file, false)
}

#[test]
fn private_directory_is_protected_with_object_and_container_inheritance() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(true)?;
    let directory = create_directory(&temporary.path().join("private"), &security)?;
    drop(security);
    verify_private(&directory, true)
}

#[test]
fn creation_descriptor_retains_sid_and_acl_ownership_across_moves() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let original = PrivateSecurity::new(false)?;
    let address = original.descriptor();
    let mut values = Vec::with_capacity(1);
    // Fill the original allocation, then force growth so pointer ownership is
    // tested across both the initial move and the vector's reallocation.
    values.extend(std::iter::once(original));
    values.push(PrivateSecurity::new(false)?);
    let moved = values.remove(0);
    assert!(
        moved.descriptor() == address,
        "descriptor storage moved with its owner"
    );
    let file = create_file(&temporary.path().join("moved.bin"), &moved)?;
    drop(values);
    drop(moved);
    verify_private(&file, false)
}

#[test]
fn private_creation_does_not_inherit_a_broad_parent_dacl() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let parent_security = PrivateSecurity::new(true)?;
    let parent_path = temporary.path().join("broad-parent");
    let parent = create_directory(&parent_path, &parent_security)?;
    let owner = CurrentUser::read()?;
    let world = well_known_sid(WinWorldSid)?;
    let acl = allowed_acl(&[
        (owner.sid_ptr(), FILE_ALL_ACCESS, inheritance_flags(true)),
        (world.as_ptr(), FILE_GENERIC_READ, inheritance_flags(true)),
    ])?;
    set_dacl(&parent, Some(&acl), true)?;
    assert_violation(
        verify_private(&parent, true),
        SecurityViolation::NonExclusiveDacl,
    );
    let child_security = PrivateSecurity::new(false)?;
    let child = create_file(&parent_path.join("child.bin"), &child_security)?;
    verify_private(&child, false)
}

#[test]
fn verify_private_rejects_an_additional_everyone_allow_entry() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let file = create_file(&temporary.path().join("wide.bin"), &security)?;
    let owner = CurrentUser::read()?;
    let world = well_known_sid(WinWorldSid)?;
    let acl = allowed_acl(&[
        (owner.sid_ptr(), FILE_ALL_ACCESS, 0),
        (world.as_ptr(), FILE_GENERIC_READ, 0),
    ])?;
    set_dacl(&file, Some(&acl), true)?;
    assert_violation(
        verify_private(&file, false),
        SecurityViolation::NonExclusiveDacl,
    );
    Ok(())
}

#[test]
fn verify_private_rejects_a_single_everyone_allow_entry() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let file = create_file(&temporary.path().join("world.bin"), &security)?;
    let world = well_known_sid(WinWorldSid)?;
    let acl = allowed_acl(&[(world.as_ptr(), FILE_ALL_ACCESS, 0)])?;
    set_dacl(&file, Some(&acl), true)?;
    assert_violation(
        verify_private(&file, false),
        SecurityViolation::UnexpectedAce,
    );
    Ok(())
}

#[test]
fn verify_private_rejects_a_null_dacl() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let file = create_file(&temporary.path().join("null.bin"), &security)?;
    set_dacl(&file, None, true)?;
    assert_violation(
        verify_private(&file, false),
        SecurityViolation::NonExclusiveDacl,
    );
    Ok(())
}

#[test]
fn verify_private_rejects_an_empty_dacl() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let file = create_file(&temporary.path().join("empty.bin"), &security)?;
    let acl = allowed_acl(&[])?;
    set_dacl(&file, Some(&acl), true)?;
    assert_violation(
        verify_private(&file, false),
        SecurityViolation::NonExclusiveDacl,
    );
    Ok(())
}

#[test]
fn verify_private_rejects_an_unprotected_dacl() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let file = create_file(&temporary.path().join("unprotected.bin"), &security)?;
    let owner = CurrentUser::read()?;
    let acl = allowed_acl(&[(owner.sid_ptr(), FILE_ALL_ACCESS, 0)])?;
    set_dacl(&file, Some(&acl), false)?;
    assert_violation(
        verify_private(&file, false),
        SecurityViolation::UnprotectedDacl,
    );
    Ok(())
}

#[test]
fn verify_private_rejects_insufficient_owner_rights() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let file = create_file(&temporary.path().join("read-only.bin"), &security)?;
    let owner = CurrentUser::read()?;
    let acl = allowed_acl(&[(owner.sid_ptr(), FILE_GENERIC_READ, 0)])?;
    set_dacl(&file, Some(&acl), true)?;
    assert_violation(
        verify_private(&file, false),
        SecurityViolation::UnexpectedAce,
    );
    Ok(())
}

#[test]
fn verify_private_rejects_a_real_deny_ace() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let file = create_file(&temporary.path().join("deny.bin"), &security)?;
    let owner = CurrentUser::read()?;
    let bytes = size_of::<ACL>() + offset_of!(ACCESS_ALLOWED_ACE, SidStart) + owner.sid_length;
    let allocation = LocalAllocation::zeroed(bytes)?;
    let acl = allocation.as_ptr().cast::<ACL>();
    // SAFETY: The uniquely owned aligned allocation has room for the ACL and
    // one complete ACE/SID, and the SID remains live through the copy.
    unsafe {
        check_bool(InitializeAcl(acl, bytes as u32, ACL_REVISION))?;
        check_bool(AddAccessDeniedAceEx(
            acl,
            ACL_REVISION,
            0,
            FILE_ALL_ACCESS,
            owner.sid_ptr(),
        ))?;
    }
    set_dacl(&file, Some(&allocation), true)?;
    assert_violation(
        verify_private(&file, false),
        SecurityViolation::UnexpectedAce,
    );
    Ok(())
}

#[test]
fn verify_private_rejects_a_directory_without_inheritance_flags() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(true)?;
    let directory = create_directory(&temporary.path().join("no-inheritance"), &security)?;
    let owner = CurrentUser::read()?;
    let acl = allowed_acl(&[(owner.sid_ptr(), FILE_ALL_ACCESS, 0)])?;
    set_dacl(&directory, Some(&acl), true)?;
    assert_violation(
        verify_private(&directory, true),
        SecurityViolation::UnexpectedAce,
    );
    Ok(())
}

#[test]
fn observation_is_stable_across_requeries_and_duplicated_handles() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let file = create_file(&temporary.path().join("stable.bin"), &security)?;
    let before = observation(&file)?;
    let duplicate = file.try_clone()?;
    assert!(
        before == observation(&file)?,
        "requery changed security observation"
    );
    assert!(
        before == observation(&duplicate)?,
        "handle clone changed security observation"
    );
    assert!(
        before.len() <= MAX_OBSERVATION_BYTES,
        "observation exceeded its bound"
    );
    Ok(())
}

#[test]
fn security_queries_require_read_control_on_the_actual_handle() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let path = temporary.path().join("limited.bin");
    let _file = create_file(&path, &security)?;
    let path = wide(&path);
    // SAFETY: The terminated path stays live. Zero desired access intentionally
    // excludes READ_CONTROL; the successful open returns a new owned handle.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: This is a valid handle newly returned by CreateFileW. File takes
    // sole ownership and closes it even when the expected query fails.
    let limited = unsafe { File::from_raw_handle(handle) };
    for result in [
        observation(&limited).map(|_| ()),
        verify_private(&limited, false),
    ] {
        let error = match result {
            Ok(()) => panic!("READ_CONTROL-free handle queried file security"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            error.raw_os_error().is_some(),
            "native query error code was lost"
        );
    }
    Ok(())
}

#[test]
fn observation_detects_real_dacl_rights_changes() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let file = create_file(&temporary.path().join("rights.bin"), &security)?;
    let before = observation(&file)?;
    let owner = CurrentUser::read()?;
    let acl = allowed_acl(&[(owner.sid_ptr(), FILE_GENERIC_READ, 0)])?;
    set_dacl(&file, Some(&acl), true)?;
    assert!(
        before != observation(&file)?,
        "DACL rights change was not observed"
    );
    Ok(())
}

#[test]
fn observation_detects_real_dacl_protection_changes() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let file = create_file(&temporary.path().join("protection.bin"), &security)?;
    let before = observation(&file)?;
    let owner = CurrentUser::read()?;
    let acl = allowed_acl(&[(owner.sid_ptr(), FILE_ALL_ACCESS, 0)])?;
    set_dacl(&file, Some(&acl), false)?;
    assert!(
        before != observation(&file)?,
        "DACL protection change was not observed"
    );
    Ok(())
}

#[test]
fn observation_remains_bound_to_the_handle_after_path_replacement() -> io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let path = temporary.path().join("name.bin");
    let original = create_file(&path, &security)?;
    let before = observation(&original)?;
    std::fs::rename(&path, temporary.path().join("retained.bin"))?;
    let replacement = create_file(&path, &security)?;
    let owner = CurrentUser::read()?;
    let acl = allowed_acl(&[(owner.sid_ptr(), FILE_GENERIC_READ, 0)])?;
    set_dacl(&replacement, Some(&acl), true)?;
    assert!(
        before == observation(&original)?,
        "original handle observation followed a replacement path"
    );
    assert!(
        before != observation(&replacement)?,
        "replacement rights must differ from the retained original"
    );
    verify_private(&original, false)
}

fn snapshot_with_owner_entry() -> io::Result<(SecuritySnapshot, Vec<u8>)> {
    let temporary = tempfile::tempdir()?;
    let security = PrivateSecurity::new(false)?;
    let file = create_file(&temporary.path().join("snapshot.bin"), &security)?;
    let snapshot = SecuritySnapshot::read(&file)?;
    let owner = CurrentUser::read()?.sid_bytes().to_vec();
    Ok((snapshot, owner))
}

fn only_ace_mut(snapshot: &mut SecuritySnapshot) -> &mut Vec<u8> {
    match &mut snapshot.dacl {
        Dacl::Present { aces, .. } if aces.len() == 1 => &mut aces[0],
        _ => panic!("expected exactly one native owner entry"),
    }
}

#[test]
fn private_snapshot_rejects_an_owner_mismatch() -> io::Result<()> {
    let (mut snapshot, owner) = snapshot_with_owner_entry()?;
    snapshot.owner[7] ^= 1;
    assert_violation(
        snapshot.verify(&owner, false),
        SecurityViolation::OwnerMismatch,
    );
    Ok(())
}

#[test]
fn private_snapshot_rejects_unknown_ace_types() -> io::Result<()> {
    let (mut snapshot, owner) = snapshot_with_owner_entry()?;
    only_ace_mut(&mut snapshot)[0] = 0xff;
    assert_violation(
        snapshot.verify(&owner, false),
        SecurityViolation::UnexpectedAce,
    );
    Ok(())
}

#[test]
fn private_snapshot_rejects_inherited_and_inherit_only_flags() -> io::Result<()> {
    let (mut snapshot, owner) = snapshot_with_owner_entry()?;
    for flag in [INHERITED_ACE, INHERIT_ONLY_ACE] {
        only_ace_mut(&mut snapshot)[1] = flag as u8;
        assert_violation(
            snapshot.verify(&owner, false),
            SecurityViolation::UnexpectedAce,
        );
    }
    Ok(())
}

#[test]
fn private_snapshot_rejects_a_truncated_or_extended_allow_entry() -> io::Result<()> {
    let (mut snapshot, owner) = snapshot_with_owner_entry()?;
    only_ace_mut(&mut snapshot).truncate(4);
    assert_violation(
        snapshot.verify(&owner, false),
        SecurityViolation::UnexpectedAce,
    );
    let (mut snapshot, owner) = snapshot_with_owner_entry()?;
    only_ace_mut(&mut snapshot).extend_from_slice(&[0; 4]);
    assert_violation(
        snapshot.verify(&owner, false),
        SecurityViolation::UnexpectedAce,
    );
    Ok(())
}

#[test]
fn observation_encoding_detects_owner_and_group_sid_changes() -> io::Result<()> {
    let (mut snapshot, _) = snapshot_with_owner_entry()?;
    let before = snapshot.encode()?;
    snapshot.owner[7] ^= 1;
    assert!(
        before != snapshot.encode()?,
        "owner SID change was not observed"
    );
    snapshot.owner[7] ^= 1;
    snapshot.group[7] ^= 1;
    assert!(
        before != snapshot.encode()?,
        "group SID change was not observed"
    );
    Ok(())
}

#[test]
fn observation_encoding_distinguishes_absent_null_and_empty_dacls() -> io::Result<()> {
    let (mut snapshot, _) = snapshot_with_owner_entry()?;
    snapshot.dacl = Dacl::Absent;
    let absent = snapshot.encode()?;
    snapshot.dacl = Dacl::Null;
    let null = snapshot.encode()?;
    snapshot.dacl = Dacl::Present {
        revision: ACL_REVISION as u8,
        aces: Vec::new(),
    };
    let empty = snapshot.encode()?;
    assert!(absent != null && null != empty && absent != empty);
    Ok(())
}

#[test]
fn bounded_sid_parser_rejects_truncation_revision_and_subauthority_overflow() {
    for malformed in [
        vec![],
        vec![1; SID_HEADER_BYTES - 1],
        vec![2, 0, 0, 0, 0, 0, 0, 0],
        vec![1, 16, 0, 0, 0, 0, 0, 0],
        vec![1, 1, 0, 0, 0, 0, 0, 0],
    ] {
        assert_violation(
            sid_length(&malformed).map(|_| ()),
            SecurityViolation::MalformedSid,
        );
    }
}

#[test]
fn observation_encoding_rejects_excessive_entry_bytes() -> io::Result<()> {
    let (mut snapshot, _) = snapshot_with_owner_entry()?;
    only_ace_mut(&mut snapshot).resize(MAX_OBSERVATION_BYTES + 1, 0);
    assert_violation(
        snapshot.encode().map(|_| ()),
        SecurityViolation::MalformedDescriptor,
    );
    Ok(())
}

#[test]
fn observation_excludes_sacl_and_pointer_representation_control_bits() -> io::Result<()> {
    let (mut snapshot, _) = snapshot_with_owner_entry()?;
    let before = snapshot.encode()?;
    snapshot.control ^= UNOBSERVED_CONTROL;
    assert!(
        before == snapshot.encode()?,
        "unrequested SACL state changed the observation"
    );
    // A legacy/unrecognized discretionary-control bit is still compared.
    snapshot.control ^= 0x0040;
    assert!(
        before != snapshot.encode()?,
        "discretionary-control bit was omitted"
    );
    Ok(())
}

#[test]
fn observation_excludes_acl_free_space() -> io::Result<()> {
    let (mut snapshot, _) = snapshot_with_owner_entry()?;
    let owner = CurrentUser::read()?;
    let plain = allowed_acl(&[(owner.sid_ptr(), FILE_ALL_ACCESS, 0)])?;
    // SAFETY: The helper returned a complete initialized ACL header in its
    // uniquely owned, aligned allocation.
    let plain_bytes =
        usize::from(unsafe { ptr::read_unaligned(plain.as_ptr().cast::<ACL>()) }.AclSize);
    snapshot.dacl = read_acl(&plain, plain_bytes, plain.as_ptr().cast())?;
    let before = snapshot.encode()?;

    let slack = 32;
    let padded = allowed_acl_with_slack(&[(owner.sid_ptr(), FILE_ALL_ACCESS, 0)], slack)?;
    // SAFETY: plain_bytes is the exact live header/ACE size; the second helper
    // allocated that size plus slack. Only initialized unused space is changed.
    unsafe {
        ptr::write_bytes(padded.as_ptr().cast::<u8>().add(plain_bytes), 0xa5, slack);
    }
    snapshot.dacl = read_acl(&padded, plain_bytes + slack, padded.as_ptr().cast())?;
    assert!(
        before == snapshot.encode()?,
        "unused ACL bytes changed the observation"
    );
    Ok(())
}
