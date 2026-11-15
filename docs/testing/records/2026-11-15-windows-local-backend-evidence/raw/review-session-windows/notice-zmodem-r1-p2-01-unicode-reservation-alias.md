# Zmodem Windows r1 blocking notice: P2 WS-ZM-01

Observed UTC: 2026-11-15T12:50:21.072408+00:00
Review mode: independent read-only source review. Native NOT_RUN. No Cargo, rustc, formatting, Git command, or target access.
Frozen manifest: zmodem-inputs-r1.json, SHA-256 8fb6ea161f2ce4d4ba0d882901ff867b9cf44f1cfcb19138af6d088a9ed01075.
HEAD observed by reading metadata: a092dd5712c0a92ee4af18970fb0500fb230b760; reviewed changes are uncommitted and bound by file hashes.

Conclusion: HOLD for the newly enabled Windows session integration. Local reservation keys use a weaker alias relation than the Windows Zmodem namespace checks, so a second spelling can bypass an active claim or its retained quarantine while the destination remains absent.

Locations:

- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem.rs:1026 (new Windows receive still acquires the shared path-based reservation through this call).
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/zmodem_local.rs:19 (passes each reviewed path to the shared local_claim).
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/queue.rs:151 (canonical existing ancestor and missing basename form the claim; lines 153 and 159 only lowercase).
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/queue.rs:99 (overlap uses component equality/prefix equality).
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem/local_windows.rs:93 (namespace alias uppercases then lowercases; also used by platform require_absent/require_unique_name).

Trigger and source trace:

1. In one existing directory, choose an absent target `STRASSE.bin`. Confirm receive. Its active destination claim ends in `strasse.bin`; the unpredictable staging name is separately claimed.
2. While that target is still absent, another connection/controller chooses `straße.bin`. Both prechecks can succeed because neither destination exists yet. The existing ancestor is canonicalized, but the missing basename is appended and only lowercased, yielding `straße.bin`. The two vectors and the distinct staging claims do not overlap.
3. The second receive is admitted, although the actual Windows local backend deliberately treats both names as one portable alias. The same bypass exists after the first ticket becomes an Unknown record with the original absent destination claim. A sigma/final-sigma pair `σ.bin` / `ς.bin` has the same reservation mismatch.
4. This allows new data acceptance/staging under a destination that should be busy/quarantined. Atomic no-replace and final alias verification can still reject publication; this notice does not claim overwrite, a successful Windows exploit, or release/deletion of the old record.

Expected fix boundary: use one conservative, consistent alias relation for process-wide local admission and quarantine (including missing components), retaining canonical-ancestor handling and all existing Unknown records. Add an integration regression that reserves one spelling, observes DestinationBusy for the other before accept, then retains a real mutated/unknown reservation and observes DestinationQuarantined for the alias. Do not substitute a Python namespace model for a Windows execution result.

Independent evidence is static: direct call and equality analysis of the current source. The current integration test at tests/zmodem.rs:1150 covers identical spellings across connections only. There is no Windows-native execution or dynamic reproduction in this review. This is separate from the two platform review P2s about directory sharing and rename-buffer extent.

Source hashes at notice time:

```json
{
  "crates/keelshell-session/src/zmodem.rs": "b61d348d414995e328ea6844f38d71408dd3251d2b726babe2fcb824abf15bff",
  "crates/keelshell-session/src/zmodem/local_windows.rs": "68ebb6fc05cf61c7b3fd8a64138038c981c03a469d99f7a9c6ca7456ba0f842f",
  "crates/keelshell-session/src/sftp/zmodem_local.rs": "cc58995f4e0eaaaf8d41f366fb6867da92946a1e4f74a6f05de7d9c16be3be30",
  "crates/keelshell-session/src/sftp/queue.rs": "8552e1034cfec8c125d2c27a1ac432516c80e9ed004b44d342c56a29694fc613",
  "crates/keelshell-session/src/sftp/queue/reservations.rs": "1e37150311fcc4d643d2405d7d3f682f99f760f8001bde28f7dce91ad923472c"
}
```
