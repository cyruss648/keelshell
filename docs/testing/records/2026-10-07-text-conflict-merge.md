# Reviewed text conflict merge — author record, 2026-10-07

Candidate base: `3873dacb9fe6608a9aec40136bb1d82200da9a51` in an isolated managed
worktree. No main-tree, shared VM or unrelated application changes are included.
The product goal remains incomplete; this record covers the text feature slice only.

Initial core text filter exited 0 (16 tests, including the initial 11 new merge/patch
cases). The first app check exited 101 due to two GPUI return-element type mismatches;
a subsequent edit introduced a field-visibility token into a function argument and
was rejected at formatting/check. Both failures are preserved in ignored author logs.
The fixes retained proper opaque/AnyElement returns and explicit test visibility.
A subsequent three-crate all-targets strict Clippy exited 0. These intermediate runs
were not source-frozen and are not the final author gate.

New tests cover exact CRLF/EOF behavior, independent/adjacent merge edits, identical
changes, simultaneous insertion/deletion conflicts, explicit/manual resolution and
limits; small exact document matrices verify every one-sided/identical merge and
round-trip generated unified LF diffs. Owned SSH/SFTP adds the independent 1 MiB
editor budget while checking unchanged 64 KiB MCP limits, external changes after
staging, revocation, type/metadata changes and denied authority. GPUI actions exercise
read-only merge, stale-save conflict discovery, every choice, manual content, patch to
draft, complete final review, remote readback, cancellation/archival, late-read draft
preservation, a new remote version, link rejection and both languages.

The initial transport run exited 101 (3 passed, 1 failed) because the test expected
an unknown quarantine after an acknowledged WRITE and later authority denial. The
protocol correctly returned a known refusal without an unknown record. The corrected
case separately proves confirmed-write revocation and a newly added dropped,
unacknowledged writable CLOSE quarantine. All five controlled TCP cases exited 0;
the original failure log remains preserved.

The first GPUI run aborted with a default-thread stack overflow before the harness
finished creating its initial window. A bounded phase probe confirmed the failure
preceded editing/network actions. Rendering was split into smaller cards/control helpers; the original six GPUI
scenarios then exited 0 on the default stack. A missing test element ID was also
corrected before those passed. The stack/deadline was not increased. Original abort,
phase and intermediate element-ID logs remain.

A preliminary read-only core review independently compiled the original modules
against the existing dependency artifacts and ran 170,112 small-document merge
assertions plus 1,806 independently generated exact patches, all actual exit 0. Its
source hashes were unchanged during the probe. This is algorithm evidence only,
not final frozen/UI/transport review.

A subsequent full-version two-axis wheel probe exposed inner gestures also moving
the outer tools viewport; different event orders and a background-only attempt did
not fix it. The production inner text bubble guard is under verification, with each
original failed probe retained. Two additional GPUI cases cover this and manual-buffer
navigation/choice invalidation.

Final scoped tests, frozen complete engineering gate, receipt hashes, native build,
independent review and integration results will be appended when actually observed.
Native macOS interaction, minimum-window/accessibility, Windows/Linux desktop and
customer-server acceptance are not established by these controlled cases.

The first frozen full gate (v1, 741 inputs) exited 1 after 78.351 seconds during
Clippy: a newly added outer-scroll assertion referenced `outer_before` without its
definition. Tests/native build did not run. The before/after inputs were unchanged,
the owned process group disappeared and its private TMP was removed. The v1 failure
receipt/log remains. Epoch v2 records the actual pre-gesture outer offset, adds a
localized complete-version metadata assertion, and labels logical CRLF/LF diff
equality separately from byte equality with a dedicated GPUI case. No assertions,
default stack sizes or deadlines were weakened.

Epoch v2 exited 1 after 12.799 seconds during Clippy: the new localized-summary
test used an integer literal that inferred `i32`, unsupported by GPUI ElementId.
It did not reach tests/native build. Its 741 input hashes were unchanged and owned
PGID/TMP were gone. Epoch v3 makes the row index explicitly `usize` and retains
the snapshot before borrowing its label. A separate frozen scoped GPUI run then
exited 0 after 83.285 seconds: all nine conflict/patch cases passed on the default
stack, including both actual wheel axes, unchanged outer scroll offset and Chinese/
English summary text. Source inputs were unchanged and its owned PGID/TMP were gone.
The linker emitted an oversized unwind-table warning; existing `block 0.1.6`
future-incompatibility warning remains. This controlled run does not prove native UI.

The full v3 gate exited 1 after 314.003 seconds: policy, six Python tests, formatting
and strict workspace Clippy passed, then app tests reported 591 passed, one failed,
two ignored. The old real-state workspace layout case failed at `transfer_tests.rs`
because the new full-width inner diff guard intercepted the outer gutter's wheel
and prevented reaching Mkdir after comparison/editor/diff. Subsequent crates and
native build did not run. All 741 inputs were unchanged; owned PGID/TMP were gone.
The independent reviewer separately reproduced the same failure without changing
the frozen map. Both failed traces are retained.

Epoch v4 gives the inner viewport an actual side margin, retaining its bubble guard
while keeping the outer navigation gutter usable, and restores the existing 48 px
confirmation budget without dropping or truncating any text rows. The existing
900×580 and 1440×900 real transfer/comparison/editor/review layout case also requires
the new merge/patch controls to fit and be reachable, for both languages/themes and
assistant visibility. A frozen scoped run exited 0: nine new GPUI cases (16.001 s
including rebuild; 3.28 s tests) and the enhanced combined layout case (15.334 s).
All source inputs stayed unchanged; both owned process groups and private TMPs were
gone. No timeout, stack or reachability assertions were weakened.

The frozen v4 full canonical engineering gate exited 0 (563.489 s), then locked
app/MCP native build exited 0 (45.260 s). All 741 before/after hashes matched;
both owned PGIDs and private TMPs were gone. Full logs report 1,649 passed, zero
failed and 22 ignored across 71 result groups, separately from six Python tests
and the controlled process controllers. App reported 592 passed/two ignored;
core unit 136 passed and owned SSH loopback 143 passed. Both native artifacts
are ARM64 Mach-O with minimum macOS 15.0. This is build evidence, not native UI.

The final independent v4 review then found a P2 concurrent-reread defect after
that gate: while a new remote read held metadata, an old merge adoption could
replace base A with B while its in-flight plan still used A. The receiver displayed
the mutable B baseline instead of the plan's A. A first test-only repro failed
because a direct click targeted an offscreen control; using the existing platform
reveal helper reproduced the actual baseline substitution (101, 12.134 s including
build; 0.29 s case). Its before/after 741 maps matched, with only the repro test
different from v4; production repairs occurred afterward. Raw failure/maps and a
source-transition receipt remain. This was a version-review defect, not write
authorization bypass; the v4 gate cannot establish its correction.

Epoch v5 carries the worker's complete captured base into the outcome, checks path/
base bytes/draft together before receiving, and displays that captured base. A new
read retires old review choices; runtime and UI adoption both require no busy/owned
operation/pending review. Two extra controlled GPUI cases cover the real held read
and an independent changed-base late-result rejection. A frozen scoped run exited
0: all eleven merge cases (18.735 s including rebuild; 3.55 s tests), then the
enhanced real layout case (15.161 s). All source inputs stayed unchanged and both
owned PGIDs/private TMPs were gone. New final full gate/review remains pending.

Final frozen v5 canonical engineering gate exited 0 (520.553 s), and locked app/MCP
native build exited 0 (7.398 s). The full 741 input maps were identical before/after
(`a7f4f1522ce5b969cffaf98349afcd979663c0a096ea12a2072c6ed361efc366`);
each owned process group and private TMP was verified gone. Logs report 1,641
workspace unit/integration passes plus 10 doctests, zero failures and 22 unchanged
ignored tests. Six Python tests and controlled agent default/small-stack process
controllers also completed. App: 594 passed/two ignored; core unit: 136 passed;
owned SSH loopback: 143 passed. Default stacks and request deadlines were retained.

The fresh non-author review independently checked the complete logs, source map,
actual exit receipts, cleanup and Mach-O headers, then sealed the 17-path feature
scope with no open P1/P2 findings. Its final-bound 11 GPUI cases (4.326 s), enhanced
combined layout (18.465 s) and five TCP cases (0.282 s) all exited 0; maps and exact
test binary hashes remained unchanged. An independent same-module core probe also
passed 370,386 merge symmetry assertions, 10,293 one-sided exact cases and 3,188
independently generated full-document patches, with boundary/refusal checks.
Frozen review SHA: `af64dafb3dfc04e541b8902f448a307fac8bc7cca02a83c8938024b6fe17071c`;
seal SHA: `9ed425b86bc0729fc6c938befb8ccdf1b048cdd161147c9935379b006c251016`.

The final native app and MCP are ARM64 Mach-O, minimum macOS 15.0. App SHA:
`9729cf10585365914668bd3600aacac94a09aa8577385a0df27da871b02704a6`;
MCP SHA: `4852d9c6a52b25885ecfedec251dedeb827ee2b0c4d4e2572e2140c38c3e472f`.
Actual full-gate app-test SHA: `623d922fce85bb960af82b7d5ccd60e3d43923575a1de3542b20807e10db64f8`;
actual full-gate TCP-test SHA: `6944aff3e3e7e367e6fda8ac045ddd33c9c695fdcd57cffcd9b7957dfdadd76f`.
Only this actual logged TCP artifact binds the final review; older cached profiles
are separate historical evidence. One supplemental v4 checksum probe named a
nonexistent artifact and exited 1; the final probe resolves paths from the actual
full-gate Running lines and exits 0.

This result paragraph and ADR status are documentation-only post-gate changes,
separately mapped/reviewed; production/test source remains the sealed v5 candidate.
Main-tree integration, new native macOS/VoiceOver interaction, Windows/Linux UI
and packages, real external/customer SSH servers and a fault-injected desktop
acknowledged-publication/failed-readback branch remain unverified by this slice.
The failed-readback branch has source review; controlled successful readback is
tested. SFTP still supplies observations without remote compare-and-swap. No GUI,
shared VM, supplier CLI/model call, push or release was performed by this author.
Full product scope remains incomplete. Root integration must consume this candidate
against its current base and record its own combined-source validation.
