# OpenSSH process inventory deadline — 2026-10-04

## Failure and preserved evidence

The docs-only commit `c66b7e25ee61c6baf5bd48f689ad5d9297adbaec`
[Quality run 37197914051](https://github.com/cyruss648/keelshell/actions/runs/37197914051)
passed all three Rust and packaging gates and the Linux OpenSSH step. The macOS
OpenSSH harness failed after 7.124 seconds because `ps -eo pid=` exceeded its
single-call 0.5-second limit. The receipt did not record a completed test count;
this run cannot count as passing macOS OpenSSH interoperability. Its owned
process and temporary credential cleanup succeeded, with nine tracked identities
and no unverified daemon ancestry.

The original failed CI log is retained at ignored
`work/quality-37197914051-failed.log`; the downloaded macOS artifact is retained
under `work/ci-37197914051-macos-interop/20261004T111622Z-ff503693cc/result.json`.
The artifact contains test-harness metadata, not application/customer logs.

## Correction

Allow up to three seconds for one process inventory query, always capped by the
remaining overall deadline. Convert a subprocess timeout into an explicit
`InteropFailure`, retaining the original exception as its cause. Do not retry,
return an empty table, change kernel birth identity/ancestry checks or weaken
owned-process cleanup. A failed inventory during cleanup continues to signal
only individually recorded identities and marks cleanup unverified.

Six isolated Python regressions cover the loaded-host allowance, short remaining
budget, no query after expiry, typed failure, identity-reading deadline and the
main failure/recorded-identity cleanup path. They start and signal no real process;
the POSIX-only harness cleanup test is explicitly skipped on Windows. The normal
developer/CI gate now runs these script regressions before the Rust checks.

Local script log: ignored `work/openssh-deadline-tests-20261004.log`.
Independent review and separate real OpenSSH results are appended after execution;
the mocked process tests alone are not transport or native acceptance.

A newly created independent reviewer inspected the exact script, six regressions
and developer-gate change, then reran all six tests successfully. No reproducible
P0/P1/P2 blocker remained. It verified that timeout failures retain their cause,
inventory never bypasses the overall deadline, and cleanup failure cannot become
a success receipt. Its review did not make another SSH or cloud request.

The corrected harness separately passed all eight opt-in interoperability tests
against an owned native macOS OpenSSH localhost server in 22.856 seconds.
The receipt recorded 51 tracked process identities, all owned processes stopped,
no unverified ancestry and the temporary credential directory removed. Logs and
receipt are retained in ignored `work/openssh-theme-mcp-20261004/` and its sibling
launcher log. This does not establish production SSH or Windows/Linux desktop
acceptance, and the next exact-source remote CI result remains to be checked.

## Exact-source remote CI

The correction commit `313771245104f88fb1071cbc8479cf9899746041` passed
[Quality 37201272070](https://github.com/cyruss648/keelshell/actions/runs/37201272070)
on macOS26, Ubuntu24.04 and Windows2025. macOS/Linux each passed900 ordinary
and7 doctests; Windows passed884 ordinary and7 doctests. The47 packaging suite
passed, with the Unix-only permission assertion skipped on Windows; the six
script cases similarly record one POSIX-only skip there.

The independent OpenSSH steps each passed all8 tests: macOS15.712s with56 tracked
identities, Linux10.885s with67. Both receipts confirm all owned processes stopped,
temporary credentials removed and no unverified daemon ancestry. Complete logs
and artifacts remain at ignored `work/quality-37201272070.log` and
`work/ci-37201272070/`. The GitHub run's exact SHA and terminal success were read
back after completion. This proves the harness correction on its source revision;
it does not cover the subsequent local-CLI or desktop-MCP changes.
