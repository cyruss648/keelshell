# ADR 0063: Explicit finite scheduled workflow authorization

- Status: implementation candidate; independent review and acceptance remain separate gates
- Date: 2026-10-06

## Decision

Extend the existing human-reviewed dependency workflow with an explicit in-memory
choice of immediate execution, one occurrence or a finite absolute interval
sequence. Immediate execution remains the default. The schedule authorizes the
same complete dependency plan, exact rendered commands, execution options and
captured authenticated sessions for every reviewed occurrence. It grants no
reconnection, replacement-session, retry or model-directed execution authority.

`keelshell-core` owns the immutable `WorkflowScheduleSpec` and the pure
`WorkflowScheduleLedger`. The spec stores a non-nil schedule identity, workflow
identity, revision, existing dependency-plan review token, UTC occurrence times,
fixed display offset, bounded grace, optional interval and count. It stores no
command or credential. The ledger reads no clocks, performs no I/O and persists
nothing; its caller supplies paired wall and monotonic samples. Existing direct
registry dependencies suffice; this domain module adds no dependency or profile
schema field.

Each occurrence has an inclusive admission window from its absolute UTC trigger
through trigger plus grace. Grace is 1–60 seconds. Interval is at least 60 seconds
and count is 1–32. A missing interval permits only one occurrence. The last trigger
is `first + (count - 1) * interval`, and the final expiry includes its grace.
The first-trigger-to-final-expiry span cannot exceed seven days. Arming also
requires a first trigger at or after the actual authorization sample and a final
expiry no more than seven days after that sample. All arithmetic is bounded and
checked before admission.

Every raw field is limited to 32 bytes before copying it into the immutable review, including inactive fields. Numeric fields accept ASCII digits only, without signs, whitespace, Unicode digits or expressions.

The editor initializes an explicit UTC `+00:00` offset and a first time five minutes ahead, avoiding synchronous OS time-zone file loading on the UI thread. Users can edit both fields before review.

Input is strict ASCII `YYYY-MM-DD HH:MM:SS` with an explicit fixed `+HH:MM` or
`-HH:MM` offset within `-14:00..=+14:00`; fourteen hours requires zero minutes.
Gregorian date validity and representable UTC/local years 1–9999 are checked.
Unicode digits, controls, surrounding whitespace, implicit machine time zones
and leap seconds are rejected. The fixed offset is an input/display convention;
absolute scheduling uses UTC and never follows a later DST or machine-zone
change. The review shows the complete finite sequence and final expiry as UTC calendar times, epoch seconds
and calendar times at the acknowledged fixed offset.

The domain ledger and its opaque due token are not cloneable or serializable.
A schedule identity must not be reused for a separately reconstructed grant.
`tick` observes the supplied sample and reserves at most one occurrence by
moving it from `Pending` to `Due`. Repeating a tick cannot re-offer that slot.
`claim` verifies the token against the immutable schedule and checks a fresh
clock sample before moving it to `Running`; a token can be claimed once and
finished once. Offering or claiming is local admission, not evidence of remote
execution. Slots that pass their inclusive window become `Missed`; an offered
token can also expire before claim. Offered or running work causes another
occurrence encountered within its window to become `SkippedBusy`. Neither
missed nor busy slots are backfilled after the previous run finishes.

The arming sample anchors the difference between absolute wall-clock elapsed
time and elapsed time from one unchanged process-local monotonic origin. Wall
clock or monotonic movement backward relative to the preceding accepted sample
permanently invalidates future admission. A cumulative elapsed-time disagreement
over 2000 ms also invalidates it; the tolerance does not reset on every tick.
Matching forward elapsed time advances absolute windows and marks passed slots
missed. Suspend/resume or clock changes that violate these invariants require
new explicit authorization. No occurrence is rescheduled relative to completion.

`keelshell-app` owns raw schedule input entities, the complete immutable review,
the process-local monotonic origin, due/running tokens and the asynchronous
timer. Raw mode and all schedule input strings participate in the final review
comparison, including numerically equivalent programmatic changes without input
events. The existing dependency review also binds task source, rendered plan,
labels, targets, prerequisites and execution options. One explicit confirmation
arms the listed finite sequence; the timer cannot edit it or expand its scope.

The workspace remains the final dispatch authority. Before each occurrence it
checks current panel identity, the complete pending review, current destination
metadata and `same_connection` against the originally captured authenticated
`SshSession`. A fresh connection to the same hostname or endpoint does not inherit
an old review. Only an offered timer token can permit a hidden retained panel to
request dispatch. The panel then claims that token under a new clock sample
before passing the acknowledged plan and captured sessions to the existing
`keelshell-session` adapter. The timer performs no blocking I/O on the UI thread.

Full aggregate receipts must match the plan fingerprint, actual execution
options and ordered task/target identities. Only complete confirmed zero exits
for every task produce `Succeeded`. Confirmed failure/rejection produces
`Failed`; incomplete or uncertain evidence produces `Unknown`; cancellation
produces `Cancelled`. Any non-success stops the remaining sequence. A single
run's continue-independent-branches policy does not authorize a later repeat
after failure. If transport startup rejects a claimed occurrence, the panel must
record its non-successful result and stop future admission; local claim is never
presented as proof that a worker or remote command started.

Cancellation withdraws unclaimed occurrences and requests cancellation of local
transport waits for already admitted work. Binding loss invalidates future
occurrences and stops further task admission through the existing cancellation
path. Clock invalidation likewise removes future authorization. A running token
may still finish after cancellation or invalidation to preserve its observed
outcome, but finish cannot reactivate the schedule. Terminal authorization needs
a new workflow and complete review. Cancellation or dropping local owners does
not prove remote termination or undo completed work.

Hiding the panel retains its owned schedule and run. Reopening uses the same
panel and captured bindings. New work cannot replace an active schedule or owned
run. The latest run's bounded task output replaces prior run output; the at-most
32 slot summaries remain in memory. Closing the application ends ownership and
future triggering. Schedule state, tokens, commands, credentials and outputs do
not enter connection metadata, command history or ordinary target-based batch
audit. Durable task audit and scheduling need a separate storage and recovery
design with their own authorization rules.

## Acceptance boundaries

The intended engineering checks cover calendar/offset validation, finite bounds,
overflow, inclusive grace boundaries, duplicate ticks and claims, forward/backward
clock movement, cumulative drift, overlap, cancellation, binding mismatch,
non-success outcomes and terminal non-revival. GPUI behavior checks must also
cover full review, hidden dispatch, programmatic changes, replacement connections,
late claim, startup failure, receipt collection and native-thread-safe lifecycle.

Pure domain tests, compilation, renderer fixtures and controlled SSH responses
provide evidence only for their exercised contracts. Isolated real loopback
programs and target-native desktop operations have distinct acceptance scopes.
This ADR records no passed run, production execution, native desktop result or
cross-platform acceptance. New independent review, exact-source checks and
appropriate native evidence remain required.

Persistent schedules, background daemon/cron execution, restart recovery,
unbounded recurrence, reconnect/retry and durable task-level audit are outside
this slice. See the [product contract](../product/SCHEDULED_WORKFLOWS.md),
[manual workflow UI decision](0044-reviewed-dependency-workflow-ui.md) and
[captured-session adapter decision](0035-reviewed-workflow-ssh-adapter.md).


## Composition with transient per-target parameters

The later composition keeps the already implemented per-target `TextareaState`
entities and literal values attached to their captured authenticated destination.
Fields are synchronized explicitly before review, and the finite sequence binds
the complete parameter-expanded plan together with reserved target metadata.
Every occurrence recomputes the complete review from current input state before
claiming its due token. An input event is not the authority boundary: a silent
`set_value` must also fail equality. A failed due-time check permanently stops the
sequence; restoring original values cannot resurrect the consumed grant.

Successful receipt handling retains the fields and only the latest bounded
output. The output view must remain available while a repeated plan waits
between occurrences, even though no transport handle exists at that moment.
The next actual claim replaces the previous output. This adds no storage schema,
history/audit record, reconnect permission or MCP client authority. Combination
tests and root integration are separate from the older independent schedule review
and from desktop-native acceptance.


## 同步组合中的等待审核撤销

保存的同步元数据不替换活动认证连接，因此不无条件撤销原计划。等待阶段的真实
连接/捕获路线变化仍必须撤销未执行审核；在改变账本终态前捕获等待状态，避免
先置complete后漏清旧审核卡片。250ms等待轮询使用同一有界完整快照复核实际值，
覆盖没有Change事件的编辑；此复核不做I/O或建立权限，原最终会话审核继续保留。
已完成和在途回执按原边界保留。新的组合作者门禁与尚待非作者/原生的范围见
[组合记录](../testing/records/2026-10-06-sync-schedule-combination.md)。
