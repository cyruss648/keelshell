# System appearance and semantic colors — 2026-10-04

## Source and local gates

The frozen implementation was based on `c66b7e2`. Its tracked source diff SHA-256
was `22bdb2e7445929016b5585ef69a74d3f6f16fad87f7f2fbcf319f63f73108a9f`;
the new `workspace/tests/themes.rs` was independently reviewed alongside it.
See [ADR 0036](../../adr/0036-system-appearance-and-semantic-palette.md).

- Final `python3 scripts/check.py`: formatting, strict workspace Clippy, direct
  dependency policy, **866 ordinary and 6 documentation tests passed**; eight
  opt-in OpenSSH tests were ignored by the normal gate. The SSH source did not
  change in this slice; its separate eight-test interop evidence stays in the
  [adapter record](2026-10-04-workflow-ssh-adapter.md).
- Four new GPUI appearance tests passed: actual bilingual buttons and state
  persistence; retained terminal/input/AI identities and unsent drafts; source
  revisions and no terminal writes; System handler inputs without file writes;
  explicit preference ignoring later system input; stale save retaining visuals
  and late edits. The minimum toolbar regression renders 900×580, Chinese/English,
  empty/two-pane states and checks visibility, nonzero bounds and no overlap or
  overflow. These are actual GPUI layout events, not GPU pixels or OS signals.
- Two palette/appearance unit tests and three locale/migration tests passed.
  Missing theme defaults System; known preferences round-trip; unknown fails.
- `cargo build -p keelshell-app --locked` passed, as did the fixture build and
  native Mach-O staging. Packaging regression: **47 tests passed**.
- Two independent reviews ran. The earlier reviewer identified and verified
  fixes for a nearly white odd connection row and fixed monitor success green.
  A newly created reviewer independently ran all four GPUI, two palette and
  three migration tests, inspected the frozen diff and found no code blocker.

Final logs: ignored `work/gate-20261004-theme-minimum-final.log`,
`work/theme-minimum-test.log`, `work/package-theme-final-20261004.log` and
`work/build-20261004-theme-native-app.log`. Review logs are preserved under
`work/mcp-server-evidence-20261004/` after copying from the review worktree.

## Preserved failures

The first full theme gate failed strict `expect_used` in a newly added migration
test. It was changed to a typed deserialization error before the successful gate.
Initial compile attempts exposed an integer-type mismatch in contrast math and
incorrect toolkit test/Selectable paths; they were repaired before acceptance.
`work/gate-20261004-theme.log` and the initial check/test logs remain available.
An initial packaging command targeted nonexistent `packaging/tests`; the correct
`python3 -m unittest discover -s packaging -p 'test_*.py'` passed all 47.
No failing check was suppressed or weakened to obtain a passing result.

## Native macOS evidence

Final aarch64 Mach-O SHA-256:
`5f5da96078517283820fd4ada957dd0a2fd44dc308a9bc26415c2e092874e609`.
Staged manifest: `work/packages/theme-native-20261004/package-manifest.json`.
The unique audit bundle used identical executable bytes, isolated mode-0700 data
and mode-0600 state. No installed app or system appearance preference changed.

Eight exact JPEG screenshots were saved and visually inspected. Their hashes,
pixel dimensions, accepted/rejected status and cleanup are in
`work/theme-native-20261004/receipt.json`:

| Frame | Observation |
| --- | --- |
| 01 | Missing-theme state started in System, resolved to current dark appearance; both connection rows are readable |
| 02 | Two authenticated controlled TCP SSH sessions, Dark selected, command and AI question still unsent |
| 03 | Light selected; the same two tags, terminal banner, command and AI drafts remain; native titlebar is light |
| 04 | Returning to System immediately restores dark; English labels and both drafts remain |
| 06 | Dark AI settings fields, focus ring, provider choices, hints and fixed footer are visible; new draft was cancelled and no profile saved |
| 07 | Fresh process loads explicit Dark and English from isolated state |
| 08 | Fresh process loads explicit Light, retaining its priority over the current dark system appearance |
| 09 | Final selection returns to System/Chinese and dark appearance |

The controlled server only echoes UTF-8 and serves its disposable SFTP tree. It
does not execute shell commands or support monitoring exec; the visible monitor
refusal is expected. No model context was sent or provider credential supplied.
These screens verify this specific native appearance flow, not arbitrary real
programs, monitor behavior or live AI.

Two resize attempts returned concurrent-state warnings and required re-reading
the window. Frame 05 does not establish 900×580 or unchanged drafts and is retained
as **rejected** evidence. Subsequent native operations used refreshed AX state.
Do not claim the minimum native window from the passing layout fixture.

Both restart controller processes and the final app exited. The owned fixture
received SIGINT and exited; its listener and temporary filesystem root were
verified absent. Its ephemeral host key was never written. The duplicate audit
bundle, launch helper and two one-off palette migration scripts were removed;
screenshots, receipts, passing/failing logs and the staged review artifact remain.

## Open boundaries and follow-up

Actual OS appearance-change notification, native 900×580, native Windows/Linux,
every modal/terminal CJK/search pixel and full accessibility remain unverified.
The System notification handler and explicit override rules have fixture evidence,
but no OS theme change was performed. This slice has no new remote CI result yet.

The native English file footer clips “Compare folders” with the AI sidebar at
1440 px; an already-open tooltip can retain English after switching to Chinese.
Track these in D2/UI-04/UI-05 alongside background modal AX isolation, rather
than calling the visual refresh complete. Theme foundations enable that work.
