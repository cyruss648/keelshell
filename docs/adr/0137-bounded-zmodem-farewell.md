# ADR 0137: Bounded receive cleanup without an optional Zmodem farewell

- Date: 2026-11-15
- Status: accepted for reviewed receive cleanup; strict interoperability and native acceptance remain open
- Baseline: `b66ec7e564e97c3cc8261b447288377ac1ffccc2`
- Extends: [ADR 0136](0136-reviewed-pty-zmodem.md)

## Problem and evidence

Independent peers can finish their file exchange and send a valid ZFIN but lose
the final OO in their own PTY output cleanup. The previous strict candidate
retains the verified per-file saves but otherwise waits until channel failure or
the long protocol retry budget. An interactive shell can return a prompt while
ordinary input remains excluded.

The exact upstream lrzsz 0.13.1 archive, SHA-256
`112f91059fbd118e016d800c1a5c5a1702e965d8b9bfbda6015e44e20bfb93c9`,
documents a short receiver wait for OO in `doc/zmodem-wip.txt`, section 6.3.
Its `src/lsz.c` writes and flushes OO before `src/rbsb.c` calls
`tcflush(fd, TCIOFLUSH)` during terminal restoration.

A controlled local PTY experiment used that unmodified peer executable and a
diagnostic interposer that observes the original syscall without changing its
arguments or result. In four cases, the output queue held two bytes immediately
before the final successful TCIOFLUSH; the peer exited 0, and the receiver,
scheduled after peer exit, received no OO. This establishes the failure in that
controlled schedule. It does not identify the unique cause of every historical
SSH failure. A separate readonly SSH trace found no verified reader ordering
defect. All previous direct-PTY failures remain original evidence.

## Decision

Keep `Finished` for actual OO. Add a distinct `EndedWithoutFarewell` result that
explicitly says the final handshake was not confirmed. It grants no new file,
command, connection or cleanup authority. Only individual `FileSaved` receipts
prove local publication; the result does not claim a saved file when none exists.

Core exposes an explicit receive-only transition from the valid Finishing state,
after all offered files were independently saved or rejected. It owns no clock
or transport evidence. It retains bounded ordinary bytes received while waiting,
preserves prompts and incomplete lookalikes, and excludes validated protocol
headers and a recognized five-CAN cancellation sequence. Ordinary tail ownership
survives peer cancellation and Failed state until its single terminal delivery;
cleanup does not replay rejected oversized input. Cancellation, disallowed
commands, failures, pending offers and
unacknowledged saves cannot enter this result. Strict `is_finished` stays false.

The session controller authorizes that transition only after the single PTY
owner acknowledges every queued protocol write for the exact lease. Producing
the response or popping a write is insufficient. Every newly generated protocol
reply invalidates the flush acknowledgement. Each captured output token binds
both the lease and its current output revision. A stale lease or earlier reply
cannot start the three-second grace period or confirm a current write.

The first review established that russh channel data completion only proves
queue admission; its channel flush is a no-op. The corrected PTY protocol lane
awaits an authenticated keepalive response queued after the data on the same
client handle FIFO. The locked russh owner stops consuming later outbound
messages during rekey and while any channel has pending data. Either success
or an explicit request denial proves a peer response; timeout, disconnect and
reply cancellation do not. This establishes transport output, not remote
application consumption or receipt of OO. Ordinary input keeps its existing
write semantics. The additional round trip applies to protocol output and can
be delayed by another blocked channel; it fails closed at the existing deadline.

After the first confirmed final reply, the controller waits three seconds
without extending that initial grace because of ordinary peer output. It may
then end with the warning only while the protocol queue is confirmed flushed.
An ordered SSH exit with code 0 and consistent success may end the wait earlier,
after preceding data has been delivered through the same channel queue.
Nonzero status, transport errors, connection replacement or unknown completion
use the existing failure path. A separate lifecycle watch is not an ordered exit.
Once the bridge observes any shell completion, it suppresses timer-only
retirement while draining preceding bytes and the ordered end event. A queued
actual OO remains strict protocol evidence. An unsuccessful end preserves the
ordinary receive tail through cancellation and still reports failure.
After the pump returns, the worker delivers any recovery-produced ordinary tail
through its existing bounded UI sender. Shell cleanup and final output share
the existing two-second cleanup budget, independent of the cancellation flag.
A permanently full or disconnected UI receiver produces an explicit cleanup
error, combined with the primary failure; it is not silently reported as
successful delivery. Transport/input ownership is retained through that cleanup.

Pending review, destination reservation, local publication or current mutation
uncertainty blocks this transition. Older sticky unknown results and backend
quarantine remain unchanged. The existing precise `ResumeInput` handoff is
required even after the warning; old queued keyboard, paste and AI input is
never replayed. The UI preserves complete save receipts, shows bilingual missing
handshake text, and uses the shared three-theme component system.

## Validation and scope

Add state-machine, parser-tail, real SSH identity and publication, bridge write
completion/input handoff, and GPUI receipt/unknown regressions. Add a separate
opt-in independent-peer workflow that returns to the same interactive SSH shell,
verifies saved bytes, observes an explicit return marker and writes a subsequent
reviewed fixture command. Its strict and missing-farewell outcomes are separately
classified. The original seven strict peer assertions remain intact.

The final engineering gate, exact test inventory, new nonauthor source review,
controlled experiments and cleanup are recorded in the
[batch record](../testing/records/2026-11-15-bounded-zmodem-farewell.md).
The r5 engineering gate exited 0 after 646.751 seconds: 2935 ordinary Rust tests
passed, 35 were ignored, 14 doctests passed, and Python ran 148 tests with one
skipped. All 59 new default Rust cases were matched by name to passing results.
Three scoped nonauthor approvals closed the six P2 findings with no open P0–P3.
The Rust reviewers executed verified parent-built artifacts without independent
recompilation. The parent subsequently bound the successful gate to the same
571 engineering inputs and 12 scripts, satisfying their integration conditions.
Historical failures and signing-time conditions remain intact.

This changes the receive cleanup contract, not historical test results.
Windows safe publication, Linux local-backend execution, native desktop, IME,
accessibility, installation, release and the broader product remain separate.
