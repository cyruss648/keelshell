use crate::AiError;

/// Stable, non-sensitive categories for localized settings and request feedback.
///
/// Categories are informational, never instructions to retry automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AiErrorCategory {
    /// Invalid local endpoint, model, credentials, limits, or request data.
    Configuration,
    /// Credentials were absent, expired or rejected (HTTP 401).
    Authentication,
    /// The provider refused permission (HTTP 403).
    PermissionDenied,
    /// The endpoint/method was unavailable (HTTP 404 or 405).
    UnsupportedEndpoint,
    /// The provider applied a rate or quota limit (HTTP 429).
    RateLimited,
    /// The provider returned a server error (HTTP 5xx).
    ProviderUnavailable,
    /// A redirect was refused to retain the original destination.
    RedirectRejected,
    /// Another unsuccessful HTTP status was returned.
    RequestRejected,
    /// The caller cancelled the local network operation.
    Cancelled,
    /// The operation exceeded its configured deadline.
    Timeout,
    /// A network or TLS failure prevented completion.
    Transport,
    /// The response exceeded the configured body limit.
    ResponseTooLarge,
    /// The response did not match the supported protocol.
    InvalidResponse,
    /// A command review was absent, changed, or expired.
    Review,
}

impl AiError {
    /// Classify an error without exposing provider bodies, keys, or endpoints.
    pub const fn category(&self) -> AiErrorCategory {
        match self {
            Self::HttpStatus(401) => AiErrorCategory::Authentication,
            Self::HttpStatus(403) => AiErrorCategory::PermissionDenied,
            Self::HttpStatus(404 | 405) | Self::UnsupportedDiscoveryEndpoint => {
                AiErrorCategory::UnsupportedEndpoint
            }
            Self::HttpStatus(429) => AiErrorCategory::RateLimited,
            Self::HttpStatus(500..=599) => AiErrorCategory::ProviderUnavailable,
            Self::HttpStatus(300..=399) => AiErrorCategory::RedirectRejected,
            Self::HttpStatus(_) => AiErrorCategory::RequestRejected,
            Self::Cancelled => AiErrorCategory::Cancelled,
            Self::Timeout => AiErrorCategory::Timeout,
            Self::Transport | Self::ClientInitialization => AiErrorCategory::Transport,
            Self::ResponseTooLarge => AiErrorCategory::ResponseTooLarge,
            Self::InvalidResponse | Self::EmptyReply | Self::InvalidModelCatalog => {
                AiErrorCategory::InvalidResponse
            }
            Self::InvalidProposal
            | Self::ReviewMismatch
            | Self::ReviewExpired
            | Self::InvalidDiagnosticPlan
            | Self::DiagnosticPlanTooLarge
            | Self::NoDiagnosticSteps
            | Self::DiagnosticPlanMismatch => AiErrorCategory::Review,
            Self::InvalidEndpoint
            | Self::InvalidModel
            | Self::ContextTooLarge
            | Self::EmptyPrompt
            | Self::InvalidBudget
            | Self::Serialization
            | Self::InvalidLimits
            | Self::InvalidMaxTokens
            | Self::InvalidApiKey
            | Self::CredentialInContext
            | Self::UnsupportedProtocol => AiErrorCategory::Configuration,
        }
    }
}
