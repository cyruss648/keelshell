# 2026-10-06 — Bounded workflow result history

状态：独立作者候选；专项通过。新的非作者复审、主线整合、后继 CI 与原生验收仍开放。

The candidate adds local read-only task results, including each terminal finite scheduled occurrence. Commands, output, addresses, credentials, custom parameters and their digests are excluded. No task/schedule recovery or replay is provided. This record distinguishes author engineering checks from independent review and native product acceptance.

## Inputs and ownership

- Exact author baseline: `b26c4c88fa54dbc9788899f86136fb14ef8cb37a`.
- Read-only reference: previously sealed parameter/finite-schedule/profile-sync combination. Only the finite scheduling domain and panel/dispatch interfaces are composed into this candidate; profile-sync production code is excluded. Scheduling dependency and audit increment are delivered separately.
- All edits, build/cache and temporary data belong to the new isolated managed worktree. One ordered CoW cache copy from a completed quiet CI review tree returned actual exit 0 in 101.111 seconds, after lock-holder/consumer checks; source/destination inode probes differ. No Cargo ran before copy completion. This is a cache-backed incremental check, not a clean rebuild.
- No application GUI, vendor CLI/model, external SSH resource or native MCP was started for this work. The new domain/adapter tests are pure or use isolated local state. GPUI tests use the production headless renderer and save workers, not native desktop acceptance. Routine full-workspace fixtures remain distinct from external/native product evidence.

## Meaningful new checks

Seven domain/storage integration checks cover stable receipt idempotency/conflict atomicity, unique finite occurrence identity, backwards wall clock with insertion-order retention, whole-run bounded trimming/disk/readback, legacy documents and all outcome states, unknown/impossible wire rejection, and signed intended schedule times within the existing Gregorian calendar domain.

The worst-shape input is 100 × 128 tasks, including `u32::MAX` nonzero exit and UUID dependency fields. The bounded ledger retained 16 complete runs / 2048 tasks. The actual isolated state file was **582,068 bytes**, below the explicit 640 KiB audit test budget and the unchanged 4 MiB application-state cap. Actual restart validation in this run was **8.7815 ms**; this measurement is evidence from one fixture run, not a portable performance guarantee. Direct oversized ledger injection is rejected.

Two pure transport-receipt projection checks cover every outcome category, output exclusion and exact immutable plan/options/task/target/inner-row bindings. Incomplete aggregate, wrong options/target/inner ID and false prerequisite are rejected. No transport or process is started by those checks.

Six production headless GPUI checks cover:

1. Actual isolated state-file error, retained unsaved result, identical retry without duplication and reconstruction of a workspace from disk with no execution handle or timer.
2. Snippet/vault modal save-lease release and hidden-panel result preservation.
3. Read-only history controls in Chinese and English; no confirm/execute action exists in the history view.
4. Forty finite terminal occurrence notes, duplicate drain rejection, distinct missed/busy/cancelled/invalidated states, and refusal to manufacture task success from a finished scheduling ledger without a task receipt.
5. The bounded 100 × 128 pending queue, original receipt preservation after FIFO projection and confirmation backpressure.
6. Long UUID/dependency-result wrapping at 900 × 580 in Chinese/English and System/Light/Dark themes, with safe controls reachable; this is headless layout evidence. Existing accepted schedule test accessors remain intact and verify that restarting does not restore reviews, parameters, ledger slots or an old schedule draft value.

## Preserved failures

Initial author compilation failed on missing imports/private constructor use, then on unsupported Clone assumptions in newly written tests. Later failures exposed an unused sizing import followed by its required use in a revised view, and an untyped fixture clock closure. Exact logs/actual wait receipts are retained; fixes use existing public fields and explicit shallow test projections, without broadening production transport behavior.

One first GPUI occurrence test expected pending records but found zero: opening a snippet editor while the workflow was visible was correctly rejected by the existing command-surface guard, so forty metadata records had already saved. The fixture now hides the panel and asserts a real snippet modal save lease before emitting receipts. The command-surface guard was not weakened.

## Gate and remaining scope

An earlier frozen 631-input candidate completed full formatting, direct `x.y` policy, strict workspace Clippy, 1427 ordinary / 8 doc / 6 Python checks and the separate small-stack controller with actual exit 0 in 530.314 seconds. All 631 bodies matched before/after. The initial full gate stopped on one collapsible-if lint; its unchanged-input failure and actual wait are preserved.

The final narrow increment preserves the accepted scheduling test interfaces, adds compact UUID wrapping/restart checks, and aligns intended occurrence time validation with the existing signed calendar domain. Its six headless GPUI checks returned actual exit 0 in 70.035 seconds. The final complete gate returned **actual exit 0 in 505.329 seconds**, covering formatting, direct `x.y` policy, strict workspace Clippy, **1429 ordinary / 8 doc / 6 Python checks**, with 16 explicitly ignored checks, and the separate small-stack controller. All **631 inputs / 13,956,755 bytes** were read before and after and matched exactly. The held owner process was waited; its group was independently absent (`ESRCH`) and private temporary data was removed. The gate used the isolated copied cache and is not a clean rebuild. Only this test record was updated after that gate; Rust sources, manifests and lockfile remain exactly those checked. No commit, push, packaging, installation or native acceptance is claimed.

Open: new non-author review, exact main integration/source binding, Linux/Windows successor CI, native macOS history/save/restart flow, minimum-window/accessibility and full language/theme/cross-platform desktop matrices. Saved task history cannot close an unproven transport, agent, external MCP or Release acceptance boundary.
