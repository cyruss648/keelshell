//! KeelShell domain types and local persistence, independent of UI and transports.
//!
//! Profiles contain credential references, never passwords or API keys. Validation
//! runs both at import and before persistence; transports must still enforce their
//! own protocol and host-key policies. [`StateStore`] uses blocking filesystem I/O
//! and belongs on a worker, outside the GPUI render/event thread.

#![deny(missing_docs)]

mod ai_profiles;
mod completion;
mod connection_library;
mod error;
mod model;
mod openssh;
mod proxy;
mod reconnect;
mod routes;
mod snippet_template;
mod snippets;
mod store;
mod vault;

pub use ai_profiles::{
    AiApiStyle, AiAuthentication, AiCustomHeader, AiModelReasoning, AiPreset, AiProfileCatalog,
    AiProxy, AiReasoningCapability, AiReasoningSelection, AiSecretRef, NamedAiProfile,
};
pub use completion::{
    CompletionAnalysis, CompletionEdit, CompletionError, CompletionPlan, CompletionQuery,
    CompletionUnsupported, LiteralCandidate, MAX_COMPLETION_INPUT_BYTES, MAX_COMPLETION_PATH_BYTES,
    analyze_completion,
};
pub use connection_library::{
    ConnectionFolder, DeletedConnection, FolderRow, MAX_RECENT_CONNECTIONS, RecentConnection,
};
pub use error::{Error, ValidationError};
pub use model::{
    AiSettings, AppState, AuthMethod, Connection, ImportReport, Language, SCHEMA_VERSION, Settings,
    SnapshotRevision, Snippet, Theme,
};
pub use openssh::{
    MAX_OPENSSH_CONFIG_BYTES, MAX_OPENSSH_ENTRIES, MAX_OPENSSH_INCLUDE_DEPTH,
    MAX_OPENSSH_LINE_BYTES, OpenSshConfig, OpenSshEntry, OpenSshImportReport, OpenSshWarning,
    parse_openssh_config, parse_openssh_config_with_includes,
};
pub use proxy::{ConnectionProxy, ProxyAuthentication, ProxyKind};
pub use reconnect::ReconnectPolicy;
pub use routes::{ConnectionRoute, HostKeyScope, MAX_JUMP_HOSTS, RouteEndpoint, RouteIdentity};
pub use snippet_template::{
    MAX_SNIPPET_TEMPLATE_BYTES, MAX_SNIPPET_VALUE_BYTES, MAX_SNIPPET_VARIABLES, SnippetTemplate,
    SnippetTemplateContext, SnippetTemplateError, compile_snippet_template,
};
pub use store::StateStore;
pub use vault::{CredentialKind, CredentialMetadata, CredentialVault, VaultStore};
