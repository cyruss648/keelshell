# ADR 0055 — Session-bound parallel remote transfers

Status: independently reviewed and integrated in the working copy, including writable-CLOSE isolation and subsequent READ EOF idle-accounting repairs. The combined-main gate, OpenSSH checks and fresh standard macOS package inspection passed. A fresh controlled macOS two-upload scene verified simultaneous Running, two fully visible Completed rows, separate task paths, acknowledged pause/continue, queue reachability, exact contents and independently checked owned cleanup. Other transfer modes, minimum-window and other-platform native acceptance remain open; see the [native record](../testing/records/2026-10-06-parallel-transfers-native-v5.md). The v1 application-writer, v2b writable-CLOSE and original zero-completion native failures remain retained. See the [combined record](../testing/records/2026-10-06-remote-workspace-main-integration.md).

## Decision

Files retains the authenticated SSH connection and session token that received the
user's approval. Upload, download, reviewed directory and reviewed continuation
jobs join one in-memory queue. The panel permits further read-only navigation,
planning and transfer reviews while jobs run. Its foreground busy policy can defer
mutations in that panel, but it is not resource authority. All existing application
file writers share transport admission across panels and approved MCP actions.
Confirmation binds the exact copied
operation, rather than rereading another job's edited path draft.

The user selects 1–4 parallel workers in Files; the default is 2. The transport API
keeps its previous default of 1. A concrete SSH connection permits at most four
active transfer reservations, including queues created by other callers. A queue
admits at most 32 unfinished jobs and returns an error immediately when full.
Each job owns its events, terminal lane, cancellation and acknowledged pause
control. Running jobs open independent raw SFTP channels; this is real concurrent
I/O, not a group of sequential calls. Pause keeps a worker slot and its path
reservations. Decreasing concurrency changes future admissions and does not stop
already active jobs. Independent queued jobs may pass a conflicting earlier job;
other conflicting jobs retain their relative order.

## Paths and publication

Reservations include local and remote source and destination. Two reads can
share a source. A write excludes the same path and all descendants/ancestors.
Local paths resolve the closest existing ancestor; remote paths use the server's
canonical ancestor. Reservations are rechecked before I/O, reject parent
traversal and fold case conservatively across platforms. A changed resolution
fails before transferring. SFTP is not a compare-and-swap filesystem; mutations
by other programs after validation remain outside this queue's isolation.

Ordinary UI file uploads now stage a bounded stream in an exclusive same-directory
temporary and use negotiated OpenSSH POSIX rename to publish. Different existing
regular-file destinations can run concurrently, including distinct hard-link
names: publication replaces each pathname rather than writing their shared inode.
Symbolic-link target entries and unavailable atomic-rename capability fail closed. Existing rwx bits
are retained; ownership, ACLs and special mode bits are not preserved. Publication
has atomic visibility, not crash durability. Downloads retain create-new local
output and may leave a partial local file. Directory jobs and explicit
continuations preserve their previous in-place/new-tree policies.

Legacy in-place uploads and continuations to an existing destination reserve
that entire filesystem side, because portable SFTP does not identify hard-link
aliases. This deliberate restriction does not apply to ordinary atomic UI file
uploads or new local downloads. Parent/child tree overlap is always excluded.

## Unknown outcomes

Each mutating OPEN, mkdir, WRITE, writable CLOSE, local creation/write/flush and atomic rename
tracks whether a reply/completion has arrived. Cancelling before mutation or
between acknowledged safe points can report an acknowledged cancellation.
Cancellation/timeout while a mutation is pending emits `Uncertain`, counts only
previously acknowledged bytes and quarantines its destination. A STATUS rejection
is a confirmed reply; loss of the reply is not.


Writable CLOSE is a destination completion request even after every WRITE has a
reply. The [SFTP v3 draft, section 6.3](https://datatracker.ietf.org/doc/html/draft-ietf-secsh-filexfer-02#section-6.3)
permits CLOSE to fail while flushing cached writes and invalidates a handle when
CLOSE is sent. Direct writes, queued uploads, directory upload children and
continuation writable targets therefore keep the same mutation owner and pending
marker until a valid CLOSE reply. Atomic temporary CLOSE uses the same marker;
cleanup clears its descriptor only when that CLOSE operation is polled after
preflight authority checks, so an actually sent CLOSE is never retried. Earlier
cancellation leaves the still-owned handle available to the existing bounded
CLOSE/REMOVE cleanup ticket. Missing cleanup CLOSE acknowledgement retains both
final/tree and temporary IDs. Late handler completion cannot remove those IDs.

A valid STATUS refusal is a known failure, including writable CLOSE; a dropped,
missing or invalid reply remains unknown. Full acknowledged byte progress is not
proof of CLOSE, publication or durability. A checkpoint cancellation before CLOSE
is polled can be known. Read-only listing, source and planning descriptors retain
their separate raw CLOSE path: stopping one cannot invent a remote-write unknown
record. Resume handles record whether they were opened writable and require the
transfer context only for the target CLOSE at completion or pause rebind. Existing
conservative inode-side locks for in-place writes/continuations are unchanged;
ordinary atomic namespace writers retain useful distinct-target concurrency.

The bounded registry is process-wide. Local destinations share reservations
across all connections. Remote claims share configured normalized host, port and
explicitly verified host key. Sharing across principals and routes is deliberately
conservative; different DNS/IP aliases or host-key spellings are not inferred.
All existing application mutators participate: ordinary save/create/rename/delete/
permissions, direct SFTP upload/download/write APIs, reviewed MCP replacement and
directory synchronization. Other processes are outside this reservation mechanism.
Physical sharing between local and remote filesystem namespaces, unknown host
aliases and all source hard-link relationships are not claimed fully identified.
A queue or a new SSH connection cannot bypass a matching unknown destination.
Independent targets continue. At most 32 active-plus-unknown action groups are retained process-wide. One action
has at most two write claims: rename endpoints, or destination plus its exclusive
temporary. A directory sync reuses one source/destination tree owner and retires
each acknowledged child temporary, rather than consuming a slot per file. The
queue waits for active ownership and rejects quarantined overlap; direct mutators
return typed busy/quarantined errors immediately. Each concrete connection permits
at most four active action owners, including direct mutators and other queues.

Read-only inspection remains available. The explicit isolation action shows the
complete canonical target, exact process-unique reservation ID, existence, type,
size and available modification time. The user must review the unresolved risk:
inspection and reconnection cannot prove a late WRITE/rename has stopped. Explicit
consent removes only those reviewed application reservations, changes no file,
retries nothing and leaves the original job unknown. Any new/removed quarantine
in the applicable remote scope or local registry invalidates the review. Current
connection identity, revocation flag and observed metadata are checked again
before release. A late old completion cannot remove a new reservation ID. This
is an explicit risk acknowledgement, not rollback or completion verification.
Quarantine is in memory, not a durable recovery journal: an application restart
loses it and does not prove old remote work stopped.

An exclusive temporary has its own exact path/ID under the parent owner. Its
cleanup retains the same final/tree reservation until a real reply arrives. A
cleanup timeout/abort quarantines both the final/root and owned temporary. A
confirmed cleanup rejection can leave a temporary; a known reply is not a claim
that deletion succeeded. Late
cleanup never clears a published unknown ID. Safe terminal results await bounded,
event-driven owned cleanup. Only valid mutation reply types or actual local
completion clear pending I/O; an unexpected atomic-rename packet remains unknown
even when a read later sees published bytes. Raw in-place writes use two bounded
32 KiB requests per batch and observe both replies before classifying a rejection.

Reviewed MCP replacement checks its authorization lease after preflight awaits
and before each write; ordinary proposal approval cannot release quarantine.
Directory sync holds both canonical trees through fresh plan validation and
all local/remote child writes. Local closures run on the transport worker, finish
synchronously, stay inside the reserved tree and launch no detached work. A scope
rejects concurrent child use and stops admitting child writes after quarantine.
Authorization callbacks must be quick, synchronous authority checks; they do no
I/O and do not run on the UI thread.

Closing an SFTP owner stops admission and requests cancellation of its queues and
direct raw writers. It preserves the distinction between pre-I/O cancellation and
a pending remote result; it does not certify rollback. Queue retirement cancels
all admitted jobs and drains the owned scheduler;
handles receive their actual per-job result. A queue drop cannot promise remote
rollback. If the UI fails to receive a terminal result within its existing
cancellation-drain deadline, it displays an unknown result instead of inventing
an acknowledged cancellation. No queue, command or transfer is restored,
reconnected, retried or replayed automatically.

## UI and verification

The bounded scrollable queue shows each job's target, acknowledged byte counts,
waiting/running/pausing/paused/resuming/cancelling/completed/cancelled/failed/unknown
state and individual controls. Selecting a row exposes the full existing path
review and progress card. The separate file-isolation inspector covers ordinary saved/selected targets
without requiring a transfer-history row. Both inspectors display every compound
action path/ID before separate risk consent. Chinese and English labels read live locale; changing
language does not rebuild jobs or controls. Completed history is bounded in
memory. The queue belongs to the existing Files action scroll area and does not
remove the browser's reserved real first row or fixed approval controls.

True overlap is proven with held second-WRITE replies from two independent
SFTP handlers; neither barrier is released until both enter. Tests additionally
cover tree exclusion, canonical aliases, admission limits, pause slots, queued
cancellation, unknown late writes across queues/connections, shared local targets, explicit
risk-consent refusal on revocation/stale observations/new isolation IDs, exact session retirement and
OpenSSH hard-link replacement and guarded ordinary/reviewed writers with spare
capacity. v2 adds actual Files Save and MCP native approval refusals in active and
unknown states, distinct target publication, child-scope/cleanup and revocation
regressions. See the [repair record](../testing/records/2026-10-06-parallel-transfers-mutation-repair.md).
Source-level/GPUI/TCP tests and OpenSSH
interoperability do not constitute Windows/Linux native desktop acceptance.

The [CLOSE repair record](../testing/records/2026-10-06-parallel-transfers-writable-close.md) keeps the independent blocked v2b evidence, exact strict probe and v3 author scope separate.
