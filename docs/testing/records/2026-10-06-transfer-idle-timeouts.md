# 2026-10-06 — Confirmed transfer progress and idle deadlines

Scope: isolated author candidate based on the existing parallel-transfer and
writable-CLOSE repair. This is not a new native, Windows/Linux desktop, supplier
CLI, cloud-model, customer-host or release acceptance result.

## Preserved causal baseline

Before production changes, two new formal Rust tests used an owned loopback
SSH/SFTP server, a one-second connection timeout, 1 MiB local files and 90 ms
WRITE latency. Both require complete bytes to succeed. The unchanged baseline
returned actual exit 101: 0 passed, 2 failed. Direct upload stopped at 1.004 seconds
with `MutationUncertain`; queued upload stopped at 1.003 seconds with `Uncertain`,
655,360 confirmed bytes and 11 started WRITE requests. Those counts belong to
these TCP tests and do not measure an earlier native application's request count.
Both tests closed their authenticated sessions before reporting failure. The
owned Cargo child was waited/reaped, with no process-group survivor.

The raw log, command receipt and exact source bodies are preserved in ignored
`work/`. An initial compiler cache copy was stopped and waited before test
execution; cached build files are not acceptance evidence. New test-only generic
annotation and owned response-hold compilation failures are also retained rather
than overwritten.

## Implemented waiting policy

[ADR 0059](../../adr/0059-confirmed-transfer-idle-timeouts.md) describes the narrow
change. Confirmed I/O renews each transfer's active idle interval. Presentation
activity and another owner's acknowledgements cannot renew it. Pause excludes
wall time; cancellation and shared active/quarantined ownership remain binding.
Full-content/tree validation retains an explicit fixed 30-second read-only limit.
Direct upload/download/atomic and every queue job type share the same idle policy.
The previous aggregate deadline is not increased to mask a slow transfer.

## Author verification

The original two tests pass after repair at about 1.50 seconds, with 16 WRITE
requests and 1,048,576 complete bytes read back. The expanded matrix verifies
ordinary and atomic direct/queued uploads, directory upload, file/directory
continuation, and direct/queued/continued/directory downloads. Slow READ continuation
also exercises full-content validation longer than the one-second idle interval.

Six control tests, including five new ones, passed in the first focused run:
confirmed local completion, non-renewing safe points/presentation, independent
owners, non-renewable fixed validation, acknowledged pause/cancellation, and the
existing full event-queue cancellation. Formal tests use outer hard bounds and
isolated temporary data. Held WRITE and CLOSE each time out independently while
another job continues acknowledged writes and completes. They retain both final
and temporary reservation IDs after a late reply, and a new SSH connection cannot
write the isolated target. An unanswered READ fails within the idle bound with a
known 65,536-byte local prefix and no unknown destination mutation.

The first complete engineering command failed in the original shared loopback
binary: 142 tests passed and three existing writable-CLOSE tests exceeded their
five-second server-entry barriers. The failed command, all 574 input bodies and
subsequent diagnostic sources/logs remain preserved. Actual diagnostic admissions
showed the three targets blocked by the same active local `whole_side` write claim
for a slow resumed download: reservation 375, costs 18/19, 17/18 unknown groups and
zero active reservations belonging to the waiting owners. The 32-capacity branch
did not run for these targets; the initial capacity hypothesis was not established
and is superseded by this observed active-claim conflict. No risk acknowledgement,
quota increase or weakening of the conservative alias rule was used.

The new slow matrix now runs in a separate, automatically discovered integration
binary, keeping its process-wide registry distinct from unrelated loopback tests.
Its minimum password-authenticated SSH/SFTP server reuses the existing filesystem
packet fixture. Relevant channel/subsystem handlers, accepted/rejected password
bodies, server configuration, session disconnect and listener-drop bodies have
normalized Rust token equality with the original helper. Unused forwarding and
shell helpers remain in the unchanged original binary. Both temporary diagnostic
files were restored byte for byte. The ten original writable-CLOSE tests, including
all three failures, then passed without changing their assertions or barriers.
The standalone matrix passed seven new TCP tests and six repeated shared-fixture
unit tests. Its first complete isolated gate failed strict Clippy because the new
READ-delay setter is unused in the original shared test target. That failed gate
and all 574 source bodies remain preserved. The shared fixture method now has a
local dead-code allowance explaining its use by the separate integration target;
production code did not change for this test-process adjustment.

The final complete author engineering command returned actual exit 0 in 485.59
seconds: dependency policy, formatting, strict workspace/all-target Clippy,
1,272 ordinary tests, 8 doc tests and 6 Python script tests passed. Twelve existing
opt-in tests were ignored by the ordinary gate. The count includes five new
control unit tests, seven new TCP tests and six existing filesystem-fixture unit
tests repeated in the separate binary. The default workspace command ran the
original 138 loopback tests and new 13-test binary automatically; no new ignore or
old assertion/deadline change was introduced. Default and explicit 2 MiB controller
runs each completed 626 ordered records and 38 TCP probes, with 12 connected and
26 refused results as checked by their original assertions; both reported a
5,080-byte future. All 574 literal inputs match before/after this successful gate.

The existing bounded OpenSSH script then passed ten opt-in tests in 20.12 seconds.
Its cleanup receipt recorded 51 observed process identities, no unverified
ancestry, stopped owned processes and removed temporary keys. An independent
readback of its actual scratch directory found no keys, and its owned port refused
a connection. This retains the script's observed-ancestry boundary and does not
claim a global process census. Fifty-seven packaging logic tests also passed;
they do not represent native desktop execution on their mocked platforms.

The macOS application, MCP companion and owned loopback fixture built successfully.
The standard dual-program stage matched both binary hashes and passed the native
structure inspector, including Mach-O dependencies, plist and macOS 15.0 minimum
checks. The application window, companion and fixture were not launched by this
build/inspection slice. The build's 574 input map still matched the successful
engineering map. Only this record and the ADR's status were updated after these
checks; their explicit pre/post documentation delta is preserved with the final
source and candidate patch. No commit, push, tag or installation occurred.

Raw failures, diagnostic bodies, source snapshots, receipts and staged byte copies
are frozen in ignored evidence. Auxiliary proof-script errors (including a wrong
TCP-summary expectation and wrong receipt/private-directory names) are retained
with corrected readbacks; they are distinct from the actual product gate results.

## Unverified boundaries

A fresh non-author review actually blocked this candidate with a delayed
metadata-reply TCP counterexample. The original archive and failures remain
unchanged. The subsequent author repair and its verification are tracked in the
[metadata record](2026-10-06-transfer-metadata-idle.md); this earlier gate does not
validate that later production change. Combined main-tree checks remain required. A new native package must
perform actual transfers and verify the owned destination bytes.
No GUI window, real local intelligent-agent process, vendor/cloud endpoint,
customer host, installation, tag or publication was started by this author slice.
Build/inspection results cannot close native UI or cross-platform acceptance.
