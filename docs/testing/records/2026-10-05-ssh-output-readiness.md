# SSH output queue readiness — 2026-10-05

## Original CI evidence

The documentation-only commit `78eae7cddf3d8540d5d060bc620233af966fd1b5`
ran [Quality 37266941663](https://github.com/cyruss648/keelshell/actions/runs/37266941663).
macOS 26 and Ubuntu 24.04 succeeded; Windows 2025 failed the application
harness with 374 passed and one failed in 356.17 seconds. The failure was
`ssh_bridge::tests::full_ui_output_queue_does_not_block_cancel_or_owned_cleanup`:
`incoming.try_recv().is_ok()` at the original test line 305. Later Windows
harnesses and OpenSSH checks did not run. This run remains failed.

The macOS/Linux Rust workspace results each contain 1060 ordinary and eight
rustdoc passes, with 11/12 ignored respectively. The Linux extra ignored test
is the existing manual process observation. The earlier source commit
`23310ee13286adb488a451277addf49b05cd476b` has its own passing three-platform
run; it does not make the new Windows run pass.

Complete API metadata, original ZIP logs and failed-step text are retained
in ignored root evidence. API head and actual checkout heads must agree.
No per-event output chronology was recorded in the failed Windows test.
Therefore the specific Windows scheduling cause is not established.

## Candidate change

The test previously assumed that output would arrive within 30 milliseconds
of the bridge reporting Ready. Ready intentionally permits a quiet remote
shell, so it is not an output arrival barrier. The new test waits for actual
SSH data, restores that event to the one-slot UI queue (or observes that the
producer already refilled it), and explicitly requires Full before cancelling.
It stops consuming that queue until the worker exits. Disconnection, error and
premature exit fail rather than being treated as readiness.

The original six-second outer deadline, bounded channel capacity, cancellation
and final queued-event/Ended(Cancelled) assertions remain. No production
transport, library dependency, lockfile or toolchain change is included.

## Verification status

The isolated candidate passed the three original SSH bridge tests and the
complete engineering gate: 1060 ordinary Rust tests, eight rustdoc tests,
11 explicitly ignored tests, six Python script tests, formatting, strict
workspace all-target Clippy and x.y policy. Default and explicit 2 MiB local
agent controllers passed separately. The final wrapper exited zero in
202.129 seconds with engineering inputs unchanged. No production changes were
made by those commands.

The first complete wrapper timed out at its 540-second compile-plus-test
budget before the separate small-stack command. Its original receipt and
logs remain; this was not a failed product test assertion. Late workspace
output finished after that wrapper deadline, so it is not counted as a
successful full gate. No recorded cargo/rustdoc/rustc process remained when
root inspected before the unchanged retry. The second complete gate above
is the passing result.

At candidate freeze, fresh non-author review and root import were pending;
the completed results are recorded below. Native GUI, Windows runtime and
a new exact-head CI result remain unverified. The earlier failed
Windows CI remains failed, regardless of this local result.

## Fresh independent review and root integration

A new non-author review passed the limited two-file scope with no remaining
P1/P2. It reran all three original TCP/SSH tests and five controlled checks.
A 200 ms real SSH output delay rejects the old 30 ms assumption; the candidate
restores the actual canary, observes Full and completes cancellation. Missing
restoration and a genuine premature exit are rejected, and no output still
fails within the original six seconds. Two expected negative panics were
recorded separately. Temporary fixture probes were removed; both candidate
files and all 475 reviewed inputs were restored before formatting, policy,
diff and strict workspace all-target Clippy passed. The reviewer did not rerun
the complete workspace suite; its author-result inspection remains separate.

Fresh GitHub API metadata and all three original checkout logs independently
agree on the exact failed 78 commit. There is no retrospective Windows pass
or proven Windows event ordering. The 64 review files plus manifest and 20
author files plus manifest were copied and verified by root before import.

Root imported the exact reviewed code and record into main. The main gate
exited zero in 316.964 seconds: 1060 ordinary, eight rustdoc and six script
tests, formatting, strict workspace all-target Clippy, x.y policy and the
separate default/2 MiB controllers passed; 11 ignored remain explicit. The
engineering input hash map is identical to the passed author candidate and
unchanged before/after the main run. The reviewed test source also retains its
exact frozen SHA. No signals were needed; the main check process exited.

The next exact-head CI still needs verification. Review and author worktree
snapshots may be recoverably archived after their ignored receipts are saved;
cleanup is checked independently. This source-only change neither requires
nor establishes native GUI acceptance.

## Exact new source CI

Commit `773a11388bd4cbf2aa7e8cb3eb41e607f7db9428` was pushed and
verified against the remote ref with zero ahead/behind and a clean tree.
[Quality 37274762365](https://github.com/cyruss648/keelshell/actions/runs/37274762365)
completed successfully on all three hosts. Independent root API metadata and
all actual checkout logs agree on that commit; the named output readiness
regression is explicitly ok on every platform. macOS/Linux each passed1060
ordinary and8 rustdoc tests; Windows1040 ordinary and8 rustdoc. Respective
ignored counts are11/12/11.

Complete ZIP309164 bytes SHA256:
`b86edf82523ee037d9bc1ff8b63b5ae23d8a7bf2a93c8f89627bb3f306cd37ac`.
Original metadata/logs remain in ignored root evidence. Earlier raw job-log
downloads were refused by the CLI ANSI guard; empty stdout and errors remain.
Explicit raw capture to private files succeeded; it was not a product failure.

Author/review worktrees were recoverably archived after receipts were copied.
Both source and own target paths are absent; the root shared cache remains.
Old Windows failures/timeouts remain failed. Source CI does not establish
Windows/Linux desktop or new unmerged feature acceptance.
