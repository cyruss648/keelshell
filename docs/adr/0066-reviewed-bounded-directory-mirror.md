# ADR 0066: Reviewed bounded directory mirror deletion

- Status: bounded increment implemented; subtree policy extended by [ADR 0068](0068-individually-reviewed-recursive-directory-mirror.md)
- Date: 2026-10-06

## Context

Directory merge preserves destination-only entries. A mirror needs an explicit
source → destination choice and reviewed deletion intent, without turning the
generic domain `IncludeDeletes` plan into an unreviewed removal capability.
The existing application mutation owner reserves both trees; active or unknown
mutations, changed host identity and revoked authority already fail closed.

## Decision

Add separate local → remote and remote → local mirror controls. Planning is
read-only. A dedicated core constructor requires complete SHA-256/size snapshots,
portable paths, explicit file/directory types and hierarchy evidence. Its private
bounded-mirror flag and distinct full review fingerprint cannot be fabricated
through a generic synchronization plan. Collect every observable conflict in a
stable list. No 100-row preview limit applies to mirror conflict or approval rows.

The first increment permits regular files and directories that are already empty
in the destination snapshot. Any destination-only nonempty directory refuses the
whole plan. There is no recursive remove primitive, implicit subtree expansion,
symlink following or automatic permission escalation. Existing copy operations
retain their checked temporary staging, atomic publication and content readback.

The confirmation panel names both canonical roots, direction, deletion count,
full plan fingerprint and every operation's path, type, size and available full
content digest. The complete bilingual review has a 128 KiB bound; oversized
reviews fail rather than truncate. Existing scan limits remain 10,000 entries per
side, depth 32, 64 MiB/file, 256 MiB total content and the existing scan/execution
budgets. Replanning consumes no write authority.

After human confirmation the worker captures both roots under the existing tree
owner, rebuilds and compares the complete plan, then rechecks each operation.
Deletion only accepts an exact `Delete` row in the privately bounded confirmed
plan and the matching owner direction. Immediately preceding a single REMOVE,
RMDIR or local filesystem deletion it checks source absence, parent chains,
destination type and complete file content or directory emptiness. A protocol
acknowledgement and checked target absence are required for completion. A received
STATUS refusal is a known failure; transport loss, dropped pending work or lost
required readback retain unknown-result quarantine. Copy/create readback also
keeps the owner pending until its observation succeeds.

Maintain an in-memory per-item journal: not started, verifying, unknown,
completed, rejected, cancelled before write, or skipped after failure. Mark an
item conservatively unknown immediately before its mutation stage. Cancellation
never changes previously completed or unknown rows; a failed SFTP channel close
is separately visible. The journal has no retry, replay, recovery or execution
entry. New plans replace the display; application restart does not prove that an
old remote operation stopped. This journal is distinct from task-level persistent
summary audit and stores only the current reviewed filesystem operation.

## Consequences and limits

SFTP v3 has no compare-and-swap deletion or atomic no-follow transaction. Local
path-based observations are also not a filesystem-wide lock. Per-item rechecks
refuse observed changes but cannot exclude every external rename or writer in
between the final check and syscall/protocol request. The confirmation explicitly
states this boundary. RMDIR's server refusal protects a newly nonempty directory
without recursive fallback. No whole-run rollback, automatic retry, reconnect,
restoration or broader authorization is claimed.

Some cancellations can stop before dispatch while the conservative item journal
already says unknown. This is intentionally distinct from proof of a remote
mutation: actual transport pending/cleanup state controls quarantine. Known,
completed and unknown results stay separate.

Meaningful core, controlled TCP/SSH/SFTP and headless GPUI tests cover both
directions, exact policy/paths, content or type changes, source appearance,
conflict completeness, rejected RMDIR, actual pending deletion cancellation and
shared quarantine. These are engineering evidence; desktop/native acceptance,
external concurrent-writer guarantees and Windows/Linux execution remain open.
