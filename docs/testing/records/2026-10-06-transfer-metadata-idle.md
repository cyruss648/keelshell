# 2026-10-06 — Owning transfer metadata and bounded idle waits

Scope: isolated author metadata repair. The previous progress-idle candidate was
blocked by a new independent real TCP counterexample. No commit, push, tag,
GUI application, local supplier agent, cloud model, customer host or installation
is started by this slice. Native and other-platform acceptance remain open.

## Preserved baseline

The independent one-second/350 ms successful LSTAT/REALPATH test returned actual
exit 101 at 1.7106 seconds. Its last valid response preceded the failure by only
298 ms, and the original destination remained unchanged. A byte-identical test
body and no-delay complete-content control are frozen in the independent blocked
archive. This author fully read, copied and rehashed all 6,653 blocked payloads
without changing them, and retains the previous 2,566-payload author archive.

Before changing production, this author applied the exact two-path independent
probe patch to the 574-input previous candidate. The 576-input baseline was
frozen and the original test returned actual exit 101 at 1.7098 seconds; four
successful metadata responses preceded the timeout and destination bytes remained
original. The no-delay control returned actual exit 0 with all 27 expected bytes.
The original 1,642-byte counterexample body remains byte for byte unchanged.

## Implementation and fixed boundaries

[ADR 0059](../../adr/0059-confirmed-transfer-idle-timeouts.md) extends the idle
observer to typed completed remote metadata/descriptor replies and actual local
metadata/source I/O. The task-local observer is bound and restored during each
operation future poll. Shared sessions do not carry an observer; nested owners,
join/select siblings and spawned children retain separate activity. Matched normal
NoSuchFile/EOF replies are completion evidence, while timeouts/malformed packets
are not. Observation never changes authority or clears a pending mutation.

Shared direct, queued, tree and continuation call sites include channel/subsystem
initialization, parent/canonical paths, OPEN/FSTAT/read-only CLOSE and snapshots.
Writable requests retain their existing mutation helpers. Spawned cleanup and
whole-session shutdown do not receive metadata wrappers. Fixed admission and
queue preparation stay outside active idle observation; existing bounded public
checked reads retain their deadlines. Full-content/tree validation remains fixed
at 30 seconds even when actual metadata or data replies continue.

## Author verification

The first repaired independent binary returned actual exit 0: original metadata
counterexample succeeded after 3.1800 seconds and read back all 27 bytes; the
no-delay control also passed. Its total eight tests include six reused filesystem
fixture unit tests and two real TCP scenarios.

Seven owner/typed-response unit tests then passed: matched success/absence,
non-renewing timeout/malformed reply, real local metadata, parallel join/select,
nested scope override/restore, spawned local metadata and a non-renewable fixed
validation deadline. An early attempt to wrap an unrelated high-level file CLOSE
failed compilation because its upstream type is std::io::Error; the wrapper was
removed and that failed log/receipt is retained.

The new metadata TCP binary has five scenarios plus six reused fixture unit tests.
It enables 150 ms metadata delays only after Started, leaving admission and queue
preparation undelayed. The fixture's FSTAT synthesizes LSTAT and therefore takes
two such delays; each actual request remains timely under its 500 ms idle budget.
Directory download and file/directory continuation verify every expected final
byte after fixed revalidation and subsequent descriptor/metadata work. A normal
queued download confirms source OPEN and read-only CLOSE. A separate pre-admission
control requires the original fixed deadline to expire despite timely metadata,
and a held read-only CLOSE must fail within idle while another owner finishes ten
confirmed atomic publications, with no unknown destination mutation.

The first new TCP command returned actual exit 101 with nine tests passing and
two reporting the public inspection API's typed no-matching-quarantine error.
The tests incorrectly expected an empty successful review; production returned
its documented rejection. Those raw logs/receipts remain preserved; a complete literal input capture was
not made for these early test-authoring attempts. Only the two
assertions and timing observation were corrected to expect the precise typed
absence result, preserving product capacity, deadlines and original assertions.
Full engineering/OpenSSH/build/package checks and fresh non-author review remain
required before this author candidate can be integrated.

The corrected default four-thread metadata binary returned actual exit 0: 11/11
tests passed. The first complete gate then failed strict Clippy on the independent
probe fixture's nested timeline type. Its entire 580-input source and raw failure
are preserved before/after unchanged. A private type alias replaces that nested
spelling without changing the packet fixture or original counterexample body.

The second complete engineering command returned actual exit 0 in 443.35 seconds:
strict workspace/all-target Clippy, formatting, x.y dependency policy, 1,298
ordinary tests, eight doc tests and six Python tests passed. Twelve existing
opt-in tests stayed ignored by the ordinary gate; both added TCP binaries ran
automatically. Seven owner unit tests passed on this final executed source,
including a separate held-owner terminal timing check during join. The original
138-test loopback binary and its unchanged five-second CLOSE barriers passed.
Default and explicit 2 MiB controller runs each emitted 626 ordered records and
38 real TCP calls (12 connected, 26 refused); this new execution measured a
5,080-byte future in each mode. All 580 literal inputs match before/after this
command. These measurements belong to this isolated source, not another branch.

The disposable OpenSSH command returned actual exit 0: ten opt-in tests passed
in 29.36 seconds. Its cleanup receipt reports 54 observed process identities,
no unverified ancestry, stopped owned processes and removed ephemeral keys.
A separate readback used the receipt's actual scratch path, found no remaining
private files and observed the owned port refusing a connection. This does not
claim a global process census. Fifty-seven packaging logic tests also passed;
mocked Windows/Linux structure results are not target-native acceptance. An
auxiliary readback first requested the wrong receipt filename; that error and
its corrected actual result.json binding are retained separately from product
command results.

The macOS application, MCP companion and self-owned fixture built successfully
in 32.20 seconds on the same 580 inputs. Both staged programs match their built
bytes, and the native structure inspector returned actual exit 0. Mach-O/plist/
minimum-OS checks do not launch the programs. Only this record, the ADR status
and roadmap were updated after the successful gate/build; an explicit three-doc
delta preserves the executed and final source separately.

## Remaining acceptance

This author candidate still requires a fresh non-author review, combined main
checks and a newly built native package performing actual complete-byte transfers.
The previous native unknown result and independent metadata failure remain
failures in their original archives. No application window, native transfer,
MCP companion process, supplier CLI, cloud request, customer host, installation,
commit, push or tag was started by this slice.

## Independent READ EOF rejection and narrow repair

The next fresh non-author review rejected this metadata candidate with a new
actual TCP counterexample. Ordinary and directory downloads each recognized a
valid READ EOF but did not renew active idle before waiting for read-only CLOSE.
The review keeps the application idle budget at one second, DATA/EOF delay at
750 ms, CLOSE delay at 350 ms and exact content at 22 bytes. Its initial shorter
outer test bound and subsequently finalized 20-second driver are both preserved;
the product budget, payload and delay did not change. Its full blocked packet
retains 31,261 payloads, including the older immutable archives. This repair
references those archives rather than recursively copying them again.

The author verified all 580 prior final inputs, applied the exact test-only
57,716-byte two-path patch and froze 582 inputs before reproducing the formal
case. Actual exit 101 retained both terminal failures: file at 2.106780292 s
after EOF at 1.858 s, tree at 8.113242333 s after EOF at 7.864 s. Both local
files contain all 22 expected bytes, but neither terminal result is Completed.
The test's original 3,612-byte body remains identical to the independent probe.

A separate real TCP empty-file scenario also failed on unchanged production:
file at 1.355647291 s after EOF at 1.105 s, tree at 7.356156584 s after EOF at
7.105 s. This controls the absence of DATA and requires Completed with zero
bytes. The two repaired production branches only call the existing current
context's confirmed-I/O notification before breaking on matched EOF. That helper
only sends the private owner watch notification and never clears the mutation
marker. Source audit retains existing READDIR typed wrappers, fixed resume
content verification and source-shrink errors. No other production path, timeout,
byte count, quota, reservation or cancellation policy changed in this slice.

Two auxiliary body-extraction assertions initially omitted a delimiter newline;
they stopped before tracked writes. A subsequently dispatched empty selector
therefore ran zero tests and is explicitly excluded from acceptance. The exact
body binding and actual empty negative control replace that auxiliary result.
The first formatting check found only the newly added empty function's layout;
its raw failure is retained and the standard formatter corrected it without
changing the original nonempty body.

The repaired TCP binary returned actual exit 0 with eight tests: six unchanged
filesystem fixture units and the two real TCP scenarios each covering file and
directory modes. Nonempty and empty modes require Completed with exact 22 and
zero bytes respectively, under the same one-second idle and 750/350 ms delays.
The one new complete engineering command returned actual exit 0 in 448.96 s:
formatting, x.y policy, strict workspace/all-target Clippy, 1,306 ordinary tests,
eight doc tests and six Python tests passed. Twelve existing opt-in tests remain
ignored by this ordinary gate. The unchanged 138-test loopback binary, seven
owner unit tests, old metadata/idle binaries and new EOF binary all ran on the
same literal source. Default and explicit 2 MiB controller runs each emitted
626 ordered records with 38 real TCP calls (12 connected, 26 refused); this new
execution measured a 5,080-byte future in each mode. All 582 inputs match
before/after. The command's direct child was waited/reaped with no surviving
owned process group.

The one new disposable OpenSSH command returned actual exit 0 in 9.59 s; its
inner receipt reports ten passed opt-in tests in 9.501 s, 63 observed process
identities, no unverified ancestry and removed ephemeral credentials. A separate
readback found its actual scratch path absent and its owned listening port
refusing a connection. This is not a global process census or desktop acceptance.
All 582 inputs still match the complete gate's source. Only this record, ADR
status and roadmap received final verification text afterward; the packet keeps
their preimages and exact three-document delta. No application build, staging,
installation or GUI run was repeated for this EOF slice.

Fresh non-author review, root combination and new actual native transfer evidence
remain required. The old metadata candidate stays blocked; neither the prior
author gate nor this new author gate closes those acceptance steps.

## Fresh nonauthor EOF review and main integration

A new reviewer reconstructed the sealed candidate from its exact baseline and
passed the unchanged original 22-byte case, the empty-file paths, genuinely
unanswered readonly CLOSE, and held WRITE/writable CLOSE with other owners
producing EOF on the same SFTP session. The full independent engineering gate
passed 1334 ordinary tests, eight doctests and six Python tests, formatting,
strict Clippy and x.y policy. All 586 inputs remained equal. Both default and
2 MiB controllers executed 626 records and 38 TCP calls; ten separate localhost
OpenSSH tests passed. Reviewer test assembly and lint failures remain retained.

Root read back all 1265 new proof payloads and applied only the 23-path root
integration delta plus five reviewer test paths: 27 unique paths with one shared
control module. All 26 non-roadmap bodies equal the sealed author/reviewer
content; existing root inputs outside this scope, including local-agent and
target-parameter changes, stayed equal. The roadmap was reconciled separately.
Current root inputs number 605 before later disk integration and result records.
The combined-main gate, fresh build/package and actual two-upload native scene
remain required. Neither this review nor main patch application closes the
preserved zero-completion macOS failure or proves other-platform desktops.
