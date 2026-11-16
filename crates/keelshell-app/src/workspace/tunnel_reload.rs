//! Explicit panel reloads retain their origin until the authoritative read is adopted.

use super::*;
use crate::tunnels::{TunnelRuleReloadEvent, TunnelsPanel};

pub(super) struct TunnelReloadOrigin {
    panel: WeakEntity<TunnelsPanel>,
    event: TunnelRuleReloadEvent,
}

impl Workspace {
    pub(super) fn handle_tunnel_rule_reload(
        &mut self,
        panel: &Entity<TunnelsPanel>,
        event: &TunnelRuleReloadEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.tunnel_reload_origin_current(panel, event, cx) {
            panel.update(cx, |panel, cx| {
                panel.reject_rule_reload(
                    event,
                    Message::new(
                        "隧道面板或重新加载请求已过期，请在当前面板重试。",
                        "The tunnel panel or reload request expired. Retry in the current panel.",
                    ),
                    cx,
                )
            });
            return;
        }
        let origin = TunnelReloadOrigin {
            panel: panel.downgrade(),
            event: event.clone(),
        };
        if !self.begin_window_configuration_reload(Some(origin), window, cx) {
            panel.update(cx, |panel, cx| {
                panel.reject_rule_reload(
                    event,
                    Message::new(
                        "请先完成配置保存、连接或其他配置操作，再重新加载。",
                        "Finish configuration saves, connection setup or other configuration operations before reloading.",
                    ),
                    cx,
                )
            });
        }
    }

    fn tunnel_reload_origin_current(
        &self,
        panel: &Entity<TunnelsPanel>,
        event: &TunnelRuleReloadEvent,
        cx: &App,
    ) -> bool {
        // Archived panels and retained worker owners can clean up their original
        // transports, but cannot adopt configuration on behalf of a live tab.
        self.panels.iter().any(|(terminal, panels)| {
            panels.tunnels.as_ref() == Some(panel)
                && self.remote_connection_ids.get(terminal).copied() == event.connection_id
        }) && panel.read(cx).accepts_rule_reload(event)
    }

    pub(super) fn tunnel_reload_current(&self, origin: &TunnelReloadOrigin, cx: &App) -> bool {
        origin
            .panel
            .upgrade()
            .is_some_and(|panel| self.tunnel_reload_origin_current(&panel, &origin.event, cx))
    }

    pub(super) fn finish_tunnel_reload(
        &mut self,
        origin: &TunnelReloadOrigin,
        message: Message,
        cx: &mut Context<Self>,
    ) {
        if let Some(panel) = origin.panel.upgrade() {
            panel.update(cx, |panel, cx| {
                panel.finish_rule_reload(&origin.event, message, cx);
            });
        }
    }
}
