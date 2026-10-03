# Reef / reef-template 只读参考评估

审查日期：2026-10-02（Asia/Shanghai）。用途：为基于 GPUI / gpui-kit 的跨平台 Shell 管理桌面产品确定工程与生命周期基线。

## 结论

**有较高工程参考价值；首版不直接依赖 Reef，也不从 reef-template 原样生成产品。** 采用其模块分层、集中依赖、显式生命周期、可验证的测试证据等模式，重新实现轻量桌面架构。未来若增加独立团队服务端，可单独评估 Reef 作为服务端框架，保持桌面客户端可独立构建。

这是针对当前可访问代码的判断，不是对 Reef 质量的否定：Reef 是服务端基础框架，当前 `reef-core` 无条件依赖私有 Git 来源的 `git-info`，`crypto` 又引入另一私有 Git 包。未来 GitHub 用户无法仅凭公开 registry 解析整个依赖图。模板本身同样固定内网 Git 依赖，且运行/配置目录、服务包装与 Maven 分发均围绕服务应用设计。

本次只读，没有修改、提交或清理两个源仓库；没有执行它们的完整构建或集成测试。报告中的源码行为是静态核验，不能当成最新远端、目标平台或实机运行验收。

## 来源与状态

| 来源 | 固定版本依据 | 当前工作区 | 使用方式 |
| --- | --- | --- | --- |
| Reef | `011702b081841cfc3078122a6f50773a239b83ff`，workspace `0.4.26`，提交日期 2026-10-02 | 初始检查有 207 个 staged 路径、1 个 untracked review 文档；正在被其它工作修改 | 关键依赖、实例生命周期建议额外以 `git show HEAD:<path>` 核对；其余标注为当前树参考 |
| reef-template | `03bd71c096fdd941e389584c44bdcb40dfed0cdf`，提交日期 2026-09-28 | 干净 | 模板源码、生成规则和脚本只读审查 |
| reef-project skill | 本会话安装的 `SKILL.md`，及 `references/configuration.md`、`references/entrypoints.md` | 只读 | 用于识别 Reef 的真实配置/启动约定，不把它变成无关桌面产品必须使用 Reef 的理由 |

本机定位（仅作为研究来源，不写入 Cargo path 依赖、不作为他人构建要求）：

- Reef checkout：`<local-checkouts>/reef`。
- 模板 checkout：`<local-checkouts>/reef-template`。
- 已查看 Reef `AGENTS.md`。模板目录及其直接父目录未发现 `AGENTS.md`。

所有下文代码证据用仓库相对路径表示。未将内网 URL、组织作者邮箱或私有源码复制到产品文件。

## 值得采用的工程模式

| 模式 | 当前证据 | 桌面产品落地 |
| --- | --- | --- |
| Workspace 与边界 | Reef `Cargo.toml`、`crates/reef-compose/src/module_export.rs`；模板 `Cargo.toml`、`crates/app/Cargo.toml` | edition 2024 / resolver 3；根统一 dependencies、package metadata、lints，成员显式继承；领域模型不依赖 GPUI，平台实现不反向依赖 UI |
| 少量有职责的 crate | Reef Core / Compose / middleware 分层 | 先用 `core`（连接配置、状态、风险策略）、`runtime`（PTY/SSH/传输/监控）、`ai`、`app` 四个边界，存储与平台适配可先作模块；规模增长再拆，避免为占位功能制造空 crate |
| 清晰 feature 组合 | Reef `crates/reef-compose/Cargo.toml`、`scripts/.githooks/project_checks.py` | 只启用实际使用的库 feature；平台后端用 `cfg` 隔离；分别检查最小 core 与 GUI，避免 GUI 系统库让领域测试无法在无显示环境运行 |
| 依赖集中与锁 | Reef 根依赖大多采用 `x.y`；模板 README 明确应用提交 `Cargo.lock` | 所有 registry 版本约束只用 `x.y`，应用必须提交精确解析后的 `Cargo.lock`，CI 使用 `--locked`；版本清单记录检查日期与采用理由 |
| 原生工具与 hooks | 模板 `scripts/dev-setup/pre-commit_install.py`、`.pre-commit-config.yaml` | 检查现有工具再安装本仓库 hooks，保留已有 hooksPath；`fmt`、Clippy、单元/集成测试及提交消息检查使用真实工具；不能靠跳过 hook 完成提交 |
| 测试分层 | Reef `.config/nextest.toml`、`scripts/integration/scenarios.toml`、`scripts/integration/run.py` | 默认单元测试与本地 fixture 测试无外部账户依赖；本地 SSH fixture 测认证、PTY、SFTP、隧道；真实 OS/UI/provider 验收显式单独记录 |
| 保留失败与当前产物证据 | Reef 当前树 `scripts/validation/evidence.py`、`scripts/ci/verify.py` | 每轮独立记录命令、退出码、源码/锁摘要、平台、测试统计；GUI 验收必须运行当次构建产物；原始日志放忽略目录，摘要放 `docs/testing/records/` |
| 设计与配置文档 | Reef `docs/design/`、`docs/config/`、模块示例 README | 维护产品能力矩阵、架构说明、ADR、测试计划/记录、发布说明；能力状态区分 planned、implemented、validated、platform-pending |
| 说明不变量的注释 | Reef `application/builder.rs`、`application/handle.rs` 的所有权/取消说明 | 注释解释资源所有者、失败回滚、线程边界和安全边界；避免为每个函数重复作者/日期头，历史由 Git 维护 |

需要改进而不照搬的点：两个仓库根都没有 `mise.toml` 或 `rust-toolchain.toml`；新桌面产品应增加项目本地工具链管理。模板默认 Nextest 带 `--no-tests pass`，适用于尚无业务的最小模板；已有 Shell/AI 功能的产品门禁不能把零用例作为通过。Reef 作为库忽略根 Cargo.lock，这不适用于可执行桌面产品。

## 配置与启动/停机代码的参考价值

### 配置：采用“类型化 + 验证后发布”的原则，调整路径和凭据边界

已核验 `crates/reef-core/src/config/config_load.rs`、`config/local/reader.rs`：默认搜索与当前工作目录有关，配置覆盖按公共、环境及显式来源处理。`config_properties` / `ConfigStore` 支持类型化读取与同 revision snapshot。Reef `AGENTS.md` 强调配置变更不等于已创建连接池或 listener 自动重建。

适用于桌面的部分：

1. 持久化配置必须有 schema/version、加载校验和迁移；相关字段以单个 snapshot 读取。
2. UI 主题/字体属于可即时生效项，SSH 身份、代理与连接超时属于下一次连接生效项；在设置 UI 说明生效时机。
3. 无效配置不能悄悄覆盖最后可用状态；对错误提供可操作提示。
4. 连接元数据与凭据分离；凭据存 OS keychain / credential store，导出默认不含密码/私钥/API token。

不直接照搬：依赖进程 cwd 的 `conf/`、`../log` 与 `bin/` 部署约定。桌面应用从 Finder、Start Menu、desktop entry 启动，cwd 并不稳定；应使用平台应用数据/配置/缓存目录，开发 fixture 显式注入临时目录。Reef 的配置密文包装不代替桌面 OS 凭据库。

### 启动：GPUI 拥有 UI 线程，业务 runtime 明确归属

`crates/reef-core/src/common/start.rs` 的 `reef_core_init!` 组合 `.env`、CLI/version、日志/TLS 引导。`crates/reef-compose/src/start.rs` 的 `compose_init()` 明确只允许进程内一次初始化，失败不能直接重试。这适合服务进程；连接管理桌面应用则需要每个会话可重连、每个窗口可独立关闭。

建议：GPUI 主线程负责事件和渲染；后台 runtime 拥有连接、传输与 AI 流式任务；通过有界消息/事件传递状态，不在 Render 回调里连接网络、阻塞读取或等待 tokio runtime。

### 停机：最值得参考的实际代码

HEAD 已核验：

- `crates/reef-compose/src/application/builder.rs`：`ManagedComponent` 把已启动资源及其 shutdown 回调一起交付；构造阶段检查重复名字/类型；按顺序启动、失败后逆序清理。
- `crates/reef-compose/src/application/handle.rs`：应用持有生命周期 JoinHandle；取消某次等待不丢 owner；`shutdown().await` 等待生命周期任务；`Drop` 只请求取消。
- `crates/reef-compose/src/application/runtime.rs`：顺序与失败回滚机制（已阅当前树，其扩展正在 staged 中）。

桌面产品采用同样的所有权原则：每个会话持有 PTY/SSH channel、读写任务与取消句柄；关闭 tab 必须取消并 join，确认子进程/远端 channel 已关闭后更新终态；关闭应用先阻止新任务，再处理传输与 AI，再停止会话，再关闭持久化和日志。AI 请求取消与终端命令终止是不同动作。SSH 断线后执行结果可能未知，不能自动重放修改性命令。

无需复制 Reef 的全局 Compose；桌面 app 的 SessionManager 与 OperationManager 可以把此原则实现成较小的本地类型。若未来确需直接 Reef 实例接入，优先重新验证 `reef/application` API，禁止混用全局 ConfigLoader/Compose 和独立实例状态。

## reef-template 哪些不能原样复制

1. 模板根含 Liquid 占位符，必须 cargo-generate 后才能运行 Cargo；本次没有误把它当 Rust 应用执行构建。
2. 当前模板固定 Reef `v0.4.25`，而本地 Reef HEAD 是 `0.4.26`；不能据模板版本宣称采用最新 Reef。
3. 根 manifest `rust-version = "1.98.1"` 只是模板的当前要求，不能据此证明它是当前公开最新 Rust；新项目须独立验证实际可用 toolchain。
4. `scripts/build/build_task/build_platform.py` 含 Windows/Linux/macOS 目标，macOS 默认只构建本机架构；这是构建选择逻辑，不能证明目标平台 GUI、PTY/ConPTY、IME、keychain 或窗口行为。
5. Linux 服务端 musl 构建、Windows service wrapper、Maven deploy/release 流程不适用于桌面分发。GPUI 所需 Linux 图形库、macOS .app 和 Windows GUI 包装需要独立打包设计。
6. `.cargo/config.toml` 强制 MSVC 静态 CRT，release `panic=abort`、fat LTO 也不能盲目复制：先核实所选 GPUI/终端/SSH/native 依赖的要求与调试体验。
7. `just release` 会组合 changelog commit/push/deploy，初次研究时用户只授权本地提交，未复用这个入口。2026-10-03 用户新增公开 GitHub 与标签发布授权；本项目以独立、可验证的 GitHub Actions 实现，见发布说明。
8. 私有 Git dependency、内网 URLs、作者身份、源码绝对路径均不写入可公开产品依赖。

## 对新项目的可审查约束

- `Cargo.toml` 中外部 registry dependencies 使用 `x.y`，内部 crate 使用仓库相对 path；无开发机绝对 path、无隐式内部 Git dependency。
- Rust/tooling 本地固定，记录与 GPUI 所需 MSRV 的关系；`Cargo.lock` 可包含 x.y.z 实际版本，这是可重现解析信息，不是 manifest 版本约束例外。
- macOS、Windows、Linux 分别有 CI job；尚未真实运行的 job/平台写明 pending。本机 check 不得称三平台验收。
- UI 只显示已实现并接通的能力；未完成项需清晰状态，避免 demo 数据伪装真实在线主机。
- AI 建议通过执行计划与人工确认进入统一命令执行器，复用主机身份、取消、日志与风险策略；不允许 AI 自建无审计 SSH 通道。
- 单元测试验证状态机/路径/风险策略；集成测试验证存储 round-trip、PTY 交互与退出、SSH host-key/认证/超时、SFTP 完整性与取消；UI 验收包含中文 IME、字体、复制粘贴、窗口缩放和键盘导航。
- 审查引用保持 revision + 相对路径；未来升级 Reef 参考设计时重新核验，不能把本次正在变动的工作树视为固定公共 API。

## 本次验证记录

只读操作：搜索和读取适用说明、manifest、模板规则、配置/生命周期源码、CI/hooks、构建脚本；`git status --short`、`git rev-parse HEAD`、`git log -1`、关键文件 `git show HEAD:<path>`。没有修改源仓库，没有运行服务、生成项目、安装工具或发起部署。

本报告完成的是“是否值得参考及如何采用”的源码评估。依赖的当前公共最新版本、gpui-kit 平台成熟度、终端渲染库/API、公开发布构建可用性需由新项目对应研究与验收记录提供证据。
