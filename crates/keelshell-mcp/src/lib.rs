//! MCP **server** for external agents, with explicit desktop-owned authority.
//!
//! The standalone executable discovers tools but denies access by default. It
//! does not connect to SSH, unlock credentials, or approve/execute suggestions.
//! [`DesktopBackend`] is the integration boundary for a future authenticated
//! desktop bridge. Protocol tests do not establish that bridge's acceptance.
//!
//! ```
//! use keelshell_mcp::{AccessPolicy, PolicyController, SessionGrant, SessionIdentity, ToolKind};
//! use uuid::Uuid;
//! let target = SessionIdentity {
//!     connection_id: Uuid::new_v4(), session_id: Uuid::new_v4(), route_revision: Uuid::new_v4(),
//! };
//! let grant = SessionGrant::new(target, [ToolKind::ListSessions], vec![], [])?;
//! let authority = PolicyController::default(); // off until explicit desktop consent
//! authority.replace(AccessPolicy::enabled(vec![grant])?)?;
//! authority.disable()?; // invalidates existing leases
//! # Ok::<(), keelshell_mcp::McpFailure>(())
//! ```
#![deny(missing_docs)]

mod backend;
mod policy;
mod server;
mod transport;

pub use backend::*;
pub use policy::*;
pub use server::KeelShellMcpServer;
pub use transport::{
    MAX_PENDING_FRAMES, MAX_REQUEST_BYTES, StdioFailure, serve_stdio, serve_stream,
};
