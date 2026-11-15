**Independent non-author review — r3 source delta approved; merge held pending the final r3 gate receipt.** Open findings: **P0: 0, P1: 0, P2: 0, P3: 0**. Both previously reported P2 findings are closed. No further source repair is requested by this bounded recheck.

Review date: November 15, 2026, Asia/Shanghai. Evidence snapshot: 11:59 CST / 03:59 UTC. I did not author the implementation.

**Candidate identity and scope**

Checkout: `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`. HEAD and the requested exact base remain `dfbc95904ec841fae64701181de4802959c5fb34`. The candidate includes tracked modifications and untracked implementation files.

This review binds to [frozen-inputs-r3.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-transfer-20261115/frozen-inputs-r3.json), SHA-256 `2b768eaeeb9a57498cdcc881ce142cd778227711aaed405d70c82b15082c71c2`. All **532 Rust/Cargo input hashes and 10 gate-script hashes matched**, before and after the independent test run and at the reporting check. Independent enumeration of the workspace build inputs produced exactly those 532 paths.

The r2-to-r3 input sets are identical. Exactly four input hashes changed: [files.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs), [archive.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive.rs), [archive_tests.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_tests.rs), and [local.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/archive_transfer/local.rs). No gate script changed. Reversing the reviewed repairs and test additions **in memory** reproduced all four frozen r2 hashes exactly. There is no undeclared production-source discrepancy in this delta.

The earlier complete batch review remains in [independent-review-r1.md](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-transfer-20261115/independent-review-r1.md). It covers the core codec/tests/exports/dependencies; session planning, local/remote execution, reservations, protocol tests and added OpenSSH cases; and app review bindings, dispatch, worker, controls, browser/view integration and tests, including untracked files. That report remains unchanged, SHA-256 `9f0afd56a39046b34c2cab63c5df36f273af38ddaac3440a96c909114fb0fc2d`. This recheck covers the four-file repair and its functional consequences, carrying forward the unchanged full-batch assessment.

AGENTS, RTK guidance, the current HANDOFF introduction, ADR 0132, ARCHIVE_TRANSFERS, SHARED_TARGET, and the requested rust-best-practices skill and relevant references were read during the review. Parent documentation updates are outside the frozen build-input set; this recheck does not approve unrelated documentation changes.

**P2 closure: archive completion no longer leaves its own close hold stuck**

At [files.rs:1445](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1445), archive admission snapshots the previous isolation target before installing the new archive target. The nested option preserves the distinction between an archive starting with no previous hold and a non-archive operation.

Worker-start failure restores that previous target at [files.rs:1639](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1639). The matching operation's terminal callback restores it at [files.rs:1708](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1708), before suspended-session handling. Existing operation-ID ownership checks still reject stale completions.

The classification at [archive.rs:33](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive.rs:33) retains the current hold for unknown outcomes, worker loss, unconfirmed cancellation, cleanup failure and quarantine. Known terminal outcomes restore the previous target. The session completion path converts pending mutations and unverified cleanup into `MutationUncertain`, which maps to the retained unknown-outcome category.

This closes the original trigger: successful upload/extraction from an otherwise idle panel no longer leaves `window_close_busy()` true solely because of the completed archive. A later verified archive also restores an older unresolved target rather than erasing it.

The executed production-path regressions assert close release after upload at [archive_tests.rs:141](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_tests.rs:141), extraction at [archive_tests.rs:190](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_tests.rs:190), and source rejection before staging at [archive_tests.rs:259](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_tests.rs:259). The new case at [archive_tests.rs:339](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_tests.rs:339) holds an actually issued SFTP WRITE, cancels through the app control, observes an unknown result, then completes another upload and checks that the original target, close hold and quarantine remain. It passed independently.

**P2 closure: Windows builder mutability is confined to Unix**

At [local.rs:389](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/archive_transfer/local.rs:389), the outer `DirBuilder` binding is immutable. Only the `#[cfg(unix)]` shadow introduces a mutable binding and applies mode `0o700`. Windows retains an immutable binding passed to `create(&self)`, eliminating the confirmed `unused_mut` cause under strict warnings.

The `stamp` binding remains used by `Ok(stamp)` on every platform. The bounded cfg inspection found no additional confirmed Windows issue. This is a source-confirmed repair; no Windows compiler or Windows strict Clippy run was executed by this reviewer.

**Independent execution and provenance**

I verified the supplied [app-r3-inventory.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-transfer-20261115/app-r3-inventory.json) against the actual binary. Its SHA-256 was `4d3b056207ecc19b55fb81587886a0a210202cb576c30a30b1b386a46bb38620` before inventory, before testing and after testing. Direct inventory execution exited 0 and listed **1,195 tests**, including all **9** expected archive cases.

`python3 -B` invoked these commands directly as subprocess argv:

```text
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-app/dd89ff795559e915/out/keelshell_app-dd89ff795559e915 --list
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-app/dd89ff795559e915/out/keelshell_app-dd89ff795559e915 files::transfer_tests::archive_tests --test-threads=4
```

The authorized focused execution exited **0**: **9 passed, 0 failed, 0 ignored, 1,186 filtered out**. The test harness reported 1.62 seconds; subprocess elapsed time was 1.637 seconds, from 03:56:16 to 03:56:17 UTC. Output is preserved in [independent-review-r3-app.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-transfer-20261115/independent-review-r3-app.log), SHA-256 `7a13b732c4d1bf00601555c08494be57fb77353f42771ec1270d669b68a9e749`.

These are independent executions of a **parent-built binary**. Hash and inventory verification establish the binary's identity and test contents; they do not replace an independent rebuild or independently prove the source-to-binary compilation chain. The parent repair log shows both session and app compilation before the successful regression run.

Additional reviewer commands were read-only `rg`/`rg --files`, `nl`, `sed`, `git --no-optional-locks rev-parse HEAD`, and a four-file `git --no-optional-locks diff --check` against the exact base, which exited 0. Python snippets read receipts/logs, verified hashes, enumerated inputs, counted tests and reconstructed the delta in memory.

Reviewer Cargo/build/format/commit/push/cleanup executions: **0**. No reviewer command changed source, index or target artifacts. Reviewer writes were the authorized new report and test log; the authorized functional tests used isolated runtime fixtures. Existing reports and failure receipts were preserved.

**Parent evidence inspected, separate from reviewer execution**

Completed wrapper log hashes below matched their receipts. OpenSSH case counts were also checked against its actual test log.

| Evidence | Actual result | Interpretation |
|---|---|---|
| `archive-lifecycle-before-fix` | Exit 101; **5 passed, 4 failed, 0 ignored** | Real app regressions exposed successful upload/extraction, early-rejection close holds and replacement of the older unknown target |
| `archive-lifecycle-after-fix` | Exit 0; **9 passed, 0 failed, 0 ignored** | Repaired app suite; independently repeated above |
| `openssh-r1`, executed on r3 source | Exit 0; **13 passed, 0 failed, 0 ignored** | Explicit ignored-test run, including both new archive interoperability cases |
| `full-gate-r2` | Exit 0 in **631.394 s**; ordinary Rust **2,600 passed / 0 failed / 25 ignored**; rustdoc **14 passed**; Python **131 run / 1 skipped** | Completed historical engineering evidence; r2 remains rejected and this does not approve r3 |
| `full-gate-r3` | **Running; no terminal receipt at snapshot** | Python **131 run / 1 skipped**, format and strict macOS Clippy passed; workspace tests underway; final totals and exit not yet asserted |

The OpenSSH result reports owned processes stopped, observed processes stopped and temporary-directory removal all true. Its 13-case success is disposable localhost OpenSSH interoperability, not native desktop or production-server acceptance.

The r2 complete gate also exercised the previously missing overall-deadline and post-staging source-recheck archive cases and the local no-replace primitive test. Those historical results close the earlier inventory uncertainty for r2; final r3 gate completion remains outstanding. Earlier compile/test/lint failures remain retained in the prior report and evidence directory.

**Platform, dependency and functional boundaries**

The unchanged full-batch assessment covers explicit complete-manifest review; draft/selection/session invalidation; exact source-byte and digest rechecks; bounded ZIP32/tar.gz processing; rejection of traversal, unsafe aliases, links and unsupported entries; exclusive staging, permission checks and no-overwrite publication; cancellation, lost acknowledgements and quarantine; and worker ownership off the UI thread. This delta introduced no additional actionable defect in those paths.

Windows extraction remains intentionally unsupported with a disabled UI and explanation. The Windows mutability finding is closed in source, while Windows compilation, strict Clippy, packed-upload runtime behavior and native UI acceptance remain unverified here. macOS/Linux extraction uses the rustix no-replace backend and fails closed when the required primitive is unavailable. The independent macOS fixture exercised extraction; Linux execution and native desktop acceptance on all three platforms remain open. Headless GPUI tests do not establish native accessibility, IME, installation or release acceptance.

The declared hostile same-user pathname-race and server-side pathname boundaries remain explicit limitations; this review does not claim complete immunity.

Direct requirements retain the required `x.y` form: flate2 `1.1`, tar `0.4`, zip `9.5`, and macOS/Linux rustix `1.1`, resolved to `1.1.6`. The live dependency check remains `NOT_COMPLETED_TIMEOUT`: 75.015 seconds, exit `-15`. Deterministic dependency-policy success does not establish latest-version or vulnerability-audit acceptance.

**Merge recommendation: HOLD until the final r3 engineering receipt is verified against the unchanged freeze.** The independent code/functional review has **zero open findings**, and both original P2 findings are closed. A passing final r3 gate would satisfy the remaining engineering condition for this review; native/platform/dependency boundaries above remain separately unverified. Any further frozen-input change requires a bounded delta recheck.

<oai-mem-citation>
<citation_entries>
MEMORY.md:322-323|note=[read only review reporting conventions only]
</citation_entries>
<rollout_ids>
01a1a5b2-40b8-7cb1-8de1-07ec8f4efa6c
</rollout_ids>
</oai-mem-citation>
