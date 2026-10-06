//! Transient bounded per-item evidence, never execution or retry authority.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::files) enum StepState {
    NotStarted,
    Verifying,
    Unknown,
    Completed,
    Rejected,
    CancelledBeforeWrite,
    SkippedAfterFailure,
}
#[derive(Clone)]
pub(in crate::files) struct Step {
    pub path: String,
    pub delete: bool,
    pub state: StepState,
}
pub(in crate::files) struct Journal {
    pub steps: Vec<Step>,
    pub finished: bool,
    pub cleanup_failed: bool,
}
impl Journal {
    pub fn new(plan: &DirectorySyncPlan) -> Self {
        Self {
            steps: plan
                .operations()
                .iter()
                .map(|op| {
                    let (path, delete) = match op {
                        DirectorySyncOperation::Copy { path, .. } => (path, false),
                        DirectorySyncOperation::Delete { path, .. } => (path, true),
                    };
                    Step {
                        path: path.clone(),
                        delete,
                        state: StepState::NotStarted,
                    }
                })
                .collect(),
            finished: false,
            cleanup_failed: false,
        }
    }
    pub fn finish(&mut self, cancelled: bool, cleanup_failed: bool) {
        for step in &mut self.steps {
            step.state = match step.state {
                StepState::NotStarted => {
                    if cancelled {
                        StepState::CancelledBeforeWrite
                    } else {
                        StepState::SkippedAfterFailure
                    }
                }
                StepState::Verifying => {
                    if cancelled {
                        StepState::CancelledBeforeWrite
                    } else {
                        StepState::Rejected
                    }
                }
                state => state,
            };
        }
        self.finished = true;
        self.cleanup_failed |= cleanup_failed;
    }
}
pub(in crate::files) type SharedJournal = Arc<std::sync::Mutex<Journal>>;

pub(in crate::files) fn mark(journal: &SharedJournal, index: usize, state: StepState) {
    if let Ok(mut journal) = journal.lock()
        && let Some(step) = journal.steps.get_mut(index)
    {
        step.state = state;
    }
}

#[cfg(test)]
mod tests {
    use super::{Journal, Step, StepState};
    #[test]
    fn cancellation_and_failure_never_turn_uncertain_or_completed_actions_into_skips() {
        for cancelled in [false, true] {
            let states = [
                StepState::Completed,
                StepState::Unknown,
                StepState::Verifying,
                StepState::NotStarted,
            ];
            let mut journal = Journal {
                steps: states
                    .into_iter()
                    .enumerate()
                    .map(|(i, state)| Step {
                        path: i.to_string(),
                        delete: true,
                        state,
                    })
                    .collect(),
                finished: false,
                cleanup_failed: false,
            };
            journal.finish(cancelled, true);
            assert_eq!(journal.steps[0].state, StepState::Completed);
            assert_eq!(journal.steps[1].state, StepState::Unknown);
            assert_eq!(
                journal.steps[2].state,
                if cancelled {
                    StepState::CancelledBeforeWrite
                } else {
                    StepState::Rejected
                }
            );
            assert_eq!(
                journal.steps[3].state,
                if cancelled {
                    StepState::CancelledBeforeWrite
                } else {
                    StepState::SkippedAfterFailure
                }
            );
            assert!(journal.finished && journal.cleanup_failed);
        }
    }
}
