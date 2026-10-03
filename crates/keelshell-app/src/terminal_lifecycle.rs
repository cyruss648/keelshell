//! Lifecycle notifications bypass bounded terminal byte queues.
use super::*;
use keelshell_session::{ConnectionEnd, ShellEnd};
use tokio::sync::watch;

#[derive(Clone)]
pub(crate) enum TransportState {
    Starting,
    Ready,
    Ended(ShellEnd),
}
#[derive(Default)]
pub(crate) struct Lifecycle {
    source: Option<watch::Receiver<TransportState>>,
    ended: Option<ShellEnd>,
}
impl TerminalView {
    pub(crate) fn attach_lifecycle(&mut self, mut source: watch::Receiver<TransportState>) {
        source.mark_changed();
        self.lifecycle.source = Some(source);
    }
    /// A typed cause only; display strings and cleanup warnings never decide reconnect eligibility.
    pub(crate) fn end_reason(&self) -> Option<ShellEnd> {
        self.lifecycle
            .ended
            .clone()
            .or_else(|| match &*self.lifecycle.source.as_ref()?.borrow() {
                TransportState::Ended(reason) => Some(reason.clone()),
                _ => None,
            })
    }
    /// History can move only after the old bridge has delivered all output and
    /// normal frame-budgeted parsing has consumed it. Never drain on the UI thread
    /// without its ordinary byte/time allowance just to accelerate reconnect.
    pub(crate) fn ready_to_archive(&self) -> bool {
        self.end_reason().is_some()
            && self.output.is_drained()
            && self
                .completion
                .as_ref()
                .is_none_or(|completion| completion.lock().is_ok_and(|result| result.is_some()))
    }
    pub(crate) fn take_reconnect_history(&mut self) -> crate::emulator::Emulator {
        self.cancelled.store(true, Ordering::Release);
        self.pending = InputQueue::default();
        self.composition.clear();
        self.output = OutputQueue::default();
        self.exited = true;
        self.emulator.take_for_next_session()
    }
    pub(crate) fn inherit_reconnect_history(
        &mut self,
        history: crate::emulator::Emulator,
        cx: &mut Context<Self>,
    ) {
        self.emulator = history;
        self.invalidate_search_feedback();
        self.pending.resize(
            self.emulator.size.rows.min(u16::MAX as usize) as u16,
            self.emulator.size.cols.min(u16::MAX as usize) as u16,
        );
        self.flush_input();
        cx.notify();
    }
    pub(super) fn lifecycle_ready(&self) -> bool {
        self.lifecycle
            .source
            .as_ref()
            .is_none_or(|source| matches!(*source.borrow(), TransportState::Ready))
    }
    pub(super) fn lifecycle_managed(&self) -> bool {
        self.lifecycle.source.is_some()
    }
    pub(super) fn poll_lifecycle(&mut self) -> bool {
        let Some(source) = &mut self.lifecycle.source else {
            return false;
        };
        let state = match source.has_changed() {
            Ok(true) => source.borrow_and_update().clone(),
            Ok(false) => return false,
            Err(_) if self.lifecycle.ended.is_none() => match source.borrow().clone() {
                state @ TransportState::Ended(_) => state,
                _ => TransportState::Ended(ShellEnd::ConnectionClosed(ConnectionEnd::Unknown)),
            },
            Err(_) => return false,
        };
        match state {
            TransportState::Starting => {}
            TransportState::Ready if self.lifecycle.ended.is_none() => {
                self.started = true;
                self.status = Message::new("已连接", "Connected");
            }
            TransportState::Ready => {}
            TransportState::Ended(reason) => {
                self.status = end_message(&reason);
                self.lifecycle.ended = Some(reason);
                self.exited = true;
                self.started = true;
                // Pending input and parser replies belong to the old shell. No
                // future transport may inherit or replay any of these bytes.
                self.pending = InputQueue::default();
                self.composition.clear();
            }
        }
        true
    }
}
fn end_message(reason: &ShellEnd) -> Message {
    match reason {
        ShellEnd::Exited { code } => {
            Message::new(format!("会话已退出（{code}）"), format!("Exited ({code})"))
        }
        ShellEnd::Signalled { signal } => Message::new(
            format!("远程进程已结束（{signal}）"),
            format!("Remote process ended ({signal})"),
        ),
        ShellEnd::ChannelClosed => Message::new(
            "远程终端已关闭（未提供退出状态）",
            "Remote shell closed without an exit status",
        ),
        ShellEnd::Cancelled | ShellEnd::ConnectionClosed(ConnectionEnd::LocalClosed) => {
            Message::new("连接已关闭", "Connection closed")
        }
        ShellEnd::ConnectionClosed(ConnectionEnd::TransportLost) => {
            Message::new("SSH 连接已断开", "SSH connection lost")
        }
        ShellEnd::ConnectionClosed(ConnectionEnd::KeepaliveTimeout) => {
            Message::new("SSH 连接无响应", "SSH keepalive timed out")
        }
        ShellEnd::ConnectionClosed(ConnectionEnd::RemoteDisconnected { code }) => Message::new(
            format!("服务器已断开连接（{code}）"),
            format!("Server disconnected ({code})"),
        ),
        ShellEnd::ConnectionClosed(ConnectionEnd::ProtocolFailure) => Message::new(
            "SSH 协议连接已关闭",
            "SSH connection ended with a protocol error",
        ),
        ShellEnd::ConnectionClosed(ConnectionEnd::Unknown) => Message::new(
            "SSH 连接已结束，原因未知",
            "SSH connection ended; cause unknown",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectionEnd, ShellEnd, TerminalView, TransportState, end_message};
    use crate::i18n::Message;
    use gpui_kit::{AppContext, TestAppContext};
    use keelshell_session::SessionEvent;
    use std::sync::mpsc;
    use std::sync::{Arc, atomic::AtomicBool};
    use tokio::sync::watch;

    #[gpui_kit::test]
    fn quiet_ready_and_typed_end_do_not_depend_on_byte_events(cx: &mut TestAppContext) {
        let (_output, incoming) = mpsc::sync_channel(1);
        let (outgoing, _commands) = mpsc::sync_channel(1);
        let (sender, state) = watch::channel(TransportState::Starting);
        let terminal = cx.new(|cx| {
            let mut view = TerminalView::from_transport(
                "test".into(),
                13.,
                100,
                incoming,
                outgoing,
                Arc::new(AtomicBool::new(false)),
                cx,
            );
            view.attach_lifecycle(state);
            view
        });
        terminal.update(cx, |view, cx| {
            view.poll(cx);
            assert!(!view.is_open());
            assert_eq!(view.end_reason(), None);
        });
        sender.send_replace(TransportState::Ready);
        terminal.update(cx, |view, cx| {
            view.poll(cx);
            assert!(view.is_open());
            assert_eq!(view.status, Message::new("已连接", "Connected"));
        });
        sender.send_replace(TransportState::Ended(ShellEnd::ConnectionClosed(
            ConnectionEnd::TransportLost,
        )));
        terminal.update(cx, |view, cx| {
            view.poll(cx);
            assert!(!view.is_open());
            assert_eq!(
                view.end_reason(),
                Some(ShellEnd::ConnectionClosed(ConnectionEnd::TransportLost))
            );
        });
    }

    #[gpui_kit::test]
    fn ended_terminal_discards_parser_replies_without_overwriting_reason(cx: &mut TestAppContext) {
        let (output, incoming) = mpsc::sync_channel(4);
        let (outgoing, commands) = mpsc::sync_channel(4);
        let (sender, state) = watch::channel(TransportState::Ready);
        let terminal = cx.new(|cx| {
            let mut view = TerminalView::from_transport(
                "test".into(),
                13.,
                100,
                incoming,
                outgoing,
                Arc::new(AtomicBool::new(false)),
                cx,
            );
            view.attach_lifecycle(state);
            view
        });
        sender.send_replace(TransportState::Ended(ShellEnd::Exited { code: 17 }));
        assert!(
            output
                .send(SessionEvent::Data(
                    b"tail\x1b[6n\x1b[c\x1b[18t\x1b]10;?\x07".to_vec()
                ))
                .is_ok()
        );
        terminal.update(cx, |view, cx| {
            for _ in 0..4 {
                view.poll(cx);
            }
            assert_eq!(view.status, end_message(&ShellEnd::Exited { code: 17 }));
            assert_eq!(view.end_reason(), Some(ShellEnd::Exited { code: 17 }));
            assert!(!view.is_open());
            assert!(commands.try_recv().is_err());
        });
    }

    #[gpui_kit::test]
    fn worker_cleanup_warning_cannot_change_typed_normal_exit(cx: &mut TestAppContext) {
        let (_output, incoming) = mpsc::sync_channel(1);
        let (outgoing, _commands) = mpsc::sync_channel(1);
        let (_sender, state) = watch::channel(TransportState::Ended(ShellEnd::Exited { code: 0 }));
        let terminal = cx.new(|cx| {
            let mut view = TerminalView::from_transport(
                "test".into(),
                13.,
                100,
                incoming,
                outgoing,
                Arc::new(AtomicBool::new(false)),
                cx,
            );
            view.attach_lifecycle(state);
            view.completion = Some(Arc::new(std::sync::Mutex::new(Some(Err(
                "SSH_SHELL_CLEANUP_TIMEOUT".into(),
            )))));
            view
        });
        terminal.update(cx, |view, cx| {
            view.poll(cx);
            assert_eq!(view.end_reason(), Some(ShellEnd::Exited { code: 0 }));
            assert!(!view.is_open());
            assert!(view.status.render(cx).contains("超时"));
        });
    }
    #[gpui_kit::test]
    fn archive_waits_for_budgeted_tail_parsing_and_worker_completion(cx: &mut TestAppContext) {
        let (output, incoming) = mpsc::sync_channel(4);
        let (outgoing, _commands) = mpsc::sync_channel(1);
        let (_sender, state) = watch::channel(TransportState::Ended(ShellEnd::Exited { code: 0 }));
        let completion = Arc::new(std::sync::Mutex::new(None));
        let observed = completion.clone();
        let terminal = cx.new(|cx| {
            let mut view = TerminalView::from_transport(
                "test".into(),
                13.,
                100,
                incoming,
                outgoing,
                Arc::new(AtomicBool::new(false)),
                cx,
            );
            view.attach_lifecycle(state);
            view.completion = Some(observed);
            view
        });
        let mut bytes = vec![b'x'; 2 * super::FRAME_BYTES + 17];
        bytes.extend_from_slice(b"\r\nARCHIVE-TAIL");
        assert!(output.send(SessionEvent::Data(bytes)).is_ok());
        drop(output);
        terminal.update(cx, |view, cx| {
            view.poll(cx);
            assert!(!view.ready_to_archive());
            for _ in 0..100 {
                view.poll(cx);
            }
            assert!(view.visible_text().contains("ARCHIVE-TAIL"));
            assert!(!view.ready_to_archive());
            if let Ok(mut result) = completion.lock() {
                *result = Some(Ok(()));
            }
            view.poll(cx);
            assert!(view.ready_to_archive());
        });
    }
}
