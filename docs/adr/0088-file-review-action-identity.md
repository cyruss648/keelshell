# ADR 0088: Bind file review actions to the displayed proposal

Status: accepted for implementation; runtime acceptance is recorded separately.

## Context

The file browser remains available beside a compact review. With an unsaved
editor, selecting Edit for another remote file can replace a pending Save
proposal with a proposal to discard the draft and read the other file. A pointer
callback from the previous frame must not approve or dismiss that replacement.

Batch proposals already have an internal plan UUID. Using its optional value
alone leaves every ordinary proposal with the same `None` identity. Comparing
rendered text would also conflate distinct proposals with equal descriptions.

## Decision

All new file proposals pass through `replace_pending_review`, which allocates a
fresh UUID and resets the review presentation. This includes directory planning,
file and directory continuation, quarantine acknowledgement and direct actions.

Confirm and Cancel capture the pending review UUID and any batch plan UUID at
render time. They act only if that complete identity still matches a currently
pending proposal. An empty review never grants an action. Clearing a proposal
invalidates its captured identity, and replacing it allocates a new identity even
when the text and operation parameters happen to match.

Both button callbacks stop pointer propagation before checking identity. A stale
review action must not reach retained mouse listeners from the earlier browser
row click, even when its own action is rejected.

The existing execution checks still verify session availability, active work,
reviewed Save bytes and transfer authority. The proposal UUID is a UI action
binding, not a replacement for transport or content validation. It stays in
memory and is neither serialized nor reused after reopening the application.

## Verification

Two GPUI regressions load a real loopback SFTP file, create a dirty Save review,
then dispatch the production Edit action for another file. Without repainting,
they activate the old Confirm or Cancel pointer callback. Both require the new
Read proposal, original draft and complete remote bytes to remain unchanged.
A newly painted Cancel must still work. The batch plan replacement control is
retained separately.

See [the integration record](../testing/records/2026-10-13-platform-batch-integration.md)
for actual negative-control, corrected-test, engineering and native results.
