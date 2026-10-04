//! Final authorization binds parameter review and batch work to live entities.
use super::*;
use crate::{
    batch_commands::{BatchAuditDraft, BatchPanel, BatchPanelEvent, Destination, Review},
    snippet_parameters::{SnippetParameters, SnippetParametersEvent},
};

pub(super) struct ParameterTicket {
    target: EntityId,
    revision: u64,
    input: String,
    source: keelshell_core::Snippet,
}

impl Workspace {
    pub(super) fn record_batch_audit(
        &mut self,
        draft: BatchAuditDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let record = match keelshell_core::BatchAuditRecord::from_digest(
            draft.command_digest,
            library::now_seconds(),
            draft.profile_ids,
            keelshell_core::BatchAuditSummary {
                target_count: draft.target_count,
                succeeded: draft.succeeded as usize,
                failed: draft.failed as usize,
                unknown: draft.unknown as usize,
                not_started: draft.not_started as usize,
                cancelled: draft.cancelled,
                stopped_after_failure: draft.stopped_after_failure,
            },
        ) {
            Ok(record) => record,
            Err(error) => {
                self.status = Message::detail(
                    "批量审计摘要无效，未保存",
                    "Batch audit summary was invalid and was not saved",
                    &error,
                );
                cx.notify();
                return;
            }
        };
        self.pending_batch_audits.push(record);
        self.flush_batch_audits(window, cx);
    }

    pub(super) fn flush_batch_audits(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving
            || self.vault_settings.is_some()
            || self.snippet_modal_open()
            || self.pending_batch_audits.is_empty()
        {
            return;
        }
        let records = std::mem::take(&mut self.pending_batch_audits);
        let mut candidate = self.state.clone();
        for record in &records {
            if let Err(error) = candidate.record_batch_audit(record.clone()) {
                self.pending_batch_audits = records;
                self.status = Message::detail(
                    "批量审计摘要未保存",
                    "Batch audit summary was not saved",
                    &error,
                );
                cx.notify();
                return;
            }
        }
        self.persist(candidate, AfterSave::BatchAudit { records }, window, cx);
    }

    pub(super) fn open_snippet_parameters(
        &mut self,
        source: keelshell_core::Snippet,
        target: EntityId,
        revision: u64,
        input: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_remote_completion(cx);
        let label = self
            .remote_hosts
            .get(&target)
            .cloned()
            .unwrap_or_else(|| t(cx, "当前 SSH 会话", "Current SSH session").into());
        let panel = cx.new(|cx| SnippetParameters::new(source.clone(), label, window, cx));
        self.parameter_ticket = Some(ParameterTicket {
            target,
            revision,
            input,
            source,
        });
        self.parameter_subscription=Some(cx.subscribe_in(&panel,window,|view,panel,event,window,cx|match event {
            SnippetParametersEvent::Cancel=>view.close_snippet_modal(window,cx),
            SnippetParametersEvent::Rendered{snippet,text}=>{
                let ticket=view.parameter_ticket.as_ref();
                let valid=view.snippet_parameters.as_ref().is_some_and(|current|current.entity_id()==panel.entity_id())
                    && ticket.is_some_and(|ticket| ticket.source==*snippet
                        && view.state.snippets.iter().any(|source|source==snippet)
                        && view.command_revision==ticket.revision && view.command.read(cx).value().as_str()==ticket.input
                        && view.command_target.is_none_or(|id|id==ticket.target)
                        && view.tabs.get(view.active).is_some_and(|tab|tab.entity_id()==ticket.target&&tab.read(cx).is_open()));
                if !valid {
                    panel.update(cx,|panel,cx|panel.set_error(Message::new("片段、命令或目标已变化。请取消后重新选择。","Snippet, command or target changed. Cancel and select it again."),cx));
                    return;
                }
                let target=ticket.map(|ticket|ticket.target);
                view.set_reviewed_command(text.clone(),target,window,cx);
                // Parameter values are ephemeral. Opting into history later is a
                // separate visible choice; edits and reconnection retain this flag.
                view.command_record_history=false;
                view.snippet_parameters=None;view.parameter_ticket=None;view.parameter_subscription=None;
                view.command.read(cx).focus_handle(cx).focus(window,cx);
                view.status=Message::new("已填入变量命令，默认不记录历史；核对目标后再执行。","Parameterized command inserted with history disabled. Review its target before running.");
                cx.notify();
            }
        }));
        self.snippet_parameters = Some(panel);
        self.focus_current_surface(window, cx);
        cx.notify();
    }

    fn live_batch_entities(&self, cx: &App) -> std::collections::HashSet<EntityId> {
        self.tabs
            .iter()
            .filter(|tab| {
                tab.read(cx).is_open()
                    && self
                        .remote_sessions
                        .get(&tab.entity_id())
                        .is_some_and(|session| !session.is_closed())
            })
            .map(Entity::entity_id)
            .collect()
    }

    pub(super) fn maintain_command_workflows(&mut self, cx: &mut Context<Self>) {
        if let Some(panel) = self.batch_panel.clone() {
            let live = self.live_batch_entities(cx);
            panel.update(cx, |panel, cx| panel.update_available(&live, cx));
        }
    }

    pub(super) fn open_batch_commands(
        &mut self,
        fresh: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_surface_blocked() && !self.show_batch {
            return;
        }
        if self
            .batch_panel
            .as_ref()
            .is_some_and(|panel| panel.read(cx).is_running())
            && fresh
        {
            return;
        }
        self.cancel_remote_completion(cx);
        if fresh || self.batch_panel.is_none() {
            let live = self.live_batch_entities(cx);
            let destinations = self
                .tabs
                .iter()
                .filter(|tab| live.contains(&tab.entity_id()))
                .map(|tab| {
                    let entity = tab.entity_id();
                    let endpoint = self.remote_hosts.get(&entity).cloned().unwrap_or_default();
                    let profile_id = self.batch_profile_id(entity);
                    let (name, route) = self
                        .batch_route_description(entity)
                        .unwrap_or_else(|| (tab.read(cx).title.clone(), endpoint.clone()));
                    let template_context =
                        self.batch_template_context(entity).unwrap_or_else(|| {
                            keelshell_core::BatchTargetContext {
                                name: name.clone(),
                                host: endpoint.clone(),
                                endpoint: endpoint.clone(),
                                ..Default::default()
                            }
                        });
                    Destination {
                        id: uuid::Uuid::new_v4(),
                        profile_id,
                        entity,
                        name,
                        endpoint,
                        route,
                        template_context,
                    }
                })
                .collect();
            let text = self.command.read(cx).value().to_string();
            let panel = cx.new(|cx| BatchPanel::new(destinations, text, window, cx));
            self.batch_subscription =
                Some(
                    cx.subscribe_in(&panel, window, |view, panel, event, window, cx| {
                        if view
                            .batch_panel
                            .as_ref()
                            .is_none_or(|current| current.entity_id() != panel.entity_id())
                        {
                            return;
                        }
                        match event {
                            BatchPanelEvent::Start(review) => {
                                view.start_reviewed_batch(panel.clone(), review, window, cx)
                            }
                            BatchPanelEvent::Completed(audit) => {
                                view.record_batch_audit(audit.clone(), window, cx)
                            }
                            BatchPanelEvent::Hide => {
                                view.show_batch = false;
                                view.focus_current_surface(window, cx);
                                cx.notify();
                            }
                            BatchPanelEvent::New => view.open_batch_commands(true, window, cx),
                        }
                    }),
                );
            self.batch_panel = Some(panel);
        }
        self.show_batch = true;
        self.maintain_command_workflows(cx);
        self.focus_current_surface(window, cx);
        cx.notify();
    }

    fn start_reviewed_batch(
        &mut self,
        panel: Entity<BatchPanel>,
        review: &Review,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.maintain_command_workflows(cx);
        let live = self.live_batch_entities(cx);
        let valid = self.show_batch && panel.read(cx).review_current(review, cx);
        let destinations: Vec<_> = panel
            .read(cx)
            .destinations()
            .filter(|destination| review.targets.contains(&destination.id))
            .cloned()
            .collect();
        if !valid
            || destinations.len() != review.targets.len()
            || destinations.iter().any(|destination| {
                !live.contains(&destination.entity)
                    || self.remote_hosts.get(&destination.entity) != Some(&destination.endpoint)
            })
        {
            panel.update(cx,|panel,cx|panel.fail_start(Message::new("审核已失效：命令或 SSH 会话发生变化。请返回修改后重新审核。","Review expired: command or SSH session changed. Edit and review the plan again."),cx));
            return;
        }
        let targets = destinations
            .into_iter()
            .filter_map(|destination| {
                let command = review
                    .commands
                    .iter()
                    .find(|(id, _)| *id == destination.id)
                    .map(|(_, command)| command.clone())?;
                self.remote_sessions
                    .get(&destination.entity)
                    .map(|session| keelshell_session::BatchTarget {
                        id: destination.id,
                        session: session.clone(),
                        command,
                    })
            })
            .collect();
        panel.update(cx, |panel, cx| {
            panel.begin(review, targets, &self.runtime, window, cx)
        });
    }

    pub(super) fn batch_modal(&self, cx: &mut Context<Self>) -> AnyElement {
        let visual = crate::design::palette(cx);
        let Some(panel) = self.batch_panel.clone().filter(|_| self.show_batch) else {
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
                    .w(px(960.))
                    .h(px(720.))
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
