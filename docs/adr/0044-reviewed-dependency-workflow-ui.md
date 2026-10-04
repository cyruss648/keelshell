# ADR 0044: Human-reviewed dependency workflow UI

- Status: accepted
- Date: 2026-10-05

## Decision

A separate native GPUI panel edits 1–128 ephemeral task drafts. Each task has a
stable UUID, optional bounded name, exact command source, one explicitly selected
already authenticated SSH session and up to 32 prerequisite task UUIDs. The
existing ordinary batch panel remains available. The editor reuses the existing
batch destination metadata and the core `BatchCommandTemplate` markers, rather
than resolving another host or creating another connection.

The complete review constructs the immutable core dependency plan from the
rendered commands. It displays every task in deterministic topological order,
its complete source and expanded command, every prerequisite, full destination
name and endpoint, route and ephemeral session binding. Invisible controls and
direction-changing Unicode are escaped using the same command display helper as
external MCP human review. Display escapes never alter execution bytes. The
review also displays concurrency 1–8, deadline 1–300 seconds per admitted task,
failure policy and the fixed combined 256 KiB output bound per task. The maximum
graph therefore retains the adapter's existing 32 MiB capture ceiling.

The explicit confirmation checks the entire pending snapshot: panel identity,
one-use starting state, draft revision, source commands, labels, target UUIDs,
dependencies, expanded plan, display metadata and execution options. It also
compares the current authenticated `SshSession` with the captured instance through
`same_connection`, which compares shared encrypted connection ownership. A fresh
connection to the same endpoint cannot inherit an old review. No review token is
an authentication credential. A final successful check passes only the captured
sessions and the acknowledged immutable plan to `start_workflow`.

Input events revoke a pending review; programmatic input changes are caught by
rebuilding the final snapshot. Language and theme changes retain draft entities,
invalid numeric text, selected targets and dependencies. Loss or replacement of
a selected connection, or a change in its endpoint/route/template metadata,
invalidates a pending review. During execution it requests cancellation of local
waits and prevents pending task admission. Refreshing targets preserves selected
unavailable bindings until the user explicitly selects an authenticated current
session; it does not redirect tasks to a replacement. Hiding the panel retains
the owned run, captured bindings and receipts. New work cannot replace a running
panel, and new drafts require a new target selection and explicit review.

The body scrolls independently of a fixed header and footer. Large task lists,
per-task target and dependency selectors have bounded viewports. All review text
is present in the scrollable body, including the last task of a maximum graph.
The footer retains edit, human confirmation, hide, cancellation and new-workflow
actions. The panel exposes the public GPUI dialog role and localized title;
inputs and task actions have explicit localized accessible names. Focus returns
to the active workspace surface when hidden. Occlusion and action guards prove
hit/action isolation only; background accessibility-tree isolation remains an
existing unverified boundary. Tasks show waiting prerequisites, pending concurrency, local admission
and complete transport or skipped receipts. Local admission is not proof that a
remote command started. Only complete zero exits release an edge; failed,
rejected, uncertain and skipped prerequisites block descendants. Continue permits
independent branches; stop-after-failure prevents pending replacement admissions
while collecting separately admitted tasks.

The workspace command-action row wraps inside its actual central column with
consistent spacing. Its four complete localized actions remain available beside
the AI sidebar at the 900×580 minimum window. Natural row height keeps the last
action within the command area without shortening labels or combining the
separate command textarea with the wrapping actions. Wide layouts retain one
action line. Running ordinary-batch and workflow labels use the same constraint;
appearance, language and sidebar changes preserve the command input entity,
source and target binding.

The UI polls bounded state events without blocking. `WorkflowHandle::try_finish`
nonblockingly collects the authoritative aggregate exactly once, leaving queued
events intact. The panel validates the complete aggregate fingerprint, actual
options and ordered task/target IDs before reconciling event rows. Each poll
carries a run UUID, so a stale result cannot mutate another run. Missing aggregate
or task evidence remains unknown. Full captured stdout/stderr are transient,
selected per task and displayed/copied with escaped controls. Output is never
injected into a terminal, command history, profile metadata or batch audit.
Task counts are not stored as existing target-based audit counts.

Cancellation skips pending tasks and cancels local transport waits. Admitted
attempts preserve `NotStarted`, `Unknown` and complete exit receipts. It neither
proves remote process termination nor rolls back completed tasks. No automatic
reconnection, retry, replay, timer, recurring schedule or model-directed execution
is added. Normal batch operation, connection storage, transport scheduling and
external MCP server scope remain their existing contracts.

## Acceptance boundaries

GPUI renderer tests and real TCP SSH fixtures exercise the production editor,
confirmation and captured-session dispatch. Fixture commands select protocol
responses and never invoke a shell. Separate system OpenSSH checks execute real
localhost programs and verify connection identity plus nonblocking collection.
Compilation, renderer fixtures and localhost protocol/program evidence are not
native macOS/Windows/Linux product acceptance or production-host evidence.
Independent review and final native integration remain separate gates. Task
persistence, task-level audit, recurring scheduling and retry policy remain
unimplemented; this panel is a manual, ephemeral workflow.
