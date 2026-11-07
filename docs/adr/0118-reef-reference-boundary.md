# ADR 0118：Reef / reef-template 工程参考边界

- 日期：2026-11-07
- 状态：已采纳为文档边界；本 ADR 不引入 Reef 依赖
- 范围：workspace 工程组织、版本继承、配置分层、生命周期与证据管理
- 关联：[Reef 参考评估](../research/reef-assessment.md)、[工程交接](../HANDOFF.md)、
  [开发路线](../ROADMAP.md)

## 背景

KeelShell 是使用 GPUI Kit 的原生桌面应用，UI、领域与存储、会话传输、AI
分别由 `keelshell-app`、`keelshell-core`、`keelshell-session` 和
`keelshell-ai` 承担。Reef 是面向服务端应用的基础框架，reef-template 是
带有生成器占位符和服务应用工程设施的模板。两者有工程组织上的参考价值，
但它们的运行时边界、部署方式和依赖来源不适合直接成为桌面应用的基础。

本 ADR 固定本轮对用户指定 Reef 与 reef-template checkout 的只读检查结论，
避免后续维护代理把“参考工程模式”误解成“必须接入 Reef”。已有的详细源码
评估仍保存在[研究记录](../research/reef-assessment.md)；本 ADR 补充当前
checkout 的 revision、采用范围和禁止引入的边界。

## 证据快照

本轮只读检查的来源和结果如下。源仓库没有被本任务修改、提交或清理。

| 来源 | revision 与工作树 | 检查的工程证据 |
| --- | --- | --- |
| Reef | `75ccbf5d864fadb543ecfa62fc9871c7cff04153`；`main` 与远端同名分支对齐，工作树已有一处 `Cargo.toml` 修改 | `Cargo.toml` 的 resolver、workspace package 和集中依赖；`crates/reef-compose/Cargo.toml`、`src/module_export.rs` 的 feature 装配与模块导出；`crates/reef-compose/src/application/{builder,runtime,handle,report}.rs` 的 owner、启动、失败回滚和停机报告；`crates/reef-core/src/config/{config_load.rs,local/reader.rs,dynamic/store.rs}` 的配置读取与 revision/snapshot 机制 |
| reef-template | `03bd71c096fdd941e389584c44bdcb40dfed0cdf`；工作树干净 | 根 `Cargo.toml` 的 resolver 3、workspace package 和成员继承；`crates/app/Cargo.toml`、`build.rs`、`src/main.rs`；`crates/app/conf/config.toml`、`config-dev.toml`、`config-release.toml`；`docs/engineering-sync.md`、`docs/template-adaptation.md` 和 README 中的生成、配置、验证约定 |

这些来源只证明当前本地 checkout 的静态结构。它们不证明 Reef 的远端最新
状态、模板生成物的多平台桌面行为，也不替代 KeelShell 的编译、原生窗口、
真实 SSH、Podman 或外部 AI provider 验收。本轮没有在模板根目录直接执行
Cargo 构建，也没有把本机路径写入 KeelShell 依赖或构建要求。

## 决策

### 1. 借鉴 workspace resolver、版本继承和职责边界

Reef 的 workspace 使用 resolver 3，集中声明 workspace package 元数据和依赖，
成员清单按职责拆分；模板的应用 crate 通过 `version.workspace`、
`edition.workspace` 和其它 workspace 字段继承公共约束。KeelShell 继续维护
自己的 workspace 与四个产品边界，沿用以下原则：

- workspace 负责公共 edition、包元数据、直接依赖约束和锁文件策略；
- 业务 crate 只依赖所需的边界，不通过 UI 反向拥有传输或 AI 的实现；
- 直接 registry 依赖继续使用 `x.y`，应用保留解析后的 `Cargo.lock`，验证使用
  `--locked`；
- Reef 的 workspace 成员、feature 名称和 server middleware 不复制到
  KeelShell。

这部分是工程组织参考，不代表将 `reef`、`reef-core` 或 `reef-compose` 加入
KeelShell，也不改变当前 GPUI Kit 的应用入口。

### 2. 借鉴配置分层与验证后发布

reef-template 将公共、开发和发布配置放在启动 crate 的 `conf/` 中，Reef
通过配置加载器和环境覆盖组装值；Reef Core 的动态配置类型支持按 revision
读取相关字段。KeelShell 只采用其中的原则：

- 配置 schema、版本、迁移和生效时机要有明确记录；
- 相关配置读取使用一个经过校验的 snapshot，错误不能悄悄覆盖最后可用状态；
- 主题、语言等即时设置与连接、代理、超时等下次连接生效的设置要区分；
- 凭据继续由 OS credential store 或加密 vault 管理，不进入普通 profile
  元数据。

不复制依赖进程当前目录的 `conf/`、`../log` 和 `bin/` 服务部署约定。桌面
应用从 Finder、Start Menu 或 Linux desktop entry 启动时，配置、缓存、日志和
临时 fixture 必须使用 KeelShell 自己的平台路径与显式注入的隔离目录。

### 3. 借鉴显式生命周期、反向停机和 owner 归属

Reef Compose 的 `ApplicationBuilder`、runtime、handle 和 report 代码把资源
owner、启动顺序、失败后的逆序清理、取消与 join、停机结果分开表达。KeelShell
采用同样的不变量，但使用桌面应用自己的类型：

- 每个窗口、Workspace、SSH/PTY 会话、传输任务和 AI worker 都要有明确 owner；
- 关闭或替换资源前先阻止新的工作，再发出有界停止请求并等待 worker/channel
  结束；
- `Drop` 或取消信号本身不等于已经停机，未知清理结果要保留为
  `UNKNOWN`/待协调状态，不能自动重放可能产生副作用的操作；
- GPUI 线程只负责事件和渲染，阻塞的 PTY、文件系统、SSH、SFTP 和 AI I/O
  由对应后台 owner 管理；
- 不采用 Reef 的全局 `compose_init()`、server listener 或服务进程关闭模型。

### 4. 借鉴证据与工程同步管理

Reef 和 reef-template 都把工程脚本、配置、生成规则和验证记录分开维护。
KeelShell 将这一点落实为：

- 设计决策进入 `docs/adr/`，产品行为进入 `docs/product/`，剩余工作进入
  `docs/ROADMAP.md`，交接和当前事实进入 `docs/HANDOFF.md`；
- 每轮验证记录精确源码 SHA、命令、退出码、实际测试计数、构建产物
  provenance、平台和限制；
- 源码检查、构建/测试、原生桌面观察、真实远端/业务验收分别报告，不能用
  编译成功、headless 测试或代理启动代替原生和业务结论；
- 失败、`UNKNOWN`、`HOLD` 和未验证边界保留在对应记录中，资源清理由 owner
  和消费者关系决定。

这套证据管理可以借鉴 Reef 的工程习惯，但不复制其 CI runner、中间件矩阵、
服务部署脚本或发布工具。

## 明确边界

本 ADR 及后续按本 ADR 发展的桌面切片不得引入下列内容：

1. Reef server runtime、全局服务端 Compose、HTTP/gRPC listener 或服务端
   middleware 运行时；
2. Reef 当前 workspace 使用的私有 Git 依赖、内网 URL、开发机绝对路径或
   其它无法由公开项目独立解析的来源；
3. Reef 的 HTTP、MQ、DB、Outbox、Cache、Discovery、Actuator、Storage、
   Telemetry 等服务端能力；
4. reef-template 的 Liquid 模板、服务 wrapper、Maven/服务端发布流程或
   `conf/` 工作目录约定直接进入 KeelShell。

KeelShell 已有的 SSH、SFTP、终端、外部 MCP 和 AI provider 能力继续由各自的
产品与安全 ADR 管理；本边界只禁止把 Reef 的服务端设施作为桌面依赖或隐式
运行时。任何未来确实需要 Reef 接入的方案，必须另建 ADR，固定可公开的
依赖来源、接口 revision、平台范围、配置和停机验收后才能实施。

## 结果与后续

本 ADR 是可独立合并的文档切片，不改变 Rust 代码、Cargo manifest、Cargo.lock、
主题 registry、运行时或现有功能状态。后续 agent 在设计工程配置、资源 owner
或测试证据时应引用本 ADR，并同时读取当前交接和路线图。

如果未来 KeelShell 拆出独立服务端，服务端可以单独评估 Reef；该评估不能反向
改变原生桌面客户端的 crate 分层、凭据边界、审核式 AI 或跨平台验收要求。
