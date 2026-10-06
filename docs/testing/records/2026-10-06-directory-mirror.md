# Reviewed bounded directory mirror — 2026-10-06

## Scope and binding

Isolated author candidate based on the committed synchronization/parameter/finite
schedule combination. Changed production paths are core mirror planning, session
exact reviewed removal/readback guards, and the file-panel planning, confirmation,
worker and result journal. Task audit, AI, MCP, workspace workflows and production
transfer/idle timing are outside this increment. Generic `IncludeDeletes` remains
an intention-only plan and cannot authorize the new deletion API.

This increment is files + already-empty directories, in both explicit directions.
A nonempty destination-only directory refuses the whole plan. Every observable
conflict and operation is shown; complete approval is bounded to 128 KiB. Existing
scan/content and execution budgets remain. No recursive remove, retry, reconnect,
permission escalation, rollback or SFTP compare-and-swap guarantee is claimed.

## Actual author evidence

- A read-only cache lookup failed because an old cache had been removed; no copy
  or Cargo started in that attempt. The repaired source cache was checked for
  process/FD/lock consumers, then copied with CoW under the source lock. Copy
  actually waited exit 0 /82.159 s and verified distinct destination inode and
  equal marker bytes before any Cargo. This is incremental cache reuse, not a
  clean rebuild. Source files/modes were unchanged.
- First Cargo check actually exited 101 because an overly broad test import
  selected the GPUI test macro recursively. Explicit test imports fixed it;
  repaired workspace all-target check actually exited 0 /105.736 s. Both logs
  and actual direct waits remain in the ignored author scope.
- The first focused precheck exited 101 for a missing headless context extension
  import and borrowed temporary button label. Those test-only mistakes were
  fixed; production ownership/deadlines were not weakened.
- Initial domain/SFTP focused check exited 0 /61.193 s: four core policy tests and
  three real controlled TCP/SSH/SFTP tests. Six headless GPUI tests then actually
  exited 0 /48.563 s, covering both directions, complete digest review, cancel
  before approval, full conflict display, changed target/source before execution,
  actual pending REMOVE cancellation, shared quarantine and 480×440 bilingual
  controls in System/Light/Dark.

- Additional race/policy focused run actually exited 0 /9.436 s: five core tests
  and five controlled TCP/SSH/SFTP tests. The complete conflict list retains 150
  rows; a directory filled between the final inspection and server RMDIR is
  rejected by valid STATUS, preserves the child and never triggers recursive
  removal or unknown quarantine. A reviewed local file replaced by a directory
  is refused and retained.
- First full gate actually exited 1 /28.080 s because new tests used prohibited
  `expect` assertions. Its 660 complete input files remained equal before/after;
  test assertions were converted to the established explicit failure style.
  No production guard or deadline was changed. Failure logs remain retained.
- Repaired complete `scripts/check.py` actually exited 0 /753.842 s: 1495 ordinary
  tests, 8 doctests, 6 Python tests, formatting, direct dependency x.y policy,
  strict workspace/all-target Clippy and the independent 2 MiB small-stack
  controller assertions. The 16 ignored tests remain separately unexecuted;
  standalone controller assertions are not added to the ordinary test count.
  All 660 complete inputs (14,292,660 bytes, map
  `d6d238b183d0496d54d23f0d65d7a17a5023cbc857b5e5931b0e1d97e33affd0`)
  were read and equal before/after. Actual direct child wait returned 0, original
  process group was observed absent with ESRCH, and the private TMP was removed.
- Packaging unit checks actually passed 57 cases /1.469 s. Their platform-shaped
  fixture/mocked inspection messages are not Windows/Linux native execution.
  A fresh macOS-host debug app and MCP build actually exited 0 /66.545 s;
  standard staging and native structural inspection each actually exited 0
  (/0.324 and /0.258 s). All six stage files (206,567,399 bytes) were read back,
  including both executable hashes, plist, package receipt and unchanged icon.
  Architecture, execute bits, hashes, runtime dependencies and the advertised
  macOS 15.0 minimum passed structural inspection. The enclosing owned run
  actually exited 0 /68.718 s with PG absent/private TMP removed. The same 660
  source inputs stayed equal after build/stage/inspect. This is a cache-assisted
  debug build; no application was launched, installed, signed or published.

After these gates only this test record was updated with actual outcomes; the
final source map will separately bind that exact documentation delta. Full
patch/preimages, current input map, failed/successful raw logs and owned waits are
retained in the ignored frozen author candidate. The early failed lookup summary
is explicitly a visible-tool observation summary, not reconstructed raw stdout
or inferred per-child wait receipts. Initial failure source bodies were not all
separately snapshotted; their actual logs/receipts remain preserved.

## Evidence boundary

Controlled loopback SSH/SFTP uses disposable fixtures; headless GPUI invokes the
real application controls without opening a desktop window. No customer machine,
existing application, cloud model or external-agent MCP was used. Build, package
structure and tests do not prove native desktop execution. New non-author review,
macOS native double-direction mirror interaction, Windows/Linux native behavior,
external concurrent-writer races and recursive subtree design remain open.
