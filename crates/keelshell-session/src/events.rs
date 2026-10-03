//! UI-independent events from a remote interactive session.

/// Bytes and lifecycle information emitted by an interactive SSH channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// Raw remote terminal bytes, including ANSI sequences and partial UTF-8.
    Data(Vec<u8>),
    /// The remote process reported an exit status through its SSH channel.
    Exited {
        /// Exit status reported by the remote SSH server.
        code: u32,
        /// Whether the remote process reported successful completion.
        success: bool,
    },
    /// An asynchronous transport operation failed; diagnostic text is preserved.
    Error(String),
}
