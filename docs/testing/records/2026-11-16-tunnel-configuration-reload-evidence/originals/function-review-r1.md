**KeelShell 隧道面板显式配置重新加载：独立非作者功能复核 r1**

日期：2026-11-16，Asia/Shanghai。结论：**APPROVE（限定范围）**。

| P0 | P1 | P2 | P3 | 已确认发现 |
| --- | --- | --- | --- | --- |
| 0 | 0 | 0 | 0 | 无 |

本次独立执行父编译测试产物，并审阅生产入口、测试实现和断言。实际结果为 **63 passed / 0 failed / 0 ignored**，其中全部 **21 个新增用例**及 **42 个既有隧道回归**通过。没有发现足以要求修改的功能缺陷或已证实的伪通过。批准仅覆盖下述冻结工作区、精确父二进制和受控 headless 功能路径，可进入父流程的最终 full gate；不授予独立编译、完整工程门禁、跨平台桌面、正常包或发行验收。

**对象与只读约束**

- cwd：`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。
- 指定 base 与两次读回 HEAD 均为 `c81f718ccc3ee5d6a8cce6b7988b1cbcab8ea694`。功能修改位于未提交工作区；本报告评审的是 base 加冻结输入，不将功能归属于 base 提交本身。
- 已读取用户提供的 AGENTS 合同、实际 `AGENTS.md`、`/Users/hdh/.codex/RTK.md`、`docs/HANDOFF.md`、ADR 0151 和共享 target 合同；使用 `rust-best-practices` 辅助检查所有权、错误和测试断言。
- reviewer 执行 Cargo、fmt、build、Git mutation、target 清理、新 chat、新 worktree、容器均为 **0**。未占用父 Cargo 槽，未改生产或测试源码。写入仅为本报告和同目录自有 `function-review-*` 日志/JSON；测试自身使用明确授权的隔离临时数据。
- 未启动原生应用，GUI 操作、截图均为 **0**。Mac 锁屏依据用户给定状态及本批 `native-availability-r1.json`，本 reviewer 没有重新操作桌面。

**冻结和父产物的独立核对**

在执行二进制前逐项读取 721 个输入，重算字节数和 SHA-256；721 路径唯一、零 mismatch。将当前 `git ls-files --cached --others --exclude-standard` 中排除 `docs/` 与 `.hallmark/` 的集合与清单比较，未列输入和额外输入均为 0。

聚合算法独立复算为：按冻结顺序将每项 `path + NUL + lowercase sha256 + newline` 编成 UTF-8，再求 SHA-256。结果：

```text
输入数：721
输入聚合 SHA-256：
442393e019c51a19b8fe49d5040813e44a06b5a77cc3a7eb4c9b162701a979f2

inputs-freeze-r2.json 文件 SHA-256：
217a3e8960fc9a720fb9194cb76837ea75d3abbe1424da9a04dd28ca78436129

父二进制字节数：273326016
父二进制 SHA-256：
1f3ac5387026afaad1d36717faaa3a976e179503562aea04bea2537e5a7cef6e

app-binary-r3.json 文件 SHA-256：
799e7e4bfd5ce9abc80fa250d92054fdf6d07c75a2f880e8b08faef4f0b1da94
```

精确二进制：

```text
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-app/e9828e40142b11cd/out/keelshell_app-e9828e40142b11cd
```

初次核对记录时间为 `2026-11-16T15:26:46Z`，复核后再次核对为 `2026-11-16T15:31:37Z`。输入聚合、冻结文件、binary manifest、二进制摘要和已记录评审文档摘要均未变。摘要和库存核对证明执行对象与父回执相符；没有独立重建编译链，不能将其写成独立编译证明。

**独立执行、逐名结果与进程终态**

使用已审阅的本批 `run_command.py`，以 `PYTHONDONTWRITEBYTECODE=1 python3 -B` 调用；它设置统一环境，本次 command 参数直接指向上述二进制，没有调用 Cargo。

```text
run_command.py function-review-list-r1 90 <精确父二进制> --list
run_command.py function-review-tunnel-r1 180 <精确父二进制> tunnel --test-threads=4
```

JSON 回执保存完整绝对路径 argv、cwd、耗时和日志摘要。

| 命令 | 实际结果 | 外层耗时 | 终态 |
| --- | --- | --- | --- |
| 二进制 `--list` | 1364 tests / 0 benchmarks；1364 唯一名 | 0.085008 秒 | observed/controller exit 0，已 reap |
| 二进制 `tunnel --test-threads=4` | 63 passed / 0 failed / 0 ignored / 0 measured；1301 filtered out | 5.192736 秒；测试内层 5.11 秒 | observed/controller exit 0，已 reap |

直接 `--list` 与 `app-binary-r3.json.inventory_names` **逐名、顺序完全相等**，也核对了父 r3 编译日志的完整库存。新增两个源码测试文件中提取的 14 + 7 个名字，与 manifest 的 21 个新增名集合完全相等。筛选 `tunnel` 得到精确 63 名，实际运行声明、63 条唯一逐名结果和汇总相等，无缺名、额外名、重复结果或未运行的新名。

两个 wrapper 均报告 `direct_reaped=true`、`active_processes=0`、`pipes_drained=true`、`errors=[]`，无 timeout/interruption。没有仅启动命令后推测通过，也未将 1301 个 filtered out 计为 passed 或 ignored。

**用例真实性与生产路径**

7 个 Workspace 新用例均从 [生产 Workspace fixture](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/workspace_tests.rs:37) 挂载：独立 `tempfile::TempDir` 下的真实 `StateStore`、实际 `Workspace::new` 和 `toggle_panel`。后者安装 [生产 reload subscription](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace.rs:3446)，随后用例点击 `reload-tunnel-configuration`。没有把直接调用权威采用 helper 当作这 7 项的 reload 成功。

[SSH fixture](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/test_support.rs:37) 启动 `127.0.0.1:0` russh 服务，客户端通过生产 `SshSession::connect` 校验预置 host key。它是真实回环 SSH，终端输出通道则为测试装配，不能据此称为完整终端或外部远端验收。fixture 退出有有界 session close 和服务任务回收。

14 个局部新用例使用实际 GPUI Kit 按钮、绘制、输入及 AX 测试能力，同时直接调用局部 helper 构造状态或伪造 token。这些 helper 调用用于验证状态边界，不是 14 次后台 StateStore 集成。21 项在本次执行中全部有唯一 `ok`。

| 关注点 | 本次核查和断言 | 绝对路径行证据 |
| --- | --- | --- |
| 精确请求身份、旧绘制回调、重复请求 | 按钮捕获旧 epoch；请求产生新 operation、绑定原 connection ID；完成检查 pending 全等，拒绝伪造 operation/epoch/connection。旧拒绝不会清除替换请求。 | [生产请求和匹配](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/reload.rs:30)、[伪造 token 用例](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/reload_tests.rs:144)、[旧绘制用例](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/reload_tests.rs:297) |
| same snapshot 也撤销审核 | 权威替换不会走普通刷新“不变则返回”的分支，重新生成 epoch、清除 pending/启动/停止/恢复/元数据审核。新 sibling 用例和原同内容元数据/停止审核回归均执行通过。 | [权威替换](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels.rs:382)、[sibling 新例](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/workspace_reload_tests.rs:183)、[原元数据回归](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/management_tests.rs:1082) |
| 草稿不自动 rebase、冲突恢复 | 局部断言保留 editor identity、Input entity、原快照 `Arc::ptr_eq` 和含中文/尾空白原文；改变配置后旧 editor 不能生成保存审核。Workspace 新例制造真实 CAS 冲突，经按钮重载读取外部胜出配置，取消旧草稿并重新编辑审核后才实际保存成功。 | [局部草稿断言](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/reload_tests.rs:317)、[生产旧快照校验](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/rules.rs:438)、[冲突恢复新例](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/workspace_reload_tests.rs:63) |
| busy 传播及新面板继承 | 准入获得 reload lease 后才置 `state_reloading`，同步现有面板 busy；新面板从 Workspace 的 saving/reloading 状态继承。失败新例在生产订阅之后另订阅观察，要求实际已进入 busy，现场新建第二标签并断言新旧面板均 busy，结束后均保留 blocked。 | [准入/后台读取](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:298)、[新面板继承](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace.rs:3453)、[失败和新面板用例](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/workspace_reload_tests.rs:118) |
| 竞争租约 | 用另一 owner 的真实 reload lease 占用共享 gate；生产事件先拒绝且无 busy/pending 遗留，释放后第二次按钮请求成功。记录的生产准入观察必须精确为 `[false, true]`。 | [竞争用例](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/workspace_reload_tests.rs:218)、[可用性与 lease](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/app_windows.rs:214) |
| 后台读取、前台采用、完整 AppState | `store.load()` 运行于 background executor；lease 被前台 continuation 持有，在同一个同步采用回调中 finish 后采用完整状态，未在后台读取结束时提前释放。成功复用现有全配置采用路径，包含 AI 临时凭据撤销、知识审核、重连、MCP/Agent、主题/语言和片段刷新。 | [读取到采用](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:321)、[完整采用路径](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/knowledge_library.rs:146) |
| failure → repair → retry | 用例将隔离配置写成真实无效 JSON，重载后断言 invalidated、Workspace blocked、新旧面板均 blocked，错误文件原样保留；明确修复文件，再从新面板按钮重试，才恢复 configuration_current 并解锁两面板。生产失败分支保留原始 error 于 Workspace 状态，未自动重试。 | [修复重试断言](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/workspace_reload_tests.rs:149)、[生产失败处理](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:369) |
| retired owner / 替换面板 | 生产准入和完成均校验 live panels 映射、原 connection ID 与精确 pending；单有 subscription、归档记录或 retained owner 不授权采用。新例在已实际准入后 suspend+archive 原面板并新建替换面板，要求旧状态未被采用、替换面板仍 blocked，再由替换面板显式重载成功。 | [生产 owner 校验](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/tunnel_reload.rs:50)、[过期完成处理](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:334)、[retired 新例](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/workspace_reload_tests.rs:249) |
| stale window 可恢复且不伪 busy | 新例持久化外部配置并发布共享 gate 的新状态，原窗先观察为 invalidated/blocked 而非永久 busy，再通过本面板真实按钮重载。生产准入观察必须为 `[true]`，最终采用外部 snapshot。 | [stale 新例](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/workspace_reload_tests.rs:367)、[生产 stale 处理](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:110) |
| 活动原 SSH/listener 和停止能力 | Workspace 新例实际启动 loopback listener、等待运行快照落盘，再外改规则名称并重载；断言新元数据被采用、原运行快照相等、listener 仍 listening、原 fixture SSH 未关闭。局部新例另断言 pending 期间旧停止审核失效、cancel 未触发、runtime owner 和 bound address 不变，再进行新的显式停止审核并等待结束。 | [完整重载保活新例](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/workspace_reload_tests.rs:316)、[owner/端点/停止新例](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/reload_tests.rs:465) |
| 三主题与双语布局 | 遍历三个 registry family × Light/Dark × En/ZhCn × 两尺寸，24 组；每组检查 runtime、editor、review、pending、blocked 状态中相关按钮/反馈的可见性、正尺寸和根视图内边界；AX Role/双语名称及反馈位于按钮下方有实际断言。另测 hover tooltip 原位翻译。 | [24 组矩阵](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/reload_tests.rs:512)、[tooltip](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/reload_tests.rs:610)、[三个注册主题](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/design/registry.rs:255) |

首轮异步断言修正没有把“事件已发出”等同于“生产已准入”：新增 [admission observer](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/workspace_reload_tests.rs:36) 在生产 subscriber 之后记录状态，冲突恢复和 stale 场景要求 `[true]`，竞争场景要求 `[false, true]`。其余关键场景还有外部元数据采用、真实损坏后阻断或过期来源不采用等结果断言。单独的 `wait_for_reload` 只是有界等待，不作为唯一成功证据。

**覆盖边界与保留证据**

- 24 组矩阵是独立面板在 **900×580、1400×900** 的 headless 绘制/AX 检查，不是完整 Workspace、平台字体、真实 GPU、键盘/IME 或屏幕阅读器的原生验收。63 项还执行了原有 900×260 固定 footer 回归，但这不补成完整三主题原生矩阵。
- 活动隧道新例主要证明配置重载前后的监听状态、运行快照及 SSH 存活；局部用例证明 request pending 时的 owner/端点和新停止审核。没有把这些断言扩大为持续流量跨重载、三种转发方向的端点修改矩阵或外部 OpenSSH peer 验收。
- 竞争新例实际占用的是 reload lease；write/maintenance 的共同互斥、前台采用阶段的持有，以及其他模态/连接/关闭准入守卫已静态核查，不宣称本次另执行了完整跨窗口竞争调度矩阵。
- retired 集成新例组合覆盖 suspend、archive、替换和新 owner 重试；连接映射变化另由局部 pending/connection 用例和生产双重校验支持，未把每一种生命周期交错都称为独立集成测试。
- 全 AppState 的刷新清单来自本次生产调用链核查，7 个新增集成用例没有逐项重新执行所有 AI、MCP、Agent 和重连子系统的行为矩阵。
- 父 r3 编译日志仍只有已记录的 linker 大 section 警告，没有 Rust 源码 warning。此处是读取父日志的观察；本 reviewer 未执行编译、fmt、Clippy、依赖策略或 full gate。Cargo.lock 的依赖变化及 Windows/Linux 结果不能由本次 63 项代签。
- 原始 `app-inventory-r1.log` 的编译失败、`panel-reload-r1.log` 的 **13 passed / 1 failed**、`tunnel-integration-r1.log` 的 **60 passed / 3 failed** 均保留；没有覆盖或改写。本次 63/0/0 是独立新增运行，不追溯重标旧运行。最终完整门禁及精确 CI/正常包/桌面验收仍由各自证据决定。

**本次原始证据**

均位于 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/tunnel-configuration-reload-20261116/`：

| 文件 | 内容 |
| --- | --- |
| `function-review-inputs-r1.json` | 执行前输入、聚合、binary、父库存及评审文档摘要核对 |
| `function-review-list-r1.log` / `.json` | 独立直接 `--list` 原文与完整终态回执 |
| `function-review-inventory-r1.json` | 1364 逐名/顺序核对、63 筛选名及 21 新名完整性 |
| `function-review-tunnel-r1.log` / `.json` | 独立 63 项执行原文、完整 argv、耗时和 reap 回执 |
| `function-review-results-r1.json` | 声明、逐名结果和汇总一致性；21 新例及 42 回归的实际结果 |
| `function-review-final-integrity-r1.json` | 执行后 721 输入、binary、文档未变；14+7 源码新名与库存相等；父原始证据摘要 |
| `function-review-report-r1.json` | 本报告和 reviewer 自有证据的最终 SHA-256 索引 |

```text
独立 --list 日志 SHA-256：
b73b69e647aa682d86779eb037c033684d2260a4acba0e7f7f8e9a75f4f18765

独立 tunnel 执行日志 SHA-256：
942d0f2120c3391a712ac765fa3c186f228c41f9b89bb088b812ebbc479119e5
```

未确认的边界如上保留，不构成本次新增 P0–P3 发现。最终限定建议为 **APPROVE**；父流程可继续一次最终 full gate。
