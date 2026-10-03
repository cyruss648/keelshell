//! AI assistance without implicit context collection or command execution.
//!
//! Prepare and display the exact JSON request before calling
//! [`PreparedRequest::approve`]. Run [`AiClient::send`] on a background worker;
//! it is blocking. Redaction is a conservative aid, not a guarantee that arbitrary
//! secrets can be recognized. The human preview remains part of the contract.
//!
//! ```
//! use keelshell_ai::{ContextDraft, ProviderConfig};
//! # fn main() -> Result<(), keelshell_ai::AiError> {
//! let provider = ProviderConfig::new(
//!     "http://127.0.0.1:11434/v1/chat/completions", "local-model",
//! )?;
//! let request = ContextDraft::new("Explain the selected failure")
//!     .with_host_label("development")
//!     .add_selection("selected output", "connection refused")
//!     .prepare(&provider, &[], 8192)?;
//! assert!(request.preview_json().contains("connection refused"));
//! // Show provider.endpoint(), request.preview_json() and the redaction report.
//! // Only an explicit user action should consume `request.approve()`.
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]

mod context;
mod discovery;
mod error_category;
mod provider;
mod redact;
mod review;

pub use context::{ApprovedRequest, ContextDraft, PreparedRequest};
pub use discovery::{
    CONNECTIVITY_PROMPT, ConnectivityReport, ModelCatalog, ProviderClient, ProviderEndpoint,
    RequestCancellation,
};
pub use error_category::AiErrorCategory;
pub use provider::{AiClient, AssistantReply, ProviderConfig};
pub use redact::{RedactionReport, Redactor};
pub use review::{CommandProposal, ReviewTicket};

/// Errors intentionally omit raw provider bodies, URLs and credentials.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum AiError {
    /// The endpoint is not an acceptable HTTPS or loopback HTTP URL.
    #[error(
        "AI endpoint must use HTTPS, or HTTP on loopback, without URL credentials, query or fragment"
    )]
    InvalidEndpoint,
    /// Model discovery requires a literal `/chat/completions` path suffix.
    #[error("Model discovery requires an endpoint ending in /chat/completions")]
    UnsupportedDiscoveryEndpoint,
    /// The model name is missing or contains unsupported control characters.
    #[error("AI model must be a nonempty name of at most 200 bytes without whitespace or controls")]
    InvalidModel,
    /// User text, number of selections or secret list exceeds preparation limits.
    #[error("AI context or secret list exceeds the preparation limit")]
    ContextTooLarge,
    /// There is no question to send.
    #[error("AI request must include a nonempty question")]
    EmptyPrompt,
    /// The requested UTF-8 byte budget is invalid or cannot hold the question.
    #[error("AI context budget must be between 1 and 65536 bytes and hold the complete question")]
    InvalidBudget,
    /// JSON serialization failed without echoing user content.
    #[error("AI request could not be serialized")]
    Serialization,
    /// The configured timeout or response bound is outside supported limits.
    #[error(
        "AI timeout must be nonzero and at most 300 seconds; response limit must be 1 byte to 8 MiB"
    )]
    InvalidLimits,
    /// HTTP client initialization failed.
    #[error("AI HTTP client could not be initialized")]
    ClientInitialization,
    /// The caller supplied an invalid authentication header value.
    #[error("AI API key is empty or contains invalid header characters")]
    InvalidApiKey,
    /// The transport key was also found in the prepared context.
    #[error("AI API key is present in the preview; add it to the secret list and prepare again")]
    CredentialInContext,
    /// The request did not complete before the configured deadline.
    #[error("AI request timed out; no automatic retry was performed")]
    Timeout,
    /// The caller cancelled the local HTTP future; provider processing may have begun.
    #[error("AI request cancelled locally; provider processing may already have begun")]
    Cancelled,
    /// Network or TLS transport failed.
    #[error("AI request failed at the network or TLS layer")]
    Transport,
    /// The provider returned a non-success status; the body is deliberately hidden.
    #[error("AI provider returned HTTP {0}")]
    HttpStatus(u16),
    /// The response exceeds its byte bound, including when no length is supplied.
    #[error("AI provider response exceeded the configured byte limit")]
    ResponseTooLarge,
    /// The response does not match the supported chat-completions schema.
    #[error("AI provider returned invalid chat-completions JSON")]
    InvalidResponse,
    /// A models response has an invalid shape, identifier, or more than 4096 entries.
    #[error("AI provider returned an invalid or excessively large model catalog")]
    InvalidModelCatalog,
    /// The provider returned no non-whitespace textual answer.
    #[error("AI provider returned an empty textual answer")]
    EmptyReply,
    /// A command proposal does not identify valid text, host and session.
    #[error("Command proposal requires nonempty command, target and session; NUL is not allowed")]
    InvalidProposal,
    /// A review ticket was used for changed content or a different session.
    #[error("Command or session changed after review; review the new proposal")]
    ReviewMismatch,
    /// A review ticket is too old to authorize insertion.
    #[error("Command review expired; review the proposal again")]
    ReviewExpired,
}
