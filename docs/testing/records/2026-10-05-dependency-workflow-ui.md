# Reviewed dependency workflow UI — 2026-10-05

## Scope

The production GPUI command area now opens a separate manual dependency-workflow
editor and complete review. It supports up to 128 transient tasks and 32 captured
SSH targets, explicit prerequisite editing, source/template-expanded commands,
concurrency, per-task timeout and failure policy, task states and full bounded
stdout/stderr. Only human confirmation sends commands on the exact captured
already authenticated connections. See [ADR 0044](../../adr/0044-reviewed-dependency-workflow-ui.md)
and the [usage guide](../../product/DEPENDENCY_WORKFLOWS.md).

No command/output persistence, task-count-to-target-audit conversion, retry,
reconnection, replay, recurring scheduling or external MCP client is introduced.
The existing ordinary batch and its audit remain their separate behavior.

## Fixture verification

Production panel tests exercise exact per-target template bytes, multi-task
success releasing a dependent, confirmed failure blocking a descendant,
independent continuation, stop policy, timeout and explicit cancellation retaining
unknown admitted attempts, stale run polling, active session replacement cancelling
local waits without redirection, complete options/edge/metadata review binding,
invalid numeric drafts and cycles, raw drafts across language changes, explicit
target refresh, task name/add/remove editing, and full 128-task review with fixed
footer controls at 960×720, 760×560 and 480×480 in both languages.

Workspace tests use the actual production controls to add/edit a second task,
select authenticated targets, select a prerequisite, inspect the complete review
and confirm both commands. They prove no request before confirmation, exact
expanded commands on the selected sessions, no terminal injection, no persisted
command/output or target-audit record, same-endpoint connection replacement
invalidating approval, blocked background actions, hide/reopen retaining one run,
explicit cancellation and a new unselected draft requiring fresh review.

The TCP fixture authenticates real encrypted loopback SSH sessions and records
exec requests; its command strings choose controlled protocol responses and are
never interpreted by a shell. This proves renderer/dispatch/protocol behavior,
not genuine product-native interaction or arbitrary production programs.

`WorkflowHandle::try_finish` is separately tested with pending and consumed
aggregate reads, preserved queued admission/finish events, shared `Arc` receipts
and distinct newly authenticated connections to the same endpoint. The existing
11 SSH dependency scheduler tests remain unchanged and pass alongside the new
12th test. Core dependency plan and ledger tests run in the full workspace gate.

## Preserved failures and corrections

Ignored `work/workflow-ui/` retains every initial failure and corrected run:

- The first app test build failed because a wildcard import also imported the
  GPUI `test` macro, recursively resolving the generated plain `#[test]`.
  Explicit imports fixed it without increasing a recursion limit.
- The next app build exposed missing test-trait imports, a private test helper
  and ambiguous `.into()` calls. They were corrected without production bypasses.
- The first executable UI run had 21 passes and four failures. One proved that
  the task list could flex-shrink to zero beneath a long complete review; bounded
  lists and explanatory text now resist shrinking within the scroll body.
  The other failures expected confirmation after intentional session invalidation
  or rendered before queued input/new-panel events had been delivered. Tests now
  preserve real event boundaries and require expired confirmation to disappear.
  The corrected 25-test filter passed; two additional review/loss regressions
  were subsequently added to the final gate.
- The first full gate stopped at strict Clippy: suspicious assignment spacing,
  a large event variant and a duplicate fixture module. The start event now boxes
  its immutable review; test-only modules share one fixture instead of loading
  the same file twice, and spacing was corrected. No lint was suppressed.

The native panel has a public GPUI dialog role and localized title; inputs and
per-task actions have explicit localized accessible names. Hit/action isolation,
input freezing and focus restoration are tested, but `occlude` does not prove
background accessibility-tree isolation. That existing boundary remains open.

## Final gate and interoperability

- `python3 scripts/check.py`: passed dependency policy, six Python script tests,
  formatting, strict locked whole-workspace/all-targets Clippy, 1003 ordinary
  tests and eight documentation tests. Eleven opt-in tests were ignored in this
  ordinary gate (two supplier-CLI cases and nine OpenSSH cases).
- After final localized accessibility names and escaped destination display were
  added, strict whole-workspace/all-targets Clippy passed again. The final app
  `workflow` filter passed all 27 tests, including all 13 new panel/workspace
  cases and the existing ordinary-batch/related modal cases. Formatting, direct
  dependency policy and `git diff --check` passed on that source.
- `cargo test -p keelshell-session --locked --test workflow_exec --
  --test-threads=4`: all 12 tests passed independently.
- `python3 scripts/openssh_interop.py --output work/workflow-ui/openssh-final
  --timeout 300`: all nine separate system OpenSSH tests passed in 7.96 seconds.
  The new test executes real POSIX `printf` programs, verifies complete stdout,
  stderr and successful exit, and polls the aggregate without losing the two
  queued task events. A second authenticated connection to the same server is
  explicitly distinct. The existing workflow marker/dependency/failure test
  remains part of that run.

`work/workflow-ui/openssh-final/result.json` confirms owned/observed processes
stopped, no unverified ancestry identities and the private temporary directory
removed. The source remains transient and test logs remain ignored; generated
key material is not preserved. All initial failures listed above remain intact.
Final strict Clippy output is `clippy-frozen.log`, the complete gate is
`gate-second.log` and final UI results are `app-workflow-frozen.log` in the same
ignored evidence directory.
`cargo build -p keelshell-app --locked` also passed. `file` identifies the actual
GUI executable as Mach-O arm64 and `otool -L` confirms native AppKit/CoreText and
other system framework links. Its SHA-256 is
`3218bcceea1a339e110fb05324872e2d8ee08c023fb82040b19a598629343586`,
recorded separately in ignored `native-build-receipt.json`. This is a native
build/link check; this worktree did not launch, install or package the application.

Native macOS workflow acceptance, native Windows/Linux acceptance, release
packaging, production hosts and final independent integration review remain
separate gates; build, renderer and localhost SSH evidence cannot close them.
