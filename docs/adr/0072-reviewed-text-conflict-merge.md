# ADR 0072 — Checked remote text conflict merge and exact patch-to-draft

Status: independent source review found no open blockers; main integration and
native desktop acceptance pending.
Date: 2026-10-07.

The former editor retained content bytes, read again before save, and then used a
separate ordinary atomic writer. Stale content was rejected without a practical
three-way resolution path, and ordinary reviewed metadata was not retained through
publication. Existing reviewed MCP replacement already offered the stronger transport
contract, with a distinct 64 KiB budget.

Retain the editor's baseline tuple for existing UI/archive behavior and add the exact
checked RegularFileSnapshot. The core owns a bounded byte-preserving three-way line
merge and strict single-file unified patch application; it performs no I/O. Exact
LCS lengths use u16 for at most 4,096 lines and a bounded approximately 32 MiB table,
released between the draft and remote comparisons. Independent changed regions combine;
overlaps and ambiguous insertion boundaries require explicit choices. No generated
conflict markers are inserted into the user's document. Final resolution is bounded
and every conflict must have a choice.

UI retains the original draft while the background worker obtains the current checked
remote version and creates the merge. The selected conflict presents all three versions
and a preserved manual replacement buffer. Adopting promotes only the draft and observed
baseline, then a separate complete-text save review authorizes remote mutation. A
new read retires prior choices, and adoption requires no active operation or pending
review. The worker result retains its captured complete base bytes; receiving checks
the same path/base/draft identities, and baseline display uses that captured value.
It cannot replace the plan's base with a mutable later editor baseline. A
strict patch has exact file headers, ordered coordinates and exact context bytes; it
only produces a new draft and cannot grant write authority.

A new desktop transport API applies the same reviewed atomic replacement protocol
with an independent 1 MiB budget. MCP keeps its existing 64 KiB API. Parent/leaf/handle
checks, complete metadata/content comparison before staging and publication, current
authority and shared mutation/quarantine scope stay intact. Verified complete readback
updates the baseline; acknowledged publication followed by failed readback preserves the
old baseline and explains the different outcome. Type changes, links, unsupported atomic
publication and unknown prior writes fail closed. No automatic retry or direct truncate
fallback is added.

Conflict detail and diff text use individual original Label rows with two-axis scroll;
review actions stay outside that viewport. Inner text wheel gestures stop bubbling
after their scroll update, so the outer action list cannot move simultaneously.
The text viewport keeps a real side gutter: outer tool navigation remains reachable
when the independent inner text fills the small visible tools area. Confirmation
text retains the existing 48 px compact budget with all literal rows and both axes.
Session suspension drops approvals and stops
owned workers without destroying the draft or treating cancellation as rollback.
SFTP v3 is still observational, not remote compare-and-swap; atomic publication preserves
only ordinary rwx bits. Native target UI and external/customer acceptance remain separate
from unit, owned TCP/GPUI, build and source CI evidence.
