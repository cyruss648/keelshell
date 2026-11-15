# SSH transport barrier regression handoff

Date: 2026-11-15, Asia/Shanghai.

## Scope and validation status

Added only `crates/keelshell-session/src/ssh/write_barrier_tests.rs` and this
note in the shared checkout. No production source, Cargo manifest, lockfile,
controller or UI source was edited. The parent already attached the test file
with `#[cfg(test)] mod write_barrier_tests;` in `ssh.rs`.

No Cargo, rustc, rustfmt, formatting, compilation, test execution, target
invalidation, worktree creation, commit or push was performed. The parent owns
the Cargo target. These are source candidates, not passing test receipts.
The parent must compile, inspect the exact inventory and execute the tests.

Read the repository `AGENTS.md`, the supplied RTK instructions,
`docs/HANDOFF.md`, `docs/testing/SHARED_TARGET.md`, the relevant
rust-best-practices testing/concurrency/error/style/documentation chapters,
and `work/pty-farewell-20261115/review-session-ui-r1/notice-P2-001.md`.
Source-only checks verified the four names against this inventory, the inner
test marker, final newlines and absence of trailing whitespace. These checks do
not compile or execute Rust and do not establish runtime acceptance.

## Exact added test names

All names use the `ssh::write_barrier_tests::` prefix:

1. `write_confirmed_keeps_farewell_active_past_three_seconds_of_transport_backpressure`
   - Creates a real authenticated TCP loopback russh server and a PTY/shell.
   - Creates a real `ZmodemController` on that exact SSH session, feeds codec
     `RequestInit` and `Finish`, retains the current output token and uses the
     generated initial/final replies as actual channel data.
   - Gates the client's underlying `AsyncWrite::poll_write` after setup.
     Ordinary `writer.write(initial_bytes)` succeeds at queue admission;
     the gate independently reports that an actual socket write is blocked.
   - Polls the same pinned `write_confirmed(final_bytes)` future in intervals
     of at most 80 ms for at least 3,100 ms of real elapsed time. Every tick has
     no events/writes, the controller remains active on the same lease/token,
     and the server has received no channel data.
   - Releases the gate, requires actual barrier success and verifies exact
     initial-then-final bytes at the server. Only then confirms the token.
     `transport_exited(session, 0, true)` must produce exactly
     `EndedWithoutFarewell`, never `Finished`, without another grace wait.
2. `write_confirmed_rejects_disconnect_after_data_without_a_peer_reply`
   - Gates the server's underlying socket writes after setup. Channel data
     reaches the real server handler while its response is held.
   - Requires no completion for 80 ms, then shuts down the real server socket.
     The pending operation must return an error within the bounded wait;
     a dropped reply cannot become success as in russh's `send_ping` behavior.
3. `write_confirmed_times_out_when_data_arrives_but_the_peer_reply_is_blocked`
   - Holds the real server response after observing the exact channel data.
   - Changes only the test writer's existing private timeout to 250 ms after
     authentication/shell setup; requires a short pending interval and then a
     production `SessionError::Timeout`. Data arrival is insufficient.
4. `write_confirmed_waits_for_a_preceding_rekey_before_receiving_a_reply`
   - Queues real `Handle::rekey_soon`, observes a blocked client KEX write,
     then admits confirmed channel output.
   - Requires 80 ms without a receipt or channel data; after release, requires
     a successful barrier and the exact subsequent payload at the peer.

## Fixture and synchronization

- The file starts with `//!` and has an inner `#![cfg(test)]` marker.
- Both gate wrappers delegate reads, flushes and shutdown to real TCP streams;
  the gate suspends nonempty writes before the real socket's `poll_write`.
  No transport method, receipt or channel write implementation is mocked.
- A mutex protects both the held flag and the registered write `Waker`.
  Release changes the flag and takes the waker under that same lock, then wakes
  outside the lock. An observer uses `Notify::notify_one`'s retained permit
  plus a sticky blocked flag, preventing a lost wake between checking and waiting.
- Initialization has a five-second outer timeout, including authentication,
  PTY/shell acknowledgment and a real setup request roundtrip. Normal writes
  use an eight-second production budget; every later wait has a finite bound.
- A cleanup owner exists before cancellable authentication. Its Drop shuts down
  both owned sockets, releases both gates and aborts the outer server task.
  Successful tests also await that task, which awaits russh's actual session.
  The owned shell's existing Drop cancels its output task and channel.
- Only a few small frames are sent, so the retained server channel's bounded
  receive queue cannot fill. The server creates no consumer subprocess/task.
  The listener is loopback/ephemeral and is closed after accepting one socket.
  Host keys, synthetic credentials and synthetic protocol bytes stay in memory.
  No host data, production logs or persisted secrets are used.

## Dependencies and source API assumptions

No new dependency is needed. Existing manifest/lock sources were inspected:
`russh = "0.65"` resolves to 0.65.1, `tokio = "1.54"` resolves to 1.54.1,
the existing dev dependency `rand = "0.10"` resolves to 0.10.3, and
`zeroize` and `keelshell-core` are already direct dependencies.

- The file is a child module of `ssh`, with access to private
  `SshSession::authenticate_stream(&SshOptions, gated_stream, Arc<TransportControl>)`,
  `SshSession.handle`, `SshShellWriter.timeout` and the transport control.
  `TransportControl::attach(TcpStream)` returns `(TcpStream, Arc<TransportControl>)`;
  the test wraps the returned stream and passes the same control into auth.
- `start_shell(24, 80)` returns an owned shell whose `writer()` supports
  `write(&[u8])` and the parent's new `write_confirmed(&[u8])`.
- The parent implementation puts `ChannelWriteHalf::data` and
  `Handle::send_global_request("keepalive@openssh.com", &[], true)` inside
  one production deadline. This suite exercises the missing-roundtrip timeout;
  it does not separately force queue saturation to measure split data/response
  deadline accounting.
- In the inspected russh 0.65.1 `client/mod.rs`, client-created channels and
  global requests use the same sender. `can_receive_outbound` and the drain loop
  wait during active rekey or pending channel data. `send_global_request`
  propagates a dropped reply as `Error::Disconnect`; `send_ping` ignores the
  receiver error and returns success.
- In russh 0.65.1 `server/encrypted.rs`, the global-request fallback replies to
  `keepalive@openssh.com` with real `REQUEST_FAILURE`. Setup asserts the actual
  `RequestDenied` result, and successful barriers therefore exercise the
  authenticated denial-as-receipt path. No success response is fabricated.
  A separate `REQUEST_SUCCESS` fixture is not included.
- The controller API accepts an exact `ZmodemOutputToken` from
  `protocol_output_token()`, invalidates it on new output, and accepts it through
  `protocol_output_flushed(token)` only after confirmation. `tick()` is driven
  while blocked; `transport_exited(&session, 0, true)` provides the clean-exit
  observation after confirmation. Input handoff and stale-token controller/UI
  tests remain with the parent.

Conclusion: the four source regressions cover actual lower write backpressure,
controller farewell hold, peer denial receipt, missing-reply disconnect/timeout
and preceding rekey. Compilation, execution, Clippy and formatting remain
unverified under the parent's exclusive Cargo ownership.
