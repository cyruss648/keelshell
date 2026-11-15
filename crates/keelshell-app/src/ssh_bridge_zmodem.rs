//! One PTY write lane, bounded protocol effects and an explicit input handoff.
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use keelshell_session::{
    SessionEvent, SshSession,
    zmodem::{ZmodemControl, ZmodemController, ZmodemFailure, ZmodemOutput, ZmodemUiEvent},
};
use uuid::Uuid;

use crate::terminal::ZmodemSafety;

const MAX_OUTPUT_EVENTS: usize = 256;
const MAX_PROTOCOL_WRITES: usize = 256;
const MAX_PROTOCOL_PENDING_BYTES: usize = 1024 * 1024;

pub(super) struct Protocol {
    controller: ZmodemController,
    pub(super) safety: Arc<ZmodemSafety>,
    pub(super) output: VecDeque<SessionEvent>,
    writes: VecDeque<Vec<u8>>,
    pending_bytes: usize,
    blocked: Option<Uuid>,
    input_rejection_seen: bool,
    resume_requested: bool,
    write_in_flight: bool,
    finished: Option<Uuid>,
    uploading: bool,
    interrupted_input: bool,
}

impl Protocol {
    pub(super) fn new(session: SshSession, target: String) -> Self {
        let controller = ZmodemController::new(session, target);
        let safety = Arc::new(ZmodemSafety::new(controller.publication_running()));
        Self {
            controller,
            safety,
            output: VecDeque::new(),
            writes: VecDeque::new(),
            pending_bytes: 0,
            blocked: None,
            input_rejection_seen: false,
            resume_requested: false,
            write_in_flight: false,
            finished: None,
            uploading: false,
            interrupted_input: false,
        }
    }

    pub(super) fn input_blocked(&self) -> bool {
        self.blocked.is_some() || self.controller.is_active()
    }

    pub(super) fn wants_input(&self) -> bool {
        self.output.is_empty() && self.pending_bytes < MAX_PROTOCOL_PENDING_BYTES / 2
    }

    pub(super) fn pop_write(&mut self) -> Option<Vec<u8>> {
        let bytes = self.writes.pop_front()?;
        self.pending_bytes = self.pending_bytes.saturating_sub(bytes.len());
        self.write_in_flight = true;
        Some(bytes)
    }

    pub(super) fn write_completed(&mut self) -> Result<(), String> {
        self.write_in_flight = false;
        self.finish_if_flushed()?;
        self.release_input();
        Ok(())
    }

    pub(super) async fn feed(
        &mut self,
        session: &SshSession,
        event: SessionEvent,
    ) -> Result<(), String> {
        match event {
            SessionEvent::Data(bytes) => {
                let effects = self.controller.feed(session, &bytes).await;
                self.effects(effects)
            }
            event => {
                let effects = self.controller.transport_failed();
                self.effects(effects)?;
                self.push_event(event)
            }
        }
    }

    pub(super) async fn control(
        &mut self,
        session: &SshSession,
        control: ZmodemControl,
    ) -> Result<(), String> {
        let effects = self.controller.control(session, control).await;
        self.effects(effects)
    }

    pub(super) fn tick(&mut self) -> Result<(), String> {
        let effects = self.controller.tick();
        self.effects(effects)
    }

    pub(super) fn fail(&mut self) -> Result<(), String> {
        if let Some(id) = self.finished.take() {
            self.safety.publish(ZmodemUiEvent::Failed {
                id,
                reason: ZmodemFailure::ConnectionClosed,
                uncertain: self.uploading
                    || self.controller.mutation_pending().load(Ordering::Acquire),
            })?;
        }
        let mut effects = self.controller.transport_failed();
        if self.interrupted_input
            && let Some(id) = self.blocked
        {
            effects.events.retain(|event| {
                !matches!(
                    event,
                    ZmodemUiEvent::Failed { .. }
                        | ZmodemUiEvent::Cancelled { .. }
                        | ZmodemUiEvent::Finished { .. }
                )
            });
            effects.events.push(ZmodemUiEvent::Failed {
                id,
                reason: ZmodemFailure::Protocol,
                uncertain: true,
            });
            self.interrupted_input = false;
        }
        self.effects(effects)
    }

    pub(super) fn interrupt_ordinary_write(&mut self) {
        self.interrupted_input = true;
    }

    pub(super) fn reject_input(&mut self) -> Result<(), String> {
        if !self.input_rejection_seen
            && let Some(id) = self.blocked
        {
            self.input_rejection_seen = true;
            self.safety.publish(ZmodemUiEvent::InputRejected { id })?;
        }
        Ok(())
    }

    pub(super) fn resume_input(&mut self, id: Uuid) {
        if self.blocked == Some(id) && !self.controller.is_active() {
            self.resume_requested = true;
            self.release_input();
        }
    }

    pub(super) fn transport_closed(&mut self) {
        self.blocked = None;
        self.writes.clear();
        self.pending_bytes = 0;
        self.write_in_flight = false;
        self.safety.active.store(false, Ordering::Release);
    }

    fn release_input(&mut self) {
        if self.resume_requested
            && !self.controller.is_active()
            && self.writes.is_empty()
            && !self.write_in_flight
            && self.finished.is_none()
        {
            self.blocked = None;
            self.input_rejection_seen = false;
            self.resume_requested = false;
            self.safety.active.store(false, Ordering::Release);
        }
    }

    fn effects(&mut self, effects: ZmodemOutput) -> Result<(), String> {
        if let Some(lease) = self.controller.lease_id() {
            if self.blocked != Some(lease) {
                self.input_rejection_seen = false;
                self.resume_requested = false;
            }
            self.blocked = Some(lease);
            self.safety.active.store(true, Ordering::Release);
        }
        if !effects.terminal.is_empty() {
            self.push_event(SessionEvent::Data(effects.terminal))?;
        }
        for event in effects.events {
            if let ZmodemUiEvent::Finished { id } = event {
                // The codec can request its final OO trailer and report
                // Finished in one step. Do not publish that result until the
                // actual SSH write completes; a failed write stays a failure.
                self.finished = Some(id);
                continue;
            }
            if matches!(event, ZmodemUiEvent::UploadRequested { .. }) {
                self.uploading = true;
            } else if matches!(event, ZmodemUiEvent::DownloadOffered { .. }) {
                self.uploading = false;
            }
            if let ZmodemUiEvent::UploadRequested { id }
            | ZmodemUiEvent::DownloadOffered { id, .. } = &event
            {
                if self.blocked != Some(*id) {
                    self.input_rejection_seen = false;
                    self.resume_requested = false;
                }
                self.blocked = Some(*id);
                self.safety.active.store(true, Ordering::Release);
            }
            // Results have a bounded, shared mailbox: a stopped bridge cannot
            // strand its terminal UI behind a full byte-output receiver.
            self.safety.publish(event)?;
        }
        for bytes in effects.writes {
            if bytes.is_empty() {
                continue;
            }
            self.pending_bytes = self
                .pending_bytes
                .checked_add(bytes.len())
                .filter(|bytes| *bytes <= MAX_PROTOCOL_PENDING_BYTES)
                .ok_or("ZMODEM_PROTOCOL_OUTPUT_LIMIT")?;
            if self.writes.len() >= MAX_PROTOCOL_WRITES {
                return Err("ZMODEM_PROTOCOL_OUTPUT_LIMIT".into());
            }
            self.writes.push_back(bytes);
        }
        self.finish_if_flushed()?;
        Ok(())
    }

    fn finish_if_flushed(&mut self) -> Result<(), String> {
        if self.writes.is_empty()
            && !self.write_in_flight
            && let Some(id) = self.finished.take()
        {
            self.safety.publish(ZmodemUiEvent::Finished { id })?;
        }
        Ok(())
    }

    fn push_event(&mut self, event: SessionEvent) -> Result<(), String> {
        // Progress is observational. Coalesce only adjacent observations of the
        // same transfer; never drop an offer, review, result or terminal byte.
        if let SessionEvent::Zmodem(ZmodemUiEvent::Progress { id, .. }) = &event
            && matches!(self.output.back(), Some(SessionEvent::Zmodem(ZmodemUiEvent::Progress { id: previous, .. })) if previous == id)
        {
            self.output.pop_back();
        }
        if self.output.len() >= MAX_OUTPUT_EVENTS {
            return Err("ZMODEM_UI_OUTPUT_LIMIT".into());
        }
        self.output.push_back(event);
        Ok(())
    }
}
