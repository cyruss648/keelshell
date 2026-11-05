//! Optional request-local classifications, without logging or user data.

use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
};

use super::LocalAgentError;

const EVENT_LIMIT: u8 = 16;

/// A static classification of a local Ask failure, without nested error data.
///
/// In particular, [`LocalAgentError::Context`] becomes [`Self::Context`]; neither
/// the inner AI error nor its debug/display representation is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocalAskErrorKind {
    /// Invalid executable or scratch configuration.
    InvalidConfiguration,
    /// Unsupported executable format or launcher.
    UnsupportedExecutable,
    /// Invalid inference endpoint.
    InvalidEndpoint,
    /// Invalid request limits.
    InvalidLimits,
    /// Executable startup failed.
    SpawnFailed,
    /// Scratch preparation or removal failed.
    ScratchFailed,
    /// Invalid selected directory.
    DirectoryInvalid,
    /// Selected directory is missing.
    DirectoryMissing,
    /// A selected path component is not a directory.
    DirectoryNotDirectory,
    /// A selected path or metadata entry is a symbolic link.
    DirectorySymlink,
    /// Directory access or identity validation failed.
    DirectoryUnavailable,
    /// Reviewed directory identity changed.
    DirectoryChanged,
    /// Invalid project metadata.
    DirectoryMetadataInvalid,
    /// Reviewed project metadata changed.
    DirectoryMetadataChanged,
    /// Selected directory requires explicit review.
    DirectoryReviewRequired,
    /// Native directory anchoring is unsupported.
    DirectoryUnsupported,
    /// Directory validation exceeded its deadline.
    DirectoryValidationTimedOut,
    /// Reviewed executable changed.
    ExecutableChanged,
    /// CLI version is outside the admitted set.
    UnsupportedVersion,
    /// Required Ask isolation capabilities are unavailable.
    IsolationUnsupported,
    /// No valid ephemeral credential was supplied.
    MissingCredential,
    /// Credential was found in context or other reviewed metadata.
    CredentialInContext,
    /// Human approval expired.
    ReviewExpired,
    /// Local request deadline expired.
    Timeout,
    /// Local request cancellation was observed.
    Cancelled,
    /// A process pipe operation failed.
    PipeFailed,
    /// Output exceeded a configured bound.
    OutputTooLarge,
    /// Output violated the supported protocol.
    InvalidProtocol,
    /// Output advertised an operation outside Ask scope.
    UnexpectedOperation,
    /// The inference service reported failure.
    InferenceFailed,
    /// The process exited unsuccessfully.
    ProcessFailed,
    /// Owned process cleanup was not confirmed.
    CleanupFailed,
    /// No complete textual answer was returned.
    EmptyReply,
    /// Context preparation failed; the inner error is deliberately omitted.
    Context,
}

impl From<&LocalAgentError> for LocalAskErrorKind {
    fn from(error: &LocalAgentError) -> Self {
        match error {
            LocalAgentError::InvalidConfiguration => Self::InvalidConfiguration,
            LocalAgentError::UnsupportedExecutable => Self::UnsupportedExecutable,
            LocalAgentError::InvalidEndpoint => Self::InvalidEndpoint,
            LocalAgentError::InvalidLimits => Self::InvalidLimits,
            LocalAgentError::SpawnFailed => Self::SpawnFailed,
            LocalAgentError::ScratchFailed => Self::ScratchFailed,
            LocalAgentError::DirectoryInvalid => Self::DirectoryInvalid,
            LocalAgentError::DirectoryMissing => Self::DirectoryMissing,
            LocalAgentError::DirectoryNotDirectory => Self::DirectoryNotDirectory,
            LocalAgentError::DirectorySymlink => Self::DirectorySymlink,
            LocalAgentError::DirectoryUnavailable => Self::DirectoryUnavailable,
            LocalAgentError::DirectoryChanged => Self::DirectoryChanged,
            LocalAgentError::DirectoryMetadataInvalid => Self::DirectoryMetadataInvalid,
            LocalAgentError::DirectoryMetadataChanged => Self::DirectoryMetadataChanged,
            LocalAgentError::DirectoryReviewRequired => Self::DirectoryReviewRequired,
            LocalAgentError::DirectoryUnsupported => Self::DirectoryUnsupported,
            LocalAgentError::DirectoryValidationTimedOut => Self::DirectoryValidationTimedOut,
            LocalAgentError::ExecutableChanged => Self::ExecutableChanged,
            LocalAgentError::UnsupportedVersion => Self::UnsupportedVersion,
            LocalAgentError::IsolationUnsupported => Self::IsolationUnsupported,
            LocalAgentError::MissingCredential => Self::MissingCredential,
            LocalAgentError::CredentialInContext => Self::CredentialInContext,
            LocalAgentError::ReviewExpired => Self::ReviewExpired,
            LocalAgentError::Timeout => Self::Timeout,
            LocalAgentError::Cancelled => Self::Cancelled,
            LocalAgentError::PipeFailed => Self::PipeFailed,
            LocalAgentError::OutputTooLarge => Self::OutputTooLarge,
            LocalAgentError::InvalidProtocol => Self::InvalidProtocol,
            LocalAgentError::UnexpectedOperation => Self::UnexpectedOperation,
            LocalAgentError::InferenceFailed => Self::InferenceFailed,
            LocalAgentError::ProcessFailed => Self::ProcessFailed,
            LocalAgentError::CleanupFailed => Self::CleanupFailed,
            LocalAgentError::EmptyReply => Self::EmptyReply,
            LocalAgentError::Context(_) => Self::Context,
        }
    }
}

/// A fixed parent-process operation; no pathname or other request value is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocalAskOperation {
    /// Create the owned scratch root.
    ScratchCreate,
    /// Create the isolated home.
    ScratchHomeCreate,
    /// Create the isolated Codex home.
    ScratchCodexHomeCreate,
    /// Create the isolated Claude home.
    ScratchClaudeHomeCreate,
    /// Create the isolated roaming application-data directory.
    ScratchAppDataCreate,
    /// Create the isolated local application-data directory.
    ScratchLocalAppDataCreate,
    /// Create the isolated cache directory.
    ScratchCacheCreate,
    /// Create the isolated configuration directory.
    ScratchConfigCreate,
    /// Create the isolated workspace.
    ScratchWorkspaceCreate,
    /// Create the isolated temporary directory.
    ScratchTemporaryCreate,
    /// Write the empty isolated-workspace marker.
    ScratchMarkerWrite,
    /// Join the scratch creation worker.
    ScratchCreateWorker,
    /// Remove the owned scratch tree.
    ScratchClose,
    /// Join the scratch removal worker.
    ScratchCloseWorker,
    /// Open a new owned executable destination.
    OwnedExecutableOpen,
    /// Write bytes to the owned executable destination.
    OwnedExecutableWrite,
    /// Flush the owned executable destination.
    OwnedExecutableFlush,
    /// Read the copied executable's link-count and permission metadata.
    OwnedExecutableMetadata,
    /// Read the copied executable's owner metadata.
    OwnedExecutableOwnerMetadata,
    /// Resolve the parent's scratch home before preparing the launcher frame.
    BootstrapHomeCanonicalize,
    /// Resolve the parent's scratch temporary directory before the launcher frame.
    BootstrapTemporaryCanonicalize,
    /// Create a directory in the owned macOS bundle.
    BundleDirectoryCreate,
    /// Open a new owned bundle metadata destination.
    BundleTargetOpen,
    /// Copy held bundle metadata; an I/O failure can be from source or destination.
    BundleTargetCopy,
    /// Flush the owned bundle metadata destination.
    BundleTargetFlush,
    /// Reopen the owned bundle metadata destination for validation.
    BundleTargetReopen,
}

/// The reason a parent scratch worker did not return normally.
///
/// Panic payloads, task identifiers and formatted join errors are never retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocalAskWorkerJoinKind {
    /// The worker panicked.
    Panic,
    /// The runtime cancelled the worker before it returned.
    Cancelled,
}

/// One optional, bounded diagnostic fact from a single local Ask request.
///
/// Every field is a static category or an OS error number. Debug formatting
/// contains no error messages, paths, arguments, input, output or credentials.
/// Facts may be dropped; their presence, absence or ordering is not a success
/// receipt. Only the final Ask result establishes success after cleanup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocalAskDiagnostic {
    /// An operation failed with an I/O error, without retaining its payload.
    Io {
        /// The fixed operation that failed in the parent.
        operation: LocalAskOperation,
        /// The standard library error category.
        kind: io::ErrorKind,
        /// The native OS error number, if the original error supplies one.
        /// Wrappers such as `tempfile` can retain the kind while hiding errno;
        /// in that case this is `None`, never an inferred or stale OS error.
        raw_os_error: Option<i32>,
    },
    /// A scratch worker failed to join; no panic payload is retained.
    WorkerJoin {
        /// The fixed worker join operation.
        operation: LocalAskOperation,
        /// Panic or cancellation classification.
        kind: LocalAskWorkerJoinKind,
    },
    /// The parent command unexpectedly lacked its required explicit cwd.
    MissingCurrentDirectory,
    /// The request result immediately before explicit scratch removal.
    ///
    /// `Ok(())` describes only this intermediate result. Later cleanup can fail
    /// and override it; successful process/protocol output is not a final reply.
    ResultBeforeClose {
        /// Result stripped of reply text and all nested error data.
        result: Result<(), LocalAskErrorKind>,
    },
    /// The explicit scratch removal result, independent of the preceding request.
    CloseResult {
        /// Result stripped of I/O and worker payloads; detailed categories are
        /// separate optional facts.
        result: Result<(), LocalAskErrorKind>,
    },
}

/// A single-request diagnostics producer, consumed by
/// [`super::LocalAskProgress::with_diagnostics`].
///
/// There is no logging, file access or upload. The producer is intentionally
/// neither cloneable nor reusable; internal workers share one request budget.
///
/// ```compile_fail
/// use keelshell_ai::LocalAskDiagnostics;
/// let (diagnostics, _) = LocalAskDiagnostics::channel();
/// let duplicate = diagnostics.clone();
/// ```
pub struct LocalAskDiagnostics {
    sink: DiagnosticSink,
}

impl LocalAskDiagnostics {
    /// Create an inert producer and receiver for exactly one request.
    ///
    /// Both the queue capacity and the total event budget are sixteen. Draining
    /// the queue never replenishes that budget. Producers use only `try_send`,
    /// so a full, slow or dropped receiver cannot delay the request or affect
    /// its result. No Tokio runtime is needed to create or drain this channel.
    pub fn channel() -> (Self, LocalAskDiagnosticsReceiver) {
        let (sender, receiver) = mpsc::sync_channel(usize::from(EVENT_LIMIT));
        (
            Self {
                sink: DiagnosticSink(Some(Arc::new(DiagnosticsInner {
                    sender,
                    remaining: AtomicU8::new(EVENT_LIMIT),
                }))),
            },
            LocalAskDiagnosticsReceiver { receiver },
        )
    }

    pub(super) fn into_sink(self) -> DiagnosticSink {
        self.sink
    }
}

/// A bounded, nonblocking receiver belonging to one exact Ask invocation.
///
/// Dropping it discards observations without cancelling or changing the request.
/// Workers already running during cancellation may emit late facts; this is
/// neither a full trace nor proof that the supplier or service stopped.
pub struct LocalAskDiagnosticsReceiver {
    receiver: Receiver<LocalAskDiagnostic>,
}

impl LocalAskDiagnosticsReceiver {
    /// Immediately receive a fact or report an empty/disconnected channel.
    pub fn try_recv(&self) -> Result<LocalAskDiagnostic, TryRecvError> {
        self.receiver.try_recv()
    }
}

struct DiagnosticsInner {
    sender: SyncSender<LocalAskDiagnostic>,
    remaining: AtomicU8,
}

// Only parent-side code can clone this sink. It is never serialized into the
// launcher frame, placed in the environment, or exposed for a second request.
#[derive(Clone, Default)]
pub(super) struct DiagnosticSink(Option<Arc<DiagnosticsInner>>);

impl DiagnosticSink {
    fn observe(&self, event: LocalAskDiagnostic) {
        let Some(inner) = &self.0 else {
            return;
        };
        // A shared monotonic budget bounds *all* attempts, even if a consumer
        // continuously drains or disconnects. No per-worker reset is possible.
        if inner
            .remaining
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok()
        {
            let _ = inner.sender.try_send(event);
        }
    }

    pub(super) fn scratch_io(
        &self,
        operation: LocalAskOperation,
        error: &io::Error,
    ) -> LocalAgentError {
        self.observe(LocalAskDiagnostic::Io {
            operation,
            kind: error.kind(),
            raw_os_error: error.raw_os_error(),
        });
        LocalAgentError::ScratchFailed
    }

    pub(super) fn scratch_join(
        &self,
        operation: LocalAskOperation,
        error: &tokio::task::JoinError,
    ) -> LocalAgentError {
        self.observe(LocalAskDiagnostic::WorkerJoin {
            operation,
            kind: if error.is_panic() {
                LocalAskWorkerJoinKind::Panic
            } else {
                LocalAskWorkerJoinKind::Cancelled
            },
        });
        LocalAgentError::ScratchFailed
    }

    #[cfg(unix)]
    pub(super) fn missing_current_directory(&self) -> LocalAgentError {
        self.observe(LocalAskDiagnostic::MissingCurrentDirectory);
        LocalAgentError::ScratchFailed
    }

    pub(super) fn result_before_close<T>(&self, result: &Result<T, LocalAgentError>) {
        self.observe(LocalAskDiagnostic::ResultBeforeClose {
            result: result.as_ref().map(|_| ()).map_err(LocalAskErrorKind::from),
        });
    }

    pub(super) fn close_result(&self, result: &Result<(), LocalAgentError>) {
        self.observe(LocalAskDiagnostic::CloseResult {
            result: result.as_ref().copied().map_err(LocalAskErrorKind::from),
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::{fmt, time::Duration};

    use super::*;

    fn drain(receiver: &LocalAskDiagnosticsReceiver) -> Vec<LocalAskDiagnostic> {
        std::iter::from_fn(|| receiver.try_recv().ok()).collect()
    }

    #[test]
    fn draining_does_not_replenish_the_request_budget() {
        let (diagnostics, receiver) = LocalAskDiagnostics::channel();
        let sink = diagnostics.into_sink();
        let mut observed = Vec::new();
        for _ in 0..1024 {
            sink.result_before_close(&Ok::<_, LocalAgentError>("private answer"));
            observed.extend(drain(&receiver));
        }
        assert_eq!(observed.len(), 16);
        assert!(
            observed
                .iter()
                .all(|event| *event == LocalAskDiagnostic::ResultBeforeClose { result: Ok(()) })
        );
        assert_eq!(receiver.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn unread_and_dropped_receivers_cannot_stall_a_worker_or_change_errors() {
        for drop_receiver in [false, true] {
            let (diagnostics, receiver) = LocalAskDiagnostics::channel();
            let receiver = (!drop_receiver).then_some(receiver);
            let (done, finished) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let sink = diagnostics.into_sink();
                for _ in 0..1024 {
                    assert_eq!(
                        sink.scratch_io(
                            LocalAskOperation::OwnedExecutableWrite,
                            &io::Error::from(io::ErrorKind::WriteZero),
                        ),
                        LocalAgentError::ScratchFailed
                    );
                }
                done.send(()).unwrap();
            });
            finished.recv_timeout(Duration::from_secs(5)).unwrap();
            worker.join().unwrap();
            if let Some(receiver) = receiver {
                assert_eq!(drain(&receiver).len(), 16);
                assert_eq!(receiver.try_recv(), Err(TryRecvError::Disconnected));
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_workers_and_a_live_consumer_share_one_total_budget() {
        let (diagnostics, receiver) = LocalAskDiagnostics::channel();
        let sink = diagnostics.into_sink();
        let consumer = tokio::spawn(async move {
            let mut facts = Vec::new();
            loop {
                match receiver.try_recv() {
                    Ok(fact) => facts.push(fact),
                    Err(TryRecvError::Empty) => tokio::task::yield_now().await,
                    Err(TryRecvError::Disconnected) => return facts,
                }
            }
        });
        let mut workers = tokio::task::JoinSet::new();
        for _ in 0..8 {
            let sink = sink.clone();
            workers.spawn(async move {
                for _ in 0..128 {
                    sink.close_result(&Ok(()));
                    tokio::task::yield_now().await;
                }
            });
        }
        drop(sink);
        let facts = tokio::time::timeout(Duration::from_secs(5), async move {
            while let Some(result) = workers.join_next().await {
                result.unwrap();
            }
            consumer.await.unwrap()
        })
        .await
        .unwrap();
        assert_eq!(facts.len(), 16);
        assert!(
            facts
                .iter()
                .all(|event| *event == LocalAskDiagnostic::CloseResult { result: Ok(()) })
        );
    }

    #[test]
    fn separate_requests_neither_mix_events_nor_share_a_budget() {
        let (first, first_receiver) = LocalAskDiagnostics::channel();
        let (second, second_receiver) = LocalAskDiagnostics::channel();
        let first = first.into_sink();
        let second = second.into_sink();
        for _ in 0..32 {
            first.result_before_close::<()>(&Err(LocalAgentError::Timeout));
        }
        second.result_before_close::<()>(&Err(LocalAgentError::Cancelled));
        assert_eq!(drain(&first_receiver).len(), 16);
        assert_eq!(
            drain(&second_receiver),
            [LocalAskDiagnostic::ResultBeforeClose {
                result: Err(LocalAskErrorKind::Cancelled),
            }]
        );
        // Discarding a different request does not close or reset this producer.
        drop(first);
        second.close_result(&Ok(()));
        assert_eq!(
            second_receiver.try_recv(),
            Ok(LocalAskDiagnostic::CloseResult { result: Ok(()) })
        );
    }

    #[derive(Debug)]
    struct SecretPayload;

    impl fmt::Display for SecretPayload {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            panic!("diagnostics must never format an I/O payload")
        }
    }

    impl std::error::Error for SecretPayload {}

    #[test]
    fn io_payload_is_never_formatted_and_only_kind_and_errno_survive() {
        let (diagnostics, receiver) = LocalAskDiagnostics::channel();
        let sink = diagnostics.into_sink();
        let errors = [
            io::Error::new(io::ErrorKind::PermissionDenied, SecretPayload),
            io::Error::other("synthetic-path/argv/stdin/credential-secret"),
            io::Error::from_raw_os_error(13),
        ];
        for error in &errors {
            sink.scratch_io(LocalAskOperation::ScratchCreate, error);
            let event = receiver.try_recv().unwrap();
            assert_eq!(
                event,
                LocalAskDiagnostic::Io {
                    operation: LocalAskOperation::ScratchCreate,
                    kind: error.kind(),
                    raw_os_error: error.raw_os_error(),
                }
            );
            let debug = format!("{event:?}");
            assert!(!debug.contains("synthetic-path"));
            assert!(!debug.contains("credential-secret"));
            assert!(!debug.contains("SecretPayload"));
        }
    }

    #[test]
    fn context_errors_and_successful_answers_keep_no_nested_data() {
        let (diagnostics, receiver) = LocalAskDiagnostics::channel();
        let sink = diagnostics.into_sink();
        for error in [
            LocalAgentError::Context(crate::AiError::HttpStatus(599)),
            LocalAgentError::Context(crate::AiError::InvalidApiKey),
        ] {
            assert_eq!(LocalAskErrorKind::from(&error), LocalAskErrorKind::Context);
            sink.result_before_close::<()>(&Err(error));
        }
        sink.result_before_close(&Ok::<_, LocalAgentError>("synthetic private reply"));
        assert_eq!(
            drain(&receiver),
            [
                LocalAskDiagnostic::ResultBeforeClose {
                    result: Err(LocalAskErrorKind::Context),
                },
                LocalAskDiagnostic::ResultBeforeClose {
                    result: Err(LocalAskErrorKind::Context),
                },
                LocalAskDiagnostic::ResultBeforeClose { result: Ok(()) },
            ]
        );
    }

    #[tokio::test]
    async fn worker_panic_and_cancellation_are_classified_without_payloads() {
        let (diagnostics, receiver) = LocalAskDiagnostics::channel();
        let sink = diagnostics.into_sink();
        let panic = tokio::task::spawn_blocking::<_, ()>(|| std::panic::panic_any(SecretPayload));
        let panic = tokio::time::timeout(Duration::from_secs(5), panic)
            .await
            .unwrap()
            .unwrap_err();
        let cancelled = tokio::spawn(std::future::pending::<()>());
        cancelled.abort();
        let cancelled = tokio::time::timeout(Duration::from_secs(5), cancelled)
            .await
            .unwrap()
            .unwrap_err();
        assert_eq!(
            sink.scratch_join(LocalAskOperation::ScratchCreateWorker, &panic),
            LocalAgentError::ScratchFailed
        );
        assert_eq!(
            sink.scratch_join(LocalAskOperation::ScratchCloseWorker, &cancelled),
            LocalAgentError::ScratchFailed
        );
        assert_eq!(
            drain(&receiver),
            [
                LocalAskDiagnostic::WorkerJoin {
                    operation: LocalAskOperation::ScratchCreateWorker,
                    kind: LocalAskWorkerJoinKind::Panic,
                },
                LocalAskDiagnostic::WorkerJoin {
                    operation: LocalAskOperation::ScratchCloseWorker,
                    kind: LocalAskWorkerJoinKind::Cancelled,
                },
            ]
        );
    }
}
