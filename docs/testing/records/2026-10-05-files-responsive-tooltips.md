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
| `bilingual_file_actions_fit_workspace_budgets_with_real_assistant` | Production FilesPanel and real AssistantPanel against a controlled TCP SSH peer; workspace monitor-width and tool-height budgets; 16 combinations of two languages, themes, sizes and AI visibility; eleven actions fully visible, 180px name/118px mode draft widths, at least a 28px browsing row. This is genuine GPUI layout with a scene wrapper, not native pixels or full workspace acceptance. |
| `wrapped_file_actions_still_review_the_exact_selected_remote_target` | At 900×580 with AI open, selects an actually listed SFTP file; clicks create/rename/delete/permissions/upload/download in both languages; checks exact source/destination/mode in pending review; cancels and verifies unchanged remote bytes and no local download. No reviewed mutation executes. |
| `already_open_tooltip_retranslates_without_pointer_movement` | A real hover opens a painted Kit tooltip. GPUI accessibility metadata and measured bounds are observed, then English→Chinese→English updates its label while the mouse position is identical. Uses GPUI's bounded test clock; no production delay/focus/hover override or extra provider. |

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

`python3 scripts/check.py` completed with exit 0. Its retained
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

Independent review and the integration owner's new native macOS package still
need to verify the patched file/footer/tooltip in the real workspace. Windows and
Linux native desktops, customer systems, all transfer/editor states and complete
screen-reader/modal isolation remain unverified. This document does not close
those boundaries or reuse baseline screenshots as after-fix acceptance.
