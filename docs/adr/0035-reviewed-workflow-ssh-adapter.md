# ADR 0035: Reviewed workflow SSH adapter

- Status: accepted
- Date: 2026-10-04

## Decision

The session library consumes `ConfirmedBatchWorkflow`, exact UUID-to-session
bindings and immutable execution options. This dependency is a local workspace
path to core, with no new registry requirement. The adapter performs no plan
editing, new connection, hostname resolution, retry, recurring scheduling or PTY
request. Every exec command comes from the consumed immutable core plan.

Before spawning work or opening a channel, the adapter checks the entire set of
bindings: 1–32 non-nil unique IDs, every reviewed target present, no extra target,
and no locally closed captured connection. The caller supplies the already
authenticated `SshSession` that was shown to the user; the adapter cannot prove
that a UUID was assigned to the right UI session. Captured sessions are owned by
the workflow, so replacing an external selection/map does not redirect pending
commands to a replacement connection.

Concurrency is 1–8 and each admitted task has a deadline of 1–300 seconds covering
OPEN, exec submission and output. Dependency wait does not use a task's deadline.
Combined stdout/stderr per task is bounded; task count times that bound must not
exceed the existing 32 MiB batch capture budget. The default 256 KiB limit supports
all 128 core tasks. The bound covers captured payload, not protocol buffers,
command storage or scheduler metadata.

The core ledger admits ready tasks in deterministic topological order. Each task
uses the existing guarded `batch_exec` on its captured connection. Only a complete
`Exited { code: 0 }` receipt releases dependencies. Confirmed failure, rejection,
unknown outcome and a transport-proven non-start prevent downstream admission.
`Continue` keeps independent branches eligible. `StopAfterFailure` observes the
existing early failure flag before admitting replacements, skips pending tasks,
and still collects separately admitted tasks. No workflow task outcome is
converted into the existing target-based audit counts.

Events and the aggregate share `Arc` task receipts and transport rows, avoiding
output duplication. At most two events per task fit in a bounded 256-entry queue;
unread events cannot block completion. The final receipt follows the plan's
topological order and carries the core fingerprint plus the actual execution
options. Core fingerprints bind task contents and target IDs, not session objects
or options. A future UI must review those together and guard its complete pending
snapshot, revision and cancellation identity before passing the core receipt.

Cancellation skips all pending tasks and requests cancellation of admitted
transport waits. Admitted attempts preserve real `BatchRowReceipt` semantics,
including `NotStarted` and `Unknown`; pending tasks preserve core skip reasons.
Dropping the workflow aborts its owned scheduler and `JoinSet`. Existing channel
owners retain bounded CLOSE or late OPEN cleanup. A cancelled OPEN may have an
independent cleanup owner until its task deadline; an unconfirmed OPEN or failed
CLOSE may shut down the captured shared connection. Cancellation/drop do not
prove remote process termination, kill a process or roll back completed tasks.

## Acceptance boundaries

New real TCP SSH fixtures verify protocol requests, exact command bytes,
dependencies, independent branches, refusal/unknown outcomes, concurrency,
budget checks, cancellation and connection ownership. The fixtures interpret
commands as deterministic protocol scenarios and never invoke a shell. They do
not prove a production server, real program behavior or target-native desktop
interaction.

An additional opt-in test uses a separate disposable system OpenSSH server and
actual shell commands: a prerequisite writes a marker, its dependent reads that
marker, a nonzero exit blocks a descendant, and an independent branch succeeds.
All eight OpenSSH interop tests passed locally with temporary keys and tracked
process cleanup. This proves the specific loopback programs and SSH adapter,
not arbitrary production hosts or a graphical workflow review.

This slice provides the domain-to-SSH adapter only. A graphical workflow editor,
complete review flow, persistent task-level audit, scheduled tasks, retry policy
and native Windows/Linux acceptance require separate implementation/verification.
