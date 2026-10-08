# Windows CDB fixed-vocabulary parser trace — 2026-10-08

This is a diagnostic candidate based on `e8c2fdc2a184903251e7b11aebdd80b75cd2d816`.
It has not been imported, committed, pushed, or dispatched on Windows. Offline
parser and owned POSIX child checks do not prove CDB or Windows application acceptance.

## Observed gap

The exact [Windows diagnostic run 37744413563](https://github.com/cyruss648/keelshell/actions/runs/37744413563)
passed the three original target-exit controls for 0, 7 and 259. Its first-chance
control started the target and observed target/debugger exits 0, with owned cleanup
complete, but returned controller 125. The published errors were
`native_module_range_invalid`, `native_required_fields_or_chance_missing`,
`native_stack_order`, `native_thread_order_or_pid`, and `unexpected_native_text`;
exception events were empty. The fifth control and fixed original 706-test
application stage were not reached. The old application exception cause remains unknown.

Only the permitted SDK inventory and native-controls summary were used. No raw
debugger stream, dump, or private target data was downloaded or published.

## Candidate behavior and privacy boundary

The generated CDB commands retain their original order, expression syntax and
unhandled `gn` continuations. Fixed nonce-bound stage echoes precede `.lastevent`,
`.exr -1`, `.ecxr`, `lm a @$ip`, `kn 0x40`, `~* kn 0x40`, and `~#s`. The parser
checks recognized stage order and rejects incomplete marked sequences. Legacy
unmarked parser fixtures retain their existing strict field validation.

Each controller receipt adds `native_parser_trace`: a version, counts indexed
only by fixed stages and shapes, the first 64 fixed stage/shape/error fault
records, a dropped-fault count, and a saturation boolean. All observation counts
saturate at 255. The trace contains no raw lines, descriptions, symbols, paths,
arguments, nonce, PID, addresses, exception parameters, or arbitrary numeric input.
Unknown internal labels project to fixed fallback enums. Snapshots are detached
from mutable parser state. Existing raw capture remains bounded and private.

Diagnostic-only command-error/context-unavailable shapes remain rejected by the
strict parser. They are coarse observations, not evidence that text came from CDB.
The existing trusted-owned-target and unauthenticated-shared-stream limitations
remain in force; counts also cannot authenticate a producer.

Microsoft documents that an expression supplied to [lm a](https://learn.microsoft.com/en-us/windows-hardware/drivers/debuggercmds/lm--list-loaded-modules-)
requires parentheses. The [.ecxr reference](https://learn.microsoft.com/en-us/windows-hardware/drivers/debuggercmds/-ecxr--display-exception-context-record-)
lists minidumps as its target scope. These are investigation leads, not a verified
cause for this run, so this candidate does not change those commands or broaden
accepted text. The longest generated command is 1,437 characters, within the
documented [4,096-character command limit](https://learn.microsoft.com/en-us/windows-hardware/drivers/debuggercmds/using-debugger-commands).

## Actual checks

- New focused checks: 10/10 PASS; owner wait 0, reaped, original group absent,
  no outer timeout, 0.194262 seconds.
- Full `test_windows_native_crash.py`: 72 tests, 71 PASS and one Windows native
  control SKIP; owner wait 0, reaped, original group absent, no outer timeout,
  4.846082 seconds. This is the whole file, not the project-wide Python suite;
  the six other original script tests are not included in that count.
- All 62 original test-method AST bodies are unchanged. Fifty-one unmarked
  parser variants produce identical errors, events and harness output in the
  exact baseline and candidate.
- New controls cover command/context rejection localization, privacy payloads,
  stage order/nonce/unknown/incomplete markers, bounded counters and faults,
  detached projections, incomplete/budget failures, actual owned target success
  with diagnostic failure, and preserved original failure precedence.
- The first compatibility helper failed before running comparisons because its
  import path was omitted. Its failure is retained; correcting only that helper
  path allowed the comparisons above, without changing candidate source.

Raw stdout/stderr, precise terminal receipts, preimages, candidate bytes, the
patch and their hashes are retained in ignored
`work/windows-cdb-first-chance-diagnostic-author-20261008-v2/`.
Independent review, full project gates on the final combined source, exact new
Windows controls and original application diagnosis remain open.

## Exact combined source and root checks

Fresh non-author source/privacy review and a separate exact Git combination
review found no confirmed P1/P2 blockers. The checkout is based on exact e8 and
contains only the two Python changes and this new record; all 830 other baseline
inputs are unchanged. Transported payload scripts had mode 0755, while the
actual Git source is 100644. The first strict import rejected that mismatch
without writing source. A new explicit Git read-back preserved actual source
mode 0644 while importing the exact reviewed contents; Python callers do not
depend on executable permission. The fresh combination review binds this
distinction rather than treating payload permissions as repository permissions.

On the frozen 833 inputs / 16,372,757 bytes, root actually passed the full project
Python suite: 78 tests, 77 PASS / one Windows-native SKIP. Formatting, x.y
dependency policy, locked workspace/all-targets Clippy with `-D warnings`, and
diff checks also returned actual wait 0. All five owners were reaped, their
groups were absent, private TMP roots were empty and deleted, and the complete
input set, bytes, hashes and modes stayed equal. The existing block future
compatibility warning remains recorded. No Rust source, Cargo manifest/lock,
toolchain or workflow changed, so this Python-only slice did not repeat the
unchanged Rust runtime matrix; its parent exact e8 macOS/Linux Quality evidence
does not substitute for the new Windows diagnostic.

Original output and receipts are in the ignored
`work/windows-cdb-parser-trace-root-20261008-v1/gates/` directory. This status
paragraph is a subsequent documentation-only successor. Functional crash
repair, exact new Windows controls, original application diagnosis, main
integration and release acceptance remain open.
