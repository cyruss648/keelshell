<p align="center">
  <img src="assets/icons/png/256.png" width="112" height="112" alt="KeelShell application icon">
</p>

<h1 align="center">KeelShell</h1>

<p align="center">
  <strong>Connect to your servers. Stay focused on your work.</strong><br>
  A native SSH workspace built with Rust and GPUI Kit, bringing remote terminals, files, host status and AI assistance together.
</p>

<p align="center">
  <a href="README.md">简体中文</a> · English
</p>

<p align="center">
  <a href="https://github.com/cyruss648/keelshell/actions/workflows/ci.yml"><img src="https://github.com/cyruss648/keelshell/actions/workflows/ci.yml/badge.svg" alt="Quality"></a>
  <a href="https://github.com/cyruss648/keelshell/actions/workflows/release.yml"><img src="https://github.com/cyruss648/keelshell/actions/workflows/release.yml/badge.svg" alt="Release"></a>
</p>

<p align="center">
  <a href="#getting-started">Getting started</a> ·
  <a href="#features">Features</a> ·
  <a href="#ai-assistance-and-external-mcp">AI and MCP</a> ·
  <a href="docs/README.md">Documentation</a> ·
  <a href="docs/ROADMAP.md">Roadmap</a>
</p>

> **Development preview:** building from source is recommended. Cross-platform desktop flows, complete external-client workflows and installed updates remain under verification. See the [capability ledger](docs/product/CAPABILITIES.md) for scope.

![KeelShell remote terminal, host status and SFTP workspace](assets/screenshots/workspace-macos.jpg)

<p align="center"><sub>A native macOS development build connected to an owned, isolated SSH/SFTP service. The interface evolves with development.</sub></p>

KeelShell is for developers and operators who connect to servers, inspect logs and work with remote files. Terminals, files and host status follow the active SSH session, while commands and AI suggestions show their intended destination.

Simplified Chinese is the default, with English available. Appearance follows the system by default, or can be set to Light or Dark. The application focuses on remote SSH workflows; AI is optional.

## Features

| Workflow | Capabilities |
| --- | --- |
| **Connections** | One-time quick connections, folders, tags, favorites, recent connections and trash; reviewed bulk organization, import/export and encrypted profile sync; password, private key, SSH Agent and keyboard-interactive authentication; jump hosts and SOCKS5 / HTTP CONNECT upstream proxies |
| **Remote terminals** | Session tabs, a two-pane split, ANSI/VT terminal, scrollback search, selection, paste and CJK input; manual reconnection in the original tab and optional bounded automatic reconnection |
| **Remote files** | SFTP browsing, file and recursive directory transfers, pause and content-verified resumption; permissions, text editing, three-way merging with individual conflict choices and strict patch drafts; folder comparison, content verification, bidirectional merging and individually reviewed directory mirrors |
| **Commands and tasks** | Session history, snippets, explicit parameters, remote command/path completion; per-target batch commands, dependency workflows, finite schedules and read-only task history |
| **Monitoring and networking** | Linux host status and per-device disk I/O; listening sockets and remote DNS / TLS / HTTP(S) HEAD diagnostics; local/remote TCP forwarding and loopback SOCKS5 tunnels |
| **AI assistance** | Named model API / Codex CLI / Claude Code profiles; model discovery and connection testing, explicit context, complete request previews, answers, diagnostic suggestions and a finite Agent workflow reviewed each round |
| **External MCP** | External agents read explicitly granted session information, terminal fragments, SFTP and monitoring; commands and file changes are proposed for review and execution in the desktop app |
| **About and updates** | Project link, version and bundled changelog; platform-aware release checks, SHA-256 verified downloads and opt-in installation/restart |

Canceling a transfer or mirror retains completed operations; disconnection does not replay commands, transfers or tasks. Directory comparison/sync has explicit size, type and path limits. Protocol diagnostics require remote POSIX / Python 3.8+. See the [file workspace](docs/product/FILES_WORKSPACE.md), [directory mirrors](docs/product/DIRECTORY_MIRROR.md), [remote protocol diagnostics](docs/product/REMOTE_PROTOCOL_DIAGNOSTICS.md) and [capability ledger](docs/product/CAPABILITIES.md) for workflows and boundaries.

Text merging and patches first produce a local draft. Saving remotely requires complete review and a fresh baseline check. File reviews can expand while keeping confirmation and cancellation visible; drafts and patch inputs have explicit editing controls. See [review and editing](docs/product/FILE_REVIEW_AND_FOCUS.md) and the [text merge guide](docs/product/TEXT_CONFLICT_MERGE.md). Two macOS draft/save sequences against an owned SSH/SFTP fixture completed with full-content readback; complete UI and cross-platform acceptance remain open in the [limited native record](docs/testing/records/2026-10-07-text-merge-patch-native.md).

## AI assistance and external MCP

**Use AI inside KeelShell:** create a named model API or local CLI profile, select context explicitly, inspect the actual request and send your question. API profiles support Chat Completions, Responses and Anthropic Messages, with request headers, proxies, reasoning and sampling options. Suggested commands remain bound to the captured SSH target and require your review before execution.

Local Claude Code / Codex CLI use explicit API credentials and a controlled environment. The working directory is empty and isolated by default; you can also select and review an absolute path. KeelShell coordinates the in-app [Agent workflow](docs/product/REVIEWED_AGENT.md): you send every inference round and separately review each remote command, file read or existing-file replacement before preparing the next request from its result. Stop or loss of the original SSH session retires the run; an already issued operation can have an unknown outcome. Supplier CLI tools remain disabled. Subscription login, full desktop and cross-platform native acceptance of this workflow and selected directories remain open. See [local agents](docs/product/LOCAL_AGENTS.md), [API request options](docs/product/AI_REQUEST_OPTIONS.md) and [inference settings](docs/product/AI_INFERENCE.md).

**Let external agents use KeelShell:** KeelShell provides an MCP server. Its stdio companion connects to the running desktop application. You select the SSH sessions, tools, terminal fragments and directories an agent may access. Agents can read granted information or propose commands and complete replacements of existing UTF-8 files. KeelShell presents the complete target and content for review; clients cannot approve their own actions. Configuration and the eight tools are documented in the [external MCP guide](docs/product/EXTERNAL_MCP.md).

The two entry points have separate configuration. SSH and AI credentials are temporary by default; explicit persistence uses a vault encrypted with a master password. Profile exports exclude local credentials and host-trust records. See [AI design plan](docs/product/DESIGN_AND_AGENT_PLAN.md) and the [connection library](docs/product/CONNECTION_LIBRARY.md).

<details>
<summary>View AI settings</summary>

![KeelShell provider settings, model discovery and connection testing](assets/screenshots/ai-settings-macos.jpg)

The screenshot uses a local mock service and contains no real provider credentials.

</details>

## Getting started

Install Rustup, Python 3.11+ and the [platform build dependencies](.github/actions/setup-build/action.yml). `rust-toolchain.toml` selects the exact Rust toolchain. Build both the application and its MCP companion.

```sh
git clone https://github.com/cyruss648/keelshell.git
cd keelshell
cargo build -p keelshell-app -p keelshell-mcp --locked
cargo run -p keelshell-app --locked
```

If you use [mise](https://mise.jdx.dev/), run `mise install` in the repository.

1. Fill in **Quick connect** on the home screen with a host, port and username to start a one-time SSH session. Choose **Save as connection…** when you want to reuse it.
2. Verify the server fingerprint against a trusted source and authenticate. A changed saved fingerprint blocks the connection until reviewed.
3. Use the remote terminal and file panel. Linux hosts also provide monitoring; configure an AI provider/model when needed.

Copy an existing OpenSSH configuration and use **Import SSH config** to review supported exact Host and jump-host entries. Import does not execute commands from the configuration.

## Platforms and releases

| Platform | Targets | Package layout |
| --- | --- | --- |
| macOS 15+ | Apple Silicon / Intel | ZIP containing the `.app` |
| Windows | ARM64 / x64 | ZIP containing GUI / MCP EXEs and resources |
| Linux | ARM64 / x64; Ubuntu 24.04 baseline | `.tar.gz` containing binaries and desktop resources |

The tag-triggered release workflow defines six build targets. Successful tagged builds publish packages and checksums to [GitHub Releases](https://github.com/cyruss648/keelshell/releases). Actual builds, desktop workflows and installed updates are recorded separately; a configured workflow does not establish acceptance.

**About / updates** reads the changelog, checks versions and verifies downloads. Installation and restart require explicit confirmation. Platform signing, notarization and native update installation remain open; see [Releasing](docs/RELEASING.md).

## Documentation and contributing

| Content | Start here |
| --- | --- |
| Implemented capabilities, limits and plans | [Capability ledger](docs/product/CAPABILITIES.md) · [Roadmap](docs/ROADMAP.md) |
| Workflows and design decisions | [Documentation](docs/README.md) · [Product design](docs/product/PRODUCT.md) · [ADRs](docs/adr/) |
| Repository structure, local development and checks | [Contributing](CONTRIBUTING.md) |
| Behavior tests and platform evidence | [Testing strategy](docs/testing/STRATEGY.md) · [Test records](docs/testing/records/) |

Use [Issues](https://github.com/cyruss648/keelshell/issues) to report bugs, share feedback and propose improvements. Reproduction steps, anonymized profiles and platform details help diagnosis. Start with the contributing guide for code and documentation changes.

KeelShell is built on [Rust](https://www.rust-lang.org/), [GPUI Kit](https://gpui-kit.com/), [Alacritty](https://github.com/alacritty/alacritty), [russh](https://github.com/Eugeny/russh) and other open-source projects. Thanks to their contributors.
