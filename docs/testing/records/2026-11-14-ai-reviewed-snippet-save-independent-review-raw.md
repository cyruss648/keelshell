# Final independent review: AI reviewed command saved as snippet

- Review type: second strict read-only independent review
- Review date: 2026-11-14 (Asia/Shanghai)
- Source checkout: `/Users/hdh/.codex/worktrees/d362/keelshell`
- Exact reviewed HEAD: `50ca0ead2fead7b0e92679ee9d531604ae040772`
- Required baseline: `b89445604b4ce44fc5a80ff26605f6e6b9039d57`
- Independent Cargo target: `/Users/hdh/.codex/worktrees/d362/keelshell/work/review-final-50ca0ea/target`
- Independent logs: `/Users/hdh/.codex/worktrees/d362/keelshell/work/review-final-50ca0ea/`
- Working tree tracked-file status: clean before and after review; only ignored review artifacts were written.

## Conclusion

**APPROVE**

Finding counts:

| Priority | Count |
| --- | ---: |
| P0 | 0 |
| P1 | 0 |
| P2 | 0 |
| P3 | 0 |

No actionable finding was identified in the reviewed slice.

## Scope and evidence review

### P0/P1: review admission and event boundaries

`AssistantPanel::review_and_emit_snippet_save` constructs the save proposal, calls
`CommandProposal::review`, consumes the resulting ticket with the exact proposal and
session identity, stores a process-local pending record, and emits `AssistantEvent::SaveSnippet`
with the command, original target, session identity, and one-use review UUID. The relevant
implementation is at `/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/assistant.rs:1847`.
The pending record is consumed only when all command, target, session, review ID, response
binding, and current context fields match; successful consumption clears it at
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/assistant.rs:1888`.

Suggestion and diagnostic-step save paths both reach that helper only after their own
proposal/diagnostic checks. The event handling path delegates to Workspace admission and
opens a draft; it does not write StateStore directly. The relevant event dispatch is at
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/workspace.rs:804` and
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/workspace/commands.rs:598`.

The one-use and target-binding behavior was exercised by:

- `assistant::tests::command_review_target::reviewed_snippet_events_carry_target_and_are_consumed_once`
- `assistant::tests::command_review_target::stale_response_target_and_plan_cannot_rebind_to_changed_panel_session`
- `assistant::tests::command_review_target::exact_captured_command_and_diagnostic_step_emit_only_after_manual_click`

### P1: Workspace second admission and fail-closed behavior

Opening a draft checks save/reload/recovery state, configuration validity, the live active
SSH terminal, terminal open state, session identity, live target, and modal scope before the
pending review is consumed and the editor is created. This is implemented at
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/workspace/commands.rs:495`.
The commit path checks the retained binding again, including configuration state, exact
SnippetEditor modal scope, active terminal identity, session identity, target, and terminal
open state, before building a candidate StateStore snapshot at
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/workspace/commands.rs:426`.
Configuration currentness includes closing and invalidation state at
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:20`.
The modal registry treats close review, closing, cleanup-unknown, and all competing panels as
active modal scopes at
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/workspace/modal_scope.rs:255`.

The focused GPUI tests passed for target switching, terminal retirement, and StateStore
conflict retention:

- `workspace::tests::ai_snippet_save::target_switch_after_opening_ai_draft_rejects_save_and_drops_candidate`
- `workspace::tests::ai_snippet_save::closing_ai_target_after_draft_open_keeps_no_save_authority`
- `workspace::tests::ai_snippet_save::state_store_conflict_keeps_complete_ai_draft_and_error`

### P1/P2: locked reviewed body, confirmation, and failure retention

`SnippetEditor::new_reviewed_draft` stores the reviewed command snapshot and disables the
command input. Save rejects any internal command mismatch, requires a first confirmation
state, and emits `SnippetEditorEvent::Save` only on the second confirmed click. These checks
are at
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/snippet_editor.rs:82` and
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/snippet_editor.rs:244`.
StateStore failure uses `set_error`, which clears the saving state while retaining the exact
input values and error; the command remains locked for the AI flow. That behavior is at
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/snippet_editor.rs:193`.
The explicit re-review event is separate from save and does not execute the command.

The editor regression passed 12 tests, including:

- `reviewed_ai_draft_locks_command_allows_metadata_and_requires_second_confirmation`
- `reviewed_ai_draft_rejects_changed_command_and_exposes_explicit_rereview`
- `save_freezes_exact_snapshot_and_failure_restores_the_draft`
- `metadata_inputs_expose_bilingual_accessible_names`
- `actual_small_editor_window_keeps_template_feedback_and_footer_inside_modal`

### P1/P2: persisted data and side effects

The persisted object is the existing `keelshell_core::Snippet`, whose fields are only ID,
name, command, parameterized flag, description, and tags at
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-core/src/model.rs:218`.
The process-local Workspace binding contains session/terminal/target/review identity but has
no serialization path. The AI snippet flow writes through the existing background
`Workspace::persist` path at
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/workspace.rs:2428`.
The focused end-to-end test verified that a saved command became searchable, that the
complete metadata round-tripped, and that both deterministic SSH panes observed no writes.
The diagnostic-step cancellation test also verified no persisted snippet and no execution
side effect.

### P2/P3: bilingual surface, themes, layout, and accessibility

The editor uses the shared design palette and component tokens, without business branching on
theme IDs. It has a fixed footer, independently scrolling form, bounded error/template areas,
bilingual text, stable accessibility IDs, and bilingual explicit `aria_label` values for
name, description, tags, and command at
`/Users/hdh/.codex/worktrees/d362/keelshell/crates/keelshell-app/src/snippet_editor/view.rs:64`.
The focused surface test covered both languages, all registered Frost/Graphite/Lavender
families, light/dark variants, the locked-body notice, fixed footer, and visible actions.
The GPUI accessibility regression asserted the actual accessible labels through
`window.find_by_label` for Chinese and English.

## Commands actually run and results

All Cargo commands below used the independent target and `--locked --offline` where shown.
Output was captured in the independent log directory.

1. `git rev-parse HEAD`
   - Result: `50ca0ead2fead7b0e92679ee9d531604ae040772`; exact match, so review continued.

2. `CARGO_TARGET_DIR=work/review-final-50ca0ea/target cargo test -p keelshell-app --bin keelshell-app --tests snippet_editor --locked --offline -- --nocapture --test-threads=1`
   - Result: `12 passed / 0 failed / 0 ignored / 1144 filtered`; exit 0.

3. `CARGO_TARGET_DIR=work/review-final-50ca0ea/target cargo test -p keelshell-app --bin keelshell-app --tests ai_snippet_save --locked --offline -- --nocapture --test-threads=1`
   - Result: `6 passed / 0 failed / 0 ignored / 1150 filtered`; exit 0.

4. `CARGO_TARGET_DIR=work/review-final-50ca0ea/target cargo test -p keelshell-app --bin keelshell-app --tests command_review_target --locked --offline -- --nocapture --test-threads=1`
   - Result: `6 passed / 0 failed / 0 ignored / 1150 filtered`; exit 0.

5. `CARGO_TARGET_DIR=work/review-final-50ca0ea/target cargo test -p keelshell-core --locked --offline -- --nocapture`
   - Result: `634 passed / 0 failed / 0 ignored`; this includes 212 unit tests and 7 doctests; exit 0.

6. `cargo fmt --all -- --check`
   - Result: exit 0.

7. `CARGO_TARGET_DIR=work/review-final-50ca0ea/target cargo clippy -p keelshell-app --bin keelshell-app --tests --locked --offline -- -D warnings`
   - Result: exit 0; strict Clippy for the app test targets passed.

8. `git diff --check b89445604b4ce44fc5a80ff26605f6e6b9039d57 50ca0ead2fead7b0e92679ee9d531604ae040772`
   - Result: exit 0.

## Unverified boundaries

This independent review did not execute or sign off on:

- macOS real window-level native GPUI acceptance, VoiceOver, IME, or the complete assistive-technology matrix;
- Windows/Linux native desktop acceptance, input methods, or assistive technologies;
- real customer SSH, host-key-change field behavior, Podman integration, or external AI provider requests;
- cross-process StateStore, power-loss durability, full Agent client flows, or CI runner behavior;
- code signing, notarization, packaging, installation/upgrade, or release artifacts;
- the author's full `scripts/check.py` gate or the author's reported workspace-wide test/Clippy counts.

The linker warning `__eh_frame section too large` appeared while linking the app test binary;
it did not change any exit code. No tracked project files or project files outside the allowed review directory were modified,
staged, committed, pushed, merged, deleted, or cleaned by this review. The only written artifacts
are ignored logs and this report under
`/Users/hdh/.codex/worktrees/d362/keelshell/work/review-final-50ca0ea/`.
