//! Remote SSH sessions, SFTP and forwarding transports, independent of the UI.
//!
//! SSH APIs are async and require an active Tokio runtime. Interactive terminal
//! sessions are remote SSH channels; this crate does not launch local shells.
#![deny(missing_docs)]

pub mod batch;
pub mod completion;
mod error;
mod events;
pub mod forwarding;
pub mod monitor;
pub mod network_diagnostic;
pub mod sftp;
pub mod ssh;
pub mod workflow;

pub use batch::{
    BatchError, BatchEvent, BatchHandle, BatchNotStartedReason, BatchOptions, BatchOutcome,
    BatchPolicy, BatchReceipt, BatchRowReceipt, BatchTarget, BatchUnknownReason, start_batch,
};
pub use completion::{
    CompletionCandidate, CompletionError, CompletionKind, CompletionQuery, CompletionResult,
};
pub use error::{Result, SessionError};
pub use events::SessionEvent;
pub use ssh::{
    ConnectionEnd, ConnectionState, ExecOutput, KeyboardInteractiveChallenge,
    KeyboardInteractivePrompt, ProxyCredentials, ProxyError, ProxyKind, RetryPolicy, ShellEnd,
    SshAuth, SshOptions, SshProxy, SshSession, SshShell, SshShellWriter,
};
pub use workflow::{
    WorkflowBinding, WorkflowError, WorkflowEvent, WorkflowHandle, WorkflowOptions,
    WorkflowReceipt, WorkflowTaskReceipt, WorkflowTaskResult, start_workflow,
};
