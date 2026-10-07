# Reviewed Agent and external MCP: exact source CI — 2026-10-07

This record belongs to exact main commit
`a4c15c04772b11a70a592ae7c95132b4bea4d818` and
[Quality 37624355451](https://github.com/cyruss648/keelshell/actions/runs/37624355451).
The earlier b54 directory-Ask timeout and e695 saved-profile-sync timeout
remain separate failures; their causes are not inferred from this run.

## Linux result

The `ubuntu-24.04` job `112802208027` actually completed with failure.
Packaging checks, formatting, dependency policy, strict Clippy and the AI
directory controller proceeded successfully. The first directory Ask did
not fail in this run. The application suite completed with 621 passed,
one failed and two ignored; the OpenSSH stage was skipped after the failure.

The only failed application test was
`files::transfer_tests::directory_sync_reviews_exact_content_and_uploads_only_after_confirmation`.
It reached an actual operation terminal state before its original byte
assertion: the directory transfer was rejected because a file target was
owned by an active application mutation. No atomic writes had started;
`/changed.txt` still contained the original `old!` bytes instead of the
expected `new!` bytes. This was not an observation timeout. The original
deadline, content assertions and mutation isolation have not been relaxed.

The process-wide conservative local mutation reservation can conflict with
another operation's local read even when the tests use separate temporary
directories. A static trace identified a concurrently completed existing
download-continuation case as a possible participant. The historical log
does not identify the actual reservation owner, so that timing alone does
not establish this run's precise cause. Controlled reproduction and a
test-harness correction were not yet available at the first log readback;
no production isolation change has been made on that inference.

A subsequent controlled GPUI/TCP case actually reproduced the conflict in
3.915 seconds with two independent fixtures. An existing local download
continuation was paused at a nonzero remote read. Another fixture's
manually confirmed directory synchronization reached `MutationBusy`, with
zero writes and exact original remote bytes. Releasing the original
ten-second hold allowed the complete 144 KiB download to finish; a fresh
manual comparison, review and confirmation then completed both expected
uploads and preserved the target-only file. The original twelve-second
idle wait and all byte assertions remained. This proves the fixture
interference mechanism, while the historical CI owner remains unidentified.
The narrow test-harness correction has its own review and gate scope.

The adjacent terminal-shutdown message about case-only paths is not proof
that this test's comparison had that error. The shared terminal-worker
registry can reap a completed worker and report its error on a later
test's current thread. The explicit operation status and byte assertion
above are the evidence attributable to the failed case.

The root read the complete 322,086-byte Linux job log, 2,318 lines,
SHA-256 `f37c9b3bea5ff627801e707252e58036871e17f9de023a8a2cb6bdc188b98483`.
The actual log and API snapshots are preserved in ignored
`work/reviewed-ai-mcp-main-integration-20261007-v1/`. A run-level log query
was unavailable while the run was active; a job-level download succeeded.

## Other targets and acceptance boundary

The run subsequently completed with Windows and macOS success and Linux
failure. The root read all three complete job logs and the terminal API
snapshot for this exact SHA. Windows application tests had 617 passed and
two ignored; macOS application tests had 622 passed and two ignored, and
its OpenSSH interoperability stage passed. Platform-specific test counts
are not compared as equivalent suites.

The complete Windows log is 566,945 bytes / 3,898 lines,
SHA-256 `231769046e66ba96ef5f5867db3276230286446c33a79a828d8c33c3f38b2616`.
The complete macOS log is 567,175 bytes / 3,958 lines,
SHA-256 `1c00ddf369475bd7feb64de43a9aaffcb1bdfe78547087c7ec2c032eecddd129`.
This run does not establish desktop interaction, supplier-model behavior,
installed update behavior or release acceptance on any platform. KeelShell
continues to provide MCP to external agents; in-application inference is a
separate entry point.
