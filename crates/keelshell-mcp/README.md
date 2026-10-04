# KeelShell MCP server

This crate exposes KeelShell capabilities to external agents. It does not connect to third-party MCP servers.

Build with `cargo build -p keelshell-mcp --locked`, then launch `keelshell-mcp` with no arguments. The process uses newline-delimited JSON-RPC on stdin/stdout; diagnostics are static stderr text. It supports MCP `2026-07-28` discovery/per-request metadata and `2025-11-25`/`2025-06-18` initialization compatibility.

**The standalone executable is disabled and disconnected by default.** Tool discovery works, but calls return `DISABLED`. This milestone contains no authenticated desktop IPC, no active SSH session access and no credential access. A future desktop integration must install explicit authority and implement `DesktopBackend` with current session/path checks. Do not treat the protocol fixture as a working SSH bridge.

The fixed tools enumerate permitted session labels, read explicit terminal selections, list/read scoped SFTP paths, read cached monitoring fields, enqueue immutable command proposals and inspect desktop action state. There is no external approval, arbitrary execution, file-write, SSH-login or vault-unlock tool. A proposal can only become an action after a separate human review in the desktop application.

See [ADR 0037](../../docs/adr/0037-external-mcp-stdio-server.md) for the backend/authentication contract, bounds and remaining integration work, and [test record](../../docs/testing/records/2026-10-04-mcp-stdio-server.md) for evidence and its limits.
