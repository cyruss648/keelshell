结论：**条件 APPROVE**。最终 r2 的本批 FilesPanel app 源码复核及指定七项预构建控制器测试通过，未发现新增阻塞项。工程集成条件由父在**相同 607 项输入、相同摘要的 full-gate-r2 成功终态**后核验。本报告现在收口，不等待该门禁，也不签署原生桌面验收。

| 严重级别 | 本次 app 范围开放数量 |
| --- | ---: |
| P0 | 0 |
| P1 | 0 |
| P2 | 0 |
| P3 | 0 |

复核日期：2026-11-15。工作树：`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。baseline 与运行后 HEAD 均为 `6e2b0a7ee3493ace52ba157049ab668ba464c4f0`；评审对象为其上的本批冻结工作树差分。r1 源码限定结论由本报告补充，不将 r1 工程摘要沿用为 r2。

**冻结、产物与库存**

- [r2 冻结清单](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/frozen-inputs-r2.json)：607 项 Rust/Cargo/Python 输入，集合摘要 `523506564206cbeca415ae2bc9a82ed670cdb210705ba49f3173a84250bd4c17`。测试前后逐项核对，数量、内容和路径集合一致，零输入漂移；八份 app 源文件相对 r1 均未变化。
- 实际执行指定稳定副本 `work/archive-validation-batch-20261115/stable-binaries/app-r2`，266,251,648 字节，SHA256 `94b2eb017e0e3965ac0f804f2291fcab7f2384aa4e256be817509b9b4109ebc2`。核对了[构建绑定](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/app-binary-binding-r2.json)、父构建原始二进制和稳定副本，三者摘要相符；运行后副本摘要未变。
- 独立执行副本 `--list`：1246 项 descriptor，与父的完整库存逐字节一致，库存 SHA256 `64b807803c8f028dacedbaccb1a94fbc17c894d882fe99ca06bfa5d98f12122c`；新增 `archive_validation_tests` 七个完整名称逐名相符，无缺失或额外名称。见[库存比对](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/review-app/inventory-comparison-r2.json)。
- 修复后的 helper SHA256 为 `f51a2e99d7f7f8c18a73900dbcd3119f891a08cee29610533b247b7d79ca2229`，其完整字节存在于该副本。完整审核命令为 **14,892 字节、394 行**，SHA256 `6bdc38e2797396357debe58e87da207798778b4d4216d9840da0dbc49215987d`。该检查和运行中的完整命令断言共同约束了新 helper 的审核正文；不将字节存在视为独立重编译证明。见[产物核验回执](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/review-app/artifact-verification-r2.json)。

**源码复核结果**

范围限定于 `crates/keelshell-app/src/files.rs` 及 `files/` 下的 `archive_inspection.rs`、`archive_validation.rs`、`worker.rs`、`test_server.rs`、`archive_tools_fixture.rs`、`transfer_tests.rs`、`archive_validation_tests.rs`，另含 ADR 0139 和工具使用指南。已读 AGENTS、HANDOFF 首段、测试策略、共享 target 规则、HALLMARK_NATIVE 及 rust-best-practices；仅跟读必要的既有审核、并行队列和主题接口。helper 监督算法及其平台回归仍由后端 reviewer 负责，本次只复查其嵌入审核正文及相关文案。

- **授权类型与实际连接身份**：[archive_inspection.rs:6](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_inspection.rs:6) 的 Kind 与第 39 行 Reply 区分 PATH/Validation，第 156 行按两者共同匹配采纳；错配不能互授权限。[files.rs:1464](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1464)、[worker.rs:398](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/worker.rs:398) 和结果采纳处共同检查 token、host、suspended 与实际 SSH connection handle 身份。同显示主机或复用 token 不足以移植授权；旧 pending review 也不能确认替换后的草稿。替换连接和 PATH 分离已有本次运行证据。
- **取消、替换、迟到和刷新失败**：[archive_validation.rs:24](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_validation.rs:24) 在开始刷新时撤下本类旧结果，另一类观察独立保留；操作 UUID 和连接归属约束回调。取消后需新审核，失败不沿用旧成功或伪装为七项缺失。本次相应四类控制器均通过，文案仍明确关闭通道不证明远端进程停止。
- **并行传输与草稿**：[files.rs:1513](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1513) 的现有调度保留恢复、比较、传输状态；第 1947 行的工具结果分支不改编辑器或路径草稿。[parallel.rs:675](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/parallel.rs:675) 将 foreground 与活动队列共同计入 busy。静态追踪未发现本增量丢弃草稿或消费其它传输审核的路径；“新 validation 与活动并行传输、未保存草稿同时存在”的组合没有独立动态执行。
- **长完整命令与固定动作**：[archive_validation.rs:49](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_validation.rs:49) 在中英文审核中嵌入实际 `validation_command()`。[review.rs:147](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/review.rs:147) 的正文可伸缩，底部动作区不收缩；第 488 行正文支持横纵滚动。[archive_validation_tests.rs:25](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_validation_tests.rs:25) 在此次 r2 副本中通过：两种语言完整包含当前命令；展开后在 **900×580、1440×900** 均可见确认/取消；取消零执行，重新审核确认后恰一次 exec，未新增 SFTP、原子写入或传输。该证据是 headless 控制器断言。
- **三主题 token 与诚实状态**：[archive_validation.rs:103](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_validation.rs:103) 使用语义 Palette，第 158 行使用组件 token；本增量未引入主题 ID 分支、字面状态色或字体。[archive_validation_tests.rs:210](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_validation_tests.rs:210) 遍历三主题 × 明/暗 × 中/英，检查结果行存在且绘制、换主题/语言、展开不再执行工具。第 263 行状态映射保留 Passed、Failed、Unavailable、NotTested、TimedOut、OutputLimit；zip/unzip/rz/sz 功能保持 NotTested，版本观察不冒充认证。
- **文案与 r2 差分**：[ADR 0139:31](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/adr/0139-reviewed-archive-tool-validation.md:31) 对 supervisor 保持组 leader、组清理及保证边界的补充已复查；[工具指南:37](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/product/ARCHIVE_TOOL_INSPECTION.md:37) 和第 85 行仍保留本批验证中状态。两类操作独立授权、取消限制、未测功能和原生验收边界均保留。文档最终通过状态等待真实工程门禁终态更新，没有提前签成功。

**实际执行与计数**

以下为本 reviewer 的实际执行，不累计作者或父的测试：

```text
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/stable-binaries/app-r2 archive_validation_tests --test-threads=4
```

运行时间为 2026-11-15 14:49:45.912092–14:49:47.811323 UTC，1.899 秒，exit 0，未超时，进程已 wait/reap。

所有名称均带前缀 `files::transfer_tests::archive_validation_tests::`：

| 实际执行用例 | 结果 |
| --- | --- |
| archive_validation_reviews_full_command_and_runs_once_without_sftp | PASS |
| archive_validation_rejects_old_approval_after_transport_replacement | PASS |
| archive_validation_discards_a_late_result_for_the_same_display_host | PASS |
| archive_validation_cancellation_does_not_replay_without_new_approval | PASS |
| archive_validation_failed_refresh_retires_only_its_previous_result | PASS |
| archive_validation_path_discovery_does_not_grant_stream_check_results | PASS |
| archive_validation_theme_language_and_detail_changes_do_not_run_tools | PASS |

| 执行项目 | 本 reviewer 实际数量 |
| --- | ---: |
| Rust/GPUI 测试调用 | 1 |
| 测试 passed / failed / ignored / measured | 7 / 0 / 0 / 0 |
| filtered out，未执行 | 1239 |
| 完整库存 `--list` 调用 | 1 |
| Cargo 命令 / 独立 Rust 重编译 | 0 / 0 |
| 新增其它独立探针 / 原 18-case 计划执行 | 0 / 0 |
| 真实工具或 disposable OpenSSH 验收 | 0 |
| macOS / Windows / Linux 原生桌面运行 | 0 / 0 / 0 |

原始[测试日志](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/review-app/app-controller-r2.log)、[执行回执](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/review-app/app-controller-r2.json)及[运行后输入/产物汇总](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/review-app/execution-summary-r2.json)均保留。

**失败及前置断言记录**

- r2 输入审核脚本曾因过宽的“文档不得变化”断言退出 1：607 项工程输入核验没有 mismatch，实际变化是预期的 ADR 0139 supervisor 说明。本次复核了该文案差分，以 app 源码、工程输入和文档分别核对后通过。原始 `input-audit-r2.json` 和[补充判定](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/review-app/source-result-r2.json)保留；该脚本失败不计为产品测试失败，也未被删除或改写成一次全绿运行。
- 父的旧 r1 门禁因后端进程组问题中断，实际 **exit -15、499.942 秒、reaped=true**；[full-gate-r1.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/full-gate-r1.json)及后端原 notice/中断证据保留。后端 reviewer 的历史 P2 不纳入本 app 开放数量，也不由本报告代签修复结论。
- 本次 r2 七项执行为 7/7，库存和运行前后输入断言通过。没有通过重跑全仓或扩展旧 18-case 计划覆盖失败记录。

**预构建限制、未验范围与工程条件**

本 reviewer 使用父重建后的稳定**预构建**副本，**没有独立重编译**。输入摘要、构建绑定、原始/副本 SHA、完整测试库存、七个逐名 descriptor 和 helper 字节均已核对；这些证据降低缓存或旧二进制误判风险，不提供独立重建证明。

七项 GPUI 测试经加密 loopback SSH 接收 fixture 的受控回复，**不执行远端 helper 或真实七项工具**。新 helper 的平台进程监督回归、真实工具行为、真实 OpenSSH 以及正常 GUI/MCP 构建归父/后端证据。本轮收口时父告知 r2 真实 OpenSSH 16/16 和正常 GUI/MCP 构建通过，最终 full-gate-r2 已在相同冻结启动；这些未作为 reviewer 独立执行计数，也未在本报告中认证最终全仓门禁成功。

原生桌面验收仍开放：macOS/Windows/Linux 实机视觉与交互、键盘/IME/辅助技术、完整失败状态视觉矩阵、实际完整工作区几何，以及新操作与活动并行传输/未保存草稿的动态组合均未独立验收。headless 的按钮可见与主题组合断言不签署原生桌面；旧原生记录不覆盖新 UI。

本次源审和七项专项没有需要新增 `notice-N.md` 的 P1/P2。**建议条件 APPROVE**：app 复核已通过，由父确认同一冻结的 full-gate-r2 成功终态并据实更新工程文档；保持以上原生和未验范围。本 reviewer 未运行 Cargo、format、commit、push、clean 或修改源码/共享 target，所有报告与回执写入均限定于本 `review-app` 目录。
