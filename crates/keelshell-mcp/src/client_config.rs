//! Pure client configuration templates containing no desktop capability values.

use std::path::Path;

use crate::{IpcFailure, MCP_ADDRESS_ENV, MCP_SECRET_ENV};

/// External MCP client whose stdio configuration should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpClientKind {
    /// Codex CLI's TOML `mcp_servers.keelshell` entry.
    CodexCli,
    /// Claude Code's JSON `mcpServers` object.
    ClaudeCode,
}

/// Render a client configuration template for a caller-verified companion path.
///
/// The caller must verify the companion executable before supplying `command`.
/// This function preserves its UTF-8 spelling without checking the filesystem,
/// resolving paths, reading the environment, or starting any process.
///
/// Codex receives a TOML `mcp_servers.keelshell` entry with `command`, empty
/// `args`, and `env_vars` naming [`MCP_ADDRESS_ENV`] and [`MCP_SECRET_ENV`].
/// Claude Code receives a complete JSON `mcpServers` object with a `keelshell`
/// stdio entry, the command, empty `args`, and `${KEELSHELL_MCP_ADDRESS}` /
/// `${KEELSHELL_MCP_SECRET}` environment references. Neither template contains
/// desktop capability values or grants. The launching environment must supply
/// the current capability through those variables.
///
/// # Errors
///
/// Returns the static [`IpcFailure::InvalidConfiguration`] error for an empty
/// or non-UTF-8 command path. Claude Code also rejects any path containing
/// literal `${`, because the client expands it even inside `command`; quoting
/// cannot preserve that installation path. Errors never include the path.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use keelshell_mcp::{McpClientKind, client_configuration_template};
///
/// // The application verifies the companion path before requesting a template.
/// let template = client_configuration_template(
///     McpClientKind::CodexCli,
///     Path::new("/opt/keelshell/bin/keelshell-mcp"),
/// )?;
/// assert!(template.starts_with("[mcp_servers.keelshell]\n"));
/// # Ok::<(), keelshell_mcp::IpcFailure>(())
/// ```
pub fn client_configuration_template(
    client: McpClientKind,
    command: &Path,
) -> Result<String, IpcFailure> {
    let command = command
        .to_str()
        .filter(|value| !value.is_empty())
        .ok_or(IpcFailure::InvalidConfiguration)?;

    match client {
        McpClientKind::CodexCli => {
            // JSON escapes every C0 control with TOML-compatible sequences.
            // DEL is legal literal JSON but must also be escaped in TOML.
            let command = serde_json::to_string(command)
                .map_err(|_| IpcFailure::InvalidConfiguration)?
                .replace('\u{7f}', "\\u007f");
            Ok(format!(
                "[mcp_servers.keelshell]\ncommand = {command}\nargs = []\nenv_vars = [\"{MCP_ADDRESS_ENV}\", \"{MCP_SECRET_ENV}\"]\n"
            ))
        }
        McpClientKind::ClaudeCode => {
            if command.contains("${") {
                return Err(IpcFailure::InvalidConfiguration);
            }
            serde_json::to_string_pretty(&serde_json::json!({
                "mcpServers": {
                    "keelshell": {
                        "type": "stdio",
                        "command": command,
                        "args": [],
                        "env": {
                            MCP_ADDRESS_ENV: format!("${{{MCP_ADDRESS_ENV}}}"),
                            MCP_SECRET_ENV: format!("${{{MCP_SECRET_ENV}}}"),
                        },
                    },
                },
            }))
            .map_err(|_| IpcFailure::InvalidConfiguration)
        }
    }
}
