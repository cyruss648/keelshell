// Diagnostic only: exact extracted state transition and helper, with a reduced state holder.
// This does not execute GPUI, store I/O, OS worker creation, or the actual app tests.
use std::collections::VecDeque;
mod uuid { pub type Uuid = u128; }
struct FilesPanel {
    session_token: uuid::Uuid,
    recovery_review_session: Option<uuid::Uuid>,
    recovery_review_active: Option<uuid::Uuid>,
    recovery_review_queue: VecDeque<uuid::Uuid>,
}
impl FilesPanel {
fn requeue_failed_recovery_review_item(&mut self, id: uuid::Uuid) {
        if self.recovery_review_active == Some(id) {
            if self.recovery_review_session == Some(self.session_token)
                && !self.recovery_review_queue.contains(&id)
            {
                self.recovery_review_queue.push_front(id);
            }
            self.recovery_review_active = None;
        }
    }
fn production_before_worker_spawn(&mut self, recovery_id: uuid::Uuid) {
if self.recovery_review_active == Some(recovery_id) {
            self.recovery_review_active = None;
            if self.recovery_review_queue.is_empty() {
                self.recovery_review_session = None;
            }
        }
}
}
fn first_claimed() -> FilesPanel {
    FilesPanel {
        session_token: 100,
        recovery_review_session: Some(100),
        recovery_review_active: Some(1),
        recovery_review_queue: VecDeque::from([2]),
    }
}
#[test]
fn capacity_failure_before_transition_restores_original_order() {
    let mut panel = first_claimed();
    panel.requeue_failed_recovery_review_item(1);
    assert_eq!(panel.recovery_review_queue, VecDeque::from([1, 2]));
    assert_eq!(panel.recovery_review_active, None);
}
#[test]
fn accepted_start_preserves_waiting_second_record() {
    let mut panel = first_claimed();
    panel.production_before_worker_spawn(1);
    assert_eq!(panel.recovery_review_queue, VecDeque::from([2]));
    assert_eq!(panel.recovery_review_session, Some(100));
    assert_eq!(panel.recovery_review_active, None);
}
#[test]
fn failed_worker_start_should_restore_original_head() {
    let mut panel = first_claimed();
    panel.production_before_worker_spawn(1);
    // Exact call made by the production spawn error branch at parallel.rs:602.
    panel.requeue_failed_recovery_review_item(1);
    assert_eq!(panel.recovery_review_queue, VecDeque::from([1, 2]));
}
