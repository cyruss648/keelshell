# Reviewed workflow SSH adapter — 2026-10-04

## Scope

The session-only adapter consumes the immutable confirmed dependency plan and
captures already authenticated UUID-to-session bindings. Each task uses existing
guarded SSH batch exec and retains its real transport receipt. No app/core API,
GUI, connection creation, recurring timer or target-count audit was changed by
this slice. See [ADR 0035](../../adr/0035-reviewed-workflow-ssh-adapter.md).

## Verification

- `cargo check -p keelshell-session`: passed and updated only the session package's
  local core dependency entry in Cargo.lock. No registry version changed.
- `cargo test -p keelshell-session --locked --test workflow_exec --
  --test-threads=4`: all 11 real TCP SSH fixture tests passed initially; after the
  cancellation-order correction below, all 11 passed again.
- The combined `--test batch_exec --test workflow_exec` run passed all 14 existing
  batch exec tests and initially exposed the cancellation-reason regression.
  No existing batch or guarded transport code was modified.
- `cargo test -p keelshell-session --locked --doc workflow`: 1 public example
  compiled successfully. This is API compilation evidence, not command execution.
- `cargo clippy -p keelshell-session --all-targets --all-features --locked --
  -D warnings`: passed, including a final run after the scheduler correction.
- The targeted active-cancellation test passed 8 sequential additional trials;
  output is retained in ignored `work/workflow-cancel-repeat.log`.
- Direct Rustfmt check of the new module, scheduler and integration test passed;
  `python3 scripts/check.py --policy-only` and `git diff --check` passed.

The new fixture suite proves complete binding/concurrency/timeout/output-budget
rejection before any channel opens, exact reviewed command bytes on distinct
captured sessions, every prerequisite completing before a join, failure and
unknown results blocking descendants while preserving independent branches,
stop policy preserving in-flight work, pre-poll and active cancellation, partial
output retention, drop-owned cleanup, original binding retention after selection
replacement, output/deadline uncertainty, rejection/late OPEN cancellation, and
128-task execution with a bounded eight-task concurrency ceiling. It consumes no
events until the maximum graph completes, then verifies all 256 state events and
shared task receipts rather than duplicated output buffers.

## Preserved intermediate failure

The first standalone 11-test run passed. A subsequent combined batch/workflow
run failed `cancelling_active_work_keeps_partial_output_and_preserves_the_connection`
at the assertion requiring all pending tasks to have `Cancelled` skip reasons.
If a cancelled transport worker joined before the scheduler's cancellation
branch ran, the core ledger propagated `DependencyNotSucceeded` to its dependent
first. Other event ordering marked that same pending task `Cancelled`. Neither
ordering admitted a descendant, but the terminal reason was inconsistent.

The scheduler now observes global cancellation and cancels pending ledger tasks
before processing each joined transport receipt, in both drain and awaited-join
paths. Running task receipts remain unchanged. The original strict assertion was
retained. All 11 tests and the 8 targeted cancellation trials passed after this
fix; the failure is preserved here rather than treated as a passing run.

## Source review

The entry point finishes all validation before creating its scheduler. Commands
are read from the consumed confirmed plan, never a caller-supplied live string.
Bindings are consumed into a private owned map. The scheduler queries only the
core ledger for readiness and records only complete transport zero exits as
success; unknown and proven non-start outcomes cannot release an edge. The early
batch failure flag prevents pending replacement admissions before cleanup joins.
The scheduler owns a `JoinSet`; cancellation keeps running receipts distinct,
and dropping transfers cleanup to the existing guarded channel owners. The final
receipt includes actual immutable options and the core fingerprint. Debug output
of the plan omits commands; output receipts retain bounded raw bytes for callers
to safely render. No task outcome is written into target-based batch audit.

## Evidence boundaries

These tests open actual loopback TCP SSH connections, authenticate ephemeral
fixture sessions and exchange real channel/exec/exit-status messages. Fixture
commands choose protocol scenarios and are never interpreted by a shell. This
does not demonstrate production program execution, arbitrary remote operating
systems, UI review correctness or native Windows/Linux desktop behavior.

Core content fingerprints do not authenticate session object selection or
execution options. The future UI must show and guard the complete review,
including target session identities and options, before dispatch. A cancelled
OPEN can retain an independent cleanup owner until its deadline; uncertainty or
failed CLOSE can stop the shared captured connection. Cancel/drop does not prove
remote process termination or undo completed tasks. GUI integration, task-level
history, retries and recurring schedules remain separate work. Independent
review, full-workspace gates and external OpenSSH acceptance belong to the
combined integration stage and must not be inferred from this fixture record.

## Independent integration review and OpenSSH

A separate read-only agent inspected the frozen adapter, core contract, guarded
transport reuse and the new design/agent plan. It found no blocking defect and
independently passed 11 workflow tests, 14 existing batch tests, 10 core unit
tests and three core integration tests. It confirmed the plan exposes only a
KeelShell MCP server to external agents, without a generic third-party client.
The review retained the caller's responsibility for human approval and complete
session/options binding, and the late-OPEN/shared-connection cleanup boundary.

`python3 scripts/openssh_interop.py --output
work/openssh-20261004-workflow --timeout 300` passed all eight opt-in tests in
9.454 seconds. The added workflow test uses actual POSIX commands on a separate
system OpenSSH server: write/read marker dependency, exit 7 blocking a descendant,
and successful independent branch. It also verifies receipt identity, stdout,
stderr, fingerprint and marker content through SFTP. This differs from the
protocol fixture that does not interpret commands.

The ignored `work/openssh-20261004-workflow/result.json` records exit zero, stopped
owned processes and removed temporary key/directory data, with the ancestry
observation boundary stated explicitly. This is disposable localhost program
interop; Windows OpenSSH, production machines and workflow UI remain unverified.

## Frozen workspace gate

`python3 scripts/check.py` exited zero on the frozen source including the new
OpenSSH test. Direct dependency policy, `cargo fmt --all --check`, strict locked
whole-workspace/all-targets Clippy, 859 ordinary tests and six documentation tests
passed with zero failures. Eight external OpenSSH tests were ignored in the
ordinary gate and executed successfully in the separate run above. Output is
preserved in ignored `work/gate-20261004-workflow-final.log`.

The previously completed 47 packaging tests and current macOS UI audit remain
their own evidence. This transport slice adds no graphical workflow editor and
does not convert the prior UI audit into native workflow acceptance. Remote CI
for the new integration commit must be checked separately after pushing.

## Remote source validation

Source commit `9802ce94d34484b5aa6d1d811edd8770b9fe0aa0` passed
[Quality 37197083353](https://github.com/cyruss648/keelshell/actions/runs/37197083353)
on macOS 26, Ubuntu 24.04 and Windows 2025. macOS/Ubuntu each passed 859 ordinary
and six documentation tests; Windows passed 843 ordinary and six documentation
tests. Standard gates ignored eight opt-in OpenSSH tests; Ubuntu also has one
platform-conditioned ignored test. Packaging ran 47 tests on each runner, with
one Unix-permission test skipped on Windows. The separate macOS/Linux OpenSSH
steps passed. Full CI output is retained in ignored `work/quality-37197083353.log`.

This is runner build/test/package and loopback SSH evidence. It does not close
native Windows/Linux desktop, complete workflow UI or production acceptance.
