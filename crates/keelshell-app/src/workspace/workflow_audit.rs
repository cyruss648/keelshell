//! Metadata saving only; none of these paths can admit or restore a workflow.
use super::*;
use keelshell_core::WorkflowAuditRecord;

impl Workspace {
    pub(super) fn refresh_workflow_audit_history(&self, cx: &mut Context<Self>) {
        if let Some(panel) = self.workflow_panel.as_ref() {
            panel.update(cx, |panel, cx| {
                panel.set_audit_history(
                    &self.state.workflow_audits,
                    &self.pending_workflow_audits,
                    cx,
                )
            });
        }
    }

    pub(super) fn record_workflow_audit(
        &mut self,
        record: WorkflowAuditRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let conflict = self.pending_workflow_audits.iter().any(|pending| {
            if pending.id == record.id { return pending != &record; }
            matches!((pending.trigger, record.trigger),
                (keelshell_core::WorkflowAuditTrigger::Scheduled {schedule_id:a,occurrence:i,..},
                 keelshell_core::WorkflowAuditTrigger::Scheduled {schedule_id:b,occurrence:j,..}) if a==b && i==j)
        });
        if conflict {
            self.status = Message::new(
                "任务记录回执冲突，原结果已保留",
                "Task history receipt conflict; original result retained",
            );
            cx.notify();
            return;
        }
        // Validate against both the persisted ledger and pending receipts before
        // changing the queue. This prevents conflicting retries from replacing
        // an earlier result and makes save failures idempotent.
        let mut candidate = self.state.clone();
        let validation = self
            .pending_workflow_audits
            .iter()
            .chain(std::iter::once(&record))
            .try_for_each(|entry| candidate.record_workflow_audit(entry.clone()));
        if let Err(error) = validation {
            self.status = Message::detail(
                "任务记录校验失败，未保存",
                "Task history validation failed; not saved",
                error,
            );
            cx.notify();
            return;
        }
        if !self
            .pending_workflow_audits
            .iter()
            .any(|pending| pending.id == record.id)
            && !self
                .state
                .workflow_audits
                .iter()
                .any(|saved| saved.id == record.id)
        {
            if self.pending_workflow_audits.len() >= keelshell_core::MAX_WORKFLOW_AUDITS {
                // The panel reserves all 32 occurrences before admission, so
                // this is an invariant violation rather than a silent eviction.
                self.status = Message::new(
                    "任务记录待保存队列已满，本条未保存",
                    "Pending task history is full; this result was not saved",
                );
                cx.notify();
                return;
            }
            self.pending_workflow_audits.push(record);
        }
        self.refresh_workflow_audit_history(cx);
        self.flush_workflow_audits(window, cx);
    }

    pub(super) fn flush_workflow_audits(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving
            || self.vault_settings.is_some()
            || self.profile_sync.is_some()
            || self.snippet_modal_open()
            || self.pending_workflow_audits.is_empty()
        {
            return;
        }
        let records = self.pending_workflow_audits.clone();
        let mut candidate = self.state.clone();
        for record in &records {
            if let Err(error) = candidate.record_workflow_audit(record.clone()) {
                self.status = Message::detail(
                    "任务记录尚未保存，请重试保存",
                    "Task history remains unsaved; retry saving",
                    error,
                );
                self.refresh_workflow_audit_history(cx);
                cx.notify();
                return;
            }
        }
        // Keep the originals pending until the actual StateStore save succeeds.
        // They survive modal closure and panel replacement, but not process exit.
        self.persist(candidate, AfterSave::WorkflowAudit { records }, window, cx);
    }
}
