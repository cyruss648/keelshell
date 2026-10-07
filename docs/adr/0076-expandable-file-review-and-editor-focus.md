# ADR 0076 — Expandable file review and explicit editor focus

Status: author scoped validation and a fresh non-author source/behavior review
passed. Main-tree combination gates, exact-commit CI and native acceptance remain
open; the separate fixture-setup correction requires its own combined review.
Date: 2026-10-07.

The controlled macOS text-merge walkthrough exposed a review readability limit:
all save text was present in the AX tree, but the approximately 112 physical-pixel
body showed only segments of the three-line result with its metadata. This does
not establish a broken vertical scroll mechanism. The earlier two-axis fixes and
their original failed evidence remain valid within their own scope. A mistaken
first input into the directory field was recovered by the operator; it is not
proof of a product focus defect. The offscreen patch AX setValue did not establish
keyboard reachability.

Keep the existing 48 logical-pixel compact review as the default so browsing is
still available. Add an explicit “Expand review” entry that uses the file pane for
a full review surface. A fixed title separates the context from its scrollable
body; a fixed footer retains the original Confirm and Cancel actions and adds
“Collapse review”. Every original line, including paths, hashes, empty lines and
literal shell-looking text, remains an unwrapped intrinsic-width Label with both
axes. Arrow keys, Page Up/Down, Home and End scroll the focused review body. Enter
on that body cannot approve. Expansion and collapse are presentation changes:
they never replace the pending message, operation, reviewed snapshot or bytes.
Each new proposal returns to compact mode and the beginning of its text.

When a file is open, a pinned, compact path row offers “Edit draft” and “Enter
patch”. These actions reveal a large editing surface and focus the corresponding
existing TextareaState. The directory field is never used as a fallback. The
surface keeps a fixed return action, draft/patch switch and review/apply actions;
it also exposes operation status. Returning to files keeps both input buffers.
The existing inline patch entry opens this same surface. Successful merge review
returns to the normal files view so the actual base/draft/remote choices remain
available. Draft and patch input share no new storage or execution mechanism.

The file pane alone changes view; terminal and assistant budgets are unaffected.
Minimum-pane constraints use min-height zero on the flexible body and nonshrinking
header/footer, instead of enlarging the compact bar until it consumes browsing
space. The expanded editor and review can return to browsing at any time.

All original save, patch, mutation exclusion, session lease, changed-draft,
suspension and checked snapshot guards remain in their original handlers. A patch
changes only the draft. Only an explicit save review followed by the existing
confirmation may write remotely. No worker, transport, API, capability or timeout
is changed; cancellation continues to make no rollback promise.

The evidence-based product audit considered readability, hierarchy, action
separation, minimum workspace budgets and correct keyboard targeting, using the
explicitly supplied original native screenshots. It did not recapture or operate
that app. New controlled GPUI tests exercise two sizes, Chinese/English and
System/Light/Dark with the real assistant, exact review lines and actual keyboard
input. These are not native pixels, VoiceOver, customer SSH, Windows/Linux desktop
or release acceptance. See the [guide](../product/FILE_REVIEW_AND_FOCUS.md) and
[author record](../testing/records/2026-10-07-file-review-viewport.md).
