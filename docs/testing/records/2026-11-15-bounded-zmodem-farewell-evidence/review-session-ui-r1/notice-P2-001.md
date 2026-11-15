# P2-001: grace begins on russh queue admission rather than completed transport flush

Status: OPEN; frozen-r1 source review; no source edits or recompile.

Baseline: b66ec7e564e97c3cc8261b447288377ac1ffccc2.
Freeze SHA-256: cf4cb0e53b80b09c44da99e3fe358cd7091622314625f881fd2769df783beba4 (582/582 matched).

New-code location: /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/ssh_bridge_zmodem.rs:68-75, specifically protocol_output_flushed(id) at line 74.

Trigger: after a valid received ZFIN, the outgoing final ZFIN reply fits the SSH window and russh message queue, but the SSH event loop is blocked flushing earlier traffic or awaiting a rekey for longer than three seconds. The application pump is independent and can continue ticking.

Evidence: ssh_bridge.rs:317-333 awaits SshShellWriter::write and maps its success to Written; ssh_bridge.rs:171 then calls Protocol::write_completed. SshShellWriter::write (crates/keelshell-session/src/ssh.rs:1122-1126) awaits ChannelWriteHalf::data only. In the exact Cargo.lock russh 0.65.1 dependency, channels/mod.rs:352-361 implements data via tokio::io::copy to ChannelTx; channels/io/tx.rs:138-146 completes a write with permit.send into mpsc, and :179-180 returns Ready(Ok(())) from poll_flush unconditionally. Actual packet flush is later, in client/mod.rs:1542-1548, in a different task. No completion receipt travels back to this write future. protocol_output_flushed then starts farewell_started at session/src/zmodem.rs:596-605 and tick retires the receive at :550-556.

Impact: the candidate can emit EndedWithoutFarewell and permit its exact ResumeInput handoff without evidence that the final protocol reply was transport-flushed. This violates ADR0137's prerequisite that producing/queuing a reply is insufficient, and can retire an active peer negotiation prematurely under transport backpressure. This does not fabricate Finished and does not prove a file publication error.

Required correction: obtain a lease-bound completion/barrier from the real transport write owner after the corresponding protocol data is flushed, or retain the existing failure/hold path when that evidence is unavailable. A ChannelTx flush call alone is insufficient because its poll_flush is a no-op. Add a regression that holds the lower outbound transport after queue admission and verifies no grace/warning/handoff until the actual completion receipt; also reject stale receipts and invalidate them on new replies.

Validation limits: this is a source-proven API-contract mismatch. No new controlled-network fixture, custom binary, source mutation or recompilation was performed. Parent-provided current session tests pass 22/22; app zmodem 22/22 and bridge 7/7 pass but their write-completion tests call Protocol::write_completed manually, so they do not prove the missing lower transport receipt.
