# WS-AR-01 / P2 — cancelled Windows archive result handoff can lose the UI unknown close hold

- Status: OPEN / HOLD, new blocking finding on session combined r1.
- Observed UTC: 2026-11-15T13:14:33.415869+00:00
- Baseline HEAD (metadata-only): `a092dd5712c0a92ee4af18970fb0500fb230b760`; candidate changes are uncommitted.
- Session freeze SHA-256: `33462196a04db7d8e9afe0c555f221dbd9e3f7a0aa76b3b8b1c55a8e32e4c3fa` (32/32 matched at intake).
- Reviewer evidence: STATIC_SOURCE_INTERLEAVING_ONLY. Native NOT_RUN; no Cargo/rustc/format/Git command or target access. This is a session result-accounting finding, distinct from the two platform findings owned by the other reviewer.

## Location and trigger

`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/archive_transfer/local_windows.rs:154` clears `scope.pending` in the blocking worker before its `Result<Staged>` / `Result<UnverifiedStage>` has been delivered to the async owner or dropped by a detached task. The live result owns the armed `UnverifiedStage` (`build_stage` 871–873, `publish_stage` 1045), which only calls the real ticket's `finish(false)` when that result is dropped (`archive_transfer.rs:212`).

A permitted interleaving is:

1. A worker completes `build_stage` (a real private staging tree now exists), or `publish_stage` (publication/readback completed), and obtains `Ok(stage_or_guard)` at 151.
2. It executes `pending.store(false, Release)` at 154 and is descheduled before returning/storing/dropping that result. Its ticket is still active and the armed result is still live on the worker stack.
3. Revocation or timeout drops the await/JoinHandle in `mutation_blocking` 172–175. This detaches the blocking task and requests stop, but does not drop its stack/result synchronously.
4. `execute_archive_transfer` invokes `scope.complete(Err(Closed_or_Timeout))` (`archive_transfer.rs:394`). At `mutation.rs:94–105`, pending is false, ticket is active, and local result handoff is not included in `cleanup_known` (the cleanup counter tracks remote temporary cleanup, `queue/reservations.rs:224–240`). It can return the plain Closed/Timeout error, rather than MutationUncertain.
5. App conversion (`files.rs:740–747`) maps that error to Transport; `archive::retains_close_hold` (`files/archive.rs:33–39`) returns false and `files.rs:1778–1781` restores the previous isolation target, releasing this archive's close hold.
6. When the worker resumes, the unobserved result is dropped and quarantines the real reservation. The reservation clone prevents an admission unlock, but there is no later UI result to reinstall the archive close hold. A user can close the application with this newly unknown staging/publication unguarded.

There is no atomic or synchronization boundary between 154 and the result handoff that rules out this schedule. The existing tests at `local_windows/tests.rs:657–731` wait for eventual quarantine; they do not assert the actual scope completion classification and UI close hold while this handoff is in flight.

## Impact

A cancelled/expired operation that actually created or published files can be reported as an ordinary transport failure and lose the archive-specific close protection, even though its unobserved result must remain unknown/quarantined. This does not claim an immediate process-wide reservation bypass or a successful receipt: the live ticket/guard keeps admission excluded, then quarantines. The defect is the caller/UI's incorrect known-versus-unknown classification during result ownership transfer and its persistent loss of the close hold.

## Suggested correction and meaningful regression

Keep a synchronous handoff/mutation obligation visible to `FileMutationScope::complete` until the async owner acknowledges the result, or ensure a dropped await marks the reservation unknown before `complete` can classify the operation as known. A result-owned guard whose pending obligation transfers with the actual result, and whose Drop retires/quarantines before making that obligation appear complete, is one possible approach. Do not merely add a sleep or weaken cleanup/close-hold assertions.

Add a bounded barrier regression using the production submit/result/complete path: stop the actual worker immediately after its final operation and before result handoff while the pending state would otherwise clear, revoke/drop the await, and run actual scope completion. Assert MutationUncertain / preserved close hold, real staging or published evidence, and busy/quarantined admission through a different owner. Resume and join the worker; assert the later retirement cannot release the unknown record. Cover both build and publish result types if the fix uses different paths. Native result remains NOT_RUN until actually executed on Windows.

## Exact observed source SHA-256

- `crates/keelshell-session/src/sftp/archive_transfer/local_windows.rs`: `3d2a16199f53c0e96f26139716fb939a9914f9bb08d9838558bcacb3ff2c21a9`
- `crates/keelshell-session/src/sftp/archive_transfer.rs`: `c64257dcc46f927b02e169bbdab95c62341d874cd9408332aa339b00f65ae3f2`
- `crates/keelshell-session/src/sftp/mutation.rs`: `63a310b60a710f0077646f8741e8763e6cf8d52eda853df48dd83bcba52dafe8`
- `crates/keelshell-session/src/sftp/queue/reservations.rs`: `1e37150311fcc4d643d2405d7d3f682f99f760f8001bde28f7dce91ad923472c`
- `crates/keelshell-app/src/files.rs`: `402ed3f18ae174fcc579858e2d3865e92da2770aa4ae7a00a1112832cedad6cc`
- `crates/keelshell-app/src/files/archive.rs`: `5dde71aaa11d936b80af55045091aa293029ed34efc5285050fc9c187176e9d8`

No existing report, HOLD, notice, failed receipt or original was overwritten.
