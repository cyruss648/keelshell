//! Bounded observations; they never authorize actions or represent a reply.

use std::sync::{
    atomic::{AtomicU8, Ordering},
    mpsc::{self, Receiver, SyncSender, TryRecvError},
};

/// An observed fact from one local Ask invocation, without supplier text or secrets.
///
/// Input delivery and protocol reads run concurrently. These facts are not a
/// percentage or a total ordering, and a later fact does not imply earlier ones
/// were observed. Only the final Ask result can confirm successful completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LocalAskStage {
    /// A fresh owned temporary workspace and isolated configuration were created.
    WorkspaceReady,
    /// The request entered its fresh version and capability admission checks.
    CheckingCli,
    /// Both version and effective capabilities passed the supported Ask policy.
    CliAdmitted,
    /// The isolated inference child was actually spawned.
    ProcessStarted,
    /// The complete reviewed stdin and terminating newline were written and closed.
    /// This does not prove the supplier or inference service consumed the input.
    InputDelivered,
    /// The adapter validated the supported Ask protocol's start receipt.
    ProtocolStarted,
    /// The adapter validated a supported protocol end receipt.
    /// Later output, exit or cleanup can still fail; this is not a successful reply.
    ProtocolCompleted,
    /// The request entered owned process or scratch cleanup, without confirming it.
    Finalizing,
}

impl LocalAskStage {
    /// The eight possible observations; this order is for presentation only.
    pub const ALL: [Self; 8] = [
        Self::WorkspaceReady,
        Self::CheckingCli,
        Self::CliAdmitted,
        Self::ProcessStarted,
        Self::InputDelivered,
        Self::ProtocolStarted,
        Self::ProtocolCompleted,
        Self::Finalizing,
    ];
}

/// A single-invocation progress producer consumed by `ask_with_progress`.
///
/// It emits each static fact at most once through nonblocking `try_send`. A
/// stopped, slow or dropped consumer cannot block pipe drainage or cleanup. No
/// supplier frame, answer, path, context, credential or diagnostic is included.
pub struct LocalAskProgress {
    sender: SyncSender<LocalAskStage>,
    observed: AtomicU8,
}

impl LocalAskProgress {
    /// Create an inert observation channel without processes, files or network I/O.
    ///
    /// Sixteen slots exceed the eight possible once-only facts, so an unread
    /// receiver cannot fill the channel during a supported invocation. The
    /// producer is intentionally not cloneable or reusable between requests.
    pub fn channel() -> (Self, LocalAskProgressReceiver) {
        let (sender, receiver) = mpsc::sync_channel(16);
        (
            Self {
                sender,
                observed: AtomicU8::new(0),
            },
            LocalAskProgressReceiver { receiver },
        )
    }

    pub(super) fn observe(&self, stage: LocalAskStage) {
        let bit = 1_u8 << stage as u8;
        if self.observed.fetch_or(bit, Ordering::Relaxed) & bit == 0 {
            // Observations are optional. In particular, a closed UI receiver
            // cannot turn a valid request into an error or delay cancellation.
            let _ = self.sender.try_send(stage);
        }
    }
}

/// A bounded, nonblocking receiver owned by the caller's exact request.
///
/// Dropping it discards observations but does not cancel the Ask invocation.
/// The caller still owns its [`crate::RequestCancellation`] token and final result.
pub struct LocalAskProgressReceiver {
    receiver: Receiver<LocalAskStage>,
}

impl LocalAskProgressReceiver {
    /// Read one observed fact immediately, without registering a foreign executor waker.
    pub fn try_recv(&self) -> Result<LocalAskStage, TryRecvError> {
        self.receiver.try_recv()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unread_channel_is_bounded_by_once_only_static_facts() {
        let (progress, receiver) = LocalAskProgress::channel();
        for _ in 0..1024 {
            for stage in LocalAskStage::ALL {
                progress.observe(stage);
            }
        }
        let facts: Vec<_> = std::iter::from_fn(|| receiver.try_recv().ok()).collect();
        assert_eq!(facts, LocalAskStage::ALL);
        assert_eq!(receiver.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn closed_receiver_never_blocks_observations() {
        let (progress, receiver) = LocalAskProgress::channel();
        drop(receiver);
        for stage in LocalAskStage::ALL {
            progress.observe(stage);
        }
    }
}
