<p align="center">
  <img src="assets/icons/png/256.png" width="112" height="112" alt="KeelShell 应用图标">
</p>

<h1 align="center">KeelShell</h1>

<p align="center">
  <strong>连接服务器，专注眼前的工作。</strong><br>
  Rust 与 GPUI Kit 构建的原生 SSH 工作区，集远程终端、文件、主机状态和 AI 助手于一处。
</p>

<p align="center">
  简体中文 · <a href="README.en.md">English</a>
</p>

<p align="center">
  <a href="https://github.com/cyruss648/keelshell/actions/workflows/ci.yml"><img src="https://github.com/cyruss648/keelshell/actions/workflows/ci.yml/badge.svg" alt="Quality"></a>
  <a href="https://github.com/cyruss648/keelshell/actions/workflows/release.yml"><img src="https://github.com/cyruss648/keelshell/actions/workflows/release.yml/badge.svg" alt="Release"></a>
</p>

<p align="center">
  <a href="#开始使用">开始使用</a> ·
  <a href="#功能">功能</a> ·
  <a href="#ai-助手与对外-mcp">AI 与 MCP</a> ·
  <a href="docs/README.md">文档</a> ·
  <a href="docs/ROADMAP.md">路线图</a>
</p>

> **开发预览**：建议从源码运行。跨平台桌面、完整外部客户端流程及安装更新仍在验收中；功能范围以[能力清单](docs/product/CAPABILITIES.md)为准。

![KeelShell 的远程终端、主机状态与 SFTP 文件工作区](assets/screenshots/workspace-macos.jpg)

<p align="center"><sub>macOS 开发版本实机截图，连接自有隔离 SSH/SFTP 服务。界面随版本迭代。</sub></p>

KeelShell 面向需要连接服务器、查看日志和处理远程文件的开发者与运维人员。终端、文件和主机状态跟随当前 SSH 会话，命令与 AI 建议显示明确的工作目标。

默认简体中文，可切换英文；外观默认跟随系统，也可选择浅色或深色。应用专注远程 SSH，AI 可按需启用。

## 功能

| 工作流 | 能力 |
| --- | --- |
| **连接管理** | 一次性快速连接、连接目录树、标签、收藏、最近连接与回收站；批量组织、导入导出、审核式加密配置同步；密码、私钥、SSH Agent 与键盘交互认证；跳板与 SOCKS5 / HTTP CONNECT 上游代理 |
| **远程终端** | 多会话标签、双栏分屏、ANSI/VT 终端、滚动区搜索、选择、粘贴与中文输入；原标签手动重连及可选的有界自动重连 |
| **远程文件** | SFTP 浏览、文件和递归目录传输、暂停与内容校验续传；权限修改、文本编辑与差异审核；目录比较、内容校验、双向合并及逐项审核的目录镜像 |
| **命令与任务** | 会话历史、命令片段、显式参数、远端命令与路径补全；逐目标批量命令、依赖工作流、有限定时与只读任务记录 |
| **监控与网络** | Linux 主机和逐设备磁盘 I/O 状态；监听端口、远程 DNS / TLS / HTTP(S) HEAD 诊断；本地及远端 TCP 转发、回环 SOCKS5 隧道 |
| **AI 助手** | 命名模型 API / Codex CLI / Claude Code 配置；模型发现与连接测试、明确上下文、完整请求预览、回答及审核式诊断建议 |
| **对外 MCP** | 外部智能体读取明确授权的会话信息、终端片段、SFTP 与监控；命令和文件修改先提交提案，在桌面应用中审阅后执行 |
| **关于与更新** | 项目主页、版本和内置变更日志；按平台检查发布、SHA-256 校验下载，以及可选的安装和重启 |

传输与镜像取消会保留已完成项；断线后不会自动重放命令、传输或任务。目录内容比较和同步具有明确的大小、类型及路径限制，协议诊断需要远端 POSIX / Python 3.8+。具体操作与边界见[文件工作区](docs/product/FILES_WORKSPACE.md)、[目录镜像](docs/product/DIRECTORY_MIRROR.md)、[远程协议诊断](docs/product/REMOTE_PROTOCOL_DIAGNOSTICS.md)和[能力清单](docs/product/CAPABILITIES.md)。

## AI 助手与对外 MCP

**在 KeelShell 内使用 AI**：创建命名配置，选择模型 API 或本地 CLI；明确选择上下文，检查实际请求，再发送问题。API 支持 Chat Completions、Responses 和 Anthropic Messages，可配置请求头、代理、推理及采样选项。回复中的命令绑定捕获的 SSH 目标，经你审阅后再执行。

本地 Claude Code / Codex CLI 当前提供单轮 Ask，使用显式 API 凭据、隔离目录与受控环境；订阅登录、自定义工作目录和受限 Agent 工作流仍在开发。配置和限制见[本地智能体](docs/product/LOCAL_AGENTS.md)；API 选项见[请求配置](docs/product/AI_REQUEST_OPTIONS.md)与[推理设置](docs/product/AI_INFERENCE.md)。

**让外部智能体使用 KeelShell**：KeelShell 提供 MCP 服务端，stdio 伴随程序连接正在运行的桌面应用。你选择可访问的 SSH 会话、工具、终端片段和目录。外部智能体可以读取授权信息，或提交命令、现有 UTF-8 文件替换提案；完整目标与内容在 KeelShell 中审阅，客户端不能自行批准。配置与八项工具见[对外 MCP 指南](docs/product/EXTERNAL_MCP.md)。

这两个入口独立配置。SSH 与 AI 凭据默认临时使用，明确保存时进入主密码加密凭据库；配置导出不携带本机凭据或主机信任。详见[AI 设计计划](docs/product/DESIGN_AND_AGENT_PLAN.md)及[连接库](docs/product/CONNECTION_LIBRARY.md)。

<details>
<summary>查看 AI 配置界面</summary>

![KeelShell AI 服务配置、模型发现与连接测试](assets/screenshots/ai-settings-macos.jpg)

截图使用本机模拟服务，不包含真实服务凭据。

</details>

## 开始使用

安装 Rustup、Python 3.11+ 和[平台构建依赖](.github/actions/setup-build/action.yml)。仓库的 `rust-toolchain.toml` 选择精确 Rust 工具链；应用和 MCP 伴随程序需一起构建。

```sh
git clone https://github.com/cyruss648/keelshell.git
cd keelshell
cargo build -p keelshell-app -p keelshell-mcp --locked
cargo run -p keelshell-app --locked
```

使用 [mise](https://mise.jdx.dev/) 时，可在仓库内执行 `mise install`。

1. 在首页 **快速连接** 中填写地址、端口和用户名，开始一次性 SSH 会话；需要复用时选择 **保存为连接…**。
2. 与可信来源核对服务器指纹，完成认证后进入工作区。已保存指纹变化时，应用会阻止连接并要求重新核对。
3. 使用远程终端和文件面板。连接 Linux 主机可查看监控；需要 AI 时再配置服务与模型。

已有 OpenSSH 配置可复制后使用 **导入 SSH 配置**，审阅支持的精确 Host 与跳板条目；导入不会执行配置中的命令。

## 平台与发布

| 平台 | 构建目标 | 发布布局 |
| --- | --- | --- |
| macOS 15+ | Apple Silicon / Intel | `.app` ZIP |
| Windows | ARM64 / x64 | GUI、MCP EXE 与资源 ZIP |
| Linux | ARM64 / x64，Ubuntu 24.04 基线 | 程序与桌面资源 `.tar.gz` |

标签发布流水线配置了六个构建目标。成功的标签构建会将发布包与校验值发布到 [GitHub Releases](https://github.com/cyruss648/keelshell/releases)；实际构建、桌面和安装验收分别记录，不能由流水线配置推导为完成。

应用内 **关于/更新** 可读取变更日志、检查版本并校验下载，安装与重启需明确确认。平台签名、公证和安装更新验收仍开放，详见[发布说明](docs/RELEASING.md)。

## 文档与参与贡献

| 内容 | 入口 |
| --- | --- |
| 已实现能力、限制与开发计划 | [能力清单](docs/product/CAPABILITIES.md) · [路线图](docs/ROADMAP.md) |
| 操作指南与设计决策 | [文档目录](docs/README.md) · [产品设计](docs/product/PRODUCT.md) · [ADR](docs/adr/) |
| 工程结构、本地开发与检查 | [贡献指南](CONTRIBUTING.md) |
| 行为测试与平台证据 | [测试策略](docs/testing/STRATEGY.md) · [测试记录](docs/testing/records/) |

欢迎通过 [Issues](https://github.com/cyruss648/keelshell/issues) 报告问题、分享体验和提出建议。提供复现步骤、匿名配置和平台信息有助于定位问题；代码与文档贡献请从贡献指南开始。

KeelShell 构建于 [Rust](https://www.rust-lang.org/)、[GPUI Kit](https://gpui-kit.com/)、[Alacritty](https://github.com/alacritty/alacritty)、[russh](https://github.com/Eugeny/russh) 等项目之上。感谢这些项目及其贡献者。
