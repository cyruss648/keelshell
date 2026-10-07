# Exact b54 source Quality — 2026-10-07

[Quality 37614596888](https://github.com/cyruss648/keelshell/actions/runs/37614596888)
finished on exact source
`b54b83089bbf2166f1a8351a5535e220f4946163`.

| Platform | Job | Conclusion | Recorded scope |
| --- | --- | --- | --- |
| Windows | 112769772497 | success | Canonical source gate and 57 packaging checks; application 589 passed |
| macOS | 112769772903 | success | Canonical source gate and 57 packaging checks; application 594 passed |
| Linux | 112769772639 | failure | First selected-directory Codex Ask timed out at its original eight-second request budget |

The Linux failure was at `local_agent_directory_process.rs:302`, after
preparation and request-preview assertions, inside the first
`LocalAgentClient.ask(...).await.unwrap()`. The logged interval was about
8.018 seconds. It was not the 304-second directory controller watchdog,
not a readiness/recv wait, and not the earlier 18-second application sync
failure. The Linux job had not reached application sync tests. Its precise
underlying stage and root cause remain unknown; no request deadline or
assertion was increased or weakened.

The root retrieved the final API job state and read all three complete raw
logs. Linux: 86,191 bytes / 909 lines, SHA-256
`2900792bd99c05cd09c1262167612f3dc1784f6db29c176c972ac83f531a7c63`.
Windows: 559,868 bytes / 3,854 lines, SHA-256
`a71ee561770e83ed1bb49a7bf893802b38066c04457fd79520ba2002e33047b5`.
macOS: 560,401 bytes / 3,908 lines, SHA-256
`c8c9da6084d48e423d908a9b766400949974647b2ce997a09c61a3eb5c2d301d`.
Failed initial log-download invocations and successful raw retries remain
in ignored `work/ci-b54-20261007/`, along with `jobs-final.json` and
`RAW_READBACK_FINAL.json`.

These are source/test/package checks for b54, not desktop acceptance or CI
for later combined source. The bounded test-only sync-stage recorder and
development KDF optimization are separate candidates; neither is currently
evidence for this eight-second Ask failure. The
[previous platform record](2026-10-07-selected-directory-app-platform-followup.md)
retains the earlier failure and its different unresolved boundary.
