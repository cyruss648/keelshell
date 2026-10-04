# Reviewed batch dependency plan — 2026-10-04

## Scope

The core-only slice adds immutable bounded DAG review, a SHA-256 consistency
token, a confirmation receipt and a pure in-memory admission ledger. It performs
no I/O and has no UI, SSH dispatch, storage or timer capability. See
[ADR 0034](../../adr/0034-reviewed-batch-dependency-plan.md).

## Verification

- `cargo test -p keelshell-core --locked --lib batch_workflow`: 10 unit tests
  passed. The final rerun after test assertion cleanup also passed all 10.
- `cargo test -p keelshell-core --locked --test batch_workflow`: all 3 public API
  integration tests passed. These use rendered target templates and synthetic
  observations, with no sockets or external services.
- `cargo test -p keelshell-core --locked --doc batch_workflow`: 1 example passed.
- `cargo clippy -p keelshell-core --all-targets --all-features --locked --
  -D warnings`: passed after the test assertion cleanup.
- `rustfmt --check --edition 2024 crates/keelshell-core/src/batch_workflow.rs
  crates/keelshell-core/tests/batch_workflow.rs`: passed.
- `python3 scripts/check.py --policy-only`: passed. No manifest dependency or
  lockfile change was required.
- `git diff --check`: passed. Whole-workspace integration remains owned by the
  final combined gate after separately developed UI and transport changes.

Tests cover duplicate task IDs and edges, missing prerequisites, self/long
cycles, nil IDs, invalid/control-containing commands, each count/byte bound and
inclusive target/dependency/storage maxima. They exercise a 128-task chain
without recursive propagation; dependency order independent of input order;
UUID ordering only among currently available nodes; and review invalidation on
exact whitespace, command, target, task or edge changes.

State tests require every prerequisite's confirmed success before admission.
Failed, unknown and explicitly skipped tasks propagate skips through dependent
pending tasks, while independent tasks retain their readiness/running state.
Invalid transitions do not mutate the ledger. A terminal record cannot be
replaced and a task cannot be admitted twice. Cancellation retains running
records until an adapter supplies a terminal observation. Debug output does
not include command text. The integration tests verify exact rendered commands
survive review, confirmation and state changes.

## Preserved intermediate failures

The initial unit compile failed with `E0716` because a test closure borrowed a
temporary one-element dependency slice across a conditional expression. Each
branch now passes its dependency slice directly to the test helper. The rerun
passed.

The first strict Clippy run rejected new tests' `unwrap`/`unwrap_err` assertions
under the workspace lint policy. Those assertions now use explicit test-only
result matching helpers or `matches!`; no production transition was changed.
Strict Clippy and the final 10-unit rerun passed after that cleanup.

The first broad `batch_workflow` name filter ran only the then-existing 8 unit
tests and filtered out the differently named integration tests. The explicit
`--test batch_workflow` run above subsequently executed all 3 integration tests;
zero filtered tests were never counted as integration acceptance.

## Source review and limits

The plan privately owns validated tasks, a canonical index and its fingerprint;
borrowing a task slice cannot mutate the acknowledged review. The versioned
length-prefixed digest covers every task and binding UUID, exact UTF-8 command
bytes and sorted dependency IDs. Construction rejects invalid graphs before a
receipt can be created. Topological traversal and skip propagation are bounded
and non-recursive. Ledger writes check admission/terminal prerequisites before
changing a state. No constructor, confirmation or query launches work.

This verifies local domain behavior on the macOS Rust toolchain, not native
Windows/Linux interaction or remote exit-status correctness. A future adapter
must bind each target identity to the exact reviewed authenticated session and
classify only observed zero exits as success. The review token does not prove
that a user clicked confirmation. The UI must own that explicit confirmation,
edit/session invalidation and dispatch gate. Concurrent admission, retry, audit
task counts, persistent workflows, scheduling and genuine remote execution are
not implemented by this slice.


## Final integrated local gate

`python3 scripts/check.py` passed on the integrated frozen source: dependency version policy, formatting, strict workspace/all-targets Clippy, and 853 Rust unit/integration/documentation tests (848 ordinary tests plus 5 doctests), with zero failures. The seven opt-in OpenSSH tests were ignored in the ordinary gate and executed separately; all seven passed. Packaging regression tests passed 47/47. The gate log is preserved at ignored `work/gate-20261004-integrated-final.log`; earlier failed logs remain preserved. These results do not certify Windows/Linux native GUI interaction or paid AI providers. Remote CI for the new commits is recorded separately.
