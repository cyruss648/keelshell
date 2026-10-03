# ADR 0022: Bounded keyboard-interactive SSH authentication

## 状态

已接受并接入工作区（2026-10-04）。

## Decision

The session transport exposes two ephemeral modes: a caller-supplied ordered response mode for non-UI integrations, and a prompted mode that delivers one bounded server challenge at a time over a `mpsc` channel. The workspace can explicitly switch a password or private-key attempt to prompted mode; every challenge is rendered in a bilingual modal and answered through a one-shot response channel. Answers are zeroizing values and are discarded after the attempt.

Challenge metadata, prompt text and responses are bounded; missing, stale or cancelled responses fail with a typed credential error. Prompt text and response values are never logged or included in errors. The route UUID and hop index are checked again before a challenge is shown or an answer is sent, so a late challenge cannot attach to another connection.

Host identity verification remains in the existing SSH handshake callback and runs before authentication. The mode does not persist responses or silently fall back to another authentication method.

## Consequences

The UI collects responses explicitly for each connection attempt and discards them after the attempt. OTP and MFA prompts are supported when the server sends the corresponding challenge. Cancelling the modal cancels the whole route and does not continue with empty answers. Saving a connection or a vault entry never saves keyboard-interactive answers.

Real external MFA-provider acceptance and native Windows/Linux visual acceptance remain follow-up verification; loopback fixtures and unit/GPUI tests do not prove those boundaries.
