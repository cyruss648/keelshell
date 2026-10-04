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

When an operation awaits explicit confirmation, the pending review temporarily
replaces the two action rows. Its complete source/destination text, Confirm and
Cancel use the existing bounded review component. The hidden inputs retain their
entities and values; cancelling restores the controls and drafts. This prevents
the wrapped toolbar plus a long review from pushing approval out of a minimum
window and keeps the user focused on the exact pending operation. It does not
change the confirmation or worker logic.

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
assistant on/off, all eleven file actions, fixed input widths and at least one
remaining browsing row. The scene mounts production FilesPanel and AssistantPanel
and reserves the same monitor width; it does not claim to be a full workspace or
a native desktop screenshot. A controlled TCP SSH/SFTP test clicks the wrapped
controls, checks exact reviewed targets, cancels, and verifies no file mutation.
The tooltip regression observes its actually painted role/label and bounds after
hover, then switches English→Chinese→English with an unchanged mouse position.

The new code still requires independent review and integrated macOS native
screenshots. Native Windows/Linux, real customer servers, comprehensive screen
reader navigation and modal accessibility isolation are outside this slice.
