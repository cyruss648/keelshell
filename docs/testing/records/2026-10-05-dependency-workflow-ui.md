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

## Entry layout correction after independent review

The initial feature freeze `2834122b0417368b8e32e2daf4155fbf9d1cc08f` received one
independent P2 finding. At 900×580 in English with the AI sidebar open, the
`Dependency workflow` action occupied x=454..628 while the command column ended
at x=522. Its final 106 pixels entered the later-painted opaque AI sidebar. The
renderer could still open the panel with a center click; the finding concerns
overflow and covered action text, not a wholly unreachable action. Review cutoff
was `2026-10-04T18:57:18.517615+00:00`, with all 438 tracked file hashes and the
clean source state unchanged. Later 128-task/32-target independent probes ran on
a separate copy of that original freeze and do not accept this correction.

The command-action row now wraps within the existing command column, retains all
four complete localized labels, uses consistent spacing, and grows vertically
without rebuilding the textarea or changing action handlers. Three renderer
test IDs expose the actual command column, action row and assistant column.
Neither connection metadata nor transport execution was changed.

Two new production Workspace/GPUI/TCP regressions cover Light/Dark × Chinese/
English × AI open/closed × 900×580/1440×900. The default-label case also checks
both history-policy labels in each of the 16 combinations. Assertions require
all four actions to have positive visible geometry entirely inside the command
row/column, no action overlap, a command textarea at least 120×60, non-overlapping
Run action, and a terminal at least 80 pixels high. The narrow English/AI layout
must use a second row and the wide layout retains one row. Theme/sidebar changes
preserve the command entity, source, target and revision. Actual renderer clicks
toggle history, clear the command, and open/hide the ordinary batch and workflow
editors without sending an exec request or terminal write.

The running-label case starts separately reviewed real TCP SSH `hold` attempts
on the two captured connections, hides both owned panels and verifies both
running entry labels in the same 16 layout combinations. Reopening each entry
then cancels local waits and collects its original run without replay; each
fixture receives exactly one request. The command text only selects fixture
responses and is never interpreted by a shell.

`work/workflow-ui-layout/` preserves the original independent report/probe with
SHA-256 and this correction's runs. `layout-before-fix.log` proves the new test
failed on the original row with the exact x=454..628 versus x=522 bounds. The
first corrected default-label test passed. The final new-source verification
and native build results are recorded below. A fresh
independent review and integrated native GUI acceptance are required; the old
freeze's gate and native build do not accept this changed source.

- `python3 scripts/check.py` passed on the corrected source: dependency policy,
  six Python script tests, formatting, strict locked whole-workspace/all-targets
  Clippy, 1005 ordinary tests and eight documentation tests, plus the separate
  2 MiB local-agent controller with all scenario groups passing. Eleven opt-in
  tests remained ignored in the ordinary run. Evidence: `gate-layout-final.log`.
- `cargo test -p keelshell-app --locked workflow -- --test-threads=4` passed all
  29 cases in 8.80 seconds; this includes both new entry regressions and all 27
  previous workflow/ordinary-batch/related-modal cases. The two new entry cases
  also passed independently in 7.82 seconds. Evidence:
  `app-workflow-layout-final.log` and `layout-after-fix-final.log`.
- The new-source isolated system OpenSSH run passed all nine tests in 5.727
  seconds. Its receipt confirms owned/observed processes stopped, no unverified
  ancestry identities and removal of the private temporary directory. Evidence:
  `openssh-layout-final/result.json`; generated key material was removed.
- `cargo build -p keelshell-app --locked` passed on the corrected production
  source. The new executable is Mach-O arm64 and links AppKit/CoreText and the
  other native system frameworks. Its SHA-256 is
  `8506ac0f774c11549c316fe050680f4ac5fd3f5ab6ced2051b28e89dc6ffb122`.
  `native-layout-build.log` and `native-layout-build-receipt.json` bind this new
  build. This worktree did not launch, package or install it; neither renderer
  geometry nor a native build proves native pixel/AX/target acceptance.

## Fresh independent entry-layout follow-up review

Frozen `cdaa20210dcd639c93e4324e7e8a882877090004` passed independent
29 app workflow tests, formatting, direct dependency policy and whole-workspace
all-target strict Clippy with `RUST_MIN_STACK` unset. No concrete P1/P2 remained
in the five-file correction. The actual GPUI mouse helper dispatches move/down/up;
the two held SSH fixtures each observed one original exec with no replay or PTY
writes while both command panels were hidden, reopened and cancelled. The
textarea setter checks draft preservation, not native keyboard acceptance.

Root verified all 14 evidence entries and unchanged HEAD/clean status/438 tracked
hashes in `work/workflow-ui-fix-review-20261005/`. Report SHA-256:
`6351871975bab109fad5bfabf1f7ef6bb6a57bfd2c42583ae2df0310990aa6ef`; evidence manifest
SHA-256: `aca8f7009b838b81f998baee1c197195cb084ee7cdcd0235e52f1ee2ed9b715e`.
The earlier 128-task/32-target copied-source and OpenSSH runs remain evidence of
the original freeze, not new native or final merged-source acceptance. Final
shared-hook checks and native behavior are tracked in the
[integration record](2026-10-05-workspace-workflows-integration.md).
