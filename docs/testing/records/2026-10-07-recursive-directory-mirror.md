# Recursive directory mirror candidate — 2026-10-07

## Scope and provenance

An isolated managed feature tree starts at `0cca8ac59d12afdc68512b45db8830810835c634`
and imports every body of the current 689-input mirror baseline. The baseline
map SHA-256 is `99dd29c8e56d828b2df8c372267ff14cfa3f51030789545da06789fcd2d3f91c`,
14,519,367 bytes. The root checkout is read-only for this author. This candidate
adds fully expanded destination-only nonempty subtrees, parent/child execution
order, source-boundary absence checks and per-deletion remaining namespace/content
checks. It preserves the task audit, profile synchronization, module defaults and
shared mutation quarantine. See [ADR 0068](../../adr/0068-individually-reviewed-recursive-directory-mirror.md).

## Actual engineering runs

Owned command wrappers use an independent target directory, isolated absolute
TMPDIR, fixed before/after source maps, unique raw logs, bounded process-group
waits and actual exit/readback receipts under ignored
`work/recursive-directory-mirror-20261007/`. No application is installed or
launched. Cache preparation held existing lock files open read-only; source
metadata and lock bodies remained equal, and the CoW cache has no shared writable
producer.

- Transport all-target check: actual 0, 51.674 seconds, 690 source inputs equal.
- First core/TCP run: actual 0, 72.399 seconds including compilation, 691 inputs
  equal; 6 core and 4 real TCP/SSH/SFTP test cases pass. Their ten owned scenarios
  close/join the listeners and require refusal at the original ports. They cover
  both directions and exact nested rows, parent-before-child refusal, replay
  refusal, observed additions/changed same-size bytes/reappearing deleted nodes
  after one completed item, old-review refusal after restoring changed content,
  authority loss at the last nested source LSTAT and a dropped second pending
  REMOVE with destination quarantine.
- First headless run: actual 101 during compilation, 134.617 seconds, 692 inputs
  equal. The new test module omitted the existing `CheckedOption` trait import.
  Raw output and both input maps remain preserved; this is a test-only compile
  error, not a native UI failure. The import is corrected before the new run.

- The next combined selected wrapper ends actual 1 after a TCP test failure,
  21.233 seconds with 694 inputs equal. Five TCP cases pass, including the new
  actual depth-budget case. The route-wait scenario used `/mirror` as its
  canonicalization gate, but an atomic namespace root resolves its parent `/`;
  the selected REMOVE completed without ever entering that incorrect test gate.
  The exact hook is corrected to `/`, with no production change, longer deadline
  or weakened assertion. The original raw test 101 and outer 1 remain preserved;
  GPUI was not started by this failing combined wrapper.

- The corrected route-wait run passes all six TCP cases (twelve owned scenarios),
  then the first production-control GPUI case actually aborts the SFTP worker with
  a stack overflow/SIGABRT. Outer actual 1, 31.378 seconds; all 694 inputs equal.
  Every original source body and the remaining private temporary fixture files
  are preserved. This is an implementation failure, not native acceptance.
- Measurements before the repair are 32,160 inline bytes for remote mirror,
  31,776 for local mirror, 15,632 for the subtree future and 36,736 for the whole
  file-worker future. The initial 64 KiB size check passes and therefore does not
  explain away the real worker abort. A conservative 8 KiB mirror-state guard
  fails on the original body. The first app measurement module also encountered
  a test-attribute name collision from a glob import; explicit imports correct
  this test-only issue, and the app size measurement runs actual 0.
- The repair heap-owns the nested checked-read and subtree futures at explicit
  boundaries; it does not increase the transport thread stack or worker deadline.
  Exact repeated headless behavior and the full engineering gate are still
  required. All original failures, maps, changed bodies and actual exits remain
  separate from later results.

The repaired selected run ends actual 0, 21.944 seconds with all 694 inputs equal,
no surviving group and an empty new private TMPDIR. Measured inline mirror state
is remote/local 592 bytes and subtree 56 bytes; the same four production-control
GPUI cases pass. They cover explicit full-row review and cancel/confirm in both
directions, copied nested source trees, exact target subtree deletion, review-time
node addition refusal before any mutation, partial completed/unknown/unstarted
parent journal rows and quarantine, and 112 complete long rows with actual wheel
events while approval controls remain visible in both languages and System,
Light and Dark at 480×440. Full long text still contains the first and last file;
this proves headless review completeness/control bounds, not native visual quality.
The original abort's temporary files remain preserved separately.

The first full official gate ends actual 0 after 886.779 seconds: dependency
x.y policy, 6 Python checks, formatting, strict workspace/all-target Clippy,
1,556 ordinary Rust tests and 8 doctests pass; 16 target/fixture-dependent cases
remain ignored. The default and 2 MiB local-agent controllers each reach sequence
625 and their terminal group assertions. Those controller stages are not extra
ordinary test cases. All 694 code-epoch inputs remain equal, the process group is
reaped with no survivors and the private TMPDIR is empty. This successful epoch
is preserved independently of the later production corrections below.

Further source-backed review adds a new TCP regression: after one acknowledged
child removal, source appearance must revoke this owner's whole approval even
when an external writer restores absence. On the original body, both directions
actually return Ok for a later deletion and the target is no longer retained.
Both fixture listeners are explicitly joined and their original ports refuse
connections before the failing assertion. The counterexample ends actual test
101 / outer 1, 23.249 seconds including the other two commands; 694 inputs remain
equal. A core regression also actually fails on `NUL .txt`. The first app command
mistakenly selected a nonexistent library target (invocation failure); the
corrected binary-target run ends actual 101 after 11.729 seconds on that same
name-policy assertion. All outputs and the entire original counterexample source
set are preserved; these are not native Windows tests.

The correction retains all Invalid/entry-budget/content-budget mirror refusals
for that owner, including source appearance and exact-leaf changes. Restoring
bytes or absence cannot make it continue. Wrong-order or invalid-plan attempts
also require a new review/owner; known Closed revocation and dropped read-only
observations retain the existing behavior, while pending writes remain unknown
and quarantined. Regression tests require those distinct boundaries. Core and
UI path joining adopt the already conservative session name policy, including
trimmed device stems and console aliases. This is a portable policy, not a claim
that every Windows API/version rejects every such name.

The corrected selected run ends actual 0 after 40.096 seconds, all 694 inputs
equal, no process-group survivors and an empty private TMPDIR. All 37 selected
ordinary tests pass: 6 core, 25 existing/new real TCP mirror cases, the standard
transport-stack state guard, the app portable-name policy and the same 4 GPUI
production-control cases. Seven new recursive TCP cases explicitly join their
fourteen owned listener scenarios and require refusal at the original ports.
The next complete gate for that correction is recorded below. A new non-author
review and root integration remain required. No successful earlier epoch is
substituted for the latest body.

## Open acceptance and protocol boundary

Controlled protocol fixtures and headless GPUI are engineering evidence, not
macOS native GUI, production SSH hosts or Windows/Linux desktop acceptance.
No candidate commit, push, tag, release, installed application overwrite or
telemetry occurs here. The independent main mirror 689-input gate and binary
package do not establish acceptance of this new recursive candidate.

SFTP and local pathname observations cannot exclude every external writer between
final observation and unlink/RMDIR. The full review states this; newly nonempty
directories are refused without recursive fallback. Bounds reject rather than
truncate; repeated full remaining-content revalidation adds I/O and remains under
the worker deadline. Completed, known failed, unknown and unstarted journal rows
are kept separate; no replay or rollback authority is added. Conflict merge/diff
application and the wider full-product goal remain open.


## Final prerequisite classification correction

The second full official gate ends actual 0 after 778.769 seconds. Its exact
694-input map is `fc7f74484a1898655a98f8e9464e922832d6d6e1f2502994d3e30cb5861576cf`;
1,557 ordinary Rust tests, 8 doctests and 6 Python checks pass, with 16 ignored.
Formatting, x.y policy and strict workspace/all-target Clippy pass. The process
group is reaped without survivors, the private TMPDIR is empty, and both
local-agent controllers reach their complete sequence/terminal assertions.

A final concentrated review finds that a whole local subtree temporarily missing
returns Io(NotFound), rather than one of the enumerated Invalid/budget errors.
The actual three-scenario TCP counterexample moves/restores the exact owned root,
deep directory and leaf after one acknowledged removal. The root case actually
revives the old approval and deletes a later file; deep/leaf controls already
refuse it. The test joins all three listeners and requires original-port refusal
before its assertion. Actual 101, 26.183 seconds including compilation, all 694
inputs equal, no process-group survivors and empty TMPDIR. Its full original
source and failure remain preserved.

The rule is now semantic and centralized: any admitted mirror child refusal except
Closed permanently withdraws that owner's review. I/O, protocol, unsupported
observations and deadlines cannot silently restore it. This alters only review
progress, preserving the returned typed error and the pending/STATUS/complete
classification. A failed read confers no global unknown write; an already-started
write with a lost reply or readback remains MutationUncertain/quarantined. A known
STATUS refusal releases ordinary exclusion for a fresh review/owner. Closed
revocation and a dropped read-only future retain the established lifecycle,
including the existing four same-owner controls. No error-string parsing, new
dependency, broader atomicity claim or enlarged deadline/stack is introduced.

The final selected semantic-rule recheck ends actual 0 after 87.094 seconds,
all 694 inputs equal, no process-group survivors and empty private TMPDIR. All 38
ordinary cases pass: 6 core, 26 existing/new actual TCP mirror cases, the standard
worker future-state guard, the app portable-name policy and the same 4 GPUI
production-control cases. Eight new recursive TCP cases join all seventeen owned
listener scenarios and require original-port refusal. The Io(NotFound) root
counterexample and deep/leaf controls now preserve the later file and refuse
reuse of the old approval. The established four Closed/dropped-read same-owner
controls, known STATUS refusal and unknown-write isolation also pass.

The final frozen complete gate ends actual 0 after 852.413 seconds. Its exact
694-input code-epoch map is
`0a36fbb885c54a21b6245338a7a103bc1bcaa6bad00352a66ba4fd6b9f79a053`,
14,598,096 bytes. Dependency x.y policy, formatting, strict workspace/all-target
Clippy, 1,558 ordinary Rust tests, 8 doctests and 6 Python checks pass; 16
target/fixture-dependent cases remain ignored. Default and 2 MiB local-agent
controllers each emit exactly sequences 0–625 and complete their group
assertions. The 402,660-byte raw log SHA-256 is
`20b840b93675167401a4ff85014d9cef10fd5169d248c32a94b8f6d0e387fdd5`.
All source inputs remain equal; the owned process group 23122 is reaped without
survivors and its new private TMPDIR is empty. A later documentation-only result
update is distinct from the frozen code epoch; the review packet checks that all
non-documentation bodies match the complete gate.

The baseline-relative review packet includes full 689 baseline bodies, all 694
candidate source bodies, exact delta preimages/postimages, added files, original
failure source epochs and raw terminal receipts. Its patch is checked/applied
only to an ignored copy of exact preimages and every resulting body is compared
with the candidate before sealing. This preparation is author evidence, not a
new non-author approval. New non-author review, integration with the current
root remote-protocol changes and latest MCP records, and real native workflows
remain required; no successful older epoch substitutes for this body.
