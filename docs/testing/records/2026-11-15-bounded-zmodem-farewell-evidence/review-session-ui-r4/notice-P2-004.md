# P2-004 — Existing full-UI cancellation test rejects the new explicit cleanup failure

Status: **OPEN / HOLD for r4 test acceptance**. This is a test-contract integration failure, not evidence that explicit output-cleanup failure should become success. P2-003's new four-case cleanup coverage passes.

Candidate: baseline `b66ec7e564e97c3cc8261b447288377ac1ffccc2`, frozen r4 SHA-256 `9133ce6141be9498034641d25fe064b60c0904fb4ced9f8d73f52a76e2e4c58c`, 571 engineering inputs + 12 scripts. The two bridge files still match the freeze after the failure. Parent-built stable `app-r5` SHA-256 is `e2a7c4842570c80c632e5c7b40b72d8e383bf19f48146b8d17ad09efd50cc369`; metadata binding and complete inventory of 1238 were independently verified. No independent recompilation.

Location: `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/ssh_bridge_tests.rs:410`, in `full_ui_output_queue_does_not_block_cancel_or_owned_cleanup`. Related production behavior: `ssh_bridge.rs:128`, `:130`, and `:152`.

Trigger: the original real-SSH OutputBacklog fixture deliberately leaves the single UI slot full while cancelling the worker. New post-pump cleanup retains undelivered ordinary output and expires at its shared two-second deadline, returning the specified `SSH_SHELL_OUTPUT_CLEANUP_TIMEOUT`. The old `worker?` still requires `Ok(())`, so the default, non-ignored test now fails on this intended error before checking the queued data, typed cancellation, or calling `session.close()`.

Impact: the r4 full bridge group is red, and this default test will also fail the app/workspace test gate. A 30/30 Zmodem filter excludes this test and therefore cannot certify full bridge acceptance. The result does not justify dropping retained output silently, treating a cleanup timeout as success, or weakening the new permanent-full regression.

Independent execution:

```text
stable-tests/app-r5 zmodem --test-threads=1 --nocapture
30 passed; 0 failed; 0 ignored; 1208 filtered; exit 0

stable-tests/app-r5 ssh_bridge::tests --test-threads=1 --nocapture
14 passed; 1 failed; 0 ignored; 1223 filtered; exit 101
stderr: Error: "SSH_SHELL_OUTPUT_CLEANUP_TIMEOUT"
```

All four new cleanup tests pass in both groups. The groups overlap in 12 names; these are separate executions, not 45 unique tests. There were no repeated executions to seek green.

Required correction: update this existing permanent-backpressure test to explicitly assert the agreed cleanup failure while retaining bounded completion, actual full-receiver setup, queued-byte checks, typed `ShellEnd::Cancelled`, and owned resource cleanup. Do not ignore the worker result, delete/ignore the test, increase its timeout to mask an unbounded wait, or turn the production error into success. Bind any test change to a new freeze/binary and execute the relevant group again.

Evidence: `receipt-app-r5-inventory.json`, `receipt-app-zmodem-r5.json`, `receipt-app-ssh-bridge-r5.json`, and the full `app-ssh-bridge-r5.stdout.txt` / `.stderr.txt` in this review directory. The execution receipt's `verified: false` records a failed acceptance check; the process result, counts, exact names, binary hashes and logs are retained.
