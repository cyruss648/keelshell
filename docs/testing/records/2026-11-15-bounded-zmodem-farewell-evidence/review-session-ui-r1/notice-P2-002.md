# P2-002: grace tick can win after a nonzero shell exit is already observed

Status: OPEN; frozen-r1 source review; no edits or recompile.

Baseline: b66ec7e564e97c3cc8261b447288377ac1ffccc2.
Freeze SHA-256: cf4cb0e53b80b09c44da99e3fe358cd7091622314625f881fd2769df783beba4.

New-code location: /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem.rs:550-556 (timer transition), interacting with crates/keelshell-app/src/ssh_bridge.rs:145-150 (existing pump order).

Trigger: a receive is waiting for farewell with its final reply marked flushed. Just before the next pump iteration, the SSH reader observes a nonzero exit (or a signalled/unknown shell close), and the first grace-expired tick occurs before the corresponding ordered terminal event is consumed. For example, the peer's own final wait times out with nonzero status near the three-second boundary.

Evidence: pump reads shell.completion() at ssh_bridge.rs:145-147 and sets completion_seen, but still calls protocol.tick at :149 before drain_output at :150. The new session tick at zmodem.rs:550-556 checks session connection closure and controller receive state, but does not know the already observed shell completion. A nonzero channel exit does not close the authenticated SshSession. end_without_farewell clears transfer through end at zmodem.rs:655 and :783. drain_output subsequently feeds the ordered SessionEvent::Exited through Protocol::feed (ssh_bridge_zmodem.rs:91-99), which calls transport_exited. Its nonzero branch calls transport_failed at zmodem.rs:625; transport_failed emits a failure only while is_active at :679. The lease was already removed by tick, so the nonzero exit cannot correct the warning. For signalled/unknown shell endings, the generic Protocol::feed failure branch has the same inactive-controller behavior. ssh.rs:614-625 publishes completion ahead of the final queued event at :654-661.

Impact: an already known unsuccessful/unknown transport completion can be represented solely by EndedWithoutFarewell, violating ADR0137's rule that nonzero, transport-error or unknown completion follows the existing failure path. The three-second timer becomes an unintended alternative to the ordered failure gate. This does not fabricate strict Finished or invalidate a prior FileSaved receipt.

Required correction: once any shell completion has been observed, suppress the timer-only retirement while draining preceding data and the ordered end event; allow only the ordered code=0/success=true path to request this warning. The independent lifecycle watch must not itself authorize a clean warning. Add a regression with grace already due and nonzero/signalled completion pending, asserting no warning and exactly the existing failure event; preserve any real queued OO and terminal tail in order.

Validation limits: the control-flow mismatch is established from the frozen source; no custom executable or fixture was created. Existing focused tests call Protocol::feed with Exited directly and do not cover the production pump's tick-before-drain ordering.
