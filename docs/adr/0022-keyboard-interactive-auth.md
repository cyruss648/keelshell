# ADR 0022: Bounded keyboard-interactive SSH authentication

## Decision

The session transport exposes an ephemeral `KeyboardInteractive` authentication mode. It answers server prompts in order from caller supplied, zeroizing responses. Challenge metadata, prompt text and responses are bounded; missing responses fail with a typed credential error. Prompt text and response values are never logged or included in errors.

Host identity verification remains in the existing SSH handshake callback and runs before authentication. The mode does not persist responses or silently fall back to another authentication method.

## Consequences

The UI must collect responses explicitly for each connection attempt and may discard them after the attempt. OTP and MFA prompts are supported when the caller supplies the corresponding ordered responses. Native UI wiring and real MFA-provider acceptance remain follow-up work.
