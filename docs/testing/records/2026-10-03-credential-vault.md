# Credential vault slice — 2026-10-03

Historical checkpoint. Its empty-vault authentication and concurrent-save limits
are superseded by the [vault hardening record](2026-10-03-vault-hardening.md).

This record covers the core encrypted credential store for SSH profiles. It is
separate from `state.json`: a profile can carry only an opaque local
`credential_ref`, while `vault.json` stores password and private-key passphrase
entries encrypted at rest. The format is cross-platform and does not require an
OS keychain.

## Implementation

- `CredentialVault` derives a 256-bit key with fixed Argon2id parameters from a
  user-supplied passphrase. Each entry uses a fresh XChaCha20-Poly1305 nonce.
- Associated data binds the ciphertext to the profile UUID and credential role,
  preventing a password entry from being moved to another profile or used as a
  private-key passphrase.
- Plaintext is accepted only for the duration of `set` and returned from `get`
  as a zeroizing string. The vault key is zeroized when the unlocked vault is
  dropped. `Debug`, JSON export and connection export never include plaintext.
- `VaultStore` writes a separate owner-only file with an atomic replacement and
  durable temporary file. Missing files remain in memory until the first save.
- Connection import/export always clears local vault references; a copied JSON
  document cannot point at another installation's secret.

## Verification

```sh
cargo test -p keelshell-core --locked
```

Result: **56 passed, 0 failed, 0 ignored** across core unit and integration
tests, including five vault tests. They cover encryption round-trip, profile and
role binding, tamper rejection, owner-only permissions, symlink rejection,
atomic save and export reference clearing.

## Boundaries

- The app does not yet prompt for, unlock or save vault entries from the
  connection and AI settings screens. Until that UI integration lands, current
  login and AI flows remain memory-only.
- The vault passphrase is not recoverable. An empty vault cannot distinguish a
  wrong passphrase until an entry is read, so the UI must keep an explicit
  unlock state and provide a clear recovery path.
- This slice provides cooperative file locking for writes but not optimistic
  multi-process merge semantics or OS keychain/secure enclave integration.
