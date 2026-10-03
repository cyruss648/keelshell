# SSH connection retry slice — 2026-10-03

This record covers the bounded retry policy for establishing a remote SSH
session. The API is transport-only and does not change terminal or workspace
behavior by itself.

## Implementation

- `RetryPolicy` bounds the total number of connection attempts, including the
  first attempt, and uses capped exponential backoff without unbounded timers.
- `SshSession::connect_with_retry` reuses the same validated endpoint and
  ephemeral authentication configuration for each complete attempt.
- `SessionError::is_retryable` permits only connection resets/refusals,
  selected transient SSH handshake/protocol failures and elapsed deadlines.
- Unknown or changed host identities, rejected authentication, credential
  provider failures, invalid options, explicit channel/server rejection and
  resource limits return immediately without another attempt.
- The retry loop owns no detached task. Dropping the future stops awaiting the
  active handshake and cancels pending backoff; callers can also use their own
  timeout around the future. A private-key read already running on a blocking
  worker may finish afterward, but its result cannot trigger another attempt.

## Verification

```sh
cargo test -p keelshell-session --all-targets
cargo clippy -p keelshell-session --all-targets -- -D warnings
cargo doc -p keelshell-session --no-deps
```

Results:

- 23 session unit tests passed.
- 4 remote-only source and manifest tests passed.
- 17 SSH/SFTP loopback integration tests passed.
- 3 example fixture tests passed.
- Clippy with warnings denied passed.
- Rust documentation generation passed.

The new unit coverage checks policy clamping and backoff caps, retryable versus
terminal error classification, retry count for transient errors, immediate
termination for authentication and invalid-option failures, public invalid
option handling, exact exhaustion of the attempt budget, and cancellation while
waiting for backoff. The cancellation regression drops the future before its
bounded delay expires, then verifies that no new attempt starts afterward.

## Boundaries

- Retry is only for creating a new authenticated session. Existing sessions,
  shell channels, SFTP operations and forwarding routes are not transparently
  replayed.
- No jitter is added so UI and integration tests remain deterministic. A caller
  coordinating many clients may add external scheduling if needed.
- A timeout or connection reset does not prove that the remote side did not
  observe a partial handshake; the policy retries only the connection setup and
  does not replay user commands or mutations.
- The current API is future-cancellable by dropping its returned future; a
  separate application-wide cancellation token is not part of this slice.
