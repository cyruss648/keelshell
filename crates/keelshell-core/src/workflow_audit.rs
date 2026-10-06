//! Bounded task results. These records contain neither execution authority nor secrets.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, Error, ValidationError};

/// Maximum retained completed workflow runs.
pub const MAX_WORKFLOW_AUDITS: usize = 100;
/// Maximum tasks in a completed workflow, matching the reviewed plan limit.
pub const MAX_WORKFLOW_AUDIT_TASKS: usize = 128;
/// Total retained tasks, bounding both disk size and startup validation work.
pub const MAX_WORKFLOW_AUDIT_TOTAL_TASKS: usize = 2048;

/// Origin of one result; it cannot be used to recreate a schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowAuditTrigger {
    /// Explicit immediate human review.
    Manual,
    /// One occurrence of a finite, explicitly reviewed schedule.
    Scheduled {
        /// Identity of the transient schedule, without its binding or parameters.
        schedule_id: Uuid,
        /// Zero-based occurrence within the finite schedule.
        occurrence: u32,
        /// Intended Unix time, independent of a later clock adjustment.
        scheduled_at: i64,
    },
}

impl<'de> Deserialize<'de> for WorkflowAuditTrigger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Serde's internally tagged unit variants ignore extra keys even with
        // deny_unknown_fields. Empty struct variants enforce that invariant
        // while retaining the public enum and its existing serialized shape.
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            Manual {},
            Scheduled {
                schedule_id: Uuid,
                occurrence: u32,
                scheduled_at: i64,
            },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Manual {} => Self::Manual,
            Wire::Scheduled {
                schedule_id,
                occurrence,
                scheduled_at,
            } => Self::Scheduled {
                schedule_id,
                occurrence,
                scheduled_at,
            },
        })
    }
}

/// Why a task was conclusively never admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAuditNotStarted {
    /// Transport admission rejected before issuing a command.
    AdmissionRejected,
    /// Captured session was unavailable at admission.
    SessionUnavailable,
    /// Deadline expired before command submission.
    Deadline,
    /// Scheduler failed before command submission.
    WorkerFailed,
    /// Workflow start failed before returning an execution handle.
    StartRejected,
    /// Occurrence expired without catch-up.
    ScheduleMissed,
    /// Previous occurrence still occupied the workflow.
    ScheduleBusy,
    /// Schedule authorization became invalid before admission.
    ScheduleInvalidated,
}

/// Confirmed task state. Unknown is never coerced into failure or success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowAuditOutcome {
    /// SSH returned exit code zero.
    Succeeded,
    /// SSH returned a confirmed non-zero exit status.
    Failed {
        /// Confirmed non-zero remote status.
        exit_code: u32,
    },
    /// Remote endpoint explicitly refused execution.
    Rejected,
    /// An admitted command's result, or the aggregate receipt, was unavailable.
    Unknown,
    /// Cancellation prevented this task's admission.
    Cancelled,
    /// A prerequisite did not return confirmed success.
    DependencyBlocked {
        /// Task identity of the prerequisite, without its name or command.
        dependency: Uuid,
    },
    /// Stop-after-failure prevented admission.
    StoppedAfterFailure,
    /// Another known pre-admission condition prevented execution.
    NotStarted {
        /// Fixed reason without an arbitrary error string.
        reason: WorkflowAuditNotStarted,
    },
}

impl<'de> Deserialize<'de> for WorkflowAuditOutcome {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Match the existing wire representation without accepting arbitrary
        // context on fieldless outcomes. All data variants remain strict too.
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            Succeeded {},
            Failed { exit_code: u32 },
            Rejected {},
            Unknown {},
            Cancelled {},
            DependencyBlocked { dependency: Uuid },
            StoppedAfterFailure {},
            NotStarted { reason: WorkflowAuditNotStarted },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Succeeded {} => Self::Succeeded,
            Wire::Failed { exit_code } => Self::Failed { exit_code },
            Wire::Rejected {} => Self::Rejected,
            Wire::Unknown {} => Self::Unknown,
            Wire::Cancelled {} => Self::Cancelled,
            Wire::DependencyBlocked { dependency } => Self::DependencyBlocked { dependency },
            Wire::StoppedAfterFailure {} => Self::StoppedAfterFailure,
            Wire::NotStarted { reason } => Self::NotStarted { reason },
        })
    }
}

/// One sanitized task result. Captured target IDs remain opaque after a session ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowTaskAudit {
    /// Reviewed task identity; display names are intentionally excluded.
    pub id: Uuid,
    /// Captured target identity (possibly ephemeral), without a host or credential.
    pub target_id: Uuid,
    /// Confirmed result or conservative unknown state.
    pub outcome: WorkflowAuditOutcome,
}

/// A completed run's read-only result. No strings, command/parameter digests or
/// recovery tokens are included in this wire representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAuditRecord {
    /// Stable run identity reused by persistence retries.
    pub id: Uuid,
    /// Unix seconds when the result was recorded; retention follows insertion order.
    pub recorded_at: u64,
    /// Immediate review or one finite scheduled occurrence.
    pub trigger: WorkflowAuditTrigger,
    /// Complete task results, in reviewed dependency order.
    pub tasks: Vec<WorkflowTaskAudit>,
    /// Cancellation was observed; it does not imply an admitted command was stopped.
    pub cancelled: bool,
    /// The failure policy prevented queued admissions.
    pub stopped_after_failure: bool,
}

impl WorkflowAuditRecord {
    /// Validate bounded identities and results without reconstructing an action.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let invalid = |message| ValidationError::new("workflow_audit", message);
        if self.id.is_nil() || self.recorded_at == 0 || self.recorded_at > i64::MAX as u64 {
            return Err(invalid(
                "requires a non-nil run ID and representable nonzero time",
            ));
        }
        if let WorkflowAuditTrigger::Scheduled {
            schedule_id,
            occurrence,
            scheduled_at,
        } = self.trigger
            && (schedule_id.is_nil()
                || occurrence >= 32
                || crate::format_fixed_offset_datetime(scheduled_at, 0).is_err())
        {
            return Err(invalid("invalid finite scheduled occurrence"));
        }
        if !(1..=MAX_WORKFLOW_AUDIT_TASKS).contains(&self.tasks.len()) {
            return Err(invalid("requires 1–128 tasks"));
        }
        let mut ids = HashSet::with_capacity(self.tasks.len());
        let mut targets = HashSet::new();
        for task in &self.tasks {
            if task.id.is_nil() || task.target_id.is_nil() || !ids.insert(task.id) {
                return Err(invalid(
                    "task and target IDs must be non-nil; task IDs unique",
                ));
            }
            targets.insert(task.target_id);
            match task.outcome {
                WorkflowAuditOutcome::Failed { exit_code: 0 } => {
                    return Err(invalid("failure requires nonzero exit status"));
                }
                WorkflowAuditOutcome::Cancelled if !self.cancelled => {
                    return Err(invalid("cancelled task requires cancellation observation"));
                }
                WorkflowAuditOutcome::StoppedAfterFailure if !self.stopped_after_failure => {
                    return Err(invalid("policy-skipped task requires policy observation"));
                }
                WorkflowAuditOutcome::NotStarted {
                    reason:
                        WorkflowAuditNotStarted::ScheduleMissed
                        | WorkflowAuditNotStarted::ScheduleBusy
                        | WorkflowAuditNotStarted::ScheduleInvalidated,
                } if matches!(self.trigger, WorkflowAuditTrigger::Manual) => {
                    return Err(invalid("scheduled skip requires scheduled origin"));
                }
                _ => {}
            }
        }
        if targets.len() > 32 {
            return Err(invalid("at most 32 target identities"));
        }
        for task in &self.tasks {
            if let WorkflowAuditOutcome::DependencyBlocked { dependency } = task.outcome
                && (dependency == task.id || !ids.contains(&dependency))
            {
                return Err(invalid(
                    "blocked dependency must name another recorded task",
                ));
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_workflow_audits(
    records: &[WorkflowAuditRecord],
) -> Result<(), ValidationError> {
    if records.len() > MAX_WORKFLOW_AUDITS
        || records
            .iter()
            .map(|record| record.tasks.len())
            .sum::<usize>()
            > MAX_WORKFLOW_AUDIT_TOTAL_TASKS
    {
        return Err(ValidationError::new(
            "workflow_audits",
            "history exceeds run or total-task budget",
        ));
    }
    let mut ids = HashSet::new();
    let mut occurrences = HashSet::new();
    for record in records {
        record.validate()?;
        if !ids.insert(record.id) {
            return Err(ValidationError::new(
                "workflow_audits",
                "run IDs must be unique",
            ));
        }
        if let WorkflowAuditTrigger::Scheduled {
            schedule_id,
            occurrence,
            ..
        } = record.trigger
            && !occurrences.insert((schedule_id, occurrence))
        {
            return Err(ValidationError::new(
                "workflow_audits",
                "occurrence must be recorded once",
            ));
        }
    }
    Ok(())
}

impl AppState {
    /// Append a completed result atomically. Identical receipt retries are no-ops;
    /// conflicting IDs/occurrences fail. Old complete records are removed in FIFO
    /// insertion order, including when the wall clock moves backward.
    pub fn record_workflow_audit(&mut self, record: WorkflowAuditRecord) -> Result<(), Error> {
        record.validate()?;
        if let Some(existing) = self
            .workflow_audits
            .iter()
            .find(|entry| entry.id == record.id)
        {
            if existing == &record {
                return Ok(());
            }
            return Err(ValidationError::new("workflow_audits", "conflicting run receipt").into());
        }
        if let WorkflowAuditTrigger::Scheduled { schedule_id, occurrence, .. } = record.trigger
            && self.workflow_audits.iter().any(|entry| matches!(entry.trigger,
                WorkflowAuditTrigger::Scheduled { schedule_id: other, occurrence: index, .. } if other == schedule_id && index == occurrence))
        {
            return Err(ValidationError::new("workflow_audits", "conflicting occurrence receipt").into());
        }
        let mut records = self.workflow_audits.clone();
        records.push(record);
        let mut tasks = records
            .iter()
            .map(|record| record.tasks.len())
            .sum::<usize>();
        let mut remove = 0;
        while records.len() - remove > MAX_WORKFLOW_AUDITS || tasks > MAX_WORKFLOW_AUDIT_TOTAL_TASKS
        {
            tasks -= records[remove].tasks.len();
            remove += 1;
        }
        records.drain(..remove);
        validate_workflow_audits(&records)?;
        self.workflow_audits = records;
        Ok(())
    }
}
