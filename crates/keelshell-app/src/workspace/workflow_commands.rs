//! Final dispatch authorization compares captured authenticated connection instances.
use super::*;
use crate::workflow_commands::{
    ConnectedDestination, WorkflowPanel, WorkflowPanelEvent, WorkflowReview,
};

impl Workspace {
    fn workflow_destinations(&self, cx: &App) -> Vec<ConnectedDestination> {
        self.batch_destinations(cx)
            .into_iter()
            .filter_map(|destination| {
                self.remote_sessions
                    .get(&destination.entity)
                    .filter(|session| !session.is_closed())
                    .map(|session| ConnectedDestination {
                        destination,
                        session: session.clone(),
                    })
            })
            .collect()
    }
    pub(super) fn maintain_workflow(&mut self, cx: &mut Context<Self>) {
        if let Some(panel) = self.workflow_panel.clone() {
            let current = self.workflow_destinations(cx);
            panel.update(cx, |panel, cx| panel.update_available(&current, cx));
        }
    }
    pub(super) fn open_workflow(
        &mut self,
        fresh: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_surface_blocked() && !self.show_workflow {
            return;
        }
        if fresh
            && self
                .workflow_panel
                .as_ref()
                .is_some_and(|panel| panel.read(cx).is_running())
        {
            return;
        }
        self.cancel_remote_completion(cx);
        if fresh || self.workflow_panel.is_none() {
            let destinations = self.workflow_destinations(cx);
            let text = self.command.read(cx).value().to_string();
            let panel = cx.new(|cx| WorkflowPanel::new(destinations, text, window, cx));
            self.workflow_subscription =
                Some(
                    cx.subscribe_in(&panel, window, |view, panel, event, window, cx| {
                        if view
                            .workflow_panel
                            .as_ref()
                            .is_none_or(|current| current.entity_id() != panel.entity_id())
                        {
                            return;
                        }
                        match event {
                            WorkflowPanelEvent::Completed(record) => {
                                view.record_workflow_audit(record.clone(), window, cx);
                            }
                            WorkflowPanelEvent::RetryAuditSave => {
                                view.flush_workflow_audits(window, cx);
                            }
                            WorkflowPanelEvent::Start(review) => {
                                view.start_reviewed_workflow(panel.clone(), review, window, cx)
                            }
                            WorkflowPanelEvent::Hide => {
                                view.show_workflow = false;
                                view.focus_current_surface(window, cx);
                                cx.notify();
                            }
                            WorkflowPanelEvent::New => view.open_workflow(true, window, cx),
                            WorkflowPanelEvent::RefreshTargets => {
                                let current = view.workflow_destinations(cx);
                                panel.update(cx, |panel, cx| panel.refresh_targets(current, cx));
                            }
                        }
                    }),
                );
            self.workflow_panel = Some(panel);
        }
        self.refresh_workflow_audit_history(cx);
        self.show_workflow = true;
        self.maintain_workflow(cx);
        self.focus_current_surface(window, cx);
        cx.notify();
    }
    fn start_reviewed_workflow(
        &mut self,
        panel: Entity<WorkflowPanel>,
        review: &WorkflowReview,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.maintain_workflow(cx);
        let current = self.workflow_destinations(cx);
        let valid = (self.show_workflow || panel.read(cx).scheduled_due(review))
            && self
                .workflow_panel
                .as_ref()
                .is_some_and(|current| current.entity_id() == panel.entity_id())
            && panel.read(cx).review_current(review, cx)
            && review.destinations.iter().all(|destination| {
                panel
                    .read(cx)
                    .connected_destinations()
                    .find(|captured| captured.destination == *destination)
                    .is_some_and(|captured| {
                        current.iter().any(|live| {
                            live.destination.entity == destination.entity
                                && live.destination.name == destination.name
                                && live.destination.endpoint == destination.endpoint
                                && live.destination.route == destination.route
                                && live.destination.template_context == destination.template_context
                                && live.session.same_connection(&captured.session)
                                && !live.session.is_closed()
                        })
                    })
            });
        if !valid {
            panel.update(cx, |panel, cx| panel.fail_start(cx));
            return;
        }
        panel.update(cx, |panel, cx| {
            panel.begin(review, &self.runtime, window, cx)
        });
    }
    pub(super) fn workflow_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let Some(panel) = self.workflow_panel.clone().filter(|_| self.show_workflow) else {
            return div().into_any_element();
        };
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x17243a66))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(1040.))
                    .h(px(760.))
                    .max_w_full()
                    .max_h_full()
                    .bg(rgb(visual.surface))
                    .rounded_lg()
                    .shadow_lg()
                    .overflow_hidden()
                    .child(panel),
            )
            .into_any_element()
    }
}
