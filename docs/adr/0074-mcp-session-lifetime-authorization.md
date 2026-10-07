# ADR 0074 — MCP authority follows the original terminal lifetime

Status: implemented and precisely imported after fresh non-author scoped review. The root combination gate and fresh macOS package checks passed. Exact-commit CI, simultaneous transport and new native acceptance have their own [integration record](../testing/records/2026-10-07-reviewed-ai-mcp-main-integration.md).

## Context

A bounded review of the exact main source observed three actual MCP file replacements after the captured target retired: production tab close, production completed reconnect and raw typed terminal End with foreground maintenance blocked. Prior human approval remained attached to a policy lease until a foreground maintenance pass revoked it. The SSH connection is shared: closing its shell channel does not close independent SFTP channels.

## Decision

Retiring a tab and installing a successful reconnect synchronously call the existing MCP maintenance path after removing the original target. This disables the original desktop policy, cancels its workers and makes pending reviews Cancelled and running actions OutcomeUnknown during the same UI turn. Even an identical endpoint cannot inherit an old entity, lease or proposal.

A private application `SessionAuthorization` pairs the existing policy lease with the captured original terminal's read-only typed lifecycle. Before I/O, after awaits and at each reviewed-write authorization check, Ready and a live producer are required. Its cancellation wait observes both policy revocation and raw lifecycle loss, without requiring a terminal poll, UI notification or rendered status. File preparation, scoped SFTP reads, command execution and file replacement use this same captured authorization; completion admission checks it again. Root validation also observes the captured lifecycle before enabling a grant.

Production SSH bridges always attach a sticky typed lifecycle. Existing byte-only controlled transports have no lifecycle producer and continue to depend on their owner and policy revocation; no client may substitute the captured source. A raw typed End or closed producer can cancel backend admission before the UI status catches up. The subsequent foreground pass revokes the service scope and discards stale completion records. A command already submitted to the remote host may continue; local cancellation never proves remote termination.

The private `TerminalView::subscribe_lifecycle` accessor is independently implemented with the same narrow signature used by a separate application Agent candidate. Integration must retain a single accessor and independently review the shared close/reconnect changes. This MCP change does not import or approve Agent orchestration.

## Preserved contracts

The eight public MCP tools, explicit grants, exact connection/session/route identities, selected context, review ownership, 64 KiB file/output limits and original time budgets remain unchanged. The external client cannot approve or execute. Conservative unknown outcomes, no automatic replay/retry, atomic reviewed replacement, full readback, mutation isolation and ephemeral credentials remain in force.

## Evidence and limits

The original three failure assertions were imported byte for byte before the first fixed execution. Additional controlled tests cover producer closure while a write is paused, raw-End cancellation of a held read and file preparation, pending command rejection and cancellation of an already running held exec future. The peer records commands but never executes their text. Original failures, new compiler mistakes in test-only references, exact input maps, actual wait results, owned process groups and private temporary-directory cleanup are retained in the [test record](../testing/records/2026-10-07-mcp-session-retirement.md).

Controlled GPUI/TCP evidence, compilation, native package inspection and CI are separate from actual native desktop/client acceptance. Other platforms, actual models/CLI clients and real target environments require their own runs.
