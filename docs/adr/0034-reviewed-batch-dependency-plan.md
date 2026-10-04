# ADR 0034: Reviewed batch dependency plans

- Status: accepted
- Date: 2026-10-04

## Decision

The core library provides a bounded, transport-free directed acyclic graph for
remote task planning. Each task has a stable non-nil UUID, one non-nil target
binding UUID, exact command text and explicit prerequisite task UUIDs. Target
bindings belong to the caller's reviewed authenticated sessions; a UUID alone
does not authenticate an endpoint. The plan never resolves hosts, opens a
connection, writes state, starts a timer or executes a command.

A review contains 1–128 tasks, at most 32 distinct targets, at most 32 immediate
dependencies per task, at most 64 KiB of UTF-8 command text per task and at most
1 MiB in aggregate. Empty commands and terminal controls other than newline and
tab are rejected. Commands retain exact whitespace and newlines. Duplicate task
IDs, duplicate edges, missing prerequisites, self-dependencies and longer cycles
fail before a confirmation receipt exists.

Kahn's algorithm uses sorted UUID sets to produce deterministic topological
order; the smallest currently available UUID breaks ties. Dependency input order
is canonicalized. A versioned, length-prefixed SHA-256 fingerprint covers every
task ID, target binding, exact command byte and dependency edge. Reordering
semantically identical input preserves the fingerprint; modifying any reviewed
value invalidates the old token. The immutable plan consumes the matching token
to produce an acknowledgement receipt. The token is a review consistency value,
not an authentication credential or standalone execution authorization. The
caller must invoke confirmation only after an explicit, complete human review.

The receipt can create only an in-memory admission ledger. A pending task is
ready when all prerequisites have a confirmed-success receipt; otherwise it is
blocked. Failed, unknown or skipped prerequisites skip dependent pending tasks
in one bounded topological pass. A ready task must be explicitly admitted by the
caller and can receive one terminal observation. The ledger trusts the future
adapter's observations; it cannot verify a remote exit status. Cancellation
skips pending tasks and preserves running tasks until an adapter reports their
actual outcome. It never claims remote process termination.

Debug output omits command plaintext. Plans and ledgers have no serialization
or persistence implementation. Commands can still contain secrets, so callers
must keep them transient and must not log the review or persist it as metadata.
Fingerprints are non-secret correlation values; candidate low-entropy commands
may be guessable to someone who already knows their task and binding IDs.

## Integration boundaries

Existing `BatchCommandTemplate` rendering can produce each exact task command
before plan construction. A future UI must show those rendered commands and
their target bindings and dependencies, invalidate reviews on edits or session
replacement, and bind dispatch to the confirmed immutable plan. The existing
single-command `BatchAuditRecord` counts target outcomes, while this plan can
contain several tasks per target. Reusing those counts as task outcomes would
be incorrect; a future workflow audit needs an explicit task-level schema.

This decision does not implement a task editor, GPUI review flow, SSH dispatcher,
concurrency or retry policy, persistent task history, cron scheduling or timers.
Cross-platform native interaction and genuine remote execution remain separate
acceptance work. No product task orchestration completion is claimed by this
domain slice.
