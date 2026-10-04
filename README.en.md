<p align="center">
  <img src="assets/icons/png/256.png" width="112" height="112" alt="KeelShell application icon">
</p>

<h1 align="center">KeelShell</h1>

<p align="center">
  <strong>Connect to your servers. Stay focused on your work.</strong><br>
  A native SSH workspace built with Rust and GPUI Kit, bringing terminals, remote files, host status, and AI assistance together.
</p>

<p align="center"><a href="README.md">简体中文</a> · English</p>

<p align="center">
  <a href="https://github.com/cyruss648/keelshell/actions/workflows/ci.yml"><img src="https://github.com/cyruss648/keelshell/actions/workflows/ci.yml/badge.svg" alt="Quality"></a>
  <a href="https://github.com/cyruss648/keelshell/actions/workflows/release.yml"><img src="https://github.com/cyruss648/keelshell/actions/workflows/release.yml/badge.svg" alt="Release"></a>
</p>

<p align="center">
  <a href="#getting-started">Getting started</a> ·
  <a href="#features">Features</a> ·
  <a href="#ai-that-keeps-you-in-control">AI assistance</a> ·
  <a href="docs/README.md">Documentation</a> ·
  <a href="docs/ROADMAP.md">Roadmap</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

> **Development preview:** [Building from source](#build-from-source) is recommended today. Basic SSH, SFTP, session management, and reviewed AI requests are implemented, including transfer pause, continue, and content-verified resumption; manual reconnection in the original tab and optional bounded automatic reconnection are available. See the [capability ledger](docs/product/CAPABILITIES.md) for implementation and verification status.

![KeelShell remote terminal, host status, and SFTP workspace](assets/screenshots/workspace-macos.jpg)

<p align="center"><sub>A native macOS development build connected to an isolated loopback SSH/SFTP fixture. The interface is evolving.</sub></p>

KeelShell is for developers and operators who work with servers, logs, and remote files every day. Terminals, files, and host status follow the current SSH session as you switch tabs. The command bar keeps the destination visible, and AI suggestions are yours to review and execute.

The interface defaults to Simplified Chinese and supports English. The project targets macOS, Windows, and Linux, with a focus on remote SSH workflows. AI assistance is optional: server connections and SFTP file operations work independently.

## Features

| Workflow | Available functionality |
| --- | --- |
| **Connections** | When no remote session is active, start a one-time SSH session from the Quick connect form without writing a profile, or explicitly save it as a connection; nested folders, tags, favorites, recent connections, and trash; explicit selection and review for bulk folder moves, tags, favorites, trash, restoration, and permanent cleanup; profile editing/copying, search, JSON import/export, and clipboard OpenSSH config import with exact-host parsing, source locations, and confirmation review; password, private key, and SSH Agent authentication; explicitly switch a login attempt to keyboard-interactive/MFA prompts with one-time answers; up to four jump hosts with per-hop identity review and authentication; an optional SOCKS5 or HTTP CONNECT upstream proxy for each hop |
| **Remote terminals** | Session tabs and a two-pane split, ANSI/VT emulation, scrollback search, selection, paste, and CJK input |
| **Command workflow** | Per-session command history; multiline snippets with explicit parameters and full command previews; local history/snippet suggestions and explicitly requested remote command and path completion |
| **Batch commands** | Select connected SSH sessions, review the command, restricted per-target metadata markers (`{{name}}`, `{{host}}`, `{{port}}`, `{{user}}`, `{{endpoint}}`), concurrency, and timeout, then run independent jobs with per-target exit status and output, stop-pending-on-failure, and cancellation; completed runs retain a summary audit without command text, output, or addresses |
| **Dependency workflows** | Manually edit 1–128 tasks, explicitly select up to 32 authenticated SSH targets and prerequisites, then review source/rendered commands, targets, dependencies and execution options before running; inspect per-task states and bounded output, release dependents only on confirmed success, continue independent branches, hide/reopen and cancel |
| **Remote files** | SFTP browsing, file and recursive directory uploads/downloads, transfer review, progress, pause/continue and cancellation, content-verified file and directory resumption, an explicit read-only recovery check for failed transfers in the same SSH session, directory creation, rename and delete, text viewing, local diff preview against the read baseline, reviewed saves, and metadata comparison, SHA-256 content verification and explicitly confirmed bidirectional directory merges preserving destination-only entries |
| **Host status** | Linux CPU, memory, load, uptime, disk capacity, network counters, processes, and listening socket diagnostics |
| **Port forwarding** | Local/remote TCP forwarding and a loopback SOCKS5 proxy, with actual listener addresses, status, and stop controls |
| **AI assistance** | Named model API / Codex CLI / Claude Code profiles, Chat Completions/Responses/Anthropic Messages model discovery and connection testing, context selection, exact protocol request previews, output-token limits and conservative context-window admission, answers, and command suggestions; replies can become session-bound diagnostic plans with step-by-step review |
| **External MCP** | Disabled by default; explicitly granted active sessions, terminal fragments, SFTP directories/files and monitor caches are available to external agents; commands become proposals for human review and grants can be revoked immediately |
| **About and updates** | In-app project link, version and changelog; platform-aware GitHub Release checks, downloads and SHA-256 verification with opt-in install and restart |

Pausing waits for in-flight operations to be acknowledged before showing “Paused.” To resume a partial file or directory, explicitly select the source and destination, review the content verification, and confirm; after reconnecting or restarting, create a new resumption plan. See the [transfer verification record](docs/testing/records/2026-10-03-sftp-resume.md).

Select profiles in the connection library to review bulk folder moves, tags, favorites, trash, or restoration. Review includes selected profiles hidden by search and checks jump-host dependencies. Permanent cleanup requires separate confirmation of the exact profiles. Organization changes local metadata while established SSH sessions stay open. See the [connection library guide](docs/product/CONNECTION_LIBRARY.md) for the workflow and credential-retention rules.

After comparing folders, choose a content-verification direction and review synchronization. Each merge rechecks both sides, replaces files through same-directory temporary files and verifies the resulting hash. Limits are 64 MiB/file and 256 MiB/both sides; destination-only files are preserved, and links or type conflicts are refused. Cancellation or failure may leave completed operations; see the [directory merge record](docs/testing/records/2026-10-04-directory-sync-execution.md).

After disconnection, output and drafts are retained. Reconnection rechecks the complete route and server identities; click Continue when authentication is required. Review old commands again before running them. Transfers and tunnels are not restarted automatically. See the [reconnection verification record](docs/testing/records/2026-10-03-reconnection.md).

Click **Complete remotely** in the command bar, or press `Ctrl+Space`, to look up the command name or literal path at the caret. Relative paths use the displayed completion directory, which can be taken from the file panel or SFTP base; insertion uses the full path. A candidate replaces only the current word, preserving multiline and Unicode input for review before execution. The independent PATH query does not include interactive shell aliases, functions, or temporary environment changes; the terminal's own Tab completion remains available. See the [completion design and verification record](docs/testing/records/2026-10-03-remote-completion.md).

Enable parameters in the snippet editor to use `{{name}}` for literal values supplied this time. Review the preview before inserting it into the command bar; expanded commands skip session history by default. Batch tasks require explicit selection of connected sessions and can render a restricted metadata marker set per target; the review panel shows each final command before confirmation. Jobs run through independent SSH exec channels after review, without inheriting the interactive terminal's current directory. Cancellation cannot confirm that a remote process has stopped. See the [batch and diff verification record](docs/testing/records/2026-10-04-batch-audit-diff.md) and the [per-target template record](docs/testing/records/2026-10-04-reviewed-batch-templates.md) for this increment's validation status.

For operations with prerequisites, click **Dependency workflow** in the command area, edit tasks, and select their targets and prerequisites. Complete review shows each command as it will be sent and its route; execution starts only after human confirmation. Failed, unknown or skipped tasks block their dependents. A run can be hidden and reopened; tasks, commands and output stay in the current workspace without automatic retry or restoration. See the [dependency workflow guide](docs/product/DEPENDENCY_WORKFLOWS.md).

File actions wrap to the available width; editor, comparison and transfer details scroll while the browser retains room for real entries and Confirm/Cancel remain separately reachable. Short windows place the completion directory and its actions on one row to leave room for the remote terminal; opening candidates temporarily folds the file area, and closing them restores the same panel. Connection-library changes, dependency workflows and file layouts passed independent code/GPUI re-review, and the new compact layout passed the full local workspace checks, fresh independent review and controlled minimum-window macOS acceptance. Native desktop acceptance on Windows/Linux remains open. Source and CI verification are tracked in the [integration record](docs/testing/records/2026-10-05-workspace-workflows-integration.md) and the [SSH fixture record](docs/testing/records/2026-10-05-ssh-timeout-fixture-stability.md).

## AI that keeps you in control

1. **Configure a provider.** Create a named model API, Codex CLI or Claude Code profile. API profiles support Chat Completions, Responses and Anthropic Messages, including self-hosted services, with optional discovery and connection testing. Local CLI profiles require a native executable path, service base URL, explicit API key and model; checking the CLI only verifies version and capabilities. See the [local agent guide](docs/product/LOCAL_AGENTS.md).
2. **Choose the context.** Select terminal content, inspect the redacted request, and send it when ready.
3. **Review the suggestions.** Read the response, place any suggested command in the command bar, and confirm its content and destination before executing it.

API keys stay in memory by default and can be explicitly saved encrypted. Saving and unlocking a configuration make no network requests. Additional protocols and assistant workflows are tracked in the [roadmap](docs/ROADMAP.md).

Appearance supports System, Light and Dark, defaulting to System and preserving SSH sessions and unsent drafts when switched. macOS switching and restart persistence are verified; native checks on other platforms and the full visual matrix remain open. See the [theme record](docs/testing/records/2026-10-04-system-themes.md). Local CLI Ask has passed macOS window checks with installed CLI versions and a loopback service, including complete answers and cancellation. It uses explicit API credentials, does not reuse subscription login, and has no CLI tool execution. The visual refresh, Agent workflows and a KeelShell MCP server for external agents continue under the [development plan](docs/product/DESIGN_AND_AGENT_PLAN.md). MCP exposes KeelShell capabilities to external agents; a general client for third-party MCP services is outside scope.

External MCP now connects a separate stdio companion to desktop grants and real SSH/SFTP handles, defaulting disabled. Native checks of the standard macOS development package containing both programs against an isolated service covered explicit fragment/file reads, forbidden requests, human approval and rejection, revocation during execution, and restart with access disabled. These checks used an owned external protocol client; actual Codex/Claude Code MCP interoperability and native checks on other platforms remain open. See the [external MCP guide](docs/product/EXTERNAL_MCP.md), [desktop bridge record](docs/testing/records/2026-10-04-mcp-desktop-bridge.md), and [standard package verification record](docs/testing/records/2026-10-04-mcp-response-cancellation.md).

<details>
<summary>View AI settings</summary>

![KeelShell provider settings, model discovery, and connection testing](assets/screenshots/ai-settings-macos.jpg)

The screenshot uses a local mock service and contains no real provider credentials.

</details>

<details>
<summary>How credentials are stored</summary>

SSH passwords and private-key passphrases are used for one connection by default. A proxy username can be stored in the profile; its password is used only for the current connection. Explicitly saved SSH credentials go into a local vault encrypted with a master password, which is required again for every connection. Unlinking removes the profile reference while retaining the encrypted entry. Vault management supports inspection and removal of unlinked entries, along with master-password rotation.

After a restart, explicitly saved AI keys must be unlocked with the master password and applied to the assistant. Changing the service endpoint clears the old key reference. Connection JSON exports exclude local credential references and host-trust records.

</details>

## Getting started

Building from source is recommended during development. Versioned packages will be available from [GitHub Releases](https://github.com/cyruss648/keelshell/releases), with SHA-256 checksums.

Open **About / updates** from the toolbar to read the bundled changelog, check for a new version and download the matching release package. Downloads stay in a private temporary directory and are verified against the checksum published with the release. **Install and restart** starts a separate helper that revalidates the package manifest, replaces only listed files, and rolls back on failure. Development builds or non-standard installations remain reviewable for manual installation; signing, permissions and native installation acceptance still need to be completed in each release environment.

| Platform | Build targets | Package |
| --- | --- | --- |
| macOS 15+ | Apple Silicon / Intel | ZIP containing the `.app` bundle |
| Windows | ARM64 / x64 | ZIP containing the GUI / MCP executables and icon |
| Linux | ARM64 / x64; Ubuntu 24.04 baseline | `.tar.gz` with the executable and desktop resources |

See the [release verification record](docs/testing/records/2026-10-03-release.md) for actual build and desktop acceptance status. Distribution does not yet include macOS Developer ID signing, notarization, or Windows code signing. Packaging and tag-triggered publication are documented in [Releasing](docs/RELEASING.md).

### Build from source

Prepare the [platform build dependencies](.github/actions/setup-build/action.yml), install Rustup and Python 3.11+, and clone the repository. `rust-toolchain.toml` selects the pinned Rust toolchain.

```sh
git clone https://github.com/cyruss648/keelshell.git
cd keelshell
cargo build -p keelshell-app -p keelshell-mcp --locked
cargo run -p keelshell-app --locked
```

If you use [mise](https://mise.jdx.dev/), run `mise install` in the repository.

### Your first connection

1. When no remote session is active, fill in **Quick connect** with the server address, port, and username to start a one-time SSH session; to reuse it later, choose **Save as connection…** and then explicitly save the profile in the editor. Existing profiles remain available in the library and through **New connection**.
2. Connect, verify the server fingerprint against a trusted source, and approve it. A changed saved fingerprint blocks the connection until you review it again.
3. Use the remote terminal and file panel in the session tab. Host status is available for Linux servers. Configure a provider and model whenever you want AI assistance.

If you already maintain an OpenSSH config, copy its text and select **Import SSH config**. The importer accepts only exact Host, HostName, Port, User, IdentityFile, ProxyJump, and explicitly supplied Include content; wildcard, conditional, and ProxyCommand entries are reported for review and are never executed.

## Documentation

| Looking for | Start here |
| --- | --- |
| Available features and current limitations | [Capability ledger](docs/product/CAPABILITIES.md) |
| Product workflows, interaction rules, and plans | [Product design](docs/product/PRODUCT.md) · [Roadmap](docs/ROADMAP.md) |
| Repository structure, local development, and check commands | [Contributing](CONTRIBUTING.md) |
| Test coverage and platform verification | [Testing strategy](docs/testing/STRATEGY.md) · [Test records](docs/testing/records/) |
| Platform packages, checksums, and tag-triggered releases | [Releasing](docs/RELEASING.md) |

## Contributing

Report bugs, share feedback, and propose improvements through [Issues](https://github.com/cyruss648/keelshell/issues). Reproduction steps, anonymized profiles, and platform verification results are all useful. Start with [Contributing](CONTRIBUTING.md) for code and documentation changes.

## Acknowledgements

Built with [Rust](https://www.rust-lang.org/), [GPUI Kit](https://gpui-kit.com/), [Alacritty](https://github.com/alacritty/alacritty), [russh](https://github.com/Eugeny/russh), and their communities. Thank you to everyone maintaining these projects.
