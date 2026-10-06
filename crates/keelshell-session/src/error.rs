/// Transport failures safe to present without including credentials.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// A native I/O operation failed.
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    /// An SSH protocol operation failed.
    #[error("SSH: {0}")]
    Ssh(#[from] russh::Error),
    /// Explicit upstream proxy authentication, policy or protocol rejection.
    #[error("{0}")]
    Proxy(#[from] crate::ssh::ProxyError),
    /// No host identity has yet been explicitly trusted.
    #[error("host key requires approval: {fingerprint}")]
    UnknownHostKey {
        /// SHA256 fingerprint observed during key exchange.
        fingerprint: String,
    },
    /// The remote identity differs from the stored identity.
    #[error("host key changed: expected {expected}, received {actual}")]
    ChangedHostKey {
        /// Previously approved fingerprint.
        expected: String,
        /// Fingerprint from the current handshake.
        actual: String,
    },
    /// Authentication was rejected. Credentials are never part of this error.
    #[error("SSH authentication rejected")]
    Authentication,
    /// Loading a key or communicating with an agent failed.
    #[error("SSH credential provider: {0}")]
    Credential(String),
    /// A timeout expired; no successful outcome may be inferred.
    #[error("operation timed out: {0}")]
    Timeout(&'static str),
    /// The requested session is closed.
    #[error("session is closed")]
    Closed,
    /// A dimension, path or option is invalid.
    #[error("invalid session option: {0}")]
    Invalid(&'static str),
    /// A worker failed while owning session state.
    #[error("session worker state is unavailable")]
    Worker,
    /// A remote channel explicitly denied a request.
    #[error("server rejected {0}")]
    Rejected(&'static str),
    /// A file transfer operation failed.
    #[error("SFTP: {0}")]
    Sftp(String),
    /// The server has not negotiated a capability required for a safe operation.
    #[error("unsupported operation: {0}")]
    Unsupported(&'static str),
    /// Reading more data would exceed a configured memory bound.
    #[error("output exceeds {0} bytes")]
    OutputLimit(usize),
    /// A directory response exceeds the caller's entry limit.
    #[error("directory exceeds {0} entries")]
    EntryLimit(usize),
    /// Another admitted application mutation owns this path or worker budget.
    #[error("file target is busy; review again after the active mutation finishes")]
    MutationBusy,
    /// A conflicting mutation has no confirmed outcome; ordinary approval is
    /// insufficient. Explicit inspection and exact risk acknowledgement are required.
    #[error(
        "file target is isolated by an unknown mutation; inspect and explicitly acknowledge its risk"
    )]
    MutationQuarantined,
    /// A mutating request lost its reply; process-wide destination isolation remains.
    #[error("file mutation outcome is unknown; destination remains isolated")]
    MutationUncertain,
    /// A remote mutation was acknowledged but its requested result could not
    /// be verified. The caller must refresh the entry before retrying.
    #[error("remote mutation acknowledged but verification failed: {0}")]
    UnverifiedMutation(&'static str),
}

impl SessionError {
    /// Return whether retrying the complete SSH connection may change the outcome.
    ///
    /// The classification is deliberately conservative. It includes transport
    /// resets, connection-level protocol handshakes and elapsed deadlines, but
    /// excludes identity, authentication, credential, validation and explicit
    /// server-policy failures. Callers must still bound attempts with
    /// [`crate::ssh::RetryPolicy`].
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Io(error) => retryable_io(error),
            Self::Ssh(error) => retryable_ssh(error),
            Self::Timeout(_) => true,
            Self::Proxy(_)
            | Self::UnknownHostKey { .. }
            | Self::ChangedHostKey { .. }
            | Self::Authentication
            | Self::Credential(_)
            | Self::Closed
            | Self::Invalid(_)
            | Self::Worker
            | Self::Rejected(_)
            | Self::Sftp(_)
            | Self::Unsupported(_)
            | Self::OutputLimit(_)
            | Self::EntryLimit(_)
            | Self::UnverifiedMutation(_)
            | Self::MutationBusy
            | Self::MutationQuarantined
            | Self::MutationUncertain => false,
        }
    }
}

fn retryable_io(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::Interrupted
            | std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::HostUnreachable
            | std::io::ErrorKind::NetworkUnreachable
            | std::io::ErrorKind::NetworkDown
    )
}

fn retryable_ssh(error: &russh::Error) -> bool {
    use russh::Error;

    matches!(
        error,
        Error::IO(error) if retryable_io(error)
    ) || matches!(
        error,
        Error::Kex
            | Error::KexInit
            | Error::ConnectionTimeout
            | Error::KeepaliveTimeout
            | Error::InactivityTimeout
            | Error::Disconnect
            | Error::HUP
            | Error::SendError
            | Error::RecvError
            | Error::Elapsed(_)
    )
}

/// Transport result type.
pub type Result<T> = std::result::Result<T, SessionError>;
