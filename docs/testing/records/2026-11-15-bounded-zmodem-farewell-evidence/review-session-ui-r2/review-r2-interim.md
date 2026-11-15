# Session/bridge/UI review r2: verification retained, final signing deferred

Status: **HOLD pending r3; no final source or engineering signoff.**

Reviewed baseline: `b66ec7e564e97c3cc8261b447288377ac1ffccc2`.
Original r2 freeze: `8efdd6ebf0253323d169abd4e87a05bef98011e17c3dfa63beed52dac5eecd1a`, 571 engineering inputs plus 12 gate scripts. Initial independent hash verification matched 583/583; the complete independently enumerated binary inventories matched the metadata byte for byte.

The reviewer independently executed parent-built stable binaries, without independently recompiling: barrier-r2 `4 passed / 0 failed / 0 ignored / exit 0`; session-r3 `23/0/0/0`; app-r3 `zmodem` `26/0/0/0`; app-r3 `ssh_bridge::tests` `11/0/0/0`. The two app groups overlap in 8 tests and are separate executions, not 37 distinct tests. Their original receipts, full inventories and stdout/stderr remain unchanged in this directory.

The user subsequently reported CORE-R2-01: retained Finishing tail can be lost when peer cancellation changes the receiver to Cancelled, or a Command error changes it to Failed, before cleanup. The parent stopped and reaped full-gate-r2 (reported exit -15, approximately 218 seconds) and is fixing core plus any necessary session cleanup. These known dependent changes revoke current-tree final acceptance under r2. Await the r3 freeze, explicit delta and binary bindings; do not infer new-source acceptance from these old passing binaries.

Within r2, the two original session/UI issues have substantial correction evidence: protocol output uses `write_confirmed` on the same client FIFO, a captured lease/revision token, and fail-closed reply/timeout handling; pump drains ordered output before ticking and suppresses timer retirement once lifecycle completion is known. The red history has 4/4 ordering failures; intermediate app-r2 has 3 failures at the incorrect immediate-unblock expectation; the final source retains the failed-shell input hold until transport cleanup. These observations are provisional review evidence, not final closure across the pending r3 input.

The original seven peer tests and their completed/receive helpers are byte-identical to HEAD. Parent receipts remain distinct: native-current-r2 8/8, Linux linux-peer-r2 7/8 with the original CRC32 Unicode strict Finished assertion failing after clean exit, and the new interactive test strict_OO=1/same_shell_continued=true. Linux reference passes and its three cleanup flags are true. This reviewer did not rerun peers. Interoperability, engineering gate, desktop-native, installation/release and full-product acceptance remain separate.

Only this review directory was written. No Cargo, rustc, formatting, target mutation, production/test source change, Git write, worktree or user chat creation was performed. The requested rust-best-practices style, error, testing, state and documentation/concurrency chapters were read. The locked russh 0.65.1 cached archive hash matches Cargo.lock and the six inspected dependency files match that archive; this is source evidence, not independent build provenance.
