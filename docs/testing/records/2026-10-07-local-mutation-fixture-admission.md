# Process-local file fixture admission — 2026-10-07

The exact `a4c15c04772b11a70a592ae7c95132b4bea4d818` Linux CI run
37624355451 / job 112802208027 ended with 621 passing application cases, one
failure and two ignored cases. The failed directory-sync case reached a terminal
state with no worker, no pending approval, zero atomic writes and original `old!`
bytes. Its detail was the typed application mutation-isolation rejection, not an
operation timeout. The complete 322086-byte raw has SHA-256
`f37c9b3bea5ff627801e707252e58036871e17f9de023a8a2cb6bdc188b98483`.

Production source tracing shows that existing local writers reserve the entire
process-local side. SSH fixture peers have independent ports, but local claims
share the process registry. A terminal-worker shutdown error can be emitted by a
later caller reaping an earlier global worker; its captured stdout is not proof of
the current test's comparison outcome. The historical precise lease owner remains
unknown and is not inferred from a subsequent local pass.

Before changing the harness, a controlled same-process two-peer test passed with
the original production isolation and limits. It held a resumed 144 KiB download
at its real READ, observed a separate directory sync reject with zero atomic
writes and exact old bytes, released the hold, completed the exact resumed bytes,
then required a fresh sync plan and confirmation for exact new bytes. The actual
case finished in 0.70 seconds; its owned runner exited 0 in 3.914628875 seconds.
The raw SHA-256 is
`fdfeb50a6facb8d2ab1aadfc3816bf2b43d629f2c473d0588a15173de27e1c50`.

The new candidate uses bounded synchronous fixture setup admission and explicit
same-group siblings. Only four old Rust paths change: the cfg(test) group module,
file harness setup plus one named destructuring guard, the MCP opt-in constructor
and its single local-transfer case. The twelve original child test files are byte
identical. All original action bodies are byte identical after reversing only the
named guard/opt-in constructor. Production registry, four-thread test setting, byte
assertions and 12-second / 10-second bounds remain unchanged.

Preserved author failures: the first formatting wrapper exited 1 because its
source-invariance check rejected intentional formatting; the child's actual wait
code was not persisted and is reported as unknown. Its process group was actually
absent and empty private TMP removed. A subsequent explicit formatting run exited
0 with the intentional source delta recorded. The first candidate compile exited
101 for a helper missing `async` and a harness pattern missing the new guard.
The full raw and source epoch remain; both mechanical issues were corrected.

The first four-thread candidate run exited 101 before the original actions: 29
cases passed, 62 failed because GPUI rejected the direct external Tokio await
(`Parking forbidden`). Its complete raw, source epoch and actual wait/reap remain.
The subsequent asynchronous v3 candidate passed 91 cases at four threads in
57.320839166 seconds and strict app Clippy in 39.615247875 seconds; format and
x.y policy also passed. These are historical limited facts for a rejected design.
Fresh independent static review found P2: GPUI allow_parking permanently sets an
internal thread-check exemption, which forbid_parking cannot restore, and parking
advances the GPUI test clock. No PASS seal or acceptance was issued for that design.
All v3 source/raw/receipts remain. The new candidate removes parking, heartbeat,
external GPUI awaits and the thirteen old constructor-await edits entirely.

The rejected synchronous v4 author candidate compiled with actual exit 0 in
22.087193375 seconds. Its own immutable 251966032-byte test executable has
SHA-256 `d205a6b13af697fd3e2f826d0314059a1ba882551c6b48dd8a69cec7db7141f9`.
The same four-thread related scope passed all 91 cases: actual exit 0 in
49.777370584 seconds (48.14 seconds reported by libtest). Strict application
all-targets Clippy exited 0 in 6.051919417 seconds; format check and dependency
x.y policy exited 0. All five gate maps contain the same 769 input files /
15494255 bytes before and after. Every raw and snapshot was fully read back;
owned leaders were reaped and process groups / private TMP are absent. Only this
record and ROADMAP status changed after those gates; Rust inputs remain unchanged.

The rejected v3 artifact/source evidence is retained separately. A precise
historical CI lease owner is still unknown. New independent non-author review,
main integration and exact integrated CI remain pending. These controlled fixtures do not establish customer, external
model or target-native desktop acceptance. The complete owned raw/source/process
receipts are kept in ignored `work/local-mutation-fixture-proof-v1`.


Fresh non-author runtime review rejected v4: normal Harness drop with a retained
real file window admitted another group while the paused existing-local transfer
still owned the application claim. The independent 2.683840208-second exit-0
counterexample is evidence of the defect, not approval. Its frozen witness seal
is `2564df4d575f27d14c287290079f0c91b51a154496727283a03791f95d0b865d`.

The v5 candidate binds weak setup groups to the actual GPUI App. Same-App original
loops/secondary panels inherit it; a different App still acquires the process
permit. Actual file entities/transport workers retain the group; registered real
queues and original runtimes retain admission through off-UI queue.close joins.
Failure/timeout/unconfirmed join keeps the permit unavailable. All six old Rust
paths restore byte-for-byte after reversing only cfg(test) hooks and mechanical
fixture setup/opt-in. All twelve original child files remain byte-identical;
production claims, four threads, original actions/assertions and limits remain.
MCP coverage is only the original complete opt-in local-read queue case, whose
explicit queue.close/SFTP.close and reviewed remote actions are unchanged.

Preserved v5 failures: compile-1 exit 101 for four new helper/type access errors;
compile-2 exit 101 for missing explicit GPUI imports. Two controlled probe runs
exited 101: one used the wrong actual staged CLOSE prefix and an empty window's
fast GPUI clock; another omitted normal App-update payload cleanup after the last
entity reference drop. No original deadline/clock/guard was relaxed. A separate
static review found runtime-weak grouping would self-lock original same-App layout
loops; it was replaced by App Global Weak<Group> before running those loops.

Compile-5 exited 0 in 18.01986075 seconds. The exact copied 253026864-byte executable
has SHA-256 `4de7cd06ac0a1f3568dcd6bb00f5d367be9b8aa0a821ececd1f5875ba735925a`.
Six controlled cases exited 0 in 5.020260083 seconds (3.37 libtest), complete raw
SHA-256 `cc0fbc8af14249a8079d0b1871647b62b982b9ac0059b8d8860e12b58aa8ea45`.
Two actual lifetime rows show Weak<FilesPanel> upgrade=false before/after flush:
that is logical strong-count state, not payload destruction by itself. Group
owners went 2 to 1 after the removed working panel flush, while CLOSE pending=1
and actual worker_finished=false; independent admission stayed Pending. The
retained/cancelled panel's group owners went 1 to 0 on its normal payload flush.
Only real terminal/queue cleanup released the next group. The captured invalid
thread-name panic was expected and caught in an independent local semaphore test;
no thread started and available permits stayed zero. A real paused SSH local queue
also blocked admission after cancellation while another Arc remained, then its
actual scheduler close/join released the contender; owned threads were joined.

The necessary related scope, using the original two filters and four threads,
passed all 95 cases (original v4 91 plus four new lifecycle/failure cases), actual
exit 0 in 50.143014042 seconds (50.06 libtest), raw SHA-256
`0266d84c0002d711ad1d0e7b77d312d6bc57688d0c004f3e6ab64122aa505e25`.
Compile/control/related gate maps each contained identical 769 input files before
and after. Strict checks, final documentation binding, new independent review,
main merge and exact CI are recorded separately. No customer/model/native desktop
acceptance follows from these controlled process-local fixtures.

Strict application all-targets Clippy exited 0 in 62.398898042 seconds; format
check exited 0 in 1.5237715 seconds and dependency x.y policy exited 0 in
0.093397416 seconds. Their 769-file / 15522453-byte before/after maps are equal.
The upstream block 0.1.6 future-compatibility notice is preserved; it is not a
project Clippy failure. All 19 v5 owned runner results/raws were fully read and
hashed; two compile and two probe exit-101 records remain. Every owned leader was
actually waited/reaped and each process group/private TMP freshly checked absent.
The actual runtime window was released before offline freezing. No Rust input
changed after the passing compile/control/95 gates; four documents changed before
strict checks, and only this record/ROADMAP status changes after strict checks.
The final seal records every epoch/delta rather than claiming identical documents
at all gates. A separate new-App/cloned-App identity control remains for fresh
non-author review; a plain-runtime waiter is not presented as that App proof.
