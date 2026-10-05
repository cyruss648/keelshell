# Modal isolation — 2026-10-05

Status: the repaired candidate passed final root integration gates, fresh
independent GPUI input-lifecycle review and scoped macOS native checks. Original
author and intermediate candidates with confirmed failures remain unaccepted.
VoiceOver, stale native AX-object activation, IME and Windows/Linux native checks
remain open. See the [integration record](2026-10-05-ai-modal-mcp-integration.md).

## Implementation scope

The workspace mounts exactly one active root modal and retains background entities
and transports. Modal ordering, close dispatch and focus handling share one typed
selection. Authentication can interrupt another retained draft. Handshake progress
remains nonmodal. The actual Kit focus trap and completed-frame focus restoration
are used; no invented accessibility hiding API or framework patch is involved.

The scoped change includes `workspace/modal_scope.rs`, workspace composition,
main/library/form/command-library retained button focus, ADR0047 and MODALS.md.
Dependencies, toolchain, API/CLI inference modules and transport semantics stay
unchanged. Root-owned status/roadmap/readme updates are outside this worktree.

The current scope has its own 34-pixel title/appearance/language bar inside the
focus trap. Preference actions persist through the real settings path and retain
the same unsaved input entities. Saving and vault operations preserve their
disabled rules. No background toolbar is rendered to expose those actions.

## Evidence discipline

All commands use an owned target and temporary directory under the ignored
`work/modal-isolation-20261005/` directory. The initial build cache was copied
read-only from an existing target; it is not a symlink or shared writable cache.
Initial formatting differences and compiler diagnostics are preserved in
`format-initial.log` and `check-initial.log`. The first compile found an ambiguous
Render method and an unnecessary wrapped popover trigger that required Selectable;
these are implementation diagnostics, not passing gates. No native app or vendor
CLI is launched by this implementation agent.

Other failed stages remain available:

- `modal-tests-initial.log` and `modal-tests-second.log`: the headless platform
  returned no live AccessKit tree. The tests now check the actual registered
  element role/label and frame membership; they do not invent native AX results.
- `modal-tests-third.log`: the real Kit focus trap replaces the supplied outer
  element ID with the trap ID. The test uses the actual public trap identifier.
- `modal-tests-matrix.log` and `modal-tests-chrome.log`: missing test/selection
  trait imports, corrected using the locked Kit APIs.
- `app-tests-final.log`: 364 passed, 5 failed. The new tracked workspace root
  stole command-input focus on mouse-down; new-layer default focus also needed
  explicit input/panel targets and could not rely on the previous layer's frame.
  These production regressions were repaired. Command insertion, snippet typing,
  vault return, profile close and profile save now pass their existing tests.
- `clippy-final.log`: 30 test-only `expect_used` errors. Test assertions now follow
  existing repository failure-reporting style. Production lint policy is unchanged.

Intermediate passing logs are retained with their actual narrower scope; they are
not substituted for the final complete application run.

## Verification

GPUI tests use isolated synthetic profiles and actual production workspace
rendering. Seven new tests cover all 19 selection variants through root, nested
library and asynchronous authentication cases. The additional root matrix renders
15 kinds across Chinese/English and Light/Dark at 900×580 (60 scenes), dispatches
Tab and Shift-Tab in each, and checks current controls and action-area bounds.
Nested tests dispatch both directions beyond a full cycle, restore the exact
keyboard-trigger handle through remounts and exercise Escape one layer at a time.
Global shortcuts, expired frame callbacks, background programmatic focus and
preference persistence use the same real production scopes.

The complete application suite also retains its existing loopback SSH, command,
route and credential tests. The global-shortcut fixture itself proves modal
dispatch and draft retention; it does not create a real SSH session.

All Rust commands below use the owned `CARGO_TARGET_DIR` and `TMPDIR` described
above. The freeze is based on `40d092ca61c4c229acb4a004d2852dd858e86aed`.

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo test -p keelshell-app --locked -- --test-threads=4` | Repair run: 369 passed, 0 failed, 0 ignored; 86.51s | `app-tests-repair.log` |
| Same complete app command after the final test-only assertion adjustment | 369 passed, 0 failed, 0 ignored; 66.82s | `app-tests-frozen.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Exit 0; 6.69s | `clippy-repair.log` |
| `cargo fmt --all -- --check` | Exit 0 | `fmt-final.log` |
| `python3 scripts/check.py --policy-only` | Exit 0 | `dependency-policy-final.log` |
| `git diff --check` | Exit 0 | `diff-check-final.log` |
| Scoped Markdown links and content policy | Exit 0; 3 documents checked | `document-policy-final.log` |

The ignored `source-manifest.json` and `evidence-manifest.json` bind all tracked
and newly added source/configuration/documents and each preserved log. Source gates
were performed in this isolated worktree; integration gates must run against the
combined source after independent review. Failure evidence stays in the ignored
owned directory and is not published as application data. No commit, main-worktree
write, native GUI operation or external-agent invocation was performed by this slice.

These checks inspect GPUI nodes, key dispatch, focus and layout bounds. They do
not measure native pixels. No live AccessKit adapter was activated by the
headless test platform. The [native checklist](../../design/MODALS.md) is still
required for VoiceOver,
Narrator and AT-SPI. Windows/Linux native acceptance, cloud inference, production
SSH and installed application update are not established by this slice.

## Independent failure and root repair

The first independent review verified 22 author proofs and 463 source inputs.
Its complete application suite passed 369 tests, but a raw mouse-up dispatched
before drawing a new MFA layer still activated the old connection Cancel button
and destroyed the retained draft. The hovered-hitbox condition on Div capture
prevented the root guard from running behind `occlude()`. Test helpers that draw
before dispatch did not exercise this window. The original independent report is
`FAIL_PENDING_AUTHOR_FIX`, with 41 immutable proofs; it is not a passing review.

Root reproduced this exact defect: the permanent regression failed exit101,
log SHA `66cccedfce3c09c3b25521915d67bb7ffc7c5b2f295251eac177677c617adeb8`.
The first window-level capture repair passed this test in both pointer-down
orders, preserving the draft and normal clicks after remounting. A second raw
test then proved that the same modal kind, route and hop with zero-field MFA
could still let the old Submit consume a replacement challenge; it failed
exit101, log SHA `5a0e15cad06930971cc5748af62ad4c4204c2723ce732cc9e212617c9cd74e7b`.
Both root runs froze their inputs and retain their original failures.

The final repair registers raw window capture before child paint, without the
ancestor hitbox requirement, and binds each fresh ephemeral prompt UUID. Existing
route checks, secret clearing, focus trap and disabled save behavior remain in
use. See [ADR0047](../../adr/0047-single-active-modal-and-retained-focus.md).
The earlier root full gate passed on the known-defective candidate (372 app
tests); that result cannot accept the repaired source. Root is running a new
complete gate and new independent source scope before native verification.

The UUID-only candidate subsequently passed a root full gate and independent
374 app tests, but fresh review reproduced held Enter and Space transferring
to a replacement challenge at key-up. A repeated mouse release after repaint
also consumed the new challenge without a new press. The SDK stores pending
activation against focus generation and persistent element state; the UUID
check on the new frame did not clear that earlier state. These are confirmed
failures, not speculative findings. The second independent scope remains FAIL,
with Enter/Space/repeated-release logs preserved; its earlier app PASS cannot
accept this candidate.

The final candidate gives each challenge a fresh content element namespace and
advances focus on challenge replacement. The outer trap and business entities
remain stable. Eleven modal tests pass on frozen root inputs, including four
permanent raw-event regressions. Final root whole-workspace gates passed 1053
ordinary and 8 documentation tests. A new independent scope replayed all eight
private cases, including the original Enter, Space and repeated-release failures,
with unchanged assertions: PASS. Its 28 proofs were copied and verified by root;
the original 41 and 33 failed proofs remain intact. Scoped macOS native AX-tree,
keyboard-trigger return, retained draft and SSH echo checks also passed, as
recorded separately; this does not accept native screen-reader or stale AX-object
activation. An intermediate compile
failure (Div versus Stateful<Div> in a conditional builder) is preserved; the
builder now constructs its ID before styling. No lint or behavioral assertion
was weakened.
