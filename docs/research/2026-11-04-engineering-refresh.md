# 2026-11-04 工程参考与依赖版本独立核验

日期：2026-11-04，Asia/Shanghai。范围：Reef、reef-template 与 KeelShell 的只读源码、工程配置及官方依赖元数据核验。仅本研究文档允许写入。

用户在本轮将依赖要求更新为：**“依赖的所有版本每次都要检查并使用最新版本，并且只用 x.y 定义版本。”** 因此，下文补丁候选仅供拆分实施与定位回归使用；停留在旧兼容系列不能算满足最新要求。本文不执行升级，不提供升级后构建、测试或原生验收结论。

## 结论

1. Reef 的独立 Application 生命周期所有权、失败逆序回收、配置修订，以及绑定源码、锁和本次二进制的验证报告仍有参考价值。KeelShell 已有自己的五 crate 边界和 GPUI/Tokio 分工，不需要为了复用这些模式引入服务端框架。
2. Reef 仍有必需的私有 Git 依赖；模板仍引用 Reef `v0.4.25`，不是当前本机 Reef 的 `0.4.26`。两者均不宜作为公开桌面产品的直接依赖或原样生成起点。
3. **旧研究中的固定工具链建议已被当前合同覆盖。** KeelShell 当前没有 Rust toolchain pin，也没有 `rust-version` 字段。模板中的 `rust-version = "1.98.1"` 是最低版本声明，不是固定编译器选择器。
4. KeelShell 的 6 份 manifest 共 35 条显式 registry 版本要求，覆盖 34 个包名，全部符合 `x.y`。其中 `anyhow` 只是未被成员继承的 workspace 声明；实际成员直接使用 33 个 registry 包名。已逐包查询官方 crates.io，包括 target-specific、build 和 dev dependencies。
5. 与查询时最新稳定版相比，15 个包的现有锁定版本需要更新，其中 10 条显式 manifest 要求需要更换 `x.y`。GPUI Kit、SSH、随机数、Base64、ZIP 跨兼容线更新应单独验证。
6. Python 图标依赖应从 `Pillow~=12.3` 更新至 `Pillow~=13.0`。5 个外部 GitHub Actions 中，checkout 已最新，其余 4 个有新稳定 release；应更新到对应完整 commit SHA。

## 来源、基线与适用说明

先读取 KeelShell 的 `AGENTS.md`、`docs/HANDOFF.md` 和用户指定的命令输出约定，再检查来源仓库说明。限定搜索 `work/develop` 未找到两个来源，随后通过本机项目登记定位 Reef，并在其相邻模板目录找到 reef-template。下列路径相对于本机 `work` 根，仅用作来源定位，不进入 Cargo path dependency；不记录私有远端地址。

| 仓库 | 本机来源位置 | 本次读取的 HEAD | 提交日期 / 状态 |
| --- | --- | --- | --- |
| Reef | `kingstar/projects/Ferris/base/reef` | `75ccbf5d864fadb543ecfa62fc9871c7cff04153` | 2026-10-03；既存未暂存 `Cargo.toml` 改动 |
| reef-template | `kingstar/projects/Ferris/template/reef-template` | `03bd71c096fdd941e389584c44bdcb40dfed0cdf` | 2026-09-28；干净 |
| KeelShell | `develop/projects/keelshell` | `4cd1809b2e1b02ddfcae0c2cdef36a060a9affd7` | 核验开始时干净 |

Reef 适用根 `AGENTS.md`；在所查来源目录及其祖先中未发现额外覆盖说明，reef-template 未发现 `AGENTS.md`。Reef 指南明确区分全局 Compose 的一次尝试与独立 Application，要求真实资源关闭、显式 feature 矩阵和证据边界。本文只读取这些指南及实现，没有运行来源仓库的 hook、生成器、Cargo、CI 或集成入口。

上述均为**本机 HEAD**，没有 fetch 两个私有来源，也没有声明它们是远端最新。Reef 的唯一已见工作树差异为 `mac_address = "1.1"` → `"1.2"`；本次研究未产生该差异。生命周期、测试和 CI 实现文件没有已见工作树差异，引用绑定上述 HEAD；根依赖比较同时区分 HEAD 与工作树。

| 输入 | SHA-256 |
| --- | --- |
| Reef HEAD `Cargo.toml` | `f0dcaa29a6cee098b502aa8ae75045453a6cab23c5e7ead78487210875c7a7ce` |
| Reef 工作树 `Cargo.toml` | `8bed133140129fed9f3d2b5050af7e971c2c7da45150dfc16a523ea55c0994d4` |
| reef-template `Cargo.toml` | `13df01cf1472a0871bb4722a018698dc23c60cd23dfa08b53bc9ec3a722e7add` |
| KeelShell `Cargo.toml` | `61ca4a9652d9191102c3fb8d3cbfd2a61cfc262a9219278a8c8989ff6fabd870` |
| KeelShell `Cargo.lock` | `fbeae2598384c700129e4ff8f7430e1c455d494cd88f1471d9eb725b8bba0294` |

## 旧研究与当前合同的差异

`docs/research/reef-assessment.md:43` 建议增加本地工具链管理，`:92` 要求 Rust/tooling 本地固定。这是 2026-10-02 的历史建议，不能继续作为当前实施约束。当前依据是 `AGENTS.md:7` 与 `docs/HANDOFF.md` 的“当前工具链策略”和“产品与工程契约”。

本次实际检查：

- 根及成员 manifest 都没有 `rust-version`；根无 `rust-toolchain` / `rust-toolchain.toml`。
- `mise.toml` 仅提供 run/check/test 任务，没有 Rust 版本项。
- `.github/actions/setup-build/action.yml` 的 Rust 步骤只运行 `rustup show`、`rustc -Vv`，没有选择固定 Rust 版本。
- 本机 `rustc -Vv` 返回 `1.99.1 (58cf8f9c3 2026-10-13)`，host 为 `aarch64-apple-darwin`。这是本次执行环境，不是项目工具链限制，也不是跨平台编译或原生运行结果。
- Reef 根同样没有 toolchain 选择文件；模板保留最低 Rust `1.98.1` 的 package metadata 并由 app 继承。不得把它复制回来作为 KeelShell 的工具链 pin。

本文记录对历史建议的修正，不修改旧研究、HANDOFF、ROADMAP 或 README。历史测试中实际使用的 Rust 版本继续保留其原有含义。

## 对当前 KeelShell 有价值的工程差异

| 主题 | 来源证据 | KeelShell 当前实现与建议 |
| --- | --- | --- |
| crate 边界 | Reef 根 `Cargo.toml`、`crates/reef-compose/Cargo.toml`；模板根及 `crates/app/Cargo.toml` | KeelShell 已分为 core、session、AI、app、MCP，采用 edition 2024 / resolver 3。继续保持领域、传输、UI 与外部协议边界；无需复制 Reef 全部中间件或将模板单 app 布局作为目标。 |
| 依赖与分发 | Reef 根私有 `git-info`、可选 `data-crypto-lib`；`reef-core/Cargo.toml` 中 `git-info.workspace = true` 无 optional | “不直接依赖 Reef”的原因仍存在。Reef 根另有 `reqsign-aws-v4 3.3.1`、`reqsign-core 3.3.2`、两个 OpenDAL `0.58.2` 要求，不能把它概括为全部满足本项目 `x.y`。 |
| 启动 | Reef `crates/reef-core/src/common/start.rs:12`、`crates/reef-compose/src/start.rs:13`；模板 `crates/app/src/main.rs` | 模板仅 core bootstrap、记录初始化日志并退出，最后关闭 logger；它不启动完整 Compose 或桌面 runtime。KeelShell `main.rs:44` 创建 4 worker Tokio runtime、在 GPUI 事件循环前加载 state，再交给 GPUI 管理 UI。不要复制全局一次尝试入口来管理可重连会话。 |
| 组件生命周期 | Reef `application/builder.rs:36,133,173`、`handle.rs:86,137,163`、`runtime.rs:105` | 成功资源与 shutdown 回调一起交付；取消等待不丢 owner；失败逆序收尾；Drop 只请求停止。KeelShell 可借鉴组件终态报告和统一 owner 清单，而不引入 Reef 类型。 |
| 关闭期限含义 | Reef `application/runtime.rs:114` 超过 grace 后继续 await 原 future；对应测试 `application/tests.rs:179,219,306` | Reef 的 grace 是诊断阈值，不是硬 deadline。KeelShell `terminal.rs:75` 显式异步等待 transport registry 后退出，`terminal_workers.rs:100` 在 UI 外 join。不能用 Reef 的 grace 改写当前 SSH/进程清理的有界合同。 |
| 完整应用关闭范围 | KeelShell `terminal_workers.rs:31` 和 `runtime_bridge.rs` | 已有 transport thread owner；通用 bridge 返回结果接收 future，取消由调用方负责。后续可逐类核对 AI、更新器、MCP、文件任务与传输的停止顺序，并形成组件报告。这里没有完成所有调用点审计，不能据单个 bridge 丢弃 JoinHandle 就判定资源泄漏，也不能称整个 app 退出已全部 join。 |
| 配置 | Reef `AGENTS.md:127–145,159–169`、`application/builder.rs:103` | 借鉴同 revision snapshot、候选验证后发布、无效候选保留旧值和明确生效时机。KeelShell 使用 StateStore 与平台目录，不复制依赖 cwd 的 conf/bin/log 部署布局，也不让配置刷新隐式重建 SSH 连接。 |
| feature 与测试 | Reef `scripts/.githooks/cargo_feature_matrix.py`、`.config/nextest.toml` | Reef DB 后端互斥，矩阵优于盲目 all-features。其宽 workspace Nextest 矩阵仍带 `--no-tests pass`，精确 actuator-job 组合不带；不能把它概括成全部拒绝零测试。KeelShell 应保持适用平台、精确目标与真实场景数量的证据。 |
| 模板门禁 | 模板 `.pre-commit-template.yaml`、`.pre-commit-config.yaml`、`scripts/verify_template.py:197–220` | 模板维护 hook 只验配置、脚本；生成后才执行 Cargo。生成应用 hook 缺 `--locked`，Nextest 带 `--no-tests pass`；真实模板验收脚本则在解析后使用 `--locked`。不要用模板维护通过表示应用行为通过。 |
| 证据绑定 | Reef `scripts/validation/evidence.py:48–95`、`scripts/ci/verify.py:106–154` | 收集包括工作树差异的源码 hash、锁 hash、逐命令真实退出码，提取本次 Cargo JSON 的 executable 并复制、校验产物；结束时再核对源码与锁。比仅记录终端摘要更容易交接。KeelShell `scripts/check.py` 停在首个失败并保留输出，但本身没有这套结构化 run/artifact 绑定；可作为后续工程切片。 |
| CI 平台 | Reef `.gitlab-ci.yml`；KeelShell `.github/workflows/ci.yml` | Reef 配置 Windows/Linux 专用 runner 和显式 Podman 联调。KeelShell 配置 macOS 26、Ubuntu 24.04、Windows 2025，macOS/Linux 另跑 OpenSSH interoperability。这里只核对配置，没有查询本次 commit 的运行结果。 |
| 发布参数 | 模板 `.cargo/config.toml`、根 release profile、`justfile` | 静态 CRT、fat LTO、panic abort、服务包装与 Maven 发布不应照搬。KeelShell 当前 thin LTO、独立桌面打包及发布流程有自己的验证要求。 |

### hooks 的实际本机状态

Reef 与 reef-template 的默认 Git hooks 目录均有可执行 `pre-commit` 和 `commit-msg`。实际读取的参数分别指向 `.pre-commit-config.yaml` 与 `.pre-commit-template.yaml`，不是仅凭仓库有 YAML 推测已安装。

KeelShell 未配置 `core.hooksPath`，默认 hooks 目录没有非 sample hook，也没有随源码提供的 pre-commit 配置。因此现有“提交前运行门禁”主要由工程合同、`scripts/check.py` 与 CI 执行，并非已安装的本地 hook 自动保证。可以借鉴模板安装器“先检查现有工具、已有 hooksPath 不覆盖”的做法；本次不安装 hook。

Reef/模板的 `scripts/dev-setup/pre-commit_install.py` 只检查已有工具后安装本仓库 hook，不自动安装 Cargo 组件或准备依赖。其行为适合借鉴，但不能把来源仓库的 hook 安装状态移植成 KeelShell 已执行事实。

## Cargo 策略与锁文件核验

实际执行 `python3 -B scripts/check.py --policy-only`，退出码 **0**，输出 `Dependency version policy: passed`。未进入完整 check 的临时目录、Rust 构建或测试步骤。

| 覆盖文件 | 显式 registry 版本来源 |
| --- | --- |
| 根 `Cargo.toml` | 28 条 workspace dependencies |
| `crates/keelshell-core/Cargo.toml` | registry 依赖全部继承 workspace |
| `crates/keelshell-app/Cargo.toml` | workspace 继承；Linux `image`、dev `gpui-kit`/`russh`/`russh-sftp`/`tokio-util`；独立 build `winresource = "0.1"` |
| `crates/keelshell-session/Cargo.toml` | workspace 继承；独立 dev `rand = "0.10"` |
| `crates/keelshell-ai/Cargo.toml` | workspace 继承；`toml = "1.1"`、`process-wrap = "10.0"`；Unix `nix = "0.31"`，Windows `win32job = "2.0"` |
| `crates/keelshell-mcp/Cargo.toml` | workspace 继承；独立 `hmac = "0.13"` |

6 份 manifest 的直接要求均符合 `^[0-9]+\.[0-9]+$`，内部 5 条 path edges 都是相对路径。Cargo.lock 已被 Git 跟踪，格式版本 4，共 991 个 package 条目：986 个 registry 条目、5 个本地 workspace 包，没有 Git source 条目。已读取五个 KeelShell lock 根节点的依赖，确认 `rand` 0.9/0.10 等多版本没有混淆。

`Cargo.lock` 记录解析版本如 `8.6.0`、`1.1.6+spec-1.1.0`，不要求写成 `x.y`。第三方 crate 自己的 exact/三段式依赖也不是本项目直接 manifest 约束违规。

现有策略脚本检查版本格式并拒绝 Git dependency，但**没有查询最新版本、解析验证整个锁图或验证 path 是否越界**；它当前扫描根与 `crates/*/Cargo.toml`。本次库存恰好覆盖全部 6 份 manifest，不能把该实现描述成对将来任意嵌套 workspace 都自动完整。每次采用最新版本需要另外记录官方查询时点、目标版本和更新后的锁定结果；仅 `--locked` 或格式检查通过不满足新要求。

### 版本范围不等于补丁范围

官方 Cargo 说明：`https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html`。

省略操作符的 `x.y` 是 caret requirement。`"1.53"` 允许后续兼容的 1.x，因此允许 Tokio 1.54.1；`"3.5"` 允许 rmcp 3.8.0；`"8.0"` 当前已解析到 ZIP 8.6.0。对 0.x，例如 `"0.7"`，上界为 0.8.0，不能自动进入 0.9。不能把一次不限定目标的 update 描述为“仅补丁更新”。

## 官方 crates.io 最新稳定版完整对照

首轮查询：2026-11-04 **16:40:09 +08:00**；补充元数据查询：**16:42:32 +08:00**。每行来源为官方 `GET https://crates.io/api/v1/crates/<包名>`，不是 cargo 本地缓存或搜索摘要。筛选非 yanked、无 prerelease 的 SemVer，并选最高版本；允许稳定版 `+build` metadata。

首轮临时分析器误将 TOML 的 build metadata 作为整数解析，未给出 TOML 结论；已修正并重新读取官方 API。最终表使用 `1.2.0+spec-1.1.0`，不沿用首次失败，也不将 `+spec` 当作预发布。

`W` 表示根 workspace 显式要求；其余 scope 标明成员中的独立要求。表覆盖全部 35 条显式要求，继承边不重复计数。

| 包 / scope | 当前要求 | 当前锁定 | 最新稳定版 | 最新版对应要求 |
| --- | --- | --- | --- | --- |
| alacritty_terminal / W | 0.26 | 0.26.0 | 0.26.0 | 0.26 |
| anyhow / W，未继承 | 1.0 | 1.0.104（传递） | 1.0.104 | 1.0 |
| argon2 / W | 0.6 | 0.6.0 | 0.6.0 | 0.6 |
| base64 / W | 0.22 | 0.22.1 | **0.23.1** | **0.23** |
| chacha20poly1305 / W | 0.11 | 0.11.0 | 0.11.0 | 0.11 |
| chrono / W | 0.4 | 0.4.45 | 0.4.45 | 0.4 |
| directories / W | 6.0 | 6.0.0 | 6.0.0 | 6.0 |
| flate2 / W | 1.1 | 1.1.10 | 1.1.10 | 1.1 |
| gpui-kit / W | 0.7 | 0.7.0 | **0.9.0** | **0.9** |
| hmac / MCP normal | 0.13 | 0.13.0 | 0.13.0 | 0.13 |
| image / W → app Linux | 0.25 | 0.25.10 | 0.25.10 | 0.25 |
| nix / AI Unix | 0.31 | 0.31.3 | 0.31.3 | 0.31 |
| process-wrap / AI normal | 10.0 | 10.0.1 | 10.0.1 | 10.0 |
| rand / W → core、MCP | 0.9 | 0.9.5 | **0.10.3** | **0.10** |
| rand / session dev | 0.10 | 0.10.3 | 0.10.3 | 0.10 |
| reqwest / W | 0.13 | 0.13.5 | 0.13.5 | 0.13 |
| rmcp / W | 3.5 | 3.5.0 | **3.8.0** | **3.8** |
| russh / W | 0.63 | 0.63.3 | **0.65.1** | **0.65** |
| russh-sftp / W | 3.0 | 3.0.1 | 3.0.1 | 3.0 |
| rust-embed / W | 8.12 | 8.12.0 | **8.13.0** | **8.13** |
| serde / W | 1.0 | 1.0.229 | 1.0.229 | 1.0 |
| serde_json / W | 1.0 | 1.0.151 | **1.0.154** | 1.0 |
| sha2 / W | 0.11 | 0.11.0 | **0.11.1** | 0.11 |
| tar / W | 0.4 | 0.4.46 | 0.4.46 | 0.4 |
| tempfile / W | 3.27 | 3.27.0 | 3.27.0 | 3.27 |
| thiserror / W | 2.0 | 2.0.21 | 2.0.21 | 2.0 |
| tokio / W | 1.53 | 1.53.1 | **1.54.1** | **1.54** |
| tokio-util / W | 0.7 | 0.7.19 | **0.7.20** | 0.7 |
| toml / AI normal | 1.1 | 1.1.6+spec-1.1.0 | **1.2.0+spec-1.1.0** | **1.2** |
| url / W | 2.5 | 2.5.8 | 2.5.8 | 2.5 |
| uuid / W | 1.26 | 1.26.1 | **1.29.0** | **1.29** |
| win32job / AI Windows | 2.0 | 2.0.3 | 2.0.3 | 2.0 |
| winresource / app build | 0.1 | 0.1.31 | **0.1.34** | 0.1 |
| zeroize / W | 1.9 | 1.9.0 | **1.9.1** | 1.9 |
| zip / W | 8.0 | 8.6.0 | **9.3.0** | **9.3** |

TOML 1.2.0 的官方发布时间为 `2026-11-04T01:31:06.108422Z`，即本日 09:31 +08:00；GPUI Kit 0.9.0 为 2026-11-02 发布；russh 0.65.1 为 `2026-11-03T16:33:12Z`，即本日 00:33 +08:00。表格只代表本轮查询快照，下一轮工作仍须重新核对。

## 更新分组与验证要求

### 同系列补丁候选

这里的“补丁候选”表示版本范围兼容、可缩小变更面，**不是已经证实安全或已经完成升级**。

| 包 | 现有 lock → 同 major/minor 最新补丁 | 必须注意 |
| --- | --- | --- |
| serde_json | 1.0.151 → 1.0.154 | StateStore、配置迁移、MCP JSON 与异常输入回归 |
| sha2 | 0.11.0 → 0.11.1 | 元数据 MSRV 从 1.85 到 1.88；保留 `profile.dev.package."sha2@0.11"` 的意图，验证完整性与认证数据 |
| tokio-util | 0.7.19 → 0.7.20 | 元数据 MSRV 从 1.71 到 1.85；取消、codec 与 owner 结束语义 |
| winresource | 0.1.31 → 0.1.34 | Windows 资源构建、图标和版本信息；macOS 不能替代 |
| zeroize | 1.9.0 → 1.9.1 | 凭据/vault/MCP 密钥生命周期，既有保护与错误传播 |
| tokio | 1.53.1 → 1.53.3 | 可用于分批定位；最终目标仍为 1.54.1 |
| rmcp | 3.5.0 → 3.5.1 | 可用于分批定位；最终目标仍为 3.8.0 |
| toml | 1.1.6+spec-1.1.0 → 1.1.8+spec-1.1.0 | 可用于分批定位；最终目标仍为 1.2.0+spec-1.1.0 |
| gpui-kit | 0.7.0 → 0.7.3 | **需要单独的 GUI 验证**：底层 gpui-pre exact 从 0.3.7 变为 0.3.9；不能按普通小补丁处理。最终目标仍为 0.9.0 |

最新稳定版中的 Tokio 1.54.1 声明 MSRV 1.85，rmcp 3.8.0 为 1.88，UUID 1.29.0 为 1.89.0，TOML 1.2.0 为 1.85。GPUI Kit API 未声明 `rust_version`，不能据 null 推断任意 Rust 可编译，也不能用这些直接包的 metadata 推导全图最终编译下限。

### 跨兼容线及需要扩大回归的更新

| 目标 | 已取得的官方源码 / 当前调用证据 | 实施边界 |
| --- | --- | --- |
| gpui-kit 0.9.0 | 官方包依赖 gpui-pre / gpui-pre-macros `=0.3.11`，gpui-base/component/kit-assets `0.9.0`；当前 lock 为 Kit 0.7.0 | 保留现有 tree-sitter 语言、assets 和 test-support 选择；迁移 editor、输入、拖放、事件与窗口 API。需要三平台构建及分别取得原生证据。 |
| russh 0.65.1 | 官方 manifest 仍使用 ssh-key `=0.7.0-rc.11`、rsa `=0.10.0-rc.18` 等传递预发布 | 稳定根包不等于整个依赖图全部稳定。检查 host key、认证、代理、跳板、SFTP、forwarding、EOF、取消和关闭；保留真实 OpenSSH 联调。 |
| rand 0.10.3 | 官方 CHANGELOG 明确 OsRng → SysRng，Rng → RngExt，底层 rand_core 的 RngCore → Rng；当前 `core/src/vault.rs:695` 与 `mcp/src/ipc.rs:23,381` 使用 TryRngCore / OsRng | 保留系统熵获取失败传播，不改成 panic 或不安全回退；session dev 已使用 0.10.3，不能把它的通过证据继承给 vault/MCP。 |
| base64 0.23.1 | 官方 manifest 默认 feature 为 std + simd-unsafe，MSRV 1.71；当前在 vault、AI、SSH 代理和诊断等路径使用 | 明确默认 feature 的变化，检查编码/解码错误、padding、历史持久化和协议字节兼容；不能只换版本不检验数据。 |
| zip 9.3.0 | 跨 major；官方 CHANGELOG 涉及读写与解包实现；当前 `app/src/updater.rs:1520` 使用 ZipArchive，测试使用 SimpleFileOptions | 保留 updater 的路径限制、大小限制、验证、取消和安装审核；不要因为新 SDK 有解包辅助就绕过现有审核或改变资产。 |
| rmcp 3.8.0、Tokio 1.54.1 | 均在当前 caret 兼容范围内，但不是同 minor 补丁 | 执行 MCP authority/stream/stdio/IPC、AI 子进程、SSH 生命周期等现有行为回归；是否需要代码适配由实际编译与测试决定。 |
| rust-embed 8.13.0、UUID 1.29.0、TOML 1.2.0 | 均在当前 major 的兼容范围内 | 分别验证静态资产全部内嵌、保存/reload 身份、CLI 目录 TOML 解析。它们不应无依据地被称为必然大规模迁移。 |

源包通过官方 `https://crates.io/api/v1/crates/<name>/<version>/download` 下载到内存，仅读取 manifest、VCS 信息及可用 CHANGELOG，没有解压写入仓库或执行包中代码。

| 官方源包 | `.cargo_vcs_info.json` commit | 下载内容 SHA-256（与对应 API checksum 相同） |
| --- | --- | --- |
| gpui-kit 0.7.0 | `0c830f4d257e69fdd17200650533ab4ca9a40cc0` | `8edb2a8eafdb6e65ad1a80625347b93f8e54cbf3fac82a584e4924cda674d5c2` |
| gpui-kit 0.7.3 | `67265a36e260ffe03c5aaebdcd8d654dbf892eab` | `0fac401f6b149bf33794da0942510240afd29b7b18556574e4cf17196b647066` |
| gpui-kit 0.9.0 | `7591a35eced761e3613c01e71ff04c10129dae1a` | `2c464934b5cdb85dd3fc511e60d09fb446fba422de159a80038368f52893c14b` |
| base64 0.23.1 | `069bf7067b949f5c0a92b6ceb82492920502f2c2` | `ac07cdecf99051d9a5238b80f35af32cdeba5b336e55d957b318b50137e18da5` |
| rand 0.10.3 | `9e7d328f60d6372ae22927f37d1a816fc980f25e` | `65c9fb96cbc91e3478eaae79a69fcd3f1ae4ad052e471fe6732fff548984b4af` |
| russh 0.65.1 | `b33092aad1e51c66258a12e8b83fc473a68d9d34` | `94be3d86c71b9ed220a675c405e35c7a02565a009bf8da2e9cc13a700320a5c5` |
| zip 9.3.0 | `dcd8d15710aa2858ca19187201c6569f55c80cc8` | `eaeb7f16d2605b49aef586996ded29e65642fab7259a3077e537351df6d80426` |

## Python 图标依赖与构建解释器

2026-11-04 16:52 +08:00 查询 `https://pypi.org/pypi/Pillow/json`，选择有未 yanked 分发文件的最高稳定版本：

| 项目 | 当前配置 | 官方最新 | 本轮结论 |
| --- | --- | --- | --- |
| 图标生成依赖 | `packaging/icon-requirements.txt`：`Pillow~=12.3` | Pillow **13.0.0**，Requires-Python `>=3.11`，2026-10-16T22:15:24Z 发布 | 目标 requirement 为 `Pillow~=13.0`，属于 major 更新 |
| 工作流 Python | 复用 setup-build 及 release / Windows 诊断均选 `3.14` | 官方下载页列出 **3.15.2** | 若“所有版本最新”包括构建解释器，目标选择 `3.15`，单独验证脚本与平台行为 |

Python 解释器来源：`https://www.python.org/downloads/`；对应发布页 `https://www.python.org/downloads/release/python-3152/`。Pillow 的最低 Python 要求不强制从 3.14 升级；解释器与图标库是不同更新项，均不改变 Rust 不 pin 的合同。

Pillow 没有当前启用的额外依赖：API 中 `requires_dist` 条目都受 extras 条件限制，本项目未要求 extras。这里不审计 wheel 内嵌的全部本地库版本。

`scripts/build-icons.py:187–218` 已有 `--verify` 读取校验路径，生成时记录 Pillow/zlib、源 PNG hash 和各产物 hash。当前图标 manifest 记录 Pillow `12.3.0`、zlib `1.3.1.zlib-ng`，这不是对系统当前已安装版本的检查。`--verify` 校验存量资产，不等同于已用新 Pillow 生成并验收产物。升级需要在隔离工具环境检查格式、尺寸、alpha、ICO/ICNS 及转换结果；更新工具依赖本身不授权改变已批准图标设计，也不证明原生 Dock/窗口显示验收。本次未安装 Python 包、生成图标或运行该脚本。

## GitHub Actions 官方版本与 SHA

已检查 `.github/actions/setup-build/action.yml` 和三个 workflow 的所有 `uses:`。共 5 个外部 action；本地 `./.github/actions/setup-build` 属于本仓库代码，不是另一个外部版本。

每个 action 查询官方 `https://api.github.com/repos/<owner>/<repo>/releases/latest`，确认非 draft / 非 prerelease；再查询新旧 tag 的 `git/ref/tags/<tag>`。annotated tag 继续读取 Git tag object，直到得到 commit，核对当前 SHA 与目标 SHA，而不是只相信 YAML 版本注释。

| Action | 当前 release | 最新稳定 release | 目标 commit SHA |
| --- | --- | --- | --- |
| actions/checkout | v7.0.1 | v7.0.1 | `3d3c42e5aac5ba805825da76410c181273ba90b1`（不需变更） |
| actions/setup-python | v7.0.0 | **v7.1.0** | `6d4fd22b20465ccc0f7354476588e8df7c9ca0e0` |
| actions/upload-artifact | v7.0.1 | **v7.0.2** | `cf430e030ddbb5b0abf93d22962f4752f3646cd9` |
| actions/download-artifact | v8.0.1 | **v8.0.2** | `9000827ccba6bdab643e8b6fd33ac0654aef8333` |
| Swatinem/rust-cache | v2.9.2 | **v2.9.3** | `d1527c8f67ef9af357290475d4af4b0fab8f12e0` |

rust-cache v2.9.3 的 tag object 为 `2201cc584a476d1b508be977bbe4b7d1c7a52e9c`，其指向上表 commit；不能把 tag object SHA 误作已经核对的 action commit。旧 v2.9.2 解引用后为当前使用的 `6323deb102c322ba6fcbdcafc7e3dddab59af2b6`。

最新 release 的发布时间分别为 checkout 2026-07-20、setup-python 2026-10-22、upload/download-artifact 2026-10-07、rust-cache 2026-10-20（UTC 日期）。

“使用最新版本”在 Actions 中应落实为**最新稳定 release 对应的完整 commit SHA + 正确版本注释**；`uses:` 是 Git revision 引用，不是 Cargo/Python dependency requirement，不能机械转换为可能不存在的 `@x.y` tag。更新需覆盖所有重复引用，并通过相关 CI 验证 runner、缓存、artifact 上传下载和发布装配。此处只核验 release/tag/commit，不代表已执行新版本 action 或逐行审计其全部源码。

Linux apt 系统库、Windows SDK/FXC、runner 镜像及其编译器是另外的构建环境输入，本表不宣称它们全部达到上游最新。当前 Rust 由开发环境/runner 提供，不能为满足依赖检查而重新写入固定 Rust 工具链。

## 执行记录、交付与边界

- 已完成：有限目录定位、适用说明读取、三个本机 HEAD/工作树检查、manifest/锁/启动/停机/测试/CI/hooks 源码核验、纯 Python dependency policy、官方 crates.io / PyPI / Python / GitHub GET 查询、7 个官方 crate 源包的内存读取。
- 当前格式策略结果：35/35 显式版本要求符合 `x.y`。当前“已使用最新稳定版本”结果：**未满足**，对应更新清单如上；本次未修改 manifest 或锁。
- 完整 Rust 格式、Clippy、单元/集成测试、Cargo 解析/构建、图标转换、新 action 实跑：本轮未执行。已有历史通过记录不移植到候选升级。
- Reef 与 reef-template 未被本次操作修改；其既存状态必须保留。没有执行来源仓库生成、安装、hook、发布或容器操作。
- 已按用户明确要求，将完整版本表、目标 `x.y`、迁移提示、Actions SHA 和 Python 补充结果发送给负责依赖升级的 Volta；该作者的后续改动与验证不属于本研究的完成范围。
- 交付前逐行核对：表内 35 条当前要求与 6 份 manifest 完全对应，当前锁定版本均在 Cargo.lock 中，10 条目标要求变化与 15 条 lock 更新计数一致。两来源 HEAD、manifest hash 和 Git 状态与核验基线相同；KeelShell HEAD、manifest 与锁 hash 不变，Git 状态仅新增本文件。
- 本次唯一交付文件：`docs/research/2026-11-04-engineering-refresh.md`。不修改 HANDOFF、ROADMAP、README 或旧研究，不提交、不推送。
- 原生 GPUI、三平台交互、真实外部业务、签名/安装/更新等 acceptance 均没有因本研究获得新的通过证据。
