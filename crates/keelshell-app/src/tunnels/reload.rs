//! Explicit configuration reload requests bound to their originating panel state.

use super::*;

/// One explicit reload request; the Workspace owns admission and StateStore I/O.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct TunnelRuleReloadEvent {
    pub(crate) operation: uuid::Uuid,
    pub(crate) connection_id: Option<uuid::Uuid>,
    pub(crate) epoch: uuid::Uuid,
}

impl TunnelsPanel {
    pub(super) fn rule_reload_button(&self, cx: &mut Context<Self>) -> Button {
        let epoch = self.rule_epoch;
        Button::new("reload-tunnel-configuration")
            .ghost()
            .compact()
            .icon(IconName::RefreshCw)
            .label(t(cx, "重新加载", "Reload"))
            .accessibility_label(t(cx, "重新加载本机配置", "Reload local configuration"))
            .localized_tooltip(
                "重新加载本机配置；保留草稿并撤销旧审核",
                "Reload local configuration; retain drafts and expire previous reviews",
            )
            .disabled(!self.accept_rule_action(epoch))
            .on_click(cx.listener(move |view, _, _, cx| view.request_rule_reload(epoch, cx)))
    }

    pub(super) fn request_rule_reload(&mut self, epoch: uuid::Uuid, cx: &mut Context<Self>) {
        if !self.accept_rule_action(epoch) {
            return;
        }
        self.rule_plan = None;
        self.rule_stop_review = None;
        self.recovery_review = None;
        self.clear_change_review(cx);
        // Clearing the review rotates its epoch. The request must bind to that
        // new state while the editor retains its original snapshot and text.
        let event = TunnelRuleReloadEvent {
            operation: uuid::Uuid::new_v4(),
            connection_id: self.connection_id,
            epoch: self.rule_epoch,
        };
        self.rule_reload_pending = Some(event.clone());
        self.sync_rule_editor_disabled(cx);
        self.rule_status = Message::new(
            "正在重新加载配置，草稿已保留，旧审核已撤销…",
            "Reloading configuration; draft retained and previous reviews expired…",
        );
        cx.emit(event);
        cx.notify();
    }

    /// Validate the exact pending request without rejecting its admitted busy phase.
    pub(crate) fn accepts_rule_reload(&self, event: &TunnelRuleReloadEvent) -> bool {
        !self.suspended
            && self.rule_mutation.is_none()
            && self.rule_epoch == event.epoch
            && self.connection_id == event.connection_id
            && self.rule_reload_pending.as_ref() == Some(event)
    }

    /// Finish only a still-current request; adopting a snapshot remains separate.
    ///
    /// This never resolves uncertain storage. Only an authoritative reload of
    /// the panel's state can clear that block.
    pub(crate) fn finish_rule_reload(
        &mut self,
        event: &TunnelRuleReloadEvent,
        message: Message,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_rule_reload(event) {
            return false;
        }
        self.rule_reload_pending = None;
        self.sync_rule_editor_disabled(cx);
        self.rule_status = message;
        cx.notify();
        true
    }

    /// Retire a matching rejected request even if its epoch has since expired.
    ///
    /// A delayed rejection cannot clear a replacement request or unblock storage.
    pub(crate) fn reject_rule_reload(
        &mut self,
        event: &TunnelRuleReloadEvent,
        message: Message,
        cx: &mut Context<Self>,
    ) {
        if self.rule_reload_pending.as_ref() != Some(event) {
            return;
        }
        self.rule_reload_pending = None;
        self.sync_rule_editor_disabled(cx);
        self.rule_status = message;
        cx.notify();
    }
}
