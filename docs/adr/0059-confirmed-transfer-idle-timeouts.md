# ADR 0059 — Confirmed I/O renews a transfer's idle wait

Status: the metadata candidate remains blocked by the independent READ EOF counterexample. The narrow EOF author repair passed a new complete engineering gate and controlled OpenSSH checks. Fresh non-author review, combined main gates and a new native transfer run remain required.

## Problem

The connection timeout was also a whole-job limit for queued file and atomic
uploads and direct upload/download APIs. With a one-second timeout, a real
loopback upload issuing timely 90 ms WRITE replies stopped after about one
second. Both direct and queued callers retained an unknown destination, despite
previously confirmed progress. A transfer's size and cumulative duration should
not make an otherwise responsive request time out.

## Decision

Each transfer context has its own confirmed-I/O notification. A completed valid
remote mutation reply or actual successful local I/O renews the active idle
interval. Source reads renew it after successful local reads or valid SFTP data.
Read-only metadata and existing descriptor replies also renew the interval:
matched SFTP ATTRS/HANDLE/NAME/STATUS, including normal NoSuchFile/EOF, and actual
local metadata/source completion. Transport timeout, malformed packet and an
unfinished request do not. These helpers retain the original typed result and
never clear a pending destination mutation. Public checked-read deadlines and
explicit path admission/preparation deadlines remain fixed.

Observation is bound by a Tokio task-local scope only during each poll of the
owning operation future. It is not stored on the shared SSH/SFTP session. Nested
owners override and then restore the scope; siblings in join/select and spawned
children cannot renew one another. Successful channel-open, subsystem acceptance
and SFTP version replies are observed through the same scope; sending a request
is not completion. Shared direct/queue/tree/resume paths observe their OPEN,
FSTAT, read-only CLOSE, canonical path, parent-chain and snapshot replies.

Presentation events, byte counters alone, safe-point polls and other owners'
activity cannot renew it. Acknowledged user pause suspends the unused interval;
resume keeps ownership and the existing content revalidation policy. Cancellation
remains the first selected branch even during a pause or validation.

All queue job types use the connection timeout for active idle. Direct upload,
download and streaming atomic upload use the same context runner and do not wrap
it in another whole-transfer timeout. Direct atomic writes retain the existing
mutation scope's authority and path revalidation before each mutating request.
An atomic destination still publishes only after writable CLOSE and POSIX rename
have confirmed replies. This changes waiting policy, not command or file authority.

Full-content and full-tree read-only checks use explicit fixed 30-second validation
phases, including post-copy and post-pause checks. Their deadline cannot be renewed
by progress. The idle runner suspends only while this separately bounded read-only
future is active and starts a new idle interval after a successful validation.
A scope guard ends this phase on return, timeout or cancellation. These checks must
never contain a destination mutation. Existing entry/depth/size limits remain.
The historical 15-minute total transfer limit is replaced by these active-idle and
fixed read-only validation limits; directory merge execution has a separate policy.

## Completion and failure

The pending mutation marker is cleared only by a valid reply or observed local
completion. Resetting the idle wait does not clear it. A missing WRITE, writable
CLOSE or publication reply still yields an unknown result and preserves the exact
reservation IDs across applicable connections. Another job can continue without
renewing this wait. A late reply does not silently release quarantine. A missing
READ can yield a bounded known failure with a visible partial local file, because
it cannot launch a later destination write after cancellation.

No retries, automatic recovery, persisted queue, remote crash durability or
cross-process filesystem isolation are introduced. Tokio deadlines cannot preempt
synchronous filesystem calls; actual local completion and existing unknown-outcome
rules are retained. Public APIs describe the renewed idle semantics without
promising a wall-clock bound for arbitrary synchronous OS work.

## Evidence and remaining work

The unchanged production baseline, owned TCP reproductions, resulting regression
matrix and engineering checks are recorded in
[the idle-timeout record](../testing/records/2026-10-06-transfer-idle-timeouts.md).
The earlier native unknown-result run remains a failed acceptance boundary; its
source-based duration estimate is separate from actual TCP request counts. This
candidate must receive a fresh independent review and a new combined native
package/run before native transfer acceptance is claimed.

## Metadata counterexample and repair

A new non-author actual TCP probe rejected the earlier author candidate: with a
one-second idle budget and successive 350 ms LSTAT/REALPATH responses, direct
atomic upload timed out at 1.7106 seconds only 298 ms after its last valid reply.
The unchanged original target and a successful no-delay control were read back.
The new author first reproduced that failure on identical production bytes and
retains the original test body and blocked evidence. This phase does not turn
the earlier native failed run into acceptance. See the
[metadata record](../testing/records/2026-10-06-transfer-metadata-idle.md) for the
expanded owner, metadata, fixed-budget and real packet matrix.

## READ EOF completion boundary

A subsequent independent real TCP probe found two active download branches that
recognized matched READ EOF but exited their loop before notifying their owner.
With a one-second idle budget, 750 ms DATA/EOF replies and a 350 ms read-only
CLOSE, both file and tree downloads failed about 251 ms after a valid EOF reply.
Their 22 bytes were already present; complete bytes alone did not prove a
successful transfer. The author reproduced both failures on the unchanged
metadata production candidate and separately reproduced the same boundary with
empty files.

These two branches now notify the current transfer context before leaving the
READ loop. EOF is actual completed I/O with zero additional bytes. It renews
only that owner's idle interval and neither increments transferred bytes nor
clears a pending destination mutation. The task-local observation scope, fixed
admission and 30-second validation phases remain unchanged. READDIR EOF is
already observed by its typed read-only helper. Resume content EOF belongs to a
fixed validation phase; unexpected EOF during copying fails as a source change.
Those branches do not need a new active waiting policy.

The metadata candidate's previous successful author gate remains historical
engineering evidence and is not acceptance of this repair. The new source and
original nonempty test body, actual failures and empty-file controls are retained
in the [metadata record](../testing/records/2026-10-06-transfer-metadata-idle.md).
