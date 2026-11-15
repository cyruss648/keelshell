# Windows security author handoff — 2026-11-15

Status: `IMPLEMENTED / NOT_COMPILED / WINDOWS_NATIVE_NOT_RUN / NON_AUTHOR_REVIEW_PENDING`.

This subtask wrote only:

- `crates/keelshell-platform/src/windows/security.rs`
- `crates/keelshell-platform/src/windows/security/tests.rs`
- `work/windows-local-backend-20261115/security-author/`

The parent owns crate Cargo/mod declarations, native filesystem handles,
component-relative NtCreateFile, metadata, rename and session integration.
No Cargo, rustc, rustfmt, Clippy or Rust test process was started. No target
directory, Git index, commit, branch, container or other author's code was changed.
The actual host reports Darwin. Source and static symbol checks are not Windows
execution or platform acceptance.

## Interfaces

```rust
pub(super) struct PrivateSecurity;

impl PrivateSecurity {
    pub(super) fn new(directory: bool) -> std::io::Result<Self>;
    pub(super) fn descriptor(&self) -> *mut std::ffi::c_void;
}

pub(super) fn verify_private(
    file: &std::fs::File,
    directory: bool,
) -> std::io::Result<()>;

pub(super) fn observation(
    file: &std::fs::File,
) -> std::io::Result<Vec<u8>>;
```

The declaration above is an interface sketch; the real struct owns allocations.
`descriptor()` borrows an absolute SECURITY_DESCRIPTOR for one synchronous
NtCreateFile call. Retain PrivateSecurity through the call; do not mutate,
release, cache or use its raw pointer after drop. Its allocations are stable
across moves, and no Send/Sync implementation is supplied. Construct it inside
the same blocking worker that creates the object.

The parent must declare the Windows module with its narrowly scoped
`#[allow(unsafe_code)]` under this crate's `unsafe_code = "deny"` policy;
existing workspace crates retain their forbid policy. The security module does
not weaken any lint itself. Make this module Windows-only in the parent.

Open every handle passed to `verify_private`/`observation` with READ_CONTROL.
These functions query owner/group/DACL through the actual file handle and do
not read file content, repair permissions, reopen a pathname or request SACL.
An observation is an instantaneous check; the parent still verifies object
identity, links, reparse status, metadata and subsequent changes.

## Parent dependency configuration

Use a direct requirement `windows-sys = "0.61"`; retain the patch in Cargo.lock.
The docs.rs latest page was actually retrieved with HTTP 200 and title
`windows-sys 0.61.2 - Docs.rs`; its timestamp, byte count and SHA-256 are in
`sources/windows-sys-latest.json`. Do not interpret that as a check of every
workspace dependency.

Required features for this security module and its tests:

```text
Win32_Foundation
Win32_Security
Win32_Security_Authorization
Win32_Storage_FileSystem
Win32_System_Memory
Win32_System_SystemServices
Win32_System_Threading
```

Native test fixtures use `tempfile.workspace = true` as a dev dependency.
The parent may need additional WDK/file features for its own implementation.

## Security behavior

`PrivateSecurity::new` queries the current **process** token with TOKEN_QUERY,
reads TokenUser into aligned LocalAlloc storage, validates the SID extent, and
sets that SID explicitly as owner. It builds one ACCESS_ALLOWED_ACE granting
FILE_ALL_ACCESS, with OI/CI for directories and zero flags for regular files.
Its explicit DACL is protected before the creation descriptor is exposed.
Neither the token default DACL nor a parent DACL is copied. All token handles
use OwnedHandle; all LocalAlloc/GetSecurityInfo buffers use a LocalFree guard.
Each unsafe block documents handle/memory ownership and the retained lifetime.

`verify_private` independently calls GetSecurityInfo for the supplied File
handle. It requires the readback's owner to equal a fresh process TokenUser,
requires SE_DACL_PROTECTED, and requires exactly one owner allow ACE with the
exact FILE_ALL_ACCESS mask and exact expected inheritance flags. Missing/null/
empty/multiple-entry DACLs, changed trustee, denied/unknown ACEs, partial rights
and unexpected inheritance flags fail closed. Native errors preserve their
Windows error code. Structural failures use InvalidData; policy failures use
PermissionDenied with a typed SecurityViolation reason containing no SID.

The queried descriptor must be revision 1 and self-relative. Its total extent
is capped at 128 KiB. Owner/group SID headers and full lengths are bounded and
validated before copying. ACL headers, revision, count, used/free lengths, ACE
positions/sizes and complete byte ranges are checked. Opaque ACE bytes may be
observed; a private verification only accepts the one known allow-entry form.

`observation` makes one GetSecurityInfo query for OWNER | GROUP | DACL and copies
bounded values while that returned descriptor remains alive. Encoding version 1:

1. Magic/version `KSSD` + byte `1`.
2. Little-endian u16 control bits, excluding SACL-related bits, RM-control-valid
   and SE_SELF_RELATIVE. Unknown remaining discretionary-control bits are kept.
3. Owner SID and group SID, each prefixed with a little-endian u16 byte length.
4. DACL tag: 0 absent, 1 null, 2 present.
5. For a present DACL: u8 ACL revision, u16 ACE count, then every ACE's live bytes
   in order, each prefixed with its u16 length.

No absolute pointer, native structure padding, ACL free area or SACL is encoded.
The output is capped at 128 KiB and is for internal equality checks only.
Never log or expose this buffer or account SID bytes. The APIs do not decode
these bytes into names or printable SID strings.

## Tests and outstanding validation

There are **27 named tests**, all compiled only for Windows test builds.
`test-inventory.json` contains every exact name and source digest. Results are
`NOT_RUN`, not passed or skipped. Parent should list the final compiled test
inventory, run the relevant Windows target on real Windows, and arrange a new
non-author review after integration.

The first 17 tests cover native creation and actual handle-based queries:

- Private file verification before its first content write and after dropping
  the descriptor owner; directory OI/CI and protected readback.
- Descriptor/SID/ACL ownership after moving the wrapper and dropping it.
- Private child creation below a real broad inheritable parent DACL.
- Actual SetSecurityInfo mutations: extra Everyone, single Everyone, null,
  empty, unprotected, reduced rights, deny ACE, and missing directory flags.
- Requeries and duplicated handles; an actual handle lacking READ_CONTROL.
- Actual DACL rights/protection changes, plus rename/path replacement while the
  original File handle retains its own observation.

The remaining tests explicitly exercise parser/encoding policy, mostly starting
with a real native descriptor snapshot: owner mismatch, unknown ACE type,
inherited/inherit-only flags, truncated/extended entry, owner/group changes,
absent/null/empty distinction, SID bounds, output bounds, omitted SACL/control
representation bits, and ACL free-space independence. Synthetic snapshot
mutations do **not** establish successful OS-level owner/group reassignment or
unknown-ACE installation. No privileged fixture or real customer file is used.

`static-check-r2.json` records a source-only import lookup: 52 production and 20
test import symbols were found in local official windows-sys 0.61.2 bindings;
no prohibited production panic/unwrap/expect or unsafe Send/Sync pattern was
found. The first lookup used an incorrect registry subdirectory and exited 1;
it was corrected without a production change. `static-check-final.json` binds
the final inputs, including the explicit self-relative readback check.
Neither lookup is a compiler, lint, parser or native test result.

No subagent/chat was created by this subtask. Source-fetch subprocesses were
actually waited/reaped, and there are no owned running sessions left.

## Official references and fetched receipts

All links below were retrieved directly from official sources with HTTP 200.
Corresponding raw HTML, extracted reading text where applicable, and JSON
timestamp/response/digest receipts are retained in `sources/`. No source was
substituted by a third-party tutorial.

- [windows-sys latest on docs.rs](https://docs.rs/crate/windows-sys/latest)
- [GetSecurityInfo](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-getsecurityinfo):
  File-handle query, READ_CONTROL requirement, borrowed component pointers and
  LocalFree ownership.
- [Absolute and Self-Relative Security Descriptors](https://learn.microsoft.com/en-us/windows/win32/secauthz/absolute-and-self-relative-security-descriptors):
  Returned descriptors are self-relative except MakeAbsoluteSD; components of
  a self-relative descriptor share one allocation.
- [GetSecurityDescriptorLength](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-getsecuritydescriptorlength):
  Structurally valid descriptor extent includes associated structures.
- [GetTokenInformation](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-gettokeninformation):
  Buffer size query and TokenUser read.
- [AddAccessAllowedAceEx](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-addaccessallowedaceex):
  Explicit mask, SID, revision and inheritance flags.
- [SetSecurityDescriptorControl](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-setsecuritydescriptorcontrol):
  Creation-time inheritance protection.
- [SetSecurityDescriptorOwner](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-setsecuritydescriptorowner):
  Explicit owner and retained SID buffer.
- [GetSecurityDescriptorDacl](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-getsecuritydescriptordacl):
  Missing, null and empty DACL have distinct meanings.
- [LocalAlloc](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-localalloc):
  Fixed/zeroed aligned allocation and LocalFree pairing.
- [GetAce](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-getace):
  ACE pointer is borrowed from the retained ACL.
- [ACL structure](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-acl):
  Header/count/size and DWORD alignment.
- [ACCESS_ALLOWED_ACE](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-access_allowed_ace):
  SID starts at SidStart and extends beyond the fixed structure.
