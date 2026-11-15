# P2-003 and P2-004 — CLOSED on final r5

**Status: CLOSED. Recommendation: APPROVE for the reviewed session/transport-barrier/bridge/UI source and targeted-test scope. Open findings: P0=0, P1=0, P2=0, P3=0.** Full engineering gate, strict peer interoperability and native/product acceptance remain separate.

Binding: HEAD `b66ec7e564e97c3cc8261b447288377ac1ffccc2`; r5 freeze SHA-256 `d8d27989768ae1827697458ee52d366d407ca8543a9eebd41baf6d8f283a2b2e`, 571 engineering inputs + 12 scripts. Stable app-r6 SHA-256 `a5d0dc66845eb7124dde4e1a7b66f586ecc22dd9f0fe5bfe13c259145046e720`; complete inventory 1238, SHA-256 `a9c65e4e9965d895d7341ca8a3200803e15a8427c04034ca48a2ae16ead5d27a`. Complete inventory bytes, exact names and binary/manifest/freeze bindings were independently verified. The reviewer did not independently recompile.

P2-003's trigger was a write-confirmation failure or cancellation after the receiver retained ordinary terminal tail. `protocol.fail()` recovered the tail, but the post-pump worker dropped its private output queue. At `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/ssh_bridge.rs:123`, r4's correction shares an absolute two-second deadline between shell close and ordered output cleanup; Full retains the same queue-front event, and timeout/disconnection returns explicit failure. The primary pump error is retained with later failures appended at line 130. Neither cleanup step selects on cancellation; close is always attempted and the fence is released after the bounded cleanup operations return. This production code is byte-identical in r4 and r5.

The four production `run_with_protocol` regressions at `crates/keelshell-app/src/ssh_bridge_tests.rs:915` exercise actual server-handler blocking and real write-confirmation timeout, cancellation, temporary Full/recovery and permanent Full. They observe the real UI receiver, exact UTF-8/ANSI bytes and order, a single Failed outcome, held ownership during backpressure, and explicit combined error when output stays unavailable. All four pass in the independent final r5 bridge run. The parent red log remains preserved, 0 passed / 4 failed / exit 101; it was read and hash-verified, not independently executed by this reviewer.

P2-004's trigger was the original permanently full output test still propagating `worker?`, requiring success after the agreed production contract began returning `SSH_SHELL_OUTPUT_CLEANUP_TIMEOUT`. The independent r4 full bridge run preserved 14 passed / 1 failed / exit 101. r5 changes only this test: at `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/ssh_bridge_tests.rs:410`, it explicitly asserts the exact error and strengthens the output check to exactly 128 `x` bytes. Its six-second outer bound, real full-receiver setup, typed Cancelled lifecycle and `session.close()` remain. No production behavior or strict peer assertion was weakened.

Independent final execution:

```text
stable-tests/app-r6 ssh_bridge::tests --test-threads=1 --nocapture
15 passed; 0 failed; 0 ignored; 1223 filtered; exit 0
```

All 15 expected names pass, including the corrected original test and four cleanup cases. Execution is 22.990 seconds; the binary SHA remains unchanged. `receipt-app-ssh-bridge-r6.json` and its full stdout/stderr preserve the result. r3/r4 HOLD reports and failures remain untouched; this new closure supersedes their open status only for r5.
