//! Bounded wall-clock triggers for an explicitly reviewed workflow.
//!
//! This module stores only a schedule and its transient admission ledger. It
//! performs no I/O, reads no clock, persists no commands or credentials, and
//! never executes a workflow. A caller must review the complete schedule and
//! session bindings before constructing a ledger, then revalidate those bindings
//! before claiming a due token. Cancellation or invalidation requires a new
//! review and a new schedule identity; an old ledger cannot be restarted.

use std::time::Duration;

use uuid::Uuid;

use crate::BatchWorkflowReviewToken;

/// Maximum number of occurrences in one explicit schedule authorization.
pub const MAX_WORKFLOW_SCHEDULE_COUNT: u32 = 32;
/// Minimum interval between the absolute UTC occurrence times.
pub const MIN_WORKFLOW_SCHEDULE_INTERVAL_SECONDS: u64 = 60;
/// Maximum lateness accepted for an occurrence, including its final boundary.
pub const MAX_WORKFLOW_SCHEDULE_GRACE_SECONDS: u32 = 60;
/// Maximum schedule span and authorization-to-final-window horizon.
pub const MAX_WORKFLOW_SCHEDULE_SPAN_SECONDS: u64 = 7 * 24 * 60 * 60;
/// Maximum absolute wall-clock versus monotonic drift from the arming sample.
pub const WORKFLOW_SCHEDULE_CLOCK_DRIFT_MILLIS: u64 = 2_000;

const MIN_UTC_SECONDS: i64 = -62_135_596_800;
const MAX_UTC_SECONDS: i64 = 253_402_300_799;
const MAX_FIXED_OFFSET_MINUTES: i16 = 14 * 60;

/// Exact reviewed workflow identity to which schedule occurrences are bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowScheduleBinding {
    workflow_id: Uuid,
    revision: u64,
    review_token: BatchWorkflowReviewToken,
}

impl WorkflowScheduleBinding {
    /// Bind a non-nil workflow identity and its revision to an immutable plan.
    ///
    /// The caller also binds authenticated sessions in its complete review;
    /// this transport-free type cannot verify or grant session authorization.
    pub fn new(
        workflow_id: Uuid,
        revision: u64,
        review_token: BatchWorkflowReviewToken,
    ) -> Result<Self, WorkflowScheduleError> {
        if workflow_id.is_nil() {
            return Err(WorkflowScheduleError::NilIdentity);
        }
        Ok(Self {
            workflow_id,
            revision,
            review_token,
        })
    }

    /// Stable identity supplied by the workflow's review owner.
    pub const fn workflow_id(&self) -> Uuid {
        self.workflow_id
    }

    /// Workflow revision acknowledged in the complete review.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Fingerprint of the exact immutable dependency plan reviewed by the user.
    pub const fn review_token(&self) -> BatchWorkflowReviewToken {
        self.review_token
    }
}

/// Immutable finite trigger specification with absolute UTC occurrence times.
///
/// Fixed offsets are only a display/input convention. They never follow DST or
/// a later change to the machine's time zone. All intervals start from the
/// original first UTC time, rather than the previous completion time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowScheduleSpec {
    schedule_id: Uuid,
    binding: WorkflowScheduleBinding,
    first_utc_seconds: i64,
    fixed_offset_minutes: i16,
    grace_seconds: u32,
    interval_seconds: Option<u64>,
    count: u32,
    last_utc_seconds: i64,
    final_window_end_utc_seconds: i64,
}

impl WorkflowScheduleSpec {
    /// Validate a single occurrence or a finite sequence without any effects.
    ///
    /// `interval_seconds = None` requires exactly one occurrence. A present
    /// interval is at least 60 seconds, the count is 1 through 32, and the span
    /// from the first occurrence through the last grace boundary is at most
    /// seven days. Times must be representable in Gregorian years 1 through
    /// 9999 in UTC and in the chosen fixed offset.
    pub fn new(
        schedule_id: Uuid,
        binding: WorkflowScheduleBinding,
        first_utc_seconds: i64,
        fixed_offset_minutes: i16,
        grace_seconds: u32,
        interval_seconds: Option<u64>,
        count: u32,
    ) -> Result<Self, WorkflowScheduleError> {
        if schedule_id.is_nil() {
            return Err(WorkflowScheduleError::NilIdentity);
        }
        validate_offset(fixed_offset_minutes)?;
        validate_utc(first_utc_seconds)?;
        if !(1..=MAX_WORKFLOW_SCHEDULE_COUNT).contains(&count) {
            return Err(WorkflowScheduleError::InvalidCount);
        }
        if !(1..=MAX_WORKFLOW_SCHEDULE_GRACE_SECONDS).contains(&grace_seconds) {
            return Err(WorkflowScheduleError::InvalidGrace);
        }
        let interval = match interval_seconds {
            Some(interval) if interval >= MIN_WORKFLOW_SCHEDULE_INTERVAL_SECONDS => interval,
            Some(_) => return Err(WorkflowScheduleError::InvalidInterval),
            None if count == 1 => 0,
            None => return Err(WorkflowScheduleError::InvalidInterval),
        };
        let last_delta = interval
            .checked_mul(u64::from(count - 1))
            .ok_or(WorkflowScheduleError::TimeOverflow)?;
        let span = last_delta
            .checked_add(u64::from(grace_seconds))
            .ok_or(WorkflowScheduleError::TimeOverflow)?;
        if span > MAX_WORKFLOW_SCHEDULE_SPAN_SECONDS {
            return Err(WorkflowScheduleError::SpanLimit);
        }
        let last_utc_seconds = first_utc_seconds
            .checked_add(
                i64::try_from(last_delta).map_err(|_| WorkflowScheduleError::TimeOverflow)?,
            )
            .ok_or(WorkflowScheduleError::TimeOverflow)?;
        let final_window_end_utc_seconds = last_utc_seconds
            .checked_add(i64::from(grace_seconds))
            .ok_or(WorkflowScheduleError::TimeOverflow)?;
        validate_utc(final_window_end_utc_seconds)?;
        format_fixed_offset_datetime(first_utc_seconds, fixed_offset_minutes)?;
        format_fixed_offset_datetime(final_window_end_utc_seconds, fixed_offset_minutes)?;
        Ok(Self {
            schedule_id,
            binding,
            first_utc_seconds,
            fixed_offset_minutes,
            grace_seconds,
            interval_seconds,
            count,
            last_utc_seconds,
            final_window_end_utc_seconds,
        })
    }

    /// Build one occurrence with an explicitly bounded lateness window.
    pub fn once(
        schedule_id: Uuid,
        binding: WorkflowScheduleBinding,
        first_utc_seconds: i64,
        fixed_offset_minutes: i16,
        grace_seconds: u32,
    ) -> Result<Self, WorkflowScheduleError> {
        Self::new(
            schedule_id,
            binding,
            first_utc_seconds,
            fixed_offset_minutes,
            grace_seconds,
            None,
            1,
        )
    }

    /// Build a finite sequence at absolute intervals from the first UTC time.
    pub fn interval(
        schedule_id: Uuid,
        binding: WorkflowScheduleBinding,
        first_utc_seconds: i64,
        fixed_offset_minutes: i16,
        grace_seconds: u32,
        interval_seconds: u64,
        count: u32,
    ) -> Result<Self, WorkflowScheduleError> {
        Self::new(
            schedule_id,
            binding,
            first_utc_seconds,
            fixed_offset_minutes,
            grace_seconds,
            Some(interval_seconds),
            count,
        )
    }

    /// Unique identity for this complete schedule authorization.
    pub const fn schedule_id(&self) -> Uuid {
        self.schedule_id
    }

    /// Exact workflow revision and immutable plan fingerprint being scheduled.
    pub const fn binding(&self) -> WorkflowScheduleBinding {
        self.binding
    }

    /// First absolute UTC occurrence in seconds since the Unix epoch.
    pub const fn first_utc_seconds(&self) -> i64 {
        self.first_utc_seconds
    }

    /// Fixed display/input offset east of UTC, in minutes.
    pub const fn fixed_offset_minutes(&self) -> i16 {
        self.fixed_offset_minutes
    }

    /// Inclusive maximum lateness of every occurrence in seconds.
    pub const fn grace_seconds(&self) -> u32 {
        self.grace_seconds
    }

    /// Absolute interval in seconds, or `None` for a single occurrence.
    pub const fn interval_seconds(&self) -> Option<u64> {
        self.interval_seconds
    }

    /// Number of explicitly authorized occurrences.
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Last absolute UTC occurrence in seconds since the Unix epoch.
    pub const fn last_utc_seconds(&self) -> i64 {
        self.last_utc_seconds
    }

    /// Inclusive end of the last occurrence's lateness window.
    pub const fn final_window_end_utc_seconds(&self) -> i64 {
        self.final_window_end_utc_seconds
    }

    /// Return the UTC time of a zero-based occurrence, or `None` out of range.
    pub fn scheduled_utc_seconds(&self, index: u32) -> Option<i64> {
        if index >= self.count {
            return None;
        }
        let delta = self
            .interval_seconds
            .unwrap_or(0)
            .checked_mul(u64::from(index))?;
        self.first_utc_seconds
            .checked_add(i64::try_from(delta).ok()?)
    }
}

/// Caller-observed pair of clocks, sampled together before each admission.
///
/// `monotonic` is the elapsed value from one stable process-local monotonic
/// origin. The same origin must be used at arming, ticking and claiming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleClockSample {
    /// Absolute UTC milliseconds since the Unix epoch.
    pub utc_millis: i64,
    /// Elapsed monotonic time from the caller's unchanged origin.
    pub monotonic: Duration,
}

/// Reason that permanently prevents any further admission in this ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowScheduleInvalidationReason {
    /// The wall clock moved backward after the preceding accepted sample.
    WallClockMovedBackward,
    /// The caller's monotonic clock moved backward or changed its origin.
    MonotonicMovedBackward,
    /// Cumulative wall/monotonic disagreement exceeds the fixed two-second bound.
    ClockDrift,
    /// The reviewed plan or revision no longer matches the active workflow.
    AuthorizationChanged,
    /// An authenticated session binding changed or ceased to be usable.
    SessionBindingChanged,
    /// A preceding occurrence failed or its complete success was unconfirmed.
    PriorRunNotSucceeded,
}

/// Observed whole-workflow result reported by the execution adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowScheduleOutcome {
    /// Every required task completed with confirmed success.
    Succeeded,
    /// An observed task failure or rejection prevented complete success.
    Failed,
    /// The adapter could not confirm the complete remote outcome.
    Unknown,
    /// The user or adapter cancelled the running workflow.
    Cancelled,
}

/// State of one absolute occurrence in the bounded admission ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowScheduleSlotStatus {
    /// Future occurrence that has never produced an admission token.
    Pending,
    /// One token was offered and still requires adapter revalidation and claim.
    Due,
    /// Its token was claimed; no other occurrence can overlap this run.
    Running,
    /// The adapter reported an observed terminal result.
    Finished(WorkflowScheduleOutcome),
    /// The inclusive grace window ended before admission; it will not be retried.
    Missed,
    /// An offered or running occurrence occupied this occurrence's due window.
    SkippedBusy,
    /// Cancellation permanently withdrew this unclaimed occurrence.
    Cancelled,
    /// Invalidation permanently withdrew this unclaimed occurrence.
    Invalidated(WorkflowScheduleInvalidationReason),
}

/// Read-only time and observed admission state of one absolute occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowScheduleSlot {
    index: u32,
    scheduled_utc_seconds: i64,
    grace_end_utc_seconds: i64,
    status: WorkflowScheduleSlotStatus,
}

impl WorkflowScheduleSlot {
    /// Zero-based index in the explicitly reviewed finite schedule.
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// Absolute UTC occurrence time in Unix seconds.
    pub const fn scheduled_utc_seconds(&self) -> i64 {
        self.scheduled_utc_seconds
    }

    /// Inclusive latest permitted claim time in UTC Unix seconds.
    pub const fn grace_end_utc_seconds(&self) -> i64 {
        self.grace_end_utc_seconds
    }

    /// Current observed admission state, without granting a transition.
    pub const fn status(&self) -> WorkflowScheduleSlotStatus {
        self.status
    }
}

/// Overall lifetime of a schedule authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowScheduleStatus {
    /// Future, offered or running occurrences remain.
    Active,
    /// Every occurrence has a terminal result; a new review is required to repeat.
    Complete,
    /// The user cancelled future admission; an already running result may finish.
    Cancelled,
    /// A permanent clock or binding failure stopped future admission.
    Invalidated(WorkflowScheduleInvalidationReason),
}

/// Opaque one-use admission identity offered by [`WorkflowScheduleLedger::tick`].
///
/// A token is neither cloneable nor serializable. Keep the same value through
/// adapter revalidation, claim and observed completion. Offering or claiming a
/// token performs no remote action. Its schedule identity must never be reused
/// for a separately reconstructed authorization.
#[derive(Debug, PartialEq, Eq)]
pub struct WorkflowScheduleDueToken {
    schedule_id: Uuid,
    binding: WorkflowScheduleBinding,
    slot_index: u32,
    scheduled_utc_seconds: i64,
    grace_end_utc_seconds: i64,
}

impl WorkflowScheduleDueToken {
    /// Schedule authorization that produced this token.
    pub const fn schedule_id(&self) -> Uuid {
        self.schedule_id
    }

    /// Reviewed workflow identity, revision and plan fingerprint to revalidate.
    pub const fn binding(&self) -> WorkflowScheduleBinding {
        self.binding
    }

    /// Zero-based occurrence index, never reused by this ledger.
    pub const fn slot_index(&self) -> u32 {
        self.slot_index
    }

    /// Authorized absolute UTC occurrence time in Unix seconds.
    pub const fn scheduled_utc_seconds(&self) -> i64 {
        self.scheduled_utc_seconds
    }

    /// Inclusive latest permitted claim time in UTC Unix seconds.
    pub const fn grace_end_utc_seconds(&self) -> i64 {
        self.grace_end_utc_seconds
    }
}

/// Validation and transition failures with no command or credential diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowScheduleError {
    /// A workflow or schedule identity is nil.
    #[error("schedule and workflow identities must be non-nil")]
    NilIdentity,
    /// The count is outside 1 through 32.
    #[error("schedule requires 1 through 32 occurrences")]
    InvalidCount,
    /// The grace window is outside 1 through 60 seconds.
    #[error("schedule grace requires 1 through 60 seconds")]
    InvalidGrace,
    /// A repeated schedule has no interval, or its interval is below 60 seconds.
    #[error("schedule interval must be at least 60 seconds")]
    InvalidInterval,
    /// A finite schedule or arming horizon would exceed seven days.
    #[error("schedule span and final authorization horizon are limited to seven days")]
    SpanLimit,
    /// The first occurrence is earlier than the explicit arming sample.
    #[error("schedule must start at or after its authorization time")]
    StartInPast,
    /// Time arithmetic cannot be represented safely.
    #[error("schedule time arithmetic overflow")]
    TimeOverflow,
    /// Fixed offset syntax or magnitude is invalid.
    #[error("fixed UTC offset requires ASCII +/-HH:MM within +/-14:00")]
    InvalidFixedOffset,
    /// Calendar syntax, date, time or representable year is invalid.
    #[error("date time requires a valid ASCII YYYY-MM-DD HH:MM:SS in years 1 through 9999")]
    InvalidDateTime,
    /// A clock anomaly permanently invalidated this ledger.
    #[error("schedule clock invalidated: {reason:?}")]
    ClockInvalidated {
        /// Clock invariant that was violated.
        reason: WorkflowScheduleInvalidationReason,
    },
    /// Admission is unavailable after cancellation, completion or invalidation.
    #[error("schedule authorization is no longer active")]
    NoLongerActive,
    /// An admission token belongs to a different schedule or reviewed plan.
    #[error("schedule admission token does not match this authorization")]
    TokenMismatch,
    /// An occurrence was not in the state required by a one-use operation.
    #[error("schedule occurrence cannot make this state transition")]
    InvalidTransition,
    /// Claim was attempted after the inclusive lateness boundary.
    #[error("schedule admission token expired without a claim")]
    TokenExpired,
}

/// Pure finite admission ledger for one explicitly reviewed schedule instance.
///
/// This type is not cloneable or serializable. Keep one ledger for the lifetime
/// of its reviewed authorization. Each `tick` offers at most one token and
/// reserves that occurrence immediately. A due or running occurrence prevents
/// overlaps: other occurrences encountered within their windows become
/// `SkippedBusy`. Passed windows become `Missed`; neither state is retried.
#[derive(Debug)]
pub struct WorkflowScheduleLedger {
    spec: WorkflowScheduleSpec,
    slots: Vec<WorkflowScheduleSlot>,
    status: WorkflowScheduleStatus,
    anchor: ScheduleClockSample,
    previous: ScheduleClockSample,
}

impl WorkflowScheduleLedger {
    /// Arm a fully reviewed immutable schedule at the supplied clock sample.
    ///
    /// Invoke only after explicit user confirmation of this complete schedule,
    /// commands, targets, plan and authenticated bindings. The first occurrence
    /// cannot be in the past; its final grace boundary cannot be more than seven
    /// days after this arming sample. No occurrence is offered by construction.
    pub fn new(
        spec: WorkflowScheduleSpec,
        initial_sample: ScheduleClockSample,
    ) -> Result<Self, WorkflowScheduleError> {
        let first_millis = seconds_to_millis(spec.first_utc_seconds)?;
        let final_millis = seconds_to_millis(spec.final_window_end_utc_seconds)?;
        if first_millis < initial_sample.utc_millis {
            return Err(WorkflowScheduleError::StartInPast);
        }
        let horizon = i128::from(final_millis) - i128::from(initial_sample.utc_millis);
        if horizon > i128::from(MAX_WORKFLOW_SCHEDULE_SPAN_SECONDS) * 1_000 {
            return Err(WorkflowScheduleError::SpanLimit);
        }
        let slots = (0..spec.count)
            .map(|index| {
                let scheduled_utc_seconds = spec
                    .scheduled_utc_seconds(index)
                    .ok_or(WorkflowScheduleError::TimeOverflow)?;
                Ok(WorkflowScheduleSlot {
                    index,
                    scheduled_utc_seconds,
                    grace_end_utc_seconds: scheduled_utc_seconds
                        .checked_add(i64::from(spec.grace_seconds))
                        .ok_or(WorkflowScheduleError::TimeOverflow)?,
                    status: WorkflowScheduleSlotStatus::Pending,
                })
            })
            .collect::<Result<Vec<_>, WorkflowScheduleError>>()?;
        Ok(Self {
            spec,
            slots,
            status: WorkflowScheduleStatus::Active,
            anchor: initial_sample,
            previous: initial_sample,
        })
    }

    /// Borrow the exact immutable schedule acknowledged for this ledger.
    pub const fn spec(&self) -> &WorkflowScheduleSpec {
        &self.spec
    }

    /// Read the overall authorization state.
    pub const fn status(&self) -> WorkflowScheduleStatus {
        self.status
    }

    /// Read every occurrence state in ascending absolute UTC order.
    pub fn slots(&self) -> &[WorkflowScheduleSlot] {
        &self.slots
    }

    /// Return whether this authorization permanently stopped offering triggers.
    ///
    /// A cancelled or invalidated ledger can still contain an already running
    /// occurrence whose observed result may be recorded by `finish`.
    pub const fn is_terminal(&self) -> bool {
        !matches!(self.status, WorkflowScheduleStatus::Active)
    }

    /// Return whether an occurrence has been claimed and has not yet finished.
    pub fn is_running(&self) -> bool {
        self.slots
            .iter()
            .any(|slot| slot.status == WorkflowScheduleSlotStatus::Running)
    }

    /// Observe clocks and offer at most one due token without executing anything.
    ///
    /// Exactly the grace boundary is permitted; one millisecond later is missed.
    /// A forward jump with matching monotonic elapsed time skips passed windows.
    /// Backward clock movement or cumulative drift permanently invalidates all
    /// unclaimed occurrences. Repeating an identical sample cannot re-offer a
    /// token, and no late or busy occurrence is ever backfilled.
    pub fn tick(
        &mut self,
        sample: ScheduleClockSample,
    ) -> Result<Option<WorkflowScheduleDueToken>, WorkflowScheduleError> {
        if self.status != WorkflowScheduleStatus::Active {
            return Ok(None);
        }
        self.observe_clock(sample)?;
        let mut offered = None;
        for index in 0..self.slots.len() {
            let scheduled_utc_seconds = self.slots[index].scheduled_utc_seconds;
            let grace_end_utc_seconds = self.slots[index].grace_end_utc_seconds;
            let end_millis = seconds_to_millis(grace_end_utc_seconds)?;
            match self.slots[index].status {
                WorkflowScheduleSlotStatus::Due if sample.utc_millis > end_millis => {
                    self.slots[index].status = WorkflowScheduleSlotStatus::Missed;
                }
                WorkflowScheduleSlotStatus::Pending => {
                    if sample.utc_millis > end_millis {
                        self.slots[index].status = WorkflowScheduleSlotStatus::Missed;
                    } else if sample.utc_millis >= seconds_to_millis(scheduled_utc_seconds)? {
                        if self.has_reserved_occurrence() {
                            self.slots[index].status = WorkflowScheduleSlotStatus::SkippedBusy;
                        } else {
                            self.slots[index].status = WorkflowScheduleSlotStatus::Due;
                            offered = Some(WorkflowScheduleDueToken {
                                schedule_id: self.spec.schedule_id,
                                binding: self.spec.binding,
                                slot_index: index as u32,
                                scheduled_utc_seconds,
                                grace_end_utc_seconds,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        self.complete_if_exhausted();
        Ok(offered)
    }

    /// Claim exactly one offered token after adapter revalidation, before I/O.
    ///
    /// The caller must revalidate the token's workflow revision, plan fingerprint
    /// and exact authenticated session bindings immediately before this call.
    /// A successful claim marks the occurrence running; it grants no transport
    /// capability by itself. Repeated claims and expired tokens are rejected.
    pub fn claim(
        &mut self,
        token: &WorkflowScheduleDueToken,
        sample: ScheduleClockSample,
    ) -> Result<(), WorkflowScheduleError> {
        let index = self.token_index(token)?;
        if self.status != WorkflowScheduleStatus::Active {
            return Err(WorkflowScheduleError::NoLongerActive);
        }
        self.observe_clock(sample)?;
        if self.slots[index].status != WorkflowScheduleSlotStatus::Due {
            return Err(WorkflowScheduleError::InvalidTransition);
        }
        if sample.utc_millis > seconds_to_millis(token.grace_end_utc_seconds)? {
            self.slots[index].status = WorkflowScheduleSlotStatus::Missed;
            self.complete_if_exhausted();
            return Err(WorkflowScheduleError::TokenExpired);
        }
        self.slots[index].status = WorkflowScheduleSlotStatus::Running;
        Ok(())
    }

    /// Record one observed completion for a previously claimed token.
    ///
    /// Any outcome other than confirmed success permanently stops all subsequent
    /// occurrences: cancellation cancels them; failed or unknown results
    /// invalidate them as `PriorRunNotSucceeded`. A running occurrence can finish
    /// after cancellation or invalidation so
    /// its actual outcome remains recorded. Completion never reactivates that
    /// authorization, and a second finish is an invalid transition.
    pub fn finish(
        &mut self,
        token: &WorkflowScheduleDueToken,
        outcome: WorkflowScheduleOutcome,
    ) -> Result<(), WorkflowScheduleError> {
        let index = self.token_index(token)?;
        if self.slots[index].status != WorkflowScheduleSlotStatus::Running {
            return Err(WorkflowScheduleError::InvalidTransition);
        }
        self.slots[index].status = WorkflowScheduleSlotStatus::Finished(outcome);
        match outcome {
            WorkflowScheduleOutcome::Succeeded => self.complete_if_exhausted(),
            WorkflowScheduleOutcome::Cancelled => {
                self.cancel();
            }
            WorkflowScheduleOutcome::Failed | WorkflowScheduleOutcome::Unknown => {
                self.invalidate(WorkflowScheduleInvalidationReason::PriorRunNotSucceeded);
            }
        }
        Ok(())
    }

    /// Permanently withdraw unclaimed occurrences; return whether state changed.
    ///
    /// The adapter separately handles cancellation of an already running remote
    /// operation. Its result may still be passed to `finish` after this call.
    pub fn cancel(&mut self) -> bool {
        if self.status != WorkflowScheduleStatus::Active {
            return false;
        }
        self.status = WorkflowScheduleStatus::Cancelled;
        for slot in &mut self.slots {
            if matches!(
                slot.status,
                WorkflowScheduleSlotStatus::Pending | WorkflowScheduleSlotStatus::Due
            ) {
                slot.status = WorkflowScheduleSlotStatus::Cancelled;
            }
        }
        true
    }

    /// Permanently stop unclaimed admission when clocks or reviewed bindings fail.
    ///
    /// Return whether the authorization changed. This does not issue remote
    /// cancellation, erase finished results or replace a previous terminal reason.
    pub fn invalidate(&mut self, reason: WorkflowScheduleInvalidationReason) -> bool {
        if self.status != WorkflowScheduleStatus::Active {
            return false;
        }
        self.status = WorkflowScheduleStatus::Invalidated(reason);
        for slot in &mut self.slots {
            if matches!(
                slot.status,
                WorkflowScheduleSlotStatus::Pending | WorkflowScheduleSlotStatus::Due
            ) {
                slot.status = WorkflowScheduleSlotStatus::Invalidated(reason);
            }
        }
        true
    }

    fn token_index(
        &self,
        token: &WorkflowScheduleDueToken,
    ) -> Result<usize, WorkflowScheduleError> {
        let index = token.slot_index as usize;
        if token.schedule_id != self.spec.schedule_id
            || token.binding != self.spec.binding
            || self.spec.scheduled_utc_seconds(token.slot_index)
                != Some(token.scheduled_utc_seconds)
            || token
                .scheduled_utc_seconds
                .checked_add(i64::from(self.spec.grace_seconds))
                != Some(token.grace_end_utc_seconds)
            || index >= self.slots.len()
        {
            return Err(WorkflowScheduleError::TokenMismatch);
        }
        Ok(index)
    }

    fn observe_clock(&mut self, sample: ScheduleClockSample) -> Result<(), WorkflowScheduleError> {
        let reason = if sample.utc_millis < self.previous.utc_millis {
            Some(WorkflowScheduleInvalidationReason::WallClockMovedBackward)
        } else if sample.monotonic < self.previous.monotonic {
            Some(WorkflowScheduleInvalidationReason::MonotonicMovedBackward)
        } else {
            let wall_elapsed = i128::from(sample.utc_millis) - i128::from(self.anchor.utc_millis);
            let monotonic_elapsed = sample
                .monotonic
                .saturating_sub(self.anchor.monotonic)
                .as_millis();
            // Duration's maximum u64 seconds is comfortably representable by i128.
            let monotonic_elapsed = i128::try_from(monotonic_elapsed)
                .map_err(|_| WorkflowScheduleError::TimeOverflow)?;
            ((wall_elapsed - monotonic_elapsed).abs()
                > i128::from(WORKFLOW_SCHEDULE_CLOCK_DRIFT_MILLIS))
            .then_some(WorkflowScheduleInvalidationReason::ClockDrift)
        };
        if let Some(reason) = reason {
            self.invalidate(reason);
            return Err(WorkflowScheduleError::ClockInvalidated { reason });
        }
        self.previous = sample;
        Ok(())
    }

    fn has_reserved_occurrence(&self) -> bool {
        self.slots.iter().any(|slot| {
            matches!(
                slot.status,
                WorkflowScheduleSlotStatus::Due | WorkflowScheduleSlotStatus::Running
            )
        })
    }

    fn complete_if_exhausted(&mut self) {
        if self.status == WorkflowScheduleStatus::Active
            && self.slots.iter().all(|slot| {
                !matches!(
                    slot.status,
                    WorkflowScheduleSlotStatus::Pending
                        | WorkflowScheduleSlotStatus::Due
                        | WorkflowScheduleSlotStatus::Running
                )
            })
        {
            self.status = WorkflowScheduleStatus::Complete;
        }
    }
}

/// Parse an ASCII fixed offset such as `+08:00`, bounded to `-14:00..=+14:00`.
///
/// No whitespace, control characters, named zones, seconds or Unicode digits
/// are accepted. Offsets at fourteen hours require zero minutes.
pub fn parse_fixed_offset(value: &str) -> Result<i16, WorkflowScheduleError> {
    let bytes = value.as_bytes();
    if bytes.len() != 6 || !matches!(bytes[0], b'+' | b'-') || bytes[3] != b':' {
        return Err(WorkflowScheduleError::InvalidFixedOffset);
    }
    let hours = ascii_number(&bytes[1..3]).ok_or(WorkflowScheduleError::InvalidFixedOffset)?;
    let minutes = ascii_number(&bytes[4..6]).ok_or(WorkflowScheduleError::InvalidFixedOffset)?;
    if hours > 14 || minutes > 59 || (hours == 14 && minutes != 0) {
        return Err(WorkflowScheduleError::InvalidFixedOffset);
    }
    let total = (hours * 60 + minutes) as i16;
    Ok(if bytes[0] == b'-' { -total } else { total })
}

/// Format a validated offset as ASCII `+HH:MM` or `-HH:MM`.
pub fn format_fixed_offset(offset_minutes: i16) -> Result<String, WorkflowScheduleError> {
    validate_offset(offset_minutes)?;
    let sign = if offset_minutes < 0 { '-' } else { '+' };
    let absolute = offset_minutes.unsigned_abs();
    Ok(format!("{sign}{:02}:{:02}", absolute / 60, absolute % 60))
}

/// Parse exact ASCII `YYYY-MM-DD HH:MM:SS` at an explicit fixed offset.
///
/// Return UTC seconds since the Unix epoch. Gregorian date validity is checked;
/// leap seconds, controls, surrounding whitespace, Unicode digits and implicit
/// local time zones are rejected. UTC and local years must both be 1 through 9999.
pub fn parse_fixed_offset_datetime(
    value: &str,
    offset_minutes: i16,
) -> Result<i64, WorkflowScheduleError> {
    validate_offset(offset_minutes)?;
    let bytes = value.as_bytes();
    if bytes.len() != 19
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b' '
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return Err(WorkflowScheduleError::InvalidDateTime);
    }
    let number = |range: std::ops::Range<usize>| {
        ascii_number(&bytes[range]).ok_or(WorkflowScheduleError::InvalidDateTime)
    };
    let year = number(0..4)?;
    let month = number(5..7)?;
    let day = number(8..10)?;
    let hour = number(11..13)?;
    let minute = number(14..16)?;
    let second = number(17..19)?;
    if year == 0
        || !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return Err(WorkflowScheduleError::InvalidDateTime);
    }
    let days = days_from_civil(i64::from(year), i64::from(month), i64::from(day));
    let utc_seconds = days * 86_400 + i64::from(hour * 3_600 + minute * 60 + second)
        - i64::from(offset_minutes) * 60;
    validate_utc(utc_seconds)?;
    Ok(utc_seconds)
}

/// Format UTC Unix seconds as the exact local calendar syntax accepted by parsing.
///
/// The offset is explicit and fixed; format it separately with
/// [`format_fixed_offset`] when showing an unambiguous review to the user.
pub fn format_fixed_offset_datetime(
    utc_seconds: i64,
    offset_minutes: i16,
) -> Result<String, WorkflowScheduleError> {
    validate_offset(offset_minutes)?;
    validate_utc(utc_seconds)?;
    let local = utc_seconds
        .checked_add(i64::from(offset_minutes) * 60)
        .ok_or(WorkflowScheduleError::TimeOverflow)?;
    let (year, month, day) = civil_from_days(local.div_euclid(86_400));
    if !(1..=9999).contains(&year) {
        return Err(WorkflowScheduleError::InvalidDateTime);
    }
    let seconds = local.rem_euclid(86_400);
    Ok(format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        seconds / 3_600,
        (seconds / 60) % 60,
        seconds % 60
    ))
}

fn validate_offset(offset_minutes: i16) -> Result<(), WorkflowScheduleError> {
    if !(-MAX_FIXED_OFFSET_MINUTES..=MAX_FIXED_OFFSET_MINUTES).contains(&offset_minutes) {
        return Err(WorkflowScheduleError::InvalidFixedOffset);
    }
    Ok(())
}

fn validate_utc(seconds: i64) -> Result<(), WorkflowScheduleError> {
    if !(MIN_UTC_SECONDS..=MAX_UTC_SECONDS).contains(&seconds) {
        return Err(WorkflowScheduleError::InvalidDateTime);
    }
    Ok(())
}

fn seconds_to_millis(seconds: i64) -> Result<i64, WorkflowScheduleError> {
    seconds
        .checked_mul(1_000)
        .ok_or(WorkflowScheduleError::TimeOverflow)
}

fn ascii_number(bytes: &[u8]) -> Option<u32> {
    bytes.iter().try_fold(0u32, |value, byte| {
        byte.is_ascii_digit()
            .then(|| value * 10 + u32::from(byte - b'0'))
    })
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        _ => 31,
    }
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let adjusted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * adjusted_month + 2) / 5 + 1;
    let month = adjusted_month + if adjusted_month < 10 { 3 } else { -9 };
    (year + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BatchTaskSpec, BatchWorkflowPlan};

    trait FixtureValue<T> {
        fn checked(self, label: &str) -> T;
    }

    impl<T, E: std::fmt::Debug> FixtureValue<T> for Result<T, E> {
        fn checked(self, label: &str) -> T {
            match self {
                Ok(value) => value,
                Err(error) => panic!("{label}: {error:?}"),
            }
        }
    }

    impl<T> FixtureValue<T> for Option<T> {
        fn checked(self, label: &str) -> T {
            match self {
                Some(value) => value,
                None => panic!("{label}: missing fixture value"),
            }
        }
    }

    fn statuses(ledger: &WorkflowScheduleLedger) -> Vec<WorkflowScheduleSlotStatus> {
        ledger
            .slots()
            .iter()
            .map(WorkflowScheduleSlot::status)
            .collect()
    }

    fn binding() -> WorkflowScheduleBinding {
        let plan = BatchWorkflowPlan::new(vec![BatchTaskSpec {
            id: Uuid::from_u128(1),
            target_id: Uuid::from_u128(2),
            command: "printf 'fixture'".to_owned(),
            dependencies: vec![],
        }])
        .checked("valid fixture plan");
        WorkflowScheduleBinding::new(Uuid::from_u128(3), 1, plan.review_token())
            .checked("valid fixture binding")
    }

    fn spec(first: i64, grace: u32, interval: Option<u64>, count: u32) -> WorkflowScheduleSpec {
        WorkflowScheduleSpec::new(
            Uuid::from_u128(4),
            binding(),
            first,
            480,
            grace,
            interval,
            count,
        )
        .checked("valid fixture schedule")
    }

    fn sample(millis: i64) -> ScheduleClockSample {
        ScheduleClockSample {
            utc_millis: millis,
            monotonic: Duration::from_millis(millis as u64),
        }
    }

    fn ledger(first: i64, grace: u32, interval: Option<u64>, count: u32) -> WorkflowScheduleLedger {
        WorkflowScheduleLedger::new(spec(first, grace, interval, count), sample(0))
            .checked("valid fixture arming")
    }

    #[test]
    fn exact_grace_boundary_can_be_claimed_and_one_millisecond_later_is_missed() {
        let mut at_boundary = ledger(10, 1, None, 1);
        let token = at_boundary
            .tick(sample(11_000))
            .checked("tick")
            .checked("due");
        at_boundary
            .claim(&token, sample(11_000))
            .checked("claim boundary");
        assert_eq!(
            statuses(&at_boundary).as_slice(),
            &[WorkflowScheduleSlotStatus::Running]
        );

        let mut after_boundary = ledger(10, 1, None, 1);
        assert_eq!(after_boundary.tick(sample(11_001)).checked("tick"), None);
        assert_eq!(
            statuses(&after_boundary).as_slice(),
            &[WorkflowScheduleSlotStatus::Missed]
        );
        assert_eq!(after_boundary.status(), WorkflowScheduleStatus::Complete);
    }

    #[test]
    fn duplicate_ticks_claims_and_finishes_cannot_repeat_an_occurrence() {
        let mut ledger = ledger(10, 5, None, 1);
        assert_eq!(ledger.tick(sample(9_999)).checked("early tick"), None);
        let token = ledger.tick(sample(10_000)).checked("tick").checked("due");
        assert_eq!(ledger.tick(sample(10_000)).checked("duplicate tick"), None);
        ledger.claim(&token, sample(10_000)).checked("claim");
        assert_eq!(
            ledger.claim(&token, sample(10_000)),
            Err(WorkflowScheduleError::InvalidTransition)
        );
        ledger
            .finish(&token, WorkflowScheduleOutcome::Succeeded)
            .checked("finish");
        assert_eq!(
            ledger.finish(&token, WorkflowScheduleOutcome::Succeeded),
            Err(WorkflowScheduleError::InvalidTransition)
        );
        assert_eq!(ledger.tick(sample(10_000)).checked("complete tick"), None);
    }

    #[test]
    fn matching_forward_jump_marks_passed_occurrences_and_never_backfills() {
        let mut ledger = ledger(10, 5, Some(60), 4);
        let token = ledger
            .tick(sample(130_000))
            .checked("jump")
            .checked("third due");
        assert_eq!(token.slot_index(), 2);
        assert_eq!(
            statuses(&ledger).as_slice(),
            &[
                WorkflowScheduleSlotStatus::Missed,
                WorkflowScheduleSlotStatus::Missed,
                WorkflowScheduleSlotStatus::Due,
                WorkflowScheduleSlotStatus::Pending,
            ]
        );
        ledger.claim(&token, sample(130_000)).checked("claim");
        ledger
            .finish(&token, WorkflowScheduleOutcome::Succeeded)
            .checked("finish");
        assert_eq!(ledger.tick(sample(196_000)).checked("past final"), None);
        assert_eq!(
            ledger.slots()[3].status(),
            WorkflowScheduleSlotStatus::Missed
        );
    }

    #[test]
    fn forward_wall_jump_without_elapsed_monotonic_time_permanently_invalidates() {
        let mut ledger = ledger(10, 5, None, 1);
        let anomalous = ScheduleClockSample {
            utc_millis: 10_000,
            monotonic: Duration::ZERO,
        };
        assert_eq!(
            ledger.tick(anomalous),
            Err(WorkflowScheduleError::ClockInvalidated {
                reason: WorkflowScheduleInvalidationReason::ClockDrift,
            })
        );
        assert_eq!(ledger.tick(sample(10_000)).checked("cannot revive"), None);
        assert_eq!(
            statuses(&ledger).as_slice(),
            &[WorkflowScheduleSlotStatus::Invalidated(
                WorkflowScheduleInvalidationReason::ClockDrift
            )]
        );
    }

    #[test]
    fn backward_wall_clock_invalidates_even_when_both_clocks_move_back() {
        let mut ledger = ledger(10, 5, None, 1);
        ledger.tick(sample(5_000)).checked("first sample");
        assert_eq!(
            ledger.tick(sample(4_999)),
            Err(WorkflowScheduleError::ClockInvalidated {
                reason: WorkflowScheduleInvalidationReason::WallClockMovedBackward,
            })
        );
        assert_eq!(
            ledger.status(),
            WorkflowScheduleStatus::Invalidated(
                WorkflowScheduleInvalidationReason::WallClockMovedBackward
            )
        );
    }

    #[test]
    fn monotonic_origin_change_invalidates_and_cumulative_drift_is_bounded() {
        let mut ledger = ledger(10, 5, None, 1);
        ledger.tick(sample(5_000)).checked("first sample");
        assert_eq!(
            ledger.tick(ScheduleClockSample {
                utc_millis: 5_001,
                monotonic: Duration::from_millis(4_999)
            }),
            Err(WorkflowScheduleError::ClockInvalidated {
                reason: WorkflowScheduleInvalidationReason::MonotonicMovedBackward,
            })
        );
        let mut drift = ledger_with_anchor();
        drift
            .tick(ScheduleClockSample {
                utc_millis: 2_000,
                monotonic: Duration::ZERO,
            })
            .checked("exact drift boundary");
        assert_eq!(
            drift.tick(ScheduleClockSample {
                utc_millis: 2_001,
                monotonic: Duration::ZERO
            }),
            Err(WorkflowScheduleError::ClockInvalidated {
                reason: WorkflowScheduleInvalidationReason::ClockDrift,
            })
        );
    }

    fn ledger_with_anchor() -> WorkflowScheduleLedger {
        ledger(10, 5, None, 1)
    }

    #[test]
    fn overlapping_occurrence_is_skipped_permanently_while_running() {
        let mut ledger = ledger(10, 5, Some(60), 3);
        let token = ledger.tick(sample(10_000)).checked("tick").checked("due");
        ledger.claim(&token, sample(10_000)).checked("claim");
        assert_eq!(ledger.tick(sample(70_000)).checked("overlap"), None);
        assert_eq!(
            ledger.slots()[1].status(),
            WorkflowScheduleSlotStatus::SkippedBusy
        );
        ledger
            .finish(&token, WorkflowScheduleOutcome::Succeeded)
            .checked("finish");
        assert_eq!(ledger.tick(sample(70_001)).checked("no backfill"), None);
        assert!(ledger.tick(sample(130_000)).checked("third tick").is_some());
    }

    #[test]
    fn inclusive_overlapping_windows_offer_only_one_token_per_tick() {
        let mut ledger = ledger(10, 60, Some(60), 2);
        let token = ledger
            .tick(sample(70_000))
            .checked("boundary tick")
            .checked("one due");
        assert_eq!(token.slot_index(), 0);
        assert_eq!(
            statuses(&ledger).as_slice(),
            &[
                WorkflowScheduleSlotStatus::Due,
                WorkflowScheduleSlotStatus::SkippedBusy
            ]
        );
        assert_eq!(ledger.tick(sample(70_000)).checked("duplicate"), None);
    }

    #[test]
    fn expired_offered_token_is_missed_and_never_claimed() {
        let mut ledger = ledger(10, 1, None, 1);
        let token = ledger.tick(sample(10_000)).checked("tick").checked("due");
        assert_eq!(
            ledger.claim(&token, sample(11_001)),
            Err(WorkflowScheduleError::TokenExpired)
        );
        assert_eq!(
            statuses(&ledger).as_slice(),
            &[WorkflowScheduleSlotStatus::Missed]
        );
        assert_eq!(
            ledger.claim(&token, sample(11_001)),
            Err(WorkflowScheduleError::NoLongerActive)
        );
    }

    #[test]
    fn cancellation_withdraws_due_and_future_slots_without_reviving_on_finish() {
        let mut due = ledger(10, 5, Some(60), 2);
        let token = due.tick(sample(10_000)).checked("tick").checked("due");
        assert!(due.cancel());
        assert!(!due.cancel());
        assert_eq!(
            due.claim(&token, sample(10_000)),
            Err(WorkflowScheduleError::NoLongerActive)
        );
        assert_eq!(
            statuses(&due).as_slice(),
            &[WorkflowScheduleSlotStatus::Cancelled; 2]
        );

        let mut running = ledger(10, 5, Some(60), 2);
        let token = running.tick(sample(10_000)).checked("tick").checked("due");
        running.claim(&token, sample(10_000)).checked("claim");
        running.cancel();
        running
            .finish(&token, WorkflowScheduleOutcome::Failed)
            .checked("observed finish");
        assert_eq!(running.status(), WorkflowScheduleStatus::Cancelled);
        assert_eq!(running.tick(sample(70_000)).checked("no revival"), None);
    }

    #[test]
    fn binding_invalidation_preserves_running_result_and_terminal_reason() {
        let mut ledger = ledger(10, 5, Some(60), 2);
        let token = ledger.tick(sample(10_000)).checked("tick").checked("due");
        ledger.claim(&token, sample(10_000)).checked("claim");
        assert!(ledger.invalidate(WorkflowScheduleInvalidationReason::SessionBindingChanged));
        assert!(!ledger.invalidate(WorkflowScheduleInvalidationReason::AuthorizationChanged));
        ledger
            .finish(&token, WorkflowScheduleOutcome::Unknown)
            .checked("finish");
        assert_eq!(
            ledger.status(),
            WorkflowScheduleStatus::Invalidated(
                WorkflowScheduleInvalidationReason::SessionBindingChanged
            )
        );
        assert_eq!(
            ledger.slots()[1].status(),
            WorkflowScheduleSlotStatus::Invalidated(
                WorkflowScheduleInvalidationReason::SessionBindingChanged
            )
        );
    }

    #[test]
    fn token_from_another_schedule_is_rejected_without_mutation() {
        let mut first = ledger(10, 5, None, 1);
        let token = first.tick(sample(10_000)).checked("tick").checked("due");
        let other_spec =
            WorkflowScheduleSpec::once(Uuid::from_u128(5), binding(), 10, 480, 5).checked("spec");
        let mut other = WorkflowScheduleLedger::new(other_spec, sample(0)).checked("ledger");
        assert_eq!(
            other.claim(&token, sample(10_000)),
            Err(WorkflowScheduleError::TokenMismatch)
        );
        assert_eq!(
            statuses(&other).as_slice(),
            &[WorkflowScheduleSlotStatus::Pending]
        );
    }

    #[test]
    fn non_success_results_stop_all_subsequent_occurrences() {
        for outcome in [
            WorkflowScheduleOutcome::Failed,
            WorkflowScheduleOutcome::Unknown,
            WorkflowScheduleOutcome::Cancelled,
        ] {
            let mut ledger = ledger(10, 5, Some(60), 2);
            let token = ledger.tick(sample(10_000)).checked("tick").checked("due");
            ledger.claim(&token, sample(10_000)).checked("claim");
            ledger.finish(&token, outcome).checked("finish");
            assert!(ledger.is_terminal());
            assert_eq!(
                ledger.tick(sample(70_000)).checked("no repeated failure"),
                None
            );
            let expected = if outcome == WorkflowScheduleOutcome::Cancelled {
                WorkflowScheduleSlotStatus::Cancelled
            } else {
                WorkflowScheduleSlotStatus::Invalidated(
                    WorkflowScheduleInvalidationReason::PriorRunNotSucceeded,
                )
            };
            assert_eq!(ledger.slots()[1].status(), expected);
            assert_eq!(
                ledger.slots()[0].status(),
                WorkflowScheduleSlotStatus::Finished(outcome)
            );
        }
    }

    #[test]
    fn revised_binding_rejects_old_token_even_when_schedule_id_is_same() {
        let mut original = ledger(10, 5, None, 1);
        let token = original.tick(sample(10_000)).checked("tick").checked("due");
        let changed_binding =
            WorkflowScheduleBinding::new(binding().workflow_id(), 2, binding().review_token())
                .checked("binding");
        let changed = WorkflowScheduleSpec::once(Uuid::from_u128(4), changed_binding, 10, 480, 5)
            .checked("changed spec");
        let mut changed = WorkflowScheduleLedger::new(changed, sample(0)).checked("ledger");
        assert_eq!(
            changed.claim(&token, sample(10_000)),
            Err(WorkflowScheduleError::TokenMismatch)
        );
        assert_eq!(
            changed.slots()[0].status(),
            WorkflowScheduleSlotStatus::Pending
        );
    }

    #[test]
    fn count_interval_grace_span_and_overflow_validation_are_explicit() {
        let new = |first, grace, interval, count| {
            WorkflowScheduleSpec::new(
                Uuid::from_u128(4),
                binding(),
                first,
                0,
                grace,
                interval,
                count,
            )
        };
        assert_eq!(new(0, 1, None, 0), Err(WorkflowScheduleError::InvalidCount));
        assert_eq!(
            new(0, 1, Some(60), 33),
            Err(WorkflowScheduleError::InvalidCount)
        );
        assert_eq!(new(0, 0, None, 1), Err(WorkflowScheduleError::InvalidGrace));
        assert_eq!(
            new(0, 61, None, 1),
            Err(WorkflowScheduleError::InvalidGrace)
        );
        assert_eq!(
            new(0, 1, Some(59), 2),
            Err(WorkflowScheduleError::InvalidInterval)
        );
        assert_eq!(
            new(0, 1, None, 2),
            Err(WorkflowScheduleError::InvalidInterval)
        );
        assert_eq!(
            new(0, 1, Some(604_800), 2),
            Err(WorkflowScheduleError::SpanLimit)
        );
        assert_eq!(
            new(0, 1, Some(u64::MAX), 3),
            Err(WorkflowScheduleError::TimeOverflow)
        );
        assert_eq!(
            new(i64::MAX, 1, None, 1),
            Err(WorkflowScheduleError::InvalidDateTime)
        );
        assert!(new(0, 1, Some(604_799), 2).is_ok());
        assert_eq!(
            seconds_to_millis(i64::MAX),
            Err(WorkflowScheduleError::TimeOverflow)
        );
    }

    #[test]
    fn arming_rejects_past_and_beyond_seven_day_final_window() {
        assert!(matches!(
            WorkflowScheduleLedger::new(spec(10, 1, None, 1), sample(10_001)),
            Err(WorkflowScheduleError::StartInPast)
        ));
        assert!(matches!(
            WorkflowScheduleLedger::new(spec(604_800, 1, None, 1), sample(0)),
            Err(WorkflowScheduleError::SpanLimit)
        ));
        assert!(WorkflowScheduleLedger::new(spec(604_799, 1, None, 1), sample(0)).is_ok());
    }

    #[test]
    fn fixed_offsets_are_strict_bounded_ascii_and_round_trip() {
        for (input, minutes) in [
            ("+08:00", 480),
            ("-05:30", -330),
            ("+00:00", 0),
            ("+14:00", 840),
            ("-14:00", -840),
        ] {
            assert_eq!(parse_fixed_offset(input), Ok(minutes));
            assert_eq!(format_fixed_offset(minutes), Ok(input.to_owned()));
        }
        for invalid in [
            "", "+14:01", "-14:01", "+15:00", "+01:60", "08:00", "+8:00", " +08:00", "+08:00\n",
            "+０8:00", "+0８:00", "UTC",
        ] {
            assert_eq!(
                parse_fixed_offset(invalid),
                Err(WorkflowScheduleError::InvalidFixedOffset),
                "{invalid:?}"
            );
        }
        assert_eq!(
            format_fixed_offset(i16::MIN),
            Err(WorkflowScheduleError::InvalidFixedOffset)
        );
    }

    #[test]
    fn calendar_parser_validates_leap_days_unicode_controls_and_boundaries() {
        for invalid in [
            "",
            "2026-02-29 00:00:00",
            "1900-02-29 00:00:00",
            "2024-04-31 00:00:00",
            "0000-01-01 00:00:00",
            "2024-13-01 00:00:00",
            "2024-01-00 00:00:00",
            "2024-01-01 24:00:00",
            "2024-01-01 00:60:00",
            "2024-01-01 00:00:60",
            "２０２４-01-01 00:00:00",
            "2024-01-01\n00:00:00",
            "2024-01-01T00:00:00",
        ] {
            assert_eq!(
                parse_fixed_offset_datetime(invalid, 0),
                Err(WorkflowScheduleError::InvalidDateTime),
                "{invalid:?}"
            );
        }
        assert_eq!(
            parse_fixed_offset_datetime("1970-01-01 08:00:00", 480),
            Ok(0)
        );
        assert_eq!(
            parse_fixed_offset_datetime("1969-12-31 18:30:00", -330),
            Ok(0)
        );
        assert_eq!(
            parse_fixed_offset_datetime("0001-01-01 00:00:00", 0),
            Ok(MIN_UTC_SECONDS)
        );
        assert_eq!(
            parse_fixed_offset_datetime("9999-12-31 23:59:59", 0),
            Ok(MAX_UTC_SECONDS)
        );
        assert_eq!(
            parse_fixed_offset_datetime("0001-01-01 00:00:00", 840),
            Err(WorkflowScheduleError::InvalidDateTime)
        );
        assert_eq!(
            parse_fixed_offset_datetime("2024-01-01 00:00:00", 841),
            Err(WorkflowScheduleError::InvalidFixedOffset)
        );
    }

    #[test]
    fn calendar_fixed_offset_round_trip_preserves_negative_epochs_and_dst_dates() {
        for value in [
            "0001-01-01 14:00:00",
            "1900-03-01 01:02:03",
            "1969-12-31 23:59:59",
            "2000-02-29 12:34:56",
            "2024-03-10 02:30:00",
            "2024-11-03 01:30:00",
            "9999-12-31 09:59:59",
        ] {
            for offset in [-840, -330, 0, 480, 840] {
                let seconds = parse_fixed_offset_datetime(value, offset)
                    .checked("valid date and fixed offset");
                assert_eq!(
                    format_fixed_offset_datetime(seconds, offset),
                    Ok(value.to_owned())
                );
            }
        }
    }
}
