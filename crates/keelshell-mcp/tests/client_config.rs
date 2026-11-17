use std::{error::Error, path::Path};

use keelshell_mcp::{IpcFailure, McpClientKind, client_configuration_template};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const CLIENTS: [McpClientKind; 2] = [McpClientKind::CodexCli, McpClientKind::ClaudeCode];
const CODEX_PREFIX: &str = "[mcp_servers.keelshell]\ncommand = ";
const CODEX_SUFFIX: &str =
    "\nargs = []\nenv_vars = [\"KEELSHELL_MCP_ADDRESS\", \"KEELSHELL_MCP_SECRET\"]\n";

// Expected literals are fixtures, independent of the production serializer.
const PATH_CASES: &[(&str, &str)] = &[
    (
        "/opt/Keel Shell/bin/keelshell-mcp",
        r#""/opt/Keel Shell/bin/keelshell-mcp""#,
    ),
    (
        "/Applications/KeelShell.app/Contents/MacOS/keelshell-mcp",
        r#""/Applications/KeelShell.app/Contents/MacOS/keelshell-mcp""#,
    ),
    (
        r"C:\Program Files\KeelShell\keelshell-mcp.exe",
        r#""C:\\Program Files\\KeelShell\\keelshell-mcp.exe""#,
    ),
    (
        r"\\server\shared apps\KeelShell\keelshell-mcp.exe",
        r#""\\\\server\\shared apps\\KeelShell\\keelshell-mcp.exe""#,
    ),
    (
        r"\\?\C:\Program Files\KeelShell\keelshell-mcp.exe",
        r#""\\\\?\\C:\\Program Files\\KeelShell\\keelshell-mcp.exe""#,
    ),
    (
        r"\\?\UNC\server\share\keelshell-mcp.exe",
        r#""\\\\?\\UNC\\server\\share\\keelshell-mcp.exe""#,
    ),
    (
        "/opt/\"quoted\"/'single'/keelshell-mcp",
        r#""/opt/\"quoted\"/'single'/keelshell-mcp""#,
    ),
    (
        "/opt/back\\slash/keelshell-mcp",
        r#""/opt/back\\slash/keelshell-mcp""#,
    ),
    (
        "/opt/\u{8}\t\n\u{c}\r\0/keelshell-mcp",
        r#""/opt/\b\t\n\f\r\u0000/keelshell-mcp""#,
    ),
    (
        "/opt/\u{7f}/keelshell-mcp",
        r#""/opt/\u007f/keelshell-mcp""#,
    ),
    (
        "/应用/霜白 café e\u{301} 🐚/keelshell-mcp",
        "\"/应用/霜白 café e\u{301} 🐚/keelshell-mcp\"",
    ),
    (
        "/opt/\u{85}\u{2028}\u{2029}/keelshell-mcp",
        "\"/opt/\u{85}\u{2028}\u{2029}/keelshell-mcp\"",
    ),
    (" keelshell-mcp ", r#"" keelshell-mcp ""#),
];

fn assert_codex_template(command: &str, quoted: &str) -> TestResult {
    let actual = client_configuration_template(McpClientKind::CodexCli, Path::new(command))?;
    assert_eq!(
        actual,
        format!("{CODEX_PREFIX}{quoted}{CODEX_SUFFIX}"),
        "unexpected TOML rendering for {command:?}"
    );
    let literal = actual
        .strip_prefix(CODEX_PREFIX)
        .and_then(|body| body.strip_suffix(CODEX_SUFFIX))
        .ok_or("unexpected TOML configuration shape")?;
    let decoded: String = serde_json::from_str(literal)?;
    assert_eq!(decoded, command, "command spelling must be preserved");
    assert!(
        !literal
            .chars()
            .any(|value| value <= '\u{1f}' || value == '\u{7f}'),
        "TOML command must not contain literal C0 controls or DEL"
    );
    Ok(())
}

fn assert_claude_template(command: &str) -> TestResult {
    let actual = client_configuration_template(McpClientKind::ClaudeCode, Path::new(command))?;
    let parsed: Value = serde_json::from_str(&actual)?;
    assert_eq!(
        parsed,
        json!({
            "mcpServers": {
                "keelshell": {
                    "type": "stdio",
                    "command": command,
                    "args": [],
                    "env": {
                        "KEELSHELL_MCP_ADDRESS": "${KEELSHELL_MCP_ADDRESS}",
                        "KEELSHELL_MCP_SECRET": "${KEELSHELL_MCP_SECRET}",
                    },
                },
            },
        }),
        "unexpected JSON configuration for {command:?}"
    );
    Ok(())
}

#[test]
fn codex_template_contains_only_command_empty_args_and_environment_names() -> TestResult {
    assert_codex_template(
        "/opt/keelshell/bin/keelshell-mcp",
        r#""/opt/keelshell/bin/keelshell-mcp""#,
    )
}

#[test]
fn claude_template_is_a_complete_stdio_configuration_with_environment_references() -> TestResult {
    assert_claude_template("/opt/keelshell/bin/keelshell-mcp")
}

#[test]
fn codex_template_quotes_platform_paths_controls_and_unicode_exactly() -> TestResult {
    for &(command, quoted) in PATH_CASES {
        assert_codex_template(command, quoted)?;
    }
    Ok(())
}

#[test]
fn claude_template_preserves_platform_paths_controls_and_unicode_semantically() -> TestResult {
    for &(command, _) in PATH_CASES {
        assert_claude_template(command)?;
    }
    Ok(())
}

#[test]
fn codex_template_escapes_every_c0_control_and_del() -> TestResult {
    for (control, escaped) in [
        ('\u{00}', r"\u0000"),
        ('\u{01}', r"\u0001"),
        ('\u{02}', r"\u0002"),
        ('\u{03}', r"\u0003"),
        ('\u{04}', r"\u0004"),
        ('\u{05}', r"\u0005"),
        ('\u{06}', r"\u0006"),
        ('\u{07}', r"\u0007"),
        ('\u{08}', r"\b"),
        ('\u{09}', r"\t"),
        ('\u{0a}', r"\n"),
        ('\u{0b}', r"\u000b"),
        ('\u{0c}', r"\f"),
        ('\u{0d}', r"\r"),
        ('\u{0e}', r"\u000e"),
        ('\u{0f}', r"\u000f"),
        ('\u{10}', r"\u0010"),
        ('\u{11}', r"\u0011"),
        ('\u{12}', r"\u0012"),
        ('\u{13}', r"\u0013"),
        ('\u{14}', r"\u0014"),
        ('\u{15}', r"\u0015"),
        ('\u{16}', r"\u0016"),
        ('\u{17}', r"\u0017"),
        ('\u{18}', r"\u0018"),
        ('\u{19}', r"\u0019"),
        ('\u{1a}', r"\u001a"),
        ('\u{1b}', r"\u001b"),
        ('\u{1c}', r"\u001c"),
        ('\u{1d}', r"\u001d"),
        ('\u{1e}', r"\u001e"),
        ('\u{1f}', r"\u001f"),
        ('\u{7f}', r"\u007f"),
    ] {
        let command = format!("/opt/prefix{control}suffix/keelshell-mcp");
        let quoted = format!("\"/opt/prefix{escaped}suffix/keelshell-mcp\"");
        assert_codex_template(&command, &quoted)?;
    }
    Ok(())
}

#[test]
fn claude_template_round_trips_every_c0_control_and_del() -> TestResult {
    for control in (0u8..=0x1f).chain(std::iter::once(0x7f)) {
        assert_claude_template(&format!("/opt/a{}b/keelshell-mcp", char::from(control)))?;
    }
    Ok(())
}

#[test]
fn shell_metacharacters_remain_command_data_with_no_wrapper_or_arguments() -> TestResult {
    for (command, quoted) in [
        (
            "/opt/$(literal);`literal` & | < > '\"/keelshell-mcp",
            r#""/opt/$(literal);`literal` & | < > '\"/keelshell-mcp""#,
        ),
        (
            r"C:\%COMSPEC%\!PATH!\$HOME\~\keelshell-mcp.exe",
            r#""C:\\%COMSPEC%\\!PATH!\\$HOME\\~\\keelshell-mcp.exe""#,
        ),
        (
            "/opt/$ {name}/$name/keelshell-mcp",
            r#""/opt/$ {name}/$name/keelshell-mcp""#,
        ),
    ] {
        assert_codex_template(command, quoted)?;
        assert_claude_template(command)?;
    }
    Ok(())
}

#[test]
fn configuration_shaped_paths_cannot_add_servers_arguments_or_grants() -> TestResult {
    for (command, quoted) in [
        (
            "/opt/\";\n[mcp_servers.injected]\ncommand = \"other\" #/keelshell-mcp",
            r#""/opt/\";\n[mcp_servers.injected]\ncommand = \"other\" #/keelshell-mcp""#,
        ),
        (
            "/opt/\"},\"injected\":{\"command\":\"other\",\"grants\":[\"all\"]}/keelshell-mcp",
            r#""/opt/\"},\"injected\":{\"command\":\"other\",\"grants\":[\"all\"]}/keelshell-mcp""#,
        ),
        (
            "/opt/\"\"\"\r\nargs = [\"--grant-all\"]\n/keelshell-mcp",
            r#""/opt/\"\"\"\r\nargs = [\"--grant-all\"]\n/keelshell-mcp""#,
        ),
    ] {
        assert_codex_template(command, quoted)?;
        assert_claude_template(command)?;
    }
    Ok(())
}

#[test]
fn claude_template_rejects_literal_expansion_markers_with_a_static_error() {
    for command in [
        "/opt/${NAME}/keelshell-mcp",
        "/opt/${NAME:-fallback}/keelshell-mcp",
        "/opt/${/keelshell-mcp",
        r"C:\apps\${NAME}\keelshell-mcp.exe",
        r"/opt/\${NAME}/keelshell-mcp",
        "/opt/$${NAME}/keelshell-mcp",
        "/应用/${KEELSHELL_MCP_SECRET}/keelshell-mcp",
    ] {
        let actual = client_configuration_template(McpClientKind::ClaudeCode, Path::new(command));
        assert_eq!(actual, Err(IpcFailure::InvalidConfiguration));
        if let Err(error) = actual {
            assert_eq!(
                error.to_string(),
                "invalid MCP desktop launch configuration"
            );
            assert_eq!(format!("{error:?}"), "InvalidConfiguration");
        }
    }
}

#[test]
fn codex_template_preserves_literal_expansion_markers() -> TestResult {
    for (command, quoted) in [
        (
            "/opt/${NAME}/keelshell-mcp",
            r#""/opt/${NAME}/keelshell-mcp""#,
        ),
        (
            r"C:\apps\${KEELSHELL_MCP_SECRET}\keelshell-mcp.exe",
            r#""C:\\apps\\${KEELSHELL_MCP_SECRET}\\keelshell-mcp.exe""#,
        ),
        (
            "/opt/${NAME:-fallback}/keelshell-mcp",
            r#""/opt/${NAME:-fallback}/keelshell-mcp""#,
        ),
    ] {
        assert_codex_template(command, quoted)?;
    }
    Ok(())
}

#[test]
fn templates_reject_empty_command_paths() {
    for client in CLIENTS {
        assert_eq!(
            client_configuration_template(client, Path::new("")),
            Err(IpcFailure::InvalidConfiguration)
        );
    }
}

#[cfg(unix)]
#[test]
fn templates_reject_non_utf8_unix_paths_without_lossy_conversion() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    let command = Path::new(OsStr::from_bytes(b"/opt/private-\xff/keelshell-mcp"));
    for client in CLIENTS {
        assert_eq!(
            client_configuration_template(client, command),
            Err(IpcFailure::InvalidConfiguration)
        );
    }
}

#[cfg(windows)]
#[test]
fn templates_reject_unpaired_windows_surrogates_without_lossy_conversion() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};

    let command = OsString::from_wide(&[0x43, 0x3a, 0x5c, 0xd800, 0x5c, 0x61]);
    for client in CLIENTS {
        assert_eq!(
            client_configuration_template(client, Path::new(&command)),
            Err(IpcFailure::InvalidConfiguration)
        );
    }
}

#[test]
fn templates_are_deterministic_and_contain_no_capability_or_authority_fields() -> TestResult {
    let command = Path::new("/opt/keelshell/bin/keelshell-mcp");
    for client in CLIENTS {
        let template = client_configuration_template(client, command)?;
        assert_eq!(template, client_configuration_template(client, command)?);
        for forbidden in [
            "127.0.0.1:",
            "capability",
            "grants",
            "connection_id",
            "session_id",
            "route_revision",
            "selection_ids",
            "password",
        ] {
            assert!(
                !template.contains(forbidden),
                "template unexpectedly contains {forbidden}"
            );
        }
    }
    assert_codex_template(
        "/opt/keelshell/bin/keelshell-mcp",
        r#""/opt/keelshell/bin/keelshell-mcp""#,
    )?;
    assert_claude_template("/opt/keelshell/bin/keelshell-mcp")
}
