# Credential vault maintenance — 2026-10-03

Scope: authenticated metadata inspection, unused-entry deletion, master-password
rotation, and an independent native maintenance panel. All fixtures use isolated
local configuration files and synthetic secrets. No external service is used.

## Core validation

```sh
cargo test -p keelshell-core --lib --test vault --locked -- --test-threads=4
cargo clippy -p keelshell-core --all-targets --locked -- -D warnings
```

Both commands passed. The test invocation ran **44 tests**: 11 library unit tests
and 33 vault integration tests; 35 are vault-specific. Thirteen new integration tests
and two private unit tests cover:

- A frozen schema 2 file remains readable; the new AI role uses code 3 without
  changing prior SSH role codes.
- Authenticated metadata enumeration preserves identity and manifest order;
  AI entry ownership/role mismatches and metadata tampering are rejected.
- Successful rotation retains IDs, ownership, roles and plaintext values while
  changing the salt and entry nonces. The old password fails after persisted
  rotation, and the new password supports reload and subsequent saves.
- Empty-vault rotation, invalid new passwords and an injected mid-entry decrypt
  failure preserve the documented all-or-nothing in-memory behavior.
- Original save snapshots remain valid across rotation, but stale writers are
  rejected in both directions for shared and separately constructed stores.
  A lock-busy error permits a later retry after the lock is released.
- `load_existing` authenticates an existing file but never creates an absent
  vault. External deletion and wrong-password failure revoke the save baseline;
  old snapshots cannot recreate a deleted file or gain save authority by restoring
  the same old bytes. Missing paths create neither directories nor lock files.

## Native panel handler validation

```sh
cargo test -p keelshell-app vault_settings:: --locked
cargo clippy -p keelshell-app --all-targets --locked -- -D warnings
```

The final focused run passed **8 tests** (six GPUI and two worker tests). Strict
all-target app Clippy passed. The GPUI fixtures use production buttons and handlers
with real encrypted files:

1. Inspect metadata without displaying secrets; linked entries cannot be deleted.
   Selecting an unlinked entry does not mutate disk. Explicit confirmation and
   renewed authentication delete it while preserving the linked entry. Status
   supports Chinese/English, and Lock removes displayed metadata.
2. Mismatched new password confirmation does not start work or change disk. A
   successful rotation through the panel preserves references and enables only
   the new password for subsequent file loads; inputs are cleared.
3. An incorrect master password exposes no entries. If another saved state links
   a previously unlinked entry after inspection, deletion is rejected after the
   disk reference refresh and both encrypted entries survive.
4. Close after submitting a real rotation requests cancellation but does not emit
   Close until the worker returns. A controlled occupied blocking worker makes
   cancellation precede save admission deterministically. The fixture checks
   cleared master input, unchanged vault bytes and deferred completion.
5. Repeated inspection reflects a newly saved external reference and later removes
   it after that reference is unlinked, while retaining the frozen workspace refs.
6. Production completion callbacks for rotation success, deletion success,
   uncertain directory-sync durability and cancellation carry the final bilingual
   status in the busy Close event. Ordinary idle Close carries no replacement
   message. This prevents closure from hiding which master password took effect.

Two worker tests additionally cover all reference locations (active and deleted
SSH; AI bearer/header authentication, custom headers and proxy credentials), plus
pre-cancelled operations performing no I/O and inspection never creating a missing
vault directory. Environment references do not become vault references.

## Safety and proof boundaries

The application performs KDF and file I/O outside the GPUI event loop. It retains
only authenticated metadata between operations. Busy cancellation can prevent a
save before admission; an admitted save may finish and is not claimed rolled back.
The modal waits for completion before emitting Close. Profile input widgets do not
provide a guaranteed complete memory-erasure boundary.

Deletion checks the current saved state immediately before saving. This is not a
cross-process transaction across state and vault files; another process can still
save references afterward, and unsaved external drafts are not observable. Saved
configuration names are display hints; authenticated ownership is the UUID and role.

The GPUI tests establish handler behavior and actual local encrypted persistence,
not screenshot quality, physical keyboard/pointer acceptance, Windows/Linux native
UI behavior, independent cryptographic audit or production-service interoperability.
The root integration task owns whole-workspace gates and native acceptance.


## Review and repaired evidence

Independent review identified and closed three P2 issues: historical disk refs
were accumulating across inspections; a separate existence precheck could race
with vault deletion and fall into create-on-missing load semantics; and busy Close
could hide a mutation/durability result. The final implementation separates the
reference baseline, uses `load_existing`, and forwards the completion message.

The first expanded GPUI run exposed two fixture errors: directly awaiting a Tokio
handle in the GPUI test scheduler, and reusing a stale StateStore snapshot for the
second simulated external write. The corrected fixture uses a bounded channel
barrier plus GPUI polling, and reloads before that second state save. The failed
log is preserved in ignored `work/vault-maintenance-ui-tests-pre-fix.log`; final
results are in `work/vault-maintenance-ui-tests.log`. Core final logs are in
`work/vault-maintenance-core/load-existing-tests.log` and
`work/vault-maintenance-core/load-existing-clippy.log`; final app Clippy is in
`work/vault-maintenance-app-clippy.log`.
