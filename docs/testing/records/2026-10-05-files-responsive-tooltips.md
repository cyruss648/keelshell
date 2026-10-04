# Responsive file controls and live tooltips — 2026-10-05

This slice starts from main `3a2df35f88466c650e86056c20bf67f23f92a7f4` in an
independent managed worktree and the pinned Rust 1.98.1 toolchain. It does not
claim the baseline CI or existing app bundle validates the new source.

## Observed baseline

The integration owner saved and reopened native screenshots from controlled SSH
and SFTP, with no model provider configured or context sent:

- `work/ui-native-baseline-20261005/screenshots/03-files-ai-system-en-wide.jpg`:
  1440×900 content, System resolving Dark, English, AI open. The final comparison
  action is visibly clipped and the name/mode drafts are compressed.
- `05-resize-attempt-light-en.jpg` and `06-files-ai-light-zh-minimum.jpg` in the same
  directory: genuine 900×580 content reached by dragging the native window edge.
  The smaller file area clips actions and compresses the name field to an
  unusable width. These are baseline evidence, not acceptance of this patch.

The slice author reopened frames 03 and 06 from disk. The root integration
record retains the exact pixels, receipts, equivalent production source boundary
and cleanup; this slice does not label a prebuilt baseline as a new build.

## Behavior tests

| Test | Evidence and boundary |
| --- | --- |
| `bilingual_file_actions_fit_workspace_budgets_with_real_assistant` | Production FilesPanel and real AssistantPanel against a controlled TCP SSH peer; workspace monitor-width and tool-height budgets; 16 combinations of two languages, production themes, sizes and AI visibility; eleven actions reached by real platform wheel input, 180px name/118px mode draft widths, an actually listed SFTP first row completely inside the list/table/browser vertical bounds. This is genuine GPUI layout with a scene wrapper, not native pixels or full workspace acceptance. |
| `wrapped_file_actions_still_review_the_exact_selected_remote_target` | At 900×580 with AI open, selects an actually listed SFTP file; clicks create/rename/delete/permissions/upload/download in both languages; checks exact source/destination/mode in pending review; cancels and verifies unchanged remote bytes and no local download. No reviewed mutation executes. |
| `already_open_tooltip_retranslates_without_pointer_movement` | A real hover opens a painted Kit tooltip. GPUI accessibility metadata and measured bounds are observed, then English→Chinese→English updates its label while the mouse position is identical. Uses GPUI's bounded test clock; no production delay/focus/hover override or extra provider. |
| `real_file_rows_and_controls_survive_transfer_comparison_editor_and_review_states` | 176 production Light/Dark scenes across the same size/language/AI matrix and eleven actual workflow states. Selects a painted first SFTP row, horizontally scrolls to its Edit button, opens its real bytes, reviews a long-path upload, confirms/pause/resumes and reads back 768 KiB, reviews/cancels exact permissions, performs directory comparison, previews a draft diff, reviews/cancels save and suspends the snapshot. Every expected state control must exist, become fully visible inside the file area through platform wheel input, and leave the real first data row inside its viewport. |

## Independent review and vertical-budget correction

Independent read-only review of `85eb16a` found one P2: non-shrinking wrapped
toolbars and secondary cards consumed the fixed file height. The original
`>=28px` browsing assertion accepted only a table header. Its valid production
Light probe measured 40 scenes, with violations in 20. Completed transfer and
directory comparison states could remove the browsing area or move controls
below the window. The original review report, raw measurements and SHA manifest
are retained under `work/file-workspace-review-20261005/` in the integration
checkout; its early compilation/default-Kit diagnostic runs are not used as
evidence for the defect.

The corrected implementation reserves 64px for the browser and bounds a separate
vertical action viewport. Natural-height toolbars, editor, comparisons and
transfers scroll there; the complete review message scrolls within a 48px cap,
with Confirm/Cancel outside the action viewport. The revised assertion checks an
actual 28px data row against the list, table and browsing bounds after every
control is reached, rather than increasing an empty-region threshold.

`state-matrix-2.log` and extracted `state-matrix-2.json`, in ignored
`work/file-polish-followup-20261005/`, contain all 176 passing measurements.
Minimum measured browsing height is 64px and action viewport height is 50px.
The initial matrix log is retained separately. Its labels such as
`comparison-plus-transfer` were misleading: production `run` retires a transfer
when comparison starts and clears comparison when Read starts. The final matrix
opens the editor before uploading, so editor plus active/completed transfer and
permissions review plus completed transfer are real simultaneous states.
It then compares with the editor retained and asserts the previous transfer is
gone. No synthetic transfer/comparison coexistence or altered lifecycle is used.

The first follow-up action check still demanded simultaneous visibility in the
new scrolling layout and failed on Download. A fixture compile attempt omitted
the public `InputEvent` trait. Both raw logs remain; the separate corrected
action/first-row and state-matrix runs passed. The tests use actual platform
scroll events in the outer gutter to avoid bypassing hit-testing or scrolling a
nested textarea/comparison by selecting its internal ID directly.

The first full follow-up gate preserved two older test failures: the narrow sync
test did not scroll to comparison actions, and the pause test measured the whole
transfer card while it was below the scroll viewport. Those tests now first
expose the actual control/card by the same platform wheel path, then retain their
complete bounds, phase, byte-count, pause/resume and atomic-write assertions.
They do not accept a hidden button or discard their protocol outcome checks.

## Preserved failures

Evidence lives in ignored `work/file-polish-20261005/` and is hashed in
`evidence-manifest.json`. The first baseline attempts contain compilation errors
in the new fixture/API imports; they were corrected before interpreting a test
as layout evidence. `footer-baseline-red-4.log` is the valid unchanged-footer
failure: the English permissions action is not visible at 900×580 without AI.
The baseline production view and regression diff are retained beside it.

The first wrapped-control test then showed its Cancel button outside the compact
panel when toolbar and confirmation were both present. The production review now
replaces the toolbar without discarding input entities or drafts. Its red log is
retained as `wrapped-actions-green.log`; the passing rerun is explicitly separate.

Initial tooltip tests included an asynchronous window activation, which cancelled
hover after a real pointer event. Diagnostic logs captured `hover true` followed
by `hover false`. A public RootPlugin/provider experiment reproduced that test
setup problem and was fully removed. The final core-API test omits unrelated
activation and proves painted-tooltip translation with the pointer unchanged.
These failures are not presented as evidence of an upstream toolkit defect.

## Gates and remaining acceptance

The initial `85eb16a` implementation's `python3 scripts/check.py` completed with
exit 0. Its retained
`final-workspace-gate.log` records:

- Direct registry `x.y` dependency policy, six script tests and
  `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked -- --test-threads=4`: 1,000 passed,
  zero failed and ten opt-in tests ignored, including 333 app tests. The ten
  skipped tests require installed supplier CLIs (two) or a separately configured
  disposable OpenSSH server (eight); controlled TCP SSH/SFTP fixtures ran.
- The separate local-agent integration controller on a 2 MiB thread stack:
  all scenario assertions passed; no supplier CLI or model call occurred.
- `cargo build -p keelshell-app --locked`: native macOS development executable
  compiled successfully; `native-build.log` is retained. Compilation does not
  imply a package launch or desktop acceptance.

Cargo reported the existing future-incompatibility notice for transitive
`block 0.1.6`; the strict current-toolchain gate succeeded. No new dependency,
credential persistence, telemetry, MCP client or background remote action is
introduced by this slice.

The follow-up `python3 scripts/check.py` completed with exit 0, retained in
`work/file-polish-followup-20261005/final-workspace-gate-2.log`: dependency policy,
six script tests, formatting, all-workspace/all-target strict Clippy, 1,001 Rust
tests passed with zero failures and the same ten opt-in tests ignored. The app
suite now has 334 passing tests; the separate 2 MiB local-agent controller again
passed all scenario assertions. `files-gate-2.log` separately records all 21 file
UI/protocol tests passing, including the corrected older scrolling assertions.
The follow-up `cargo build -p keelshell-app --locked` also exited 0; its separate
`native-build.log` and executable SHA are retained in the follow-up gate receipt.
This is compilation evidence; the slice did not launch the new executable.

Independent review and the integration owner's new native macOS package still
need to verify the patched file/footer/tooltip in the real workspace. Windows and
Linux native desktops, customer systems, all transfer/editor states and complete
screen-reader/modal isolation remain unverified. This document does not close
those boundaries or reuse baseline screenshots as after-fix acceptance.
