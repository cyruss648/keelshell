# Credential vault hardening — 2026-10-03

This supersedes the authentication and concurrent-save limitations of the
[initial credential vault record](2026-10-03-credential-vault.md). The public API
remains create/unlock/get/set/remove/to_bytes, with `VaultStore::save` now taking
`&mut CredentialVault` to advance its opaque snapshot after replacement.

## Changes

- Schema 2 authenticates the complete ordered manifest with an independent
  XChaCha20-Poly1305 tag, including an empty vault. Unlock fails before returning
  a mutable vault for an incorrect key or modified manifest.
- Per-entry authentication binds reference, connection and credential role;
  updating an existing reference cannot silently change its binding.
- Fixed, validated Argon2id costs bound KDF work. Zeroizing allocations cover
  derived keys, caller-owned KDF memory, in-place AEAD buffers and returned
  plaintext. Cryptographic library zeroization features are enabled.
- Allocation-free serialization counting enforces the aggregate 4 MiB budget
  during insertion and replacement, with rollback on failure.
- Authenticated loads establish per-store snapshots. Save compares both disk
  bytes and snapshot tokens under the cooperative lock. Wrong-password loads
  discard save authority. Lock symlinks and unsuitable parents are rejected.
- Writes keep the original file intact on pre-replacement failures. Directory
  sync failures after replacement retain their explicit durability error.

## Verification

The first targeted run passed 19 vault tests. After adding the aggregate-budget
case, the complete core suite passed all 20 vault tests and existing core tests.
Clippy initially rejected four `expect` calls in the test fixture; the fixture
was changed to return a propagated error, and the strict Clippy rerun passed.
Original logs were retained in ignored `work/`. The final rerun passed **71 core
unit/integration tests, 0 failed, 0 ignored**, including **20 vault tests** in
23.68 seconds. Final evidence is `work/vault-hardening-core-gate-2.log` and
`work/vault-hardening-clippy-2.log`. Formatting and `git diff --check` passed.

```sh
cargo test -p keelshell-core --locked -- --test-threads=2
cargo clippy -p keelshell-core --all-targets --locked -- -D warnings
```

The vault regression suite covers:

- Correct-key round trips for both roles, fresh sealing nonces, and redacted Debug.
- Incorrect passwords for empty and nonempty vaults.
- Entry deletion, emptying, insertion, replacement, reordering and metadata changes.
- KDF salt changes, unsupported work costs, malformed legacy schema and size bounds.
- Missing/mismatched/copied references and attempts to rebind an existing reference.
- Invalid secrets and aggregate-limit rollback of both insertions and replacements.
- Successive atomic saves, two independent writers, two snapshots from one store,
  save without load, foreign snapshots and failed-load save authority revocation.
- Changed, deleted and malformed disk files remaining untouched after rejection.
- Cooperative lock contention, symlinked data/lock/parent paths and Unix permissions.
- Connection export excluding local credential references.

## Proof boundaries

These are core API tests against isolated temporary files on macOS. The fixtures
contain synthetic secrets only. They do not establish Windows ACL enforcement,
production credential migration, UI behavior or independent cryptographic audit.
Schema 1 is rejected rather than silently migrated. The store assumes a trusted
configuration directory and cannot detect replacement with a whole older valid
vault after restarting. See [ADR 0006](../../adr/0006-authenticated-credential-vault.md).
