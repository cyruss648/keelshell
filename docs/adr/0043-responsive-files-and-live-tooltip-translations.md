# ADR 0043: Responsive file controls and live tooltip translations

Status: accepted for implementation, 2026-10-05; integrated native acceptance is pending.

## Context

The current macOS application reproduced a clipped English “Compare folders”
control at 1440×900 with the assistant open. A native 900×580 Light window also
compressed file-name and permission inputs and moved later actions outside the
visible file area. The application allocated 40% of window height to file tools,
so a horizontal-only fix also had to preserve the review controls in that height.
The source and screenshots are recorded in the [test record](../testing/records/2026-10-05-files-responsive-tooltips.md).

## Decision

Keep every existing operation, label, icon, stable element ID and reviewed
transport binding. Both file-action rows wrap within the available file width.
Their height follows the rows rather than staying fixed at 38px. The name and
octal-permission drafts retain 180px and 118px widths. Transfer actions cannot
shrink into clipped labels, and the transfer row no longer relies on horizontal
scrolling to expose its final action. The header limits the secondary host label
so the navigation input and primary navigation actions retain space.

Independent review of the first implementation (`85eb16a`) found that wrapping
still consumed the workspace's fixed 232px minimum file area. A 28px browsing
area showed only its header; completed transfers and comparisons could remove
the browser entirely. The corrected layout reserves at least 64px for the table
header, one actual 28px SFTP row and scrollbar space. A single bounded vertical
scroll area contains the action rows, editor, comparison and transfer cards.
The Kit scrollbar and ordinary GPUI scroll handle expose all controls
without allowing their natural height to push browsing or status outside the
file panel. The editor uses the full action-area width, so it no longer narrows
the file list when the assistant is open. Its content and diff retain their
existing drafts and reviewed save behavior.

When an operation awaits explicit confirmation, the pending review temporarily
replaces the two action rows. Its complete source/destination text, Confirm and
Cancel use the existing bounded review component. The hidden inputs retain their
entities and values; cancelling restores the controls and drafts. Review text is
complete in its scrollable viewport, capped at 48px; Confirm/Cancel stay outside
the action scroll area. Existing comparison and transfer controls remain
reachable by scrolling during review, including the comparison close action
that cancels its own pending sync. This budgets long review messages alongside
secondary states without losing a real file row. It does not change confirmation
targets or worker logic.

File diff previews use the application's semantic canvas/text tokens in both
appearances, extending ADR 0036's file-preview decision. Terminal ANSI/OSC colors
remain independent. Location/status summaries can elide long text within the
footer; transfer paths and complete operation text remain in their existing
review/details surfaces.

`LocalizedTooltipExt` attaches a public GPUI tooltip builder that owns a standard
Kit `Tooltip::element`. That element reads the current language during render,
so an already visible hint changes language without moving the pointer. It adds
no root plugin, provider, global hover setting, extra window or focus handler.
GPUI owns the hover delay, positioning, active tooltip and cancellation when the
owner disappears. Existing translated application tooltip callsites use this
extension; untranslated user metadata stays outside translation. One tooltip
builder is permitted per element.

The discarded provider experiment and initial failing tests are preserved as
diagnostic evidence. They were not evidence of a toolkit defect: asynchronous
activation of the test window moved its pointer and cancelled the hover. The
final fixture dispatches a real hover event and advances the toolkit's test clock
without an unrelated activation event. No delay or pointer semantics are changed
in the application to make the test pass.

## Verification and limits

Real GPUI rendering checks both languages, Light/Dark, 900×580 and 1440×900,
assistant on/off, all eleven file actions reached by platform wheel input,
fixed input widths and an actually loaded/painted SFTP data row inside the list,
table and browser bounds. The scene mounts production FilesPanel and AssistantPanel
and reserves the same monitor width; it does not claim to be a full workspace or
a native desktop screenshot. Further production-theme scenes cover actual
running/paused/completed uploads, directory comparison, editor/diff, pending
long-path upload/save and suspended snapshots. Tests click the first data row,
scroll to controls, confirm and read back uploaded bytes, pause/resume, open an
actual file, cancel save and verify the remote file and unsent draft. Comparison
starts a new workflow and retires the previous transfer card; these tests retain
that production lifecycle rather than inventing simultaneous comparison/transfer
cards. The exact-target cancellation regression remains in place.
The tooltip regression observes its actually painted role/label and bounds after
hover, then switches English→Chinese→English with an unchanged mouse position.

The frozen follow-up passed a fresh independent review: 21 Files UI/protocol
tests, the 176-scene production-theme matrix, a strengthened copied-source probe,
formatting, dependency policy and strict app Clippy. Integrated macOS native
screenshots remain pending. Native Windows/Linux, real customer servers, comprehensive screen
reader navigation and modal accessibility isolation are outside this slice.
