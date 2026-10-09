# ADR0078: Process-local file fixture setup admission

Status: v5 candidate; fresh independent review and exact integrated CI remain open.

The production registry conservatively treats an existing local writer as owning
the whole process-local side against possible aliases. Independent UUID temporary
trees do not isolate this application resource, even with separate SSH peer ports.

Controlled file tests acquire a shared permit in synchronous fixture setup before
mount/actions, alongside the existing peer startup/connect. A test-only GPUI App
Global stores only Weak<FixtureGroup>. Fixtures and secondary panels in the same
actual App inherit one group, including original layout loops creating different
runtimes. A different App must acquire separately. An alive binding cannot be
replaced by a different group. The weak binding retains neither App nor group and
cannot bypass semaphore admission after the last real owner ends.

The actual FilesPanel entity and foreground/queued transport closures hold strong
group references. Merely dropping Harness does not release admission while its
window is retained. GPUI payload destruction occurs in a normal App update; a
Weak<Entity> that cannot upgrade is not alone proof that its payload was dropped.
The single MCP transfer-isolation case opts in before its original actions. Its
original local-read queue.close() and SFTP close remain intact; this does not claim
that every MCP background resource automatically retains a fixture group.

Each real UI transfer queue is retained once with its original runtime, in a
one-way group-to-queue/runtime relation. Queue schedulers do not retain the group.
When the last entity/transport owner ends, cleanup moves the original permit and
queues to an owned thread off GPUI. Extra queue references must disappear before
cleanup can consume each queue and await the existing production close(), which
joins its actual scheduler and in-flight tasks. Transport terminal observation,
Server::Drop abort and a dropped queue handle do not substitute for this join.
Only successful joins release the permit. Failed spawn/captured-closure drop,
panic, remaining references, timeout and unknown join keep admission closed; the
cleanup object's Drop forgets any permit not explicitly released after success.
Finished cleanup threads are reaped without joining a live thread on GPUI.

Setup uses a real Tokio-clock 120-second admission timeout. Cleanup has a real
120-second budget for final references/queue joins; the owned process runner has
a 300-second watchdog covering runtime teardown too. These test-only setup and
teardown waits precede original actions and do not enter GPUI parking or change
its thread guard/clock. Production ownership, global four-thread test parallelism,
all original byte assertions and 12-second idle / 10-second READ holds remain.

The deterministic shared-domain test holds an existing local continuation, proves
another peer's sync rejects with no mutation, and requires a new plan/approval for
exact bytes after release. New controls preserve the non-author retained-window
witness and prove independent real-runtime admission remains Pending while the
window or a removed window's actual CLOSE-blocked worker lives. Another real SSH
queue proves an extra Arc cannot release admission before close joins. An actual
standard-library pre-spawn rejection proves captured cleanup drops fail closed.
These controlled facts do not identify the historical CI's competing lease owner.

Rejected designs remain preserved: asynchronous v3 changed GPUI's permanent thread
exemption/clock despite 91 passes; synchronous v4 passed 91 but let a retained
window outlive its Harness guard. Initial v5 runtime-weak grouping also had a
static same-App loop self-deadlock and was replaced before the related suite.
All failed compilations/probes and their exact source epochs remain. The current
v5 six controls and 95 related four-thread cases passed in owned author runs;
strict checks and new non-author acceptance are separate gates.
