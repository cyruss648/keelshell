//! Diagnostic consent follows one uninterrupted pane selection.

use super::*;

#[derive(Default)]
pub(super) struct ProtocolSelection {
    target: Option<(keelshell_core::PaneNodeId, Option<EntityId>)>,
    token: uuid::Uuid,
}

impl Workspace {
    /// Called from a leased terminal/input; do not read either entity here.
    pub(super) fn select_terminal_input(&mut self, id: EntityId, cx: &mut Context<Self>) {
        if self.active_modal().is_some() || self.connecting {
            return;
        }
        let Some(index) = self.tabs.iter().position(|tab| tab.entity_id() == id) else {
            return;
        };
        if self.active != index {
            self.cancel_remote_completion(cx);
        }
        self.active = index;
        if self.sync_session_panes() {
            self.session_panes.select(id);
            self.refresh_protocol_selection(cx);
        }
        cx.notify();
    }

    /// Observe completed Tab actions before a draw can merge focus transitions.
    pub(super) fn observe_terminal_selection(
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        let owner = window.window_handle().window_id();
        cx.observe_keystrokes(move |view, event, window, cx| {
            if window.window_handle().window_id() != owner
                || event.keystroke.key != "tab"
                || event.action.is_none()
            {
                return;
            }
            // Resolved action observers run after child leases return.
            // Ordinary terminal Tab input is consumed before this observer.
            let selected = view
                .tabs
                .iter()
                .find(|terminal| terminal.read(cx).owns_input_focus(window, cx))
                .map(Entity::entity_id);
            if let Some(id) = selected {
                view.select_terminal_input(id, cx);
            }
        })
    }

    fn protocol_selection_target(&self) -> (keelshell_core::PaneNodeId, Option<EntityId>) {
        (
            self.session_panes.active_pane(),
            self.tabs.get(self.active).map(Entity::entity_id),
        )
    }

    /// Revoke diagnostic consent in the same foreground turn as selection.
    ///
    /// A fresh identity prevents A→B→A from resurrecting a review or adopting a
    /// held reply. Closing an inactive tab retains the selected pane and token.
    pub(super) fn refresh_protocol_selection(&mut self, cx: &mut Context<Self>) {
        let target = self.protocol_selection_target();
        if self.protocol_selection.target == Some(target) {
            return;
        }
        self.protocol_selection.target = Some(target);
        self.protocol_selection.token = uuid::Uuid::new_v4();
        for monitor in self
            .panels
            .values()
            .filter_map(|panels| panels.monitor.as_ref())
        {
            monitor.update(cx, |monitor, cx| monitor.invalidate_protocol_selection(cx));
        }
    }

    /// Read-only, selection-scoped authority for explicit diagnostics.
    ///
    /// General session authority deliberately retains its existing semantics:
    /// agents and other owners can remain valid while a different pane is active.
    pub(crate) fn protocol_selection_token(
        &self,
        tab: EntityId,
        session: &SshSession,
    ) -> Option<uuid::Uuid> {
        let target = self.protocol_selection_target();
        (target.1 == Some(tab)
            && self.session_panes.active_session() == Some(tab)
            && self.protocol_selection.target == Some(target)
            && self.protocol_session_current(tab, session))
        .then_some(self.protocol_selection.token)
    }
}
