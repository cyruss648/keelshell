//! Bounded, reviewed dependency plans and a transport-free admission ledger.
//!
//! Commands are exact transient review values. This module performs no I/O,
//! persists nothing, schedules no timers and never claims to execute a command.
//! A later adapter must bind each target identity to the reviewed authenticated
//! session and report only observed outcomes to the ledger.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Maximum number of tasks in one reviewed dependency plan.
pub const MAX_BATCH_WORKFLOW_TASKS: usize = 128;
/// Maximum distinct target identities, matching the current batch transport.
pub const MAX_BATCH_WORKFLOW_TARGETS: usize = 32;
/// Maximum number of immediate dependencies per task.
pub const MAX_BATCH_TASK_DEPENDENCIES: usize = 32;
/// Maximum exact UTF-8 command size per task.
pub const MAX_BATCH_TASK_COMMAND_BYTES: usize = 64 * 1024;
/// Maximum aggregate UTF-8 command storage in one plan.
pub const MAX_BATCH_WORKFLOW_COMMAND_BYTES: usize = 1024 * 1024;

/// One caller-owned task specification consumed when building a review.
///
/// Target IDs identify the caller's reviewed session bindings, not hostnames.
/// Commands may be produced by [`crate::BatchCommandTemplate`] before construction;
/// this type neither expands markers nor reads target metadata. Debug omits
/// command text because commands can contain sensitive values.
#[derive(Clone, PartialEq, Eq)]
pub struct BatchTaskSpec {
    /// Stable non-nil identity, unique within this plan.
    pub id: Uuid,
    /// Stable non-nil target binding to be resolved by a future transport adapter.
    pub target_id: Uuid,
    /// Exact command text; whitespace and newlines are preserved.
    pub command: String,
    /// Existing task identities that must each finish with confirmed success.
    pub dependencies: Vec<Uuid>,
}

impl fmt::Debug for BatchTaskSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatchTaskSpec")
            .field("id", &self.id)
            .field("target_id", &self.target_id)
            .field("command_bytes", &self.command.len())
            .field("dependencies", &self.dependencies)
            .finish()
    }
}

/// Immutable, validated DAG in deterministic topological review order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchWorkflowPlan {
    tasks: Vec<BatchTaskSpec>,
    index: BTreeMap<Uuid, usize>,
    fingerprint: BatchWorkflowReviewToken,
}

/// Fixed failures; diagnostics include identities but never command text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BatchWorkflowError {
    /// Task count is outside the bounded range.
    #[error("dependency plan requires 1 through 128 tasks")]
    InvalidTaskCount,
    /// Task or target identity is nil.
    #[error("dependency plan task and target identities must be non-nil")]
    NilIdentity,
    /// Two specifications share one task identity.
    #[error("duplicate task identity: {id}")]
    DuplicateTask {
        /// Conflicting stable identity.
        id: Uuid,
    },
    /// Too many distinct targets were selected.
    #[error("dependency plan has more than 32 targets")]
    TargetLimit,
    /// Empty, oversized, or terminal-control-containing command text.
    #[error("invalid bounded command for task: {id}")]
    InvalidCommand {
        /// Task whose command was rejected.
        id: Uuid,
    },
    /// Aggregate transient command storage exceeds one MiB.
    #[error("dependency plan command storage exceeds 1 MiB")]
    CommandBudget,
    /// A task has too many immediate dependencies.
    #[error("task has more than 32 dependencies: {id}")]
    DependencyLimit {
        /// Task whose edges exceeded the bound.
        id: Uuid,
    },
    /// An immediate dependency appeared more than once.
    #[error("task {id} repeats dependency {dependency}")]
    DuplicateDependency {
        /// Dependent task.
        id: Uuid,
        /// Repeated prerequisite.
        dependency: Uuid,
    },
    /// A task names an identity outside the same plan.
    #[error("task {id} refers to missing dependency {dependency}")]
    MissingDependency {
        /// Dependent task.
        id: Uuid,
        /// Missing prerequisite.
        dependency: Uuid,
    },
    /// The graph cannot be topologically ordered, including self-dependencies.
    #[error("dependency plan contains a cycle")]
    Cycle,
    /// The acknowledgement belongs to a different immutable review.
    #[error("dependency plan review fingerprint changed")]
    ReviewMismatch,
    /// A ledger operation names no task in its confirmed plan.
    #[error("unknown dependency task: {id}")]
    UnknownTask {
        /// Identity supplied by the caller.
        id: Uuid,
    },
    /// Only ready tasks may be admitted, only running tasks may finish, and
    /// only not-yet-admitted tasks may be explicitly skipped.
    #[error("invalid dependency task transition: {id}")]
    InvalidTransition {
        /// Task with the rejected state transition.
        id: Uuid,
    },
}

impl BatchWorkflowPlan {
    /// Validate the complete graph without any side effects.
    ///
    /// Task/dependency input order has no semantic effect. Lexicographic UUID
    /// order resolves ties between ready nodes, producing an identical review
    /// fingerprint for semantically identical input. Duplicate edges, missing
    /// nodes and cycles are rejected instead of being silently repaired.
    ///
    /// ```
    /// use keelshell_core::{BatchTaskSpec, BatchWorkflowPlan};
    /// use uuid::Uuid;
    /// # fn main() -> Result<(), keelshell_core::BatchWorkflowError> {
    /// let first = Uuid::from_u128(1);
    /// let second = Uuid::from_u128(2);
    /// let target = Uuid::from_u128(3);
    /// let plan = BatchWorkflowPlan::new(vec![
    ///     BatchTaskSpec { id: first, target_id: target,
    ///         command: "printf 'check'".into(), dependencies: vec![] },
    ///     BatchTaskSpec { id: second, target_id: target,
    ///         command: "printf 'after successful check'".into(), dependencies: vec![first] },
    /// ])?;
    /// // Display every task and binding, then obtain explicit user confirmation.
    /// let token = plan.review_token();
    /// let ledger = plan.confirm(token)?.into_ledger();
    /// assert_eq!(ledger.ready_tasks(), vec![first]);
    /// // No command was executed by constructing or querying the ledger.
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(tasks: Vec<BatchTaskSpec>) -> Result<Self, BatchWorkflowError> {
        if tasks.is_empty() || tasks.len() > MAX_BATCH_WORKFLOW_TASKS {
            return Err(BatchWorkflowError::InvalidTaskCount);
        }
        let mut by_id = BTreeMap::new();
        let mut targets = BTreeSet::new();
        let mut command_bytes = 0usize;
        for mut task in tasks {
            if task.id.is_nil() || task.target_id.is_nil() {
                return Err(BatchWorkflowError::NilIdentity);
            }
            if by_id.contains_key(&task.id) {
                return Err(BatchWorkflowError::DuplicateTask { id: task.id });
            }
            if task.command.trim().is_empty()
                || task.command.len() > MAX_BATCH_TASK_COMMAND_BYTES
                || task
                    .command
                    .chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
            {
                return Err(BatchWorkflowError::InvalidCommand { id: task.id });
            }
            command_bytes = command_bytes
                .checked_add(task.command.len())
                .filter(|count| *count <= MAX_BATCH_WORKFLOW_COMMAND_BYTES)
                .ok_or(BatchWorkflowError::CommandBudget)?;
            targets.insert(task.target_id);
            if targets.len() > MAX_BATCH_WORKFLOW_TARGETS {
                return Err(BatchWorkflowError::TargetLimit);
            }
            if task.dependencies.len() > MAX_BATCH_TASK_DEPENDENCIES {
                return Err(BatchWorkflowError::DependencyLimit { id: task.id });
            }
            task.dependencies.sort_unstable();
            if let Some(pair) = task.dependencies.windows(2).find(|pair| pair[0] == pair[1]) {
                return Err(BatchWorkflowError::DuplicateDependency {
                    id: task.id,
                    dependency: pair[0],
                });
            }
            by_id.insert(task.id, task);
        }
        let mut unmet = BTreeMap::new();
        let mut dependents: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
        for task in by_id.values() {
            for dependency in &task.dependencies {
                if !by_id.contains_key(dependency) {
                    return Err(BatchWorkflowError::MissingDependency {
                        id: task.id,
                        dependency: *dependency,
                    });
                }
                dependents.entry(*dependency).or_default().push(task.id);
            }
            unmet.insert(task.id, task.dependencies.len());
        }
        let mut ready: BTreeSet<_> = unmet
            .iter()
            .filter_map(|(id, count)| (*count == 0).then_some(*id))
            .collect();
        let mut ordered = Vec::with_capacity(by_id.len());
        while let Some(id) = ready.pop_first() {
            ordered.push(id);
            for dependent in dependents.get(&id).into_iter().flatten() {
                if let Some(count) = unmet.get_mut(dependent) {
                    *count -= 1;
                    if *count == 0 {
                        ready.insert(*dependent);
                    }
                }
            }
        }
        if ordered.len() != by_id.len() {
            return Err(BatchWorkflowError::Cycle);
        }
        let tasks: Vec<_> = ordered
            .into_iter()
            .filter_map(|id| by_id.remove(&id))
            .collect();
        let index = tasks
            .iter()
            .enumerate()
            .map(|(index, task)| (task.id, index))
            .collect();
        let fingerprint = fingerprint(&tasks);
        Ok(Self {
            tasks,
            index,
            fingerprint,
        })
    }

    /// Exact commands and dependencies in deterministic topological order.
    pub fn tasks(&self) -> &[BatchTaskSpec] {
        &self.tasks
    }

    /// Fingerprint of identities, bindings, precise command bytes and all edges.
    /// This value is not a credential or standalone execution authorization.
    pub const fn review_token(&self) -> BatchWorkflowReviewToken {
        self.fingerprint
    }

    /// Consume this immutable plan after the user confirms the exact review.
    ///
    /// The receipt has no transport or write capability. The caller must only
    /// use this transition from an explicit UI confirmation, after displaying
    /// all commands, target bindings and dependencies.
    pub fn confirm(
        self,
        token: BatchWorkflowReviewToken,
    ) -> Result<ConfirmedBatchWorkflow, BatchWorkflowError> {
        if token != self.fingerprint {
            return Err(BatchWorkflowError::ReviewMismatch);
        }
        Ok(ConfirmedBatchWorkflow { plan: self })
    }
}

/// Opaque deterministic SHA-256 review fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchWorkflowReviewToken([u8; 32]);

impl BatchWorkflowReviewToken {
    /// Bytes suitable for a non-secret digest receipt; no command text is stored.
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
    /// Lowercase hexadecimal for display beside the full human review.
    pub fn hex(self) -> String {
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            output.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
            output.push(char::from(b"0123456789abcdef"[(byte & 15) as usize]));
        }
        output
    }
}

/// Acknowledged plan; owns no connections, threads, timers or storage.
#[derive(Debug)]
pub struct ConfirmedBatchWorkflow {
    plan: BatchWorkflowPlan,
}

impl ConfirmedBatchWorkflow {
    /// Borrow the exact acknowledged review.
    pub fn plan(&self) -> &BatchWorkflowPlan {
        &self.plan
    }
    /// Create a pure in-memory ledger. This starts no task or network request.
    pub fn into_ledger(self) -> BatchWorkflowLedger {
        let states = vec![TaskRecord::Pending; self.plan.tasks.len()];
        BatchWorkflowLedger {
            plan: self.plan,
            states,
        }
    }
}

/// Externally observed execution classification; only Success releases edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchTaskOutcome {
    /// The adapter observed a completed, confirmed zero exit status.
    Success,
    /// Confirmed non-zero exit or an explicit execution rejection.
    Failed,
    /// A request may have executed but its complete result is unconfirmed.
    Unknown,
}

/// Why a task was never admitted. No variant claims a running process stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchTaskSkipReason {
    /// Caller explicitly cancelled a not-yet-admitted task.
    Cancelled,
    /// Caller policy prevented admission after a failure.
    StoppedAfterFailure,
    /// A prerequisite failed, was unknown or was itself skipped.
    DependencyNotSucceeded {
        /// First unsuccessful dependency in UUID order at the skip transition.
        dependency: Uuid,
    },
}

/// Current local admission state, derived from an immutable plan and receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchTaskStatus {
    /// Every prerequisite has confirmed success; caller may explicitly admit it.
    Ready,
    /// At least one prerequisite has not yet reached a terminal state.
    Blocked {
        /// Waiting prerequisites in deterministic UUID order.
        waiting_for: Vec<Uuid>,
    },
    /// Locally admitted; this is not proof of remote command execution.
    Running,
    /// Adapter-reported terminal classification.
    Finished(BatchTaskOutcome),
    /// Never admitted due to explicit policy/cancellation or unsuccessful edges.
    Skipped(BatchTaskSkipReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskRecord {
    Pending,
    Running,
    Finished(BatchTaskOutcome),
    Skipped(BatchTaskSkipReason),
}

/// Pure transition ledger created from a confirmed plan.
///
/// The ledger trusts adapter-reported observations; it cannot prove remote
/// outcomes. Ready tasks stay pending until admit is explicitly called. Each
/// task may be admitted once, terminal receipts cannot be replaced, and only
/// confirmed success makes a dependent ready. Unknown outcomes fail closed.
#[derive(Debug)]
pub struct BatchWorkflowLedger {
    plan: BatchWorkflowPlan,
    states: Vec<TaskRecord>,
}

impl BatchWorkflowLedger {
    /// Borrow immutable commands and target bindings; there is no editing API.
    pub fn plan(&self) -> &BatchWorkflowPlan {
        &self.plan
    }

    /// Return a task's current state, without changing or admitting any task.
    pub fn status(&self, id: Uuid) -> Result<BatchTaskStatus, BatchWorkflowError> {
        let index = self.index(id)?;
        Ok(match self.states[index] {
            TaskRecord::Running => BatchTaskStatus::Running,
            TaskRecord::Finished(outcome) => BatchTaskStatus::Finished(outcome),
            TaskRecord::Skipped(reason) => BatchTaskStatus::Skipped(reason),
            TaskRecord::Pending => {
                let waiting_for: Vec<_> = self.plan.tasks[index]
                    .dependencies
                    .iter()
                    .copied()
                    .filter(|dependency| {
                        self.plan.index.get(dependency).is_none_or(|index| {
                            self.states[*index] != TaskRecord::Finished(BatchTaskOutcome::Success)
                        })
                    })
                    .collect();
                if waiting_for.is_empty() {
                    BatchTaskStatus::Ready
                } else {
                    BatchTaskStatus::Blocked { waiting_for }
                }
            }
        })
    }

    /// Ready identities in the plan's deterministic topological order.
    /// Merely querying readiness starts nothing and grants no transport access.
    pub fn ready_tasks(&self) -> Vec<Uuid> {
        self.plan
            .tasks
            .iter()
            .filter_map(|task| {
                matches!(self.status(task.id), Ok(BatchTaskStatus::Ready)).then_some(task.id)
            })
            .collect()
    }

    /// Mark one ready task locally admitted. Caller must separately issue its
    /// exact reviewed command on the reviewed session, then report the outcome.
    pub fn admit(&mut self, id: Uuid) -> Result<(), BatchWorkflowError> {
        if self.status(id)? != BatchTaskStatus::Ready {
            return Err(BatchWorkflowError::InvalidTransition { id });
        }
        let index = self.index(id)?;
        self.states[index] = TaskRecord::Running;
        Ok(())
    }

    /// Record one terminal observation for an admitted task exactly once.
    /// Failed/unknown outcomes deterministically skip dependent pending tasks.
    /// Running tasks are never reclassified by another task's failure.
    pub fn finish(
        &mut self,
        id: Uuid,
        outcome: BatchTaskOutcome,
    ) -> Result<(), BatchWorkflowError> {
        let index = self.index(id)?;
        if self.states[index] != TaskRecord::Running {
            return Err(BatchWorkflowError::InvalidTransition { id });
        }
        self.states[index] = TaskRecord::Finished(outcome);
        self.propagate_skips();
        Ok(())
    }

    /// Skip a task before admission due to explicit cancellation or policy.
    /// Dependency-derived skips are owned by the ledger and cannot be injected
    /// by a caller. A running task needs an observed terminal outcome instead.
    pub fn skip(
        &mut self,
        id: Uuid,
        reason: BatchTaskSkipReason,
    ) -> Result<(), BatchWorkflowError> {
        let index = self.index(id)?;
        if self.states[index] != TaskRecord::Pending
            || matches!(reason, BatchTaskSkipReason::DependencyNotSucceeded { .. })
        {
            return Err(BatchWorkflowError::InvalidTransition { id });
        }
        self.states[index] = TaskRecord::Skipped(reason);
        self.propagate_skips();
        Ok(())
    }

    /// Explicitly cancel all tasks not yet admitted, retaining running states.
    /// This cannot terminate or prove termination of a remote process.
    pub fn cancel_pending(&mut self) {
        for state in &mut self.states {
            if *state == TaskRecord::Pending {
                *state = TaskRecord::Skipped(BatchTaskSkipReason::Cancelled);
            }
        }
    }

    /// Whether every task has a terminal local record.
    pub fn is_finished(&self) -> bool {
        self.states
            .iter()
            .all(|state| matches!(state, TaskRecord::Finished(_) | TaskRecord::Skipped(_)))
    }

    fn index(&self, id: Uuid) -> Result<usize, BatchWorkflowError> {
        self.plan
            .index
            .get(&id)
            .copied()
            .ok_or(BatchWorkflowError::UnknownTask { id })
    }

    fn propagate_skips(&mut self) {
        // Topological order puts every prerequisite before its dependents, so
        // one forward pass propagates transitive skips without recursion.
        for index in 0..self.plan.tasks.len() {
            if self.states[index] != TaskRecord::Pending {
                continue;
            }
            let unsuccessful =
                self.plan.tasks[index]
                    .dependencies
                    .iter()
                    .copied()
                    .find(|dependency| {
                        self.plan.index.get(dependency).is_some_and(|index| {
                            matches!(
                                self.states[*index],
                                TaskRecord::Finished(
                                    BatchTaskOutcome::Failed | BatchTaskOutcome::Unknown
                                ) | TaskRecord::Skipped(_)
                            )
                        })
                    });
            if let Some(dependency) = unsuccessful {
                self.states[index] =
                    TaskRecord::Skipped(BatchTaskSkipReason::DependencyNotSucceeded { dependency });
            }
        }
    }
}

fn fingerprint(tasks: &[BatchTaskSpec]) -> BatchWorkflowReviewToken {
    let mut digest = Sha256::new();
    digest.update(b"keelshell-batch-workflow-v1\0");
    digest.update((tasks.len() as u64).to_be_bytes());
    for task in tasks {
        digest.update(task.id.as_bytes());
        digest.update(task.target_id.as_bytes());
        digest.update((task.command.len() as u64).to_be_bytes());
        digest.update(task.command.as_bytes());
        digest.update((task.dependencies.len() as u64).to_be_bytes());
        for dependency in &task.dependencies {
            digest.update(dependency.as_bytes());
        }
    }
    BatchWorkflowReviewToken(digest.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    trait CheckedResult<T, E> {
        fn checked(self) -> T;
        fn checked_error(self) -> E;
    }

    impl<T: fmt::Debug, E: fmt::Debug> CheckedResult<T, E> for Result<T, E> {
        fn checked(self) -> T {
            match self {
                Ok(value) => value,
                Err(error) => panic!("unexpected error: {error:?}"),
            }
        }
        fn checked_error(self) -> E {
            match self {
                Ok(value) => panic!("unexpected success: {value:?}"),
                Err(error) => error,
            }
        }
    }

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }
    fn task(value: u128, dependencies: &[u128]) -> BatchTaskSpec {
        BatchTaskSpec {
            id: id(value),
            target_id: id(1000),
            command: format!("echo task-{value}"),
            dependencies: dependencies.iter().copied().map(id).collect(),
        }
    }
    fn ledger(tasks: Vec<BatchTaskSpec>) -> BatchWorkflowLedger {
        let plan = BatchWorkflowPlan::new(tasks).checked();
        let token = plan.review_token();
        plan.confirm(token).checked().into_ledger()
    }

    #[test]
    fn topology_and_review_are_independent_of_task_and_edge_input_order() {
        let one = BatchWorkflowPlan::new(vec![
            task(4, &[3, 2]),
            task(3, &[1]),
            task(2, &[1]),
            task(1, &[]),
        ])
        .checked();
        let two = BatchWorkflowPlan::new(vec![
            task(1, &[]),
            task(2, &[1]),
            task(3, &[1]),
            task(4, &[2, 3]),
        ])
        .checked();
        assert_eq!(
            one.tasks().iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![id(1), id(2), id(3), id(4)]
        );
        assert_eq!(one, two);
        assert_eq!(one.review_token().hex().len(), 64);
    }

    #[test]
    fn topology_prioritizes_dependencies_before_lexicographic_ties() {
        let plan = BatchWorkflowPlan::new(vec![
            task(4, &[1]),
            task(1, &[3]),
            task(3, &[]),
            task(2, &[]),
        ])
        .checked();
        assert_eq!(
            plan.tasks().iter().map(|task| task.id).collect::<Vec<_>>(),
            vec![id(2), id(3), id(1), id(4)]
        );
    }

    #[test]
    fn graph_rejects_duplicate_missing_self_and_long_cycles() {
        assert_eq!(
            BatchWorkflowPlan::new(vec![task(1, &[]), task(1, &[])]).checked_error(),
            BatchWorkflowError::DuplicateTask { id: id(1) }
        );
        assert_eq!(
            BatchWorkflowPlan::new(vec![task(1, &[]), task(2, &[1, 1])]).checked_error(),
            BatchWorkflowError::DuplicateDependency {
                id: id(2),
                dependency: id(1)
            }
        );
        assert_eq!(
            BatchWorkflowPlan::new(vec![task(1, &[9])]).checked_error(),
            BatchWorkflowError::MissingDependency {
                id: id(1),
                dependency: id(9)
            }
        );
        assert_eq!(
            BatchWorkflowPlan::new(vec![task(1, &[1])]).checked_error(),
            BatchWorkflowError::Cycle
        );
        assert_eq!(
            BatchWorkflowPlan::new(vec![task(1, &[3]), task(2, &[1]), task(3, &[2])])
                .checked_error(),
            BatchWorkflowError::Cycle
        );
    }

    #[test]
    fn command_identity_and_storage_bounds_fail_before_confirmation() {
        assert_eq!(
            BatchWorkflowPlan::new(Vec::new()).checked_error(),
            BatchWorkflowError::InvalidTaskCount
        );
        let mut nil = task(1, &[]);
        nil.target_id = Uuid::nil();
        assert_eq!(
            BatchWorkflowPlan::new(vec![nil]).checked_error(),
            BatchWorkflowError::NilIdentity
        );
        let mut nil = task(1, &[]);
        nil.id = Uuid::nil();
        assert_eq!(
            BatchWorkflowPlan::new(vec![nil]).checked_error(),
            BatchWorkflowError::NilIdentity
        );
        for command in [
            String::new(),
            " \n\t".into(),
            "echo\0hidden".into(),
            "echo\u{1b}[31m".into(),
            "x".repeat(MAX_BATCH_TASK_COMMAND_BYTES + 1),
        ] {
            let mut spec = task(1, &[]);
            spec.command = command;
            assert_eq!(
                BatchWorkflowPlan::new(vec![spec]).checked_error(),
                BatchWorkflowError::InvalidCommand { id: id(1) }
            );
        }
        let too_many: Vec<_> = (1..=129).map(|value| task(value, &[])).collect();
        assert_eq!(
            BatchWorkflowPlan::new(too_many).checked_error(),
            BatchWorkflowError::InvalidTaskCount
        );
        let too_much: Vec<_> = (1..=17)
            .map(|value| {
                let mut spec = task(value, &[]);
                spec.command = "x".repeat(MAX_BATCH_TASK_COMMAND_BYTES);
                spec
            })
            .collect();
        assert_eq!(
            BatchWorkflowPlan::new(too_much).checked_error(),
            BatchWorkflowError::CommandBudget
        );
        let targets: Vec<_> = (1..=33)
            .map(|value| {
                let mut spec = task(value, &[]);
                spec.target_id = id(value + 1000);
                spec
            })
            .collect();
        assert_eq!(
            BatchWorkflowPlan::new(targets).checked_error(),
            BatchWorkflowError::TargetLimit
        );
        let mut edges = task(1, &[]);
        edges.dependencies = (2..=34).map(id).collect();
        assert_eq!(
            BatchWorkflowPlan::new(vec![edges]).checked_error(),
            BatchWorkflowError::DependencyLimit { id: id(1) }
        );
    }

    #[test]
    fn inclusive_command_target_and_dependency_bounds_remain_usable() {
        let commands: Vec<_> = (1..=16)
            .map(|value| {
                let mut spec = task(value, &[]);
                spec.command = "x".repeat(MAX_BATCH_TASK_COMMAND_BYTES);
                spec
            })
            .collect();
        let plan = BatchWorkflowPlan::new(commands).checked();
        assert_eq!(
            plan.tasks()
                .iter()
                .map(|task| task.command.len())
                .sum::<usize>(),
            MAX_BATCH_WORKFLOW_COMMAND_BYTES
        );
        let mut tasks: Vec<_> = (1..=32)
            .map(|value| {
                let mut spec = task(value, &[]);
                spec.target_id = id(value + 1000);
                spec
            })
            .collect();
        tasks.push(task(33, &(1..=32).collect::<Vec<_>>()));
        // The join reuses a selected target, so it does not increase the 32 bindings.
        tasks[32].target_id = id(1001);
        let mut run = ledger(tasks);
        assert_eq!(run.ready_tasks().len(), 32);
        for value in 1..=32 {
            run.admit(id(value)).checked();
            run.finish(id(value), BatchTaskOutcome::Success).checked();
        }
        assert_eq!(run.ready_tasks(), vec![id(33)]);
    }

    #[test]
    fn fingerprint_covers_exact_command_target_identity_and_edges() {
        let base = BatchWorkflowPlan::new(vec![task(1, &[]), task(2, &[1])]).checked();
        let token = base.review_token();
        let mut variants = Vec::new();
        let mut command = task(2, &[1]);
        command.command.push(' ');
        variants.push(vec![task(1, &[]), command]);
        let mut target = task(2, &[1]);
        target.target_id = id(1001);
        variants.push(vec![task(1, &[]), target]);
        variants.push(vec![task(1, &[]), task(2, &[])]);
        variants.push(vec![task(1, &[]), task(3, &[1])]);
        for tasks in variants {
            let changed = BatchWorkflowPlan::new(tasks).checked();
            assert_ne!(token, changed.review_token());
            assert_eq!(
                changed.confirm(token).checked_error(),
                BatchWorkflowError::ReviewMismatch
            );
        }
        assert_eq!(base.confirm(token).checked().plan().review_token(), token);
    }

    #[test]
    fn only_success_releases_dependencies_and_failed_unknown_or_skipped_cascade() {
        for outcome in [BatchTaskOutcome::Failed, BatchTaskOutcome::Unknown] {
            let mut run = ledger(vec![
                task(1, &[]),
                task(2, &[1]),
                task(3, &[2]),
                task(4, &[]),
            ]);
            assert_eq!(run.ready_tasks(), vec![id(1), id(4)]);
            assert_eq!(
                run.status(id(2)).checked(),
                BatchTaskStatus::Blocked {
                    waiting_for: vec![id(1)]
                }
            );
            assert!(run.admit(id(2)).is_err());
            run.admit(id(1)).checked();
            run.finish(id(1), outcome).checked();
            assert_eq!(
                run.status(id(2)).checked(),
                BatchTaskStatus::Skipped(BatchTaskSkipReason::DependencyNotSucceeded {
                    dependency: id(1)
                })
            );
            assert_eq!(
                run.status(id(3)).checked(),
                BatchTaskStatus::Skipped(BatchTaskSkipReason::DependencyNotSucceeded {
                    dependency: id(2)
                })
            );
            assert_eq!(run.ready_tasks(), vec![id(4)]);
        }
        let mut run = ledger(vec![task(1, &[]), task(2, &[1])]);
        run.skip(id(1), BatchTaskSkipReason::StoppedAfterFailure)
            .checked();
        assert!(matches!(run.status(id(2)), Ok(BatchTaskStatus::Skipped(_))));
        assert!(run.is_finished());
    }

    #[test]
    fn all_dependencies_need_confirmed_success_and_invalid_transitions_are_atomic() {
        let mut run = ledger(vec![task(1, &[]), task(2, &[]), task(3, &[1, 2])]);
        assert!(run.finish(id(1), BatchTaskOutcome::Success).is_err());
        assert_eq!(run.status(id(1)).checked(), BatchTaskStatus::Ready);
        run.admit(id(1)).checked();
        run.admit(id(2)).checked();
        run.finish(id(1), BatchTaskOutcome::Success).checked();
        assert_eq!(
            run.status(id(3)).checked(),
            BatchTaskStatus::Blocked {
                waiting_for: vec![id(2)]
            }
        );
        assert!(run.admit(id(1)).is_err());
        assert!(run.finish(id(1), BatchTaskOutcome::Failed).is_err());
        assert!(run.skip(id(2), BatchTaskSkipReason::Cancelled).is_err());
        run.finish(id(2), BatchTaskOutcome::Success).checked();
        assert_eq!(run.ready_tasks(), vec![id(3)]);
        run.admit(id(3)).checked();
        run.finish(id(3), BatchTaskOutcome::Success).checked();
        assert!(run.is_finished());
        assert_eq!(
            run.status(id(9)).checked_error(),
            BatchWorkflowError::UnknownTask { id: id(9) }
        );
    }

    #[test]
    fn cancellation_never_claims_a_running_task_or_process_was_stopped() {
        let mut run = ledger(vec![task(1, &[]), task(2, &[]), task(3, &[1])]);
        run.admit(id(1)).checked();
        run.cancel_pending();
        assert_eq!(run.status(id(1)).checked(), BatchTaskStatus::Running);
        assert_eq!(
            run.status(id(2)).checked(),
            BatchTaskStatus::Skipped(BatchTaskSkipReason::Cancelled)
        );
        assert_eq!(
            run.status(id(3)).checked(),
            BatchTaskStatus::Skipped(BatchTaskSkipReason::Cancelled)
        );
        assert!(!run.is_finished());
        run.finish(id(1), BatchTaskOutcome::Unknown).checked();
        assert!(run.is_finished());
    }

    #[test]
    fn maximum_depth_cascades_without_recursive_walk_or_command_logging() {
        let tasks: Vec<_> = (1..=128)
            .map(|value| {
                if value == 1 {
                    task(value, &[])
                } else {
                    task(value, &[value - 1])
                }
            })
            .collect();
        let plan = BatchWorkflowPlan::new(tasks).checked();
        assert!(!format!("{plan:?}").contains("echo task"));
        let token = plan.review_token();
        let mut run = plan.confirm(token).checked().into_ledger();
        assert!(!format!("{run:?}").contains("echo task"));
        run.admit(id(1)).checked();
        run.finish(id(1), BatchTaskOutcome::Failed).checked();
        assert!(run.is_finished());
        assert!(matches!(
            run.status(id(128)),
            Ok(BatchTaskStatus::Skipped(_))
        ));
    }
}
