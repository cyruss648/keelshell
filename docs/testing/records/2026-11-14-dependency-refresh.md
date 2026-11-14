# 2026-11-14 直接依赖官方复查与升级

## 结论与范围

隔离实现以 `aab1718f6682a82860ded8f0510ab7ccb843a902` 为基线，随后集成到
`4340b19e0099a00e460281502fd5cebd7c753ab9` 的父工作树。升级后的在线官方检查为
`33 packages / 103 declarations and uses / 5 workspace members`，全部通过。
此计数来自当前源码清单，不能沿用历史记录的 `34 / 104`。

八个直接包的锁定版本已升级；其中四处 manifest 要求改变，其余继续使用原有
`x.y` 要求。所有直接 registry 声明仍严格采用 `x.y`，不添加 Rust toolchain
pin 或 `rust-version`。macOS arm64 上五个项目包的 `--all-targets` 严格
Clippy 和格式检查通过，没有发现需要修改 Rust 源码的 API 兼容问题。

依赖输入仅修改根 `Cargo.toml`、`crates/keelshell-ai/Cargo.toml` 和
`Cargo.lock`；父线程另行维护本记录、HANDOFF、ROADMAP 和测试索引。没有修改
Rust 源码、产品行为或视觉布局。公司内网暂不可用，本片只做本地提交，不执行 push。
下文先保留隔离实现回执，再单列父线程完整门禁，不能把静态检查计为测试执行。

## 官方查询与失败事实

执行项目原有 `scripts/check_dependencies.py`，每轮重新 GET 官方
`https://crates.io/api/v1/crates/<package>` 元数据，不使用离线版本快照或
父线程的旧查询结果。每次请求超时为 `30s`，并发为 `4`；脚本直接检查每个
version record 的 stable / non-yanked 状态，核对所有直接锁文件边，覆盖
workspace 声明、继承、normal/dev/build 依赖和平台条件依赖。

本轮两次查询的实际时间保留为原始 UTC：

| 轮次 | 开始 / 完成 UTC | exit | 结果 |
| --- | --- | --- | --- |
| `official-before` | `2026-11-14T00:54:14Z` / `00:54:30Z` | `1` | 33 个包均取得元数据；16 处声明/使用因旧 requirement 或旧直接锁定版本失败 |
| `official-after` | `2026-11-14T00:57:42Z` / `00:58:01Z` | `0` | 33 个包、103 处声明/使用全部通过 |

两轮均没有 `registry_check_failed` 或顶层查询错误。首次非零退出是依赖策略
发现版本落后，不是网络失败。父线程曾报告 `uuid`、`tempfile`、`flate2`
的 `5s` 查询超时；本片以 `30s` 上限重新取得三者的官方元数据：
`uuid 1.29.2`、`tempfile 3.27.0`、`flate2 1.1.11`。其中前者和后者需要
升级，`tempfile` 无需改变。

本机全局 Cargo 配置存在 registry 镜像替换。为避免配置继承，Cargo
子进程以 `/` 为工作目录，通过显式 `--manifest-path` 指向本 worktree
的根 manifest，并设置：

```text
CARGO_HOME=<repo>/work/dependency-refresh-20261114/cargo-home
CARGO_TARGET_DIR=<repo>/work/dependency-refresh-20261114/target
CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
CARGO_NET_RETRY=1
CARGO_HTTP_TIMEOUT=30
CARGO_BUILD_JOBS=4
```

本节中的 `<repo>` 表示隔离实现 worktree 的根目录。该配置只作用于本轮子进程，没有
修改全局配置或项目配置。新 Cargo home 的 index 配置实际读回为
`https://static.crates.io/crates` 和 `https://crates.io`，使用官方 sparse
index；本片没有访问公司资源，也没有执行 Git fetch 或 push。

为减少下载，新 Cargo home 仅复用本地已有的 2,124 份 `.crate` 归档的
硬链接；没有复用旧 registry index 或任何项目编译产物。Cargo 根据新解析的
官方 index 和锁文件校验包。target 在本轮开始前不存在，所有项目包均在
本片专属 target 中重新检查，没有使用父工作树的 target，也没有执行 clean。
这符合 [共享 target 归属规则](../SHARED_TARGET.md)关于产物来源的约束。

## 精确版本

下表覆盖本轮全部 33 个直接 registry 包；升级后每行均由第二次在线检查
确认是最新非 yanked 稳定版本。manifest 中相同的 workspace/直接使用已
合并展示，原始报告保留全部 103 处声明/使用。

| 包 | 升级后的 `x.y` 要求 | 基线直接锁定版本 | 本轮直接锁定版本 |
| --- | --- | --- | --- |
| `alacritty_terminal` | `0.26` | `0.26.0` | `0.26.0` |
| `argon2` | `0.6` | `0.6.0` | `0.6.0` |
| `base64` | `0.23` | `0.23.1` | `0.23.1` |
| `chacha20poly1305` | `0.11` | `0.11.0` | `0.11.0` |
| `chrono` | `0.4` | `0.4.45` | `0.4.45` |
| `directories` | `6.0` | `6.0.0` | `6.0.0` |
| `flate2` | `1.1` | `1.1.10` | `1.1.11` |
| `gpui-kit` | `0.9` | `0.9.0` | `0.9.1` |
| `hmac` | `0.13` | `0.13.0` | `0.13.0` |
| `image` | `0.25` | `0.25.10` | `0.25.10` |
| `nix` | `0.31` | `0.31.3` | `0.31.3` |
| `process-wrap` | `10.0` | `10.0.1` | `10.0.1` |
| `rand` | `0.10` | `0.10.3` | `0.10.3` |
| `reqwest` | `0.13` | `0.13.5` | `0.13.5` |
| `rmcp` | `3.10`（原 `3.9`） | `3.9.0` | `3.10.0` |
| `russh` | `0.65` | `0.65.1` | `0.65.1` |
| `russh-sftp` | `3.0` | `3.0.1` | `3.0.1` |
| `rust-embed` | `8.14`（原 `8.13`） | `8.13.0` | `8.14.0` |
| `serde` | `1.0` | `1.0.229` | `1.0.229` |
| `serde_json` | `1.0` | `1.0.154` | `1.0.154` |
| `sha2` | `0.11` | `0.11.1` | `0.11.1` |
| `tar` | `0.4` | `0.4.46` | `0.4.46` |
| `tempfile` | `3.27` | `3.27.0` | `3.27.0` |
| `thiserror` | `2.0` | `2.0.21` | `2.0.21` |
| `tokio` | `1.54` | `1.54.1` | `1.54.1` |
| `tokio-util` | `0.7` | `0.7.20` | `0.7.20` |
| `toml` | `1.2` | `1.2.0+spec-1.1.0` | `1.2.1+spec-1.1.0` |
| `url` | `2.5` | `2.5.8` | `2.5.8` |
| `uuid` | `1.29` | `1.29.0` | `1.29.2` |
| `win32job` | `2.1`（原 `2.0`） | `2.0.3` | `2.1.0` |
| `winresource` | `0.1` | `0.1.34` | `0.1.34` |
| `zeroize` | `1.9` | `1.9.1` | `1.9.1` |
| `zip` | `9.5`（原 `9.4`） | `9.4.0` | `9.5.1` |

锁文件通过只选择上述八个旧直接包的 `cargo update -p ...` 更新，而非对
全部传递依赖执行无范围刷新。解析同时更新了 46 个已有 package record 的
版本，并新增 `constant_time_eq 0.6.1`；总 package record 从 `977` 增至
`978`。相关传递变更包括：

- `gpui-base`、`gpui-component`、`gpui-component-macros`、
  `gpui-kit-assets` 升至 `0.9.1`；
- `gpui-fast` 家族升至 `0.1.15`，`gpui-pre` 家族升至 `0.3.12`；
- `rust-embed-impl`、`rust-embed-utils` 升至 `8.14.0`；
- `serde_spanned` 的 `1.x` 实例升至 `1.1.3`，`toml_datetime` 的 `1.x`
  实例升至 `1.1.3+spec-1.1.0`；仍为传递依赖所需的旧 `0.x` 实例保留；
- `win32job` 的 Windows 依赖边由 `windows 0.61.3` 改为
  `windows 0.62.2`；
- `zip` 增加 `constant_time_eq` 和已有 `zeroize` 的依赖边。

Cargo 提示另有 46 个传递依赖落后于各自最新版本。该提示不表示直接依赖
检查失败，也不证明这些传递版本均能在现有约束下更新；本片没有扩大范围
升级全部传递依赖。完整版本集合差异保存在 `lock-version-diff.json`。

## 实际命令与退出

以下命令均从本隔离 worktree 交给有界日志 runner 执行。Cargo runner 使用
上文环境和显式根 manifest，所有 Cargo 命令串行执行，原进程均已实际
`wait` 并退出；没有超时或提前宣布完成。命令输出和真实退出码分别保存为
同名 `.log`、`.exit.json`。

| 日志前缀 | 实际命令（省略统一的 `--manifest-path`） | 上限 | 实际耗时 | exit |
| --- | --- | --- | --- | --- |
| `official-before` | `python3 -B scripts/check_dependencies.py --timeout 30 --jobs 4 --json-report work/dependency-refresh-20261114/official-before.json` | `600s` | `16.042s` | `1` |
| `cargo-update` | 见下方完整选择列表 | `900s` | `5.752s` | `0` |
| `metadata` | `cargo metadata --format-version 1 --no-deps --locked --offline` | `120s` | `0.022s` | `0` |
| `official-after` | `python3 -B scripts/check_dependencies.py --timeout 30 --jobs 4 --json-report work/dependency-refresh-20261114/official-after.json` | `600s` | `19.244s` | `0` |
| `clippy-domain-transport-ai-mcp` | `cargo clippy -p keelshell-core -p keelshell-session -p keelshell-ai -p keelshell-mcp --all-targets --locked -- -D warnings` | `1200s` | `31.280s` | `0` |
| `clippy-app-compatibility` | `cargo clippy -p keelshell-app --all-targets --locked -- -D warnings` | `1200s` | `53.468s` | `0` |
| `fmt` | `cargo fmt --all -- --check` | `120s` | `1.725s` | `0` |
| `diff-check` | `git diff --check` | `60s` | `0.021s` | `0` |

```sh
cargo update \
  -p gpui-kit@0.9.0 \
  -p rust-embed@8.13.0 \
  -p zip@9.4.0 \
  -p rmcp@3.9.0 \
  -p toml@1.2.0+spec-1.1.0 \
  -p win32job@2.0.3 \
  -p uuid@1.29.0 \
  -p flate2@1.1.10
```

本机实际使用 `rustc 1.100.0 (0ef1bc386 2026-11-09)`、
`cargo 1.100.0 (3acc79c0c 2026-11-05)`，active/default toolchain 为
`stable-aarch64-apple-darwin`。本片没有改变工具链设置。metadata 实际读回
五个本地 crate 的 `rust_version` 均为 `null`，target 目录为本片指定目录。
隔离实现交回时，新增记录尚未纳入 Git index；另行读回确认其无个人绝对路径、
无行尾空白并保留末尾换行。当时 HANDOFF、ROADMAP 和 Rust 源码差分为空；
父线程集成后的文档更新单列记录。

## 验证边界与原件

隔离实现的结论是依赖官方版本策略通过、macOS arm64 编译与严格 Clippy 兼容。
`--all-targets` 包含宿主平台的测试目标静态检查，但没有运行测试，也没有
生成可作为测试执行证据的测试清单；该隔离阶段执行的 Rust 测试数为 `0`。
该阶段没有执行 app 全量测试或完整 `scripts/check.py`，父线程集成后统一执行。

`win32job 2.1.0` 和其他非 macOS 条件分支已经过官方元数据、manifest
要求与锁文件边检查；本机只安装 `aarch64-apple-darwin` target，未做
Windows/Linux 编译或原生验收。尤其不能用本机 Clippy 证明 Windows Job
Object 行为兼容。GPUI 上游变更的布局、焦点、IME、辅助技术与真实窗口表现，
ZIP 更新解包、资源嵌入运行时、真实 SSH/MCP/AI 工作流，以及最终签名安装包，
分别由父线程集成门禁与后续原生验证覆盖，源码门禁不能代签原生行为。

原始日志、官方 JSON、退出回执和辅助索引已复制到父工作树的
`work/dependency-refresh-20261114/`；隔离实现 worktree 已归档。该目录被 Git
忽略，不随源码自动公开：

- `official-before.json`、`official-after.json` 及同名 `.log`、
  `.exit.json`：完整 103 处检查与查询时间；
- `cargo-update.log`、`cargo-update.exit.json`：实际解析变更；
- `metadata.log`、`metadata.exit.json`、`metadata-summary.json`：本地包
  metadata、`rust_version` 和独立 target 读回；
- 两组 `clippy-*.log`、`clippy-*.exit.json` 和 `fmt.*`：静态兼容检查；
- `isolation.json`、`lock-version-diff.json`：缓存/target 归属和完整锁版本差异；
- `diff-check.*`、`final-audit.json`、`source-inputs.json`：最终范围、回执与
  输入摘要；
- `run_logged.py`：本轮有界进程等待与退出回执 runner。

## 父工作树集成环境与首轮失败

父线程逐字节核对根与各 crate manifest、`Cargo.lock` 和依赖检查脚本，均与
隔离实现的 `source-inputs.json` 相符。升级没有改变任何 Rust 源码；集成基线
`4340b19` 与隔离实现基线 `aab1718` 之间只有已提交的文档更新。

父线程从项目原有 `scripts/check.py` 运行完整门禁，脚本保持未修改。临时 Cargo
入口仅接受 `fmt`、`clippy`、`test`、`metadata` 子命令，从 `/` 启动实际 Cargo，
显式传入本工作树的根 manifest，继续使用官方 registry 的隔离 Cargo home 和
已校验的离线包。全程没有修改开发机或仓库的 Cargo 配置，没有访问公司资源。

首轮回执为 `integrated-gate.exit.json`：UTC `2026-11-14T01:09:15.081170Z`
开始，`01:10:48.634876Z` 完成，耗时 `93.553s`，实际 wait，exit `1`，
不是超时。Python、fmt 和严格 Clippy 通过后，workspace tests 编译报 `E0514`：
`ff` 使用的 `subtle` 等缓存产物与当前编译器不兼容。诊断 help 提及 Rust
`1.85.0`；随后在项目目录和 `/` 直接探测的 Rust/Cargo/rustfmt/Clippy 均为
当前 `1.100.0` 系列。现有证据不足以确定首轮具体根因，不能把诊断 help
解释成已证实的 PATH、mise 或工具链切换问题。原始失败日志、退出回执和
`toolchain-probes-r1.json` 保留，不覆盖失败状态。

第二轮使用此前不存在的 `work/dependency-refresh-20261114/target-r2`，显式设置
当前 `RUSTC`、`RUSTDOC`、`RUSTFMT` 的工具链绝对路径，并将同一工具链 bin
加入该进程 PATH。这个选择排除旧 target 和间接工具选择的干扰，没有修改产品
源码，也不能单凭后续成功反推首轮错误的根因。两轮没有并行消费同一 target。

## 父线程完整门禁与测试清单

第二轮原始回执 `integrated-gate-r2.exit.json` 已实际收取：UTC
`2026-11-14T01:12:54.942693Z` 开始，`01:25:20.080232Z` 完成，耗时
`745.135s`，实际 wait，exit `0`，没有超时。主检查脚本的全部阶段均已执行：

| 阶段 | 真实结果 |
| --- | --- |
| 直接依赖 `x.y` 策略 | 通过 |
| Python unittest | `131 run / 1 skipped` |
| `cargo fmt --all --check` | exit `0` |
| workspace `--all-targets` 严格 Clippy | exit `0` |
| workspace ordinary Rust tests | `2440 passed / 0 failed / 23 ignored` |
| 其中 app tests | `1132 passed / 0 failed / 2 ignored` |
| doctest | `14 passed / 0 failed`，其中包含 `2` 项 compile-fail |
| readiness / observer / small-stack controller | 三组均 exit `0`，全部断言与场景通过 |

普通测试与 doctest 分开统计，14 项 doctest 中的 compile-fail 不重复相加；
AI 自定义控制器不伪装成 libtest 项目数。原始日志的逐二进制分段汇总保存在
`integrated-test-summary-r2.json`。

本轮 app 测试二进制从全新 target 构建，直接执行 `--list` 退出 `0`，清单为
`1134` 项。已核对具体新增名称，包括监控层级双语可达性、磁盘末条完整 bounds
与选择、工作台 chrome 主题矩阵，以及原生窗口 options 契约；对应组分别为
monitor `21` 项、workbench layout `12` 项、design `4` 项，窗口契约的两个新增
用例也实际进入二进制并在全量中通过。清单与摘要分别保存在
`app-test-list-r2.txt`、`app-binary-r2.json`。二进制 SHA-256 为：

```text
52fd382e6155b6be0d129df4d35243e9be712d218a1112187add243215056b69
```

命令完成后另行执行依赖策略与 `git diff --check`，均退出 `0`。依赖输入与本节
作者门禁固定在本地提交 `98f8f9d0558098061563fcb9c24f500551fb361a`。

## 首轮独立复核与后续修复

新审查 agent 对固定 `98f8f9d` 的依赖输入，以及窗口契约 `84a5c7b`、
工作台 chrome `31ca92b` 的增量返回 `REQUEST_CHANGES`：
P0 `0`、P1 `0`、P2 `1`、P3 `0`。唯一 P2 是此前 chrome 状态灯只检查
`end_reason()`，把尚未连接的恢复占位和 Starting 终端误画为成功色；依赖升级
本身没有需修改的发现。该拒绝结论保留，不能把作者完整门禁写成独立批准。
修复、真实绘制回归与后续独立复核从
[状态灯修复记录](2026-11-14-connection-indicator-review-fix.md)接续。

审查独立执行作者预构建二进制，核对本节 SHA-256 与 `1134` 项清单：
窗口管理 `10/1124`、workbench layout `12/1122`、design `4/1130`、inactive
生命周期和 restored compact 各 `1/1133`（通过/过滤），总计
`28 passed / 0 failed / 0 ignored`。所有命令实际 wait 并退出 `0`；这不是
独立重新编译，也不是独立 app 全量或原生验收。

独立官方复查首轮使用 `15s` 请求超时，exit `1`：32 个包通过，zip 的两处
声明/使用为 `registry_check_failed`。随后只对 zip 做一次 `30s` 上限补查，
耗时 `3.068s`，UTC `2026-11-14T01:33:15Z` 完成，确认最新非 yanked 稳定版本
`9.5.1`、当前 `9.5` 要求和直接锁边一致。两份结果合计覆盖 33 个包、
103 处声明/使用，但不能称为一条全量官方命令 exit `0`。首轮失败查询原件保留；
它表示查询失败，不表示包版本落后。

审查还逐文件比较 win32job 官方归档，当前使用的 Job 创建、进程分配、
kill-on-close 和进程列表接口静态兼容；47 个新版本锁记录均匹配官方索引缓存，
已有归档的 37 项摘要匹配，没有同版本 checksum 异常。Windows 编译与运行仍未
验证。首轮独立原件位于 `work/dependency-refresh-20261114/review/`，包含
`review-report-r1.json`、`official-independent-r1.*`、`official-zip-r2.json`、
源码/锁边/归档核对与各组测试回执。

## 原生与交付边界

上述结果覆盖 macOS arm64 上的工程、受控 GPUI 与 loopback 协议集成。Windows/Linux
构建和原生桌面，窗口尺寸/焦点/IME、VoiceOver/NVDA、真实远端、外部 AI provider、
Podman、签名、公证和安装升级没有由本片验收。旧 GPUI Kit `0.9.0` 的包不能作为
本片 `0.9.1` 的原生证据；桌面通道上一轮确认锁屏，原生主题与紧凑窗口保持
`HOLD`，见[原生观察记录](2026-11-14-monitor-panel-native-smoke.md)。

本片没有启动原生应用或 Podman 容器，没有连接真实远端，没有安装应用、发布、
打 tag 或 push。实现 worktree 已归档，必要回执已保留到父工作树；本轮新建的
隔离 target 保留为当前输入的有效构建缓存，不清理其他任务的 target 或全局缓存。
