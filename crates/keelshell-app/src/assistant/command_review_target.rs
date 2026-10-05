//! Captured response scope and actionable reasons beside manual review controls.
use gpui_kit::*;

use super::AssistantPanel;
use crate::i18n::{Message, t};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ReviewTargetState {
    Missing,
    Incomplete,
    Changed,
    Available,
}

impl AssistantPanel {
    pub(super) fn review_target_state(&self) -> ReviewTargetState {
        let Some((host, session)) = self.response_target.as_ref() else {
            return ReviewTargetState::Missing;
        };
        if host.trim().is_empty() && session.trim().is_empty() {
            ReviewTargetState::Missing
        } else if host.trim().is_empty()
            || session.trim().is_empty()
            || host.contains('\0')
            || session.contains('\0')
        {
            ReviewTargetState::Incomplete
        } else if host != &self.host || session != &self.session_id {
            ReviewTargetState::Changed
        } else {
            ReviewTargetState::Available
        }
    }

    pub(super) fn review_target_unavailable(&self) -> Option<Message> {
        if self.busy {
            return Some(Message::new(
                "请求尚未完成，请等待最终回复后再审核。",
                "The request is still pending. Wait for the final reply before reviewing.",
            ));
        }
        match self.review_target_state() {
            ReviewTargetState::Missing => Some(Message::new(
                "无法送入：此回复未绑定 SSH 主机和会话。请先主动选择终端上下文，再重新生成回复。",
                "Cannot send: this reply has no SSH host or session. Select terminal context explicitly, then generate a new reply.",
            )),
            ReviewTargetState::Incomplete => Some(Message::new(
                "无法送入：此回复的主机或会话信息不完整或无效。请重新选择终端上下文并生成回复。",
                "Cannot send: this reply's host or session is incomplete or invalid. Select terminal context again and generate a new reply.",
            )),
            ReviewTargetState::Changed => Some(Message::new(
                "无法送入：此回复绑定的目标已变化。请重新选择终端上下文并生成回复。",
                "Cannot send: this reply's captured target has changed. Select terminal context again and generate a new reply.",
            )),
            ReviewTargetState::Available => None,
        }
    }

    pub(super) fn review_target_view(&self, id: ElementId, cx: &App) -> AnyElement {
        let visual = crate::design::palette(cx);
        // Read only the target captured for this reply. An active terminal is
        // checked separately by Workspace when the user actually clicks send.
        let label = match self.response_target.as_ref() {
            Some((host, session)) if !host.trim().is_empty() || !session.trim().is_empty() => {
                format!(
                    "{}: {} · {}: {}",
                    t(cx, "此回复的 SSH 目标", "SSH target for this reply"),
                    if host.trim().is_empty() {
                        t(cx, "缺少主机", "Missing host")
                    } else {
                        host
                    },
                    t(cx, "会话", "Session"),
                    if session.trim().is_empty() {
                        t(cx, "缺少会话", "Missing session")
                    } else {
                        session
                    },
                )
            }
            _ => t(
                cx,
                "此回复未绑定 SSH 审核目标",
                "This reply has no captured SSH review target",
            )
            .to_owned(),
        };
        let reason = self
            .review_target_unavailable()
            .map(|message| message.render(cx));
        let accessible = reason
            .as_ref()
            .map_or_else(|| label.clone(), |reason| format!("{label}. {reason}"));
        let mut view = div()
            .id(id)
            .test_support()
            .aria_label(accessible)
            .flex()
            .flex_col()
            .gap_1()
            .text_xs()
            .text_color(rgb(visual.muted))
            .child(label);
        if let Some(reason) = reason {
            view = view.child(div().text_color(rgb(visual.danger)).child(reason));
        }
        view.into_any_element()
    }

    #[cfg(test)]
    pub(crate) fn receive_review_response_for_test(
        &mut self,
        text: String,
        cx: &mut Context<Self>,
    ) {
        self.finish_reply(
            self.request_revision,
            (self.host.clone(), self.session_id.clone()),
            Ok(text),
            cx,
        );
    }
}
