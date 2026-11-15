**Independent non-author review — REQUEST_CHANGES.** Frozen candidate r2 has **P0: 0, P1: 0, P2: 2, P3: 0**. Repair the two findings below before merging. This is a source and functional-path review; the parent’s final engineering gate and native acceptance remain separate.

Review date: November 15, 2026, Asia/Shanghai. Evidence snapshot: 11:45 CST / 03:45 UTC. I did not author the implementation or change its source.

**Candidate identity and read-only boundary**

Checkout: `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`. HEAD and the requested exact base both resolve to `dfbc95904ec841fae64701181de4802959c5fb34`; the candidate includes tracked modifications and untracked implementation files.

Final review binding: [frozen-inputs-r2.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-transfer-20261115/frozen-inputs-r2.json), SHA-256 `1facaaa0024025cb2c6e840c3d9f2fff57af9764c8812fea61d31dc0d56ab64f`. All **532 Rust/Cargo input hashes and 10 script hashes matched**, including the final check before reporting. Independently enumerating Rust files and Cargo manifests/lockfile produced exactly the same 532 paths, with no missing or additional inputs.

The initial r1 freeze also matched. The r1-to-r2 changed-input set contains only the core archive tests and the session archive local module. Reconstructing the three reported lint changes in memory reproduced both r1 file hashes exactly: remove the unused test import, remove the unfulfilled lint expectation, and change byte-kind iteration. There was no additional source or script discrepancy.

I read AGENTS, RTK guidance, the current HANDOFF introduction, ADR 0132, ARCHIVE_TRANSFERS, SHARED_TARGET, and the requested rust-best-practices skill and relevant references. Review covered all eight new Rust files and every changed Rust/Cargo integration surface listed in the request: codec and tests; exports and dependencies; session planning, local/remote execution and reservation admission; protocol tests and both added ignored OpenSSH cases; app planning, review bindings, dispatch, worker, controls, local-browser subscriptions, view and test integration. Supporting lifecycle code was read to establish finding 1.

**1. [P2] Verified archive success leaves the window-close guard permanently busy**

Primary location: [files.rs:2437](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:2437), the new `TransferArchive` isolation-target arm.

Trigger: start from an otherwise idle Files panel, explicitly confirm either archive direction, and let execution, publication readback and worker cleanup succeed.

`run()` stores the archive destination in `self.isolation_target` at [files.rs:1446](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1446). The completion callback retires `busy`, `operation_id` and stop/pause state, but `Outcome::Done` only updates the status at [files.rs:1836](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1836). There is no reset of `isolation_target` anywhere in the current app source; its only other assignment is constructor initialization.

`window_close_busy()` unconditionally treats this remaining `Some` as busy at [files.rs:1331](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1331). Workspace close checks active, archived and retired panels and refuses close on that result at [window_lifecycle.rs:195](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_lifecycle.rs:195). Retired-panel retention also uses this guard at [workspace.rs:3138](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace.rs:3138).

Impact: a completely successful archive leaves the window unable to close normally and can retain an otherwise finished panel and its session owner. Archiving or retiring the tab does not remove the window’s blocker. I confirmed the window-close and retention consequences; I do not claim that every tab-close path itself refuses the action. Other mutations already share the underlying lifecycle problem, but this new arm introduces it into the archive workflow.

Requested repair: distinguish an inspectable destination from an unresolved close hold, or retire the owned archive hold after verified success. Preserve genuine uncertain writes and quarantine. Add meaningful regressions for successful upload and extraction returning to `window_close_busy() == false`, plus an unknown-result case that remains held. The existing archive success tests check bytes and permissions but do not assert this lifecycle.

**2. [P2] Windows strict Clippy rejects Unix-only builder mutability**

Primary location: [local.rs:389](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/archive_transfer/local.rs:389), `let mut builder = tokio::fs::DirBuilder::new()`.

Trigger: validate the session crate or workspace on Windows with the required strict Clippy gate, `cargo clippy --workspace --all-targets --locked -- -D warnings`.

The builder’s only mutable use is `builder.mode(0o700)`, guarded by `#[cfg(unix)]`. Tokio’s installed 1.54.1 source confirms that `DirBuilder::create` takes `&self`, while `mode` takes `&mut self`. After Windows cfg selection, this binding has an unnecessary `mut`, producing `unused_mut`, which `-D warnings` rejects. Runtime rejection of extraction does not remove this function from compilation.

Impact: the new session implementation fails the Windows engineering gate, including builds intended to retain packed uploads while extraction is disabled. This finding is confirmed by source and dependency-signature reasoning; I did not execute a Windows compiler.

Requested repair: confine mutable builder construction/configuration to the Unix branch and keep the Windows binding immutable, then validate with the Windows strict gate when available. The nearby `stamp` binding is **not** an unused-variable finding: `Ok(stamp)` uses it on every platform. The bounded cfg review found no additional confirmed Windows defect in the new archive modules or their tests.

**Functional assessment beyond the findings**

The reviewed implementation carries the complete manifest into explicit confirmation. Browser revision, session identity, directory, selections, archive format and path drafts participate in invalidation. Compression/decompression is dispatched through blocking workers, and filesystem/network work is owned by the transport worker.

Local upload revalidation compares metadata, full content digests and exact member bytes, including a second scan after staging. Download extraction rereads and compares the entire encoded remote source before staging and publication. Codec validation bounds encoded/expanded bytes, entries, depth and cumulative names; it rejects traversal, unsafe names, conflicting paths, links and unsupported entry types and checks ZIP/TAR/gzip integrity.

Remote uploads use exclusive private staging, standard SFTP rename and complete readback. Local extraction uses private staging, new files, mode checks and the macOS/Linux no-replace publication primitive. Temporary ownership and pending mutations retain quarantine on abandoned or unverified results; the lost-ACK paths do not automatically replay. I found no additional actionable defect in these reviewed paths. This assessment does not extend the implementation’s declared protection against hostile same-user pathname races or server-side path changes.

**Commands and actual verification**

Reviewer Cargo, build, formatting, cleanup and test executions: **0**. No test binaries were run. No source, target or index mutation was performed. The only authored file is this permitted report.

Executed read-only commands included `pwd`, `rg`/`rg --files`, `cat`, `nl`/`sed`, `wc`, `git rev-parse`, `git --no-optional-locks status --short --untracked-files=all`, scoped `git diff`/`--stat`/`--name-only`, and `git --no-optional-locks diff --check dfbc95904ec841fae64701181de4802959c5fb34`. The diff check exited 0. `python3 -B` snippets performed JSON/SHA-256 verification, complete input enumeration, the in-memory lint-delta reconstruction, test-name counting and existing log/receipt inspection; they created no files.

The following are **parent-run results read from existing evidence**, not independent executions. Completed receipt log hashes matched their recorded SHA-256 values.

| Parent evidence | Actual observed result | Scope |
|---|---|---|
| `core-archive-r1` | Exit 0; 40 passed, 0 failed, 0 ignored | Targeted run before the final freeze |
| `app-archive-r3` | Exit 0; 8 passed, 0 failed, 0 ignored | Targeted archive app suite before the final freeze |
| `session-archive-r1` | Exit 0; 16 passed, 0 failed, 0 ignored | 10 archive cases plus 6 reused fixture cases |
| `full-gate-r1` | Exit 1 after 13.081 s | Python: 131 run, 1 skipped; format passed; strict Clippy rejected the three subsequently repaired lint issues |
| `full-gate-r2` | **Running; no terminal receipt at snapshot** | Dependency policy, Python 131 run / 1 skipped, format and strict macOS Clippy passed; all 8 app archive cases printed `ok` during workspace execution |

Earlier failures remain visible: `app-compile-r1` and `app-archive-r2` exited 101 during compilation; `app-archive-r1` exited 101 with 6 passed and 2 failed. The later targeted app result and r2 archive case output pass, without replacing those earlier receipts.

Frozen r2 contains **12** archive integration cases, whereas the earlier 16-test session run contains only 10 archive cases. The overall-deadline and post-staging source-recheck cases, and the local no-replace primitive unit test, had no execution result in the r2 gate log at this snapshot. A complete r2 or repaired-candidate gate must establish their actual inventory and results. The two new ignored OpenSSH cases were reviewed but not observed executing. No complete workspace, rustdoc or controller-group total is asserted for the still-running r2 gate.

**Platform, dependency and approval boundaries**

Windows extraction is intentionally unsupported and visibly disabled with an explanation; that limitation is acceptable for this declared scope. Windows packed-upload behavior, Windows compilation after finding 2, Linux execution, and native archive interactions on all three platforms remain unverified here. macOS headless GPUI and controlled SFTP evidence do not establish native GUI, IME, accessibility, installation or release acceptance.

Direct archive requirements use the required `x.y` form: flate2 `1.1`, tar `0.4`, zip `9.5`, and the macOS/Linux rustix dependency `1.1`. Cargo.lock records resolved versions; rustix resolves to `1.1.6`. The live dependency check timed out after 75.015 s with exit `-15` and no completed latest-version report. Deterministic dependency-policy success does not establish latest-version or vulnerability-audit acceptance.

**Merge recommendation: do not merge frozen r2.** Batch-repair the two P2 findings, freeze the resulting source, complete its engineering gate, and perform a bounded independent delta recheck covering both fixes and their regressions. This report grants no approval to later source and no native/platform acceptance.

<oai-mem-citation>
<citation_entries>
MEMORY.md:322-323|note=[read only review reporting conventions only]
</citation_entries>
<rollout_ids>
01a1a5b2-40b8-7cb1-8de1-07ec8f4efa6c
</rollout_ids>
</oai-mem-citation>
