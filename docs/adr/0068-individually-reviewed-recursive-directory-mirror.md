# ADR 0068: Individually reviewed recursive directory mirror

- Status: isolated implementation candidate; new independent review and native acceptance pending
- Date: 2026-10-07
- Extends: [ADR 0066](0066-reviewed-bounded-directory-mirror.md)

## Context

A complete mirror must handle destination-only nonempty directory trees. Refusing
every such tree left the bounded increment incomplete. A recursive shell command
or a recursive filesystem removal would erase nodes never shown in the review.

## Decision

The core constructor now expands complete destination-only subtrees into exact
file and directory rows. It retains portable names, complete hierarchy, typed
objects and full size/SHA-256 content evidence. Copies create parents before
children; deletions remove children before empty parents. A version-two mirror
fingerprint includes this order and policy. Generic sync intentions still grant
no mirror removal capability. The complete approval lists every row, both roots,
direction, full fingerprint and irreversible deletion notice; there is no
100-row approval cutoff.

The existing shared tree owner binds one reviewed fingerprint and records a
deleted path only after its mutation acknowledgement and checked absence. Before
each subtree deletion it checks source absence at the outermost destination-only
directory, observes the complete remaining destination namespace and rehashes all
remaining regular files. A second namespace sweep detects additions, disappearance
and reappearing completed nodes during content reads. Observed unsafe namespace,
content, source absence or budget failures invalidate that owner's reviewed
plan; repairing the external file or restoring source absence cannot silently
restore its approval. Any admitted mirror child refusal except Closed invalidates the
review, including I/O/protocol errors, incomplete observations and deadlines.
Invalid-plan and out-of-order refusals also stop this owner. This plan withdrawal
adds no global unknown-write quarantine for a failed read. Existing pending/STATUS
classification still determines known versus unknown writes. Closed authority
revocation and dropped read-only futures retain the existing session lifecycle. A fresh comparison, review
and owner are required.

Source absence is checked at the subtree boundary so missing source parents are
not misinterpreted as an SFTP inspection failure. Every consumed observation
checks the same session, authority and active reservation after its await. Root
and route claims are revalidated before final deletion observations. Remote
dispatch uses the fixed owner immediately after those observations, with no new
canonicalization wait that could consume stale subtree evidence. It keeps the
existing pending-write and transport-status classification; dropped replies or
lost mandatory readback retain quarantine. Local deletion similarly checks
authority after the final remote source observation and local target readback.

Nested checked-read and subtree async state is heap-owned at transport boundaries
so debug worker polls retain the standard thread stack; no larger stack is
configured to mask an observed overflow.

All removals remain one REMOVE, one RMDIR, one remove_file or one remove_dir.
Nothing expands an observed child into new permission, follows a link, retries,
reconnects, escalates privileges or performs rollback. Entry, depth, per-file,
combined byte, review-text and worker time bounds reject rather than truncate.
The depth is 32 nested directories, with a file allowed directly beneath the
last directory; content remains 64 MiB/file and 256 MiB across both sides.

## Consequences and limits

SFTP has no conditional unlink transaction; local pathname checks are not a
filesystem-wide lock. External writers can still race the final observation and
actual syscall/request, and the approval retains this explicit limit. A newly
nonempty directory receives RMDIR/remove_dir refusal; there is no recursive
fallback. Revalidating remaining files before every deletion adds I/O, so the
existing bounded worker deadline can refuse large or slow trees.

Completed, known-rejected, unknown and unstarted rows remain separate during
partial failure or cancellation. This in-memory journal is neither task-summary
audit nor execution authority. Controlled TCP/SSH/SFTP and headless GPUI checks
are engineering evidence. New independent review, new main-tree gate, native
macOS workflows and Windows/Linux desktop execution remain separate gates.
