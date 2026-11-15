# KeelShell r2 app / SSH bridge / 原生审核 UI 独立差分复核

结论：**APPROVE，仅限 r2 冻结候选的 app/SSH bridge/原生审核 UI 源码及本次专项范围**。开放 **P0 0 / P1 0 / P2 0 / P3 0**；r1 的三项 P2、一项 P3 均关闭。**direct-PTY 互操作仍为 HOLD / PARTIAL / OPEN，产品总验收不通过。** 本报告不授权用工程结论替代互操作、完整门禁或原生验收。

唯一目录 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`，HEAD 仍为 `721bad2157da8d3a0bc11a1eab5ac59b6616aa9c`。只读读取 Git 元数据核对，未运行 git。r2 冻结 SHA-256 为 `5119d1724074afb5332148f5002b4614767f6ec2837acd6af62a596b0d8ebfb1`；开始和结束逐项检查 **570 个工程输入 + 12 个脚本均零差异**。自行计算 r1→r2 delta 与 `freeze-delta-r1-r2.json` 完全相同：六个 Rust 文件变化、脚本零变化。

本轮完整差分复核三个 app 文件：`ssh_bridge.rs`、`terminal_zmodem.rs`、`terminal_zmodem_tests.rs`；另检查不变的协议写队列/UUID 交接及相关 session 合约。session 的三个变化文件不在本报告完整源码批准范围，由其专职复核及父门禁负责。Linux fixture 的十二个脚本输入未变，沿用 r1 的源码结论；**本轮未重跑 Python 测试、未启动或清理容器/SSH fixture**。

**r1 问题逐项关闭**

1. **APP-BRIDGE-R1-001 / P2 — CLOSED。** `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/terminal_zmodem.rs:392`，关键交接在 425–436 行。终态不再以存在 current 卡片为前提，全部按事件 UUID 排入有界去重交接队列；背压时保留待发项，不同 UUID 的旧结果不改写当前 offer。`retry_zmodem_resume` 在 183–197 行重试，最终解锁仍由不变 bridge 的精确 UUID、非 active 状态和写队列清空条件决定。新增 Cancelled/Failed/Finished 三种 pre-offer 终态、背压及 stale ID 五项 GPUI 回归全部通过，测试位置 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/terminal_zmodem_tests.rs:256`。测试通过 mailbox 注入终态并模拟 bridge 释放 latch；不声称完整生产 wire 复现。

2. **APP-BRIDGE-R1-002 / P2 — CLOSED。** `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/ssh_bridge.rs:159`，以及 224–239、298–348 行。Written/ShellEnded/Stopped 分开处理；**只有 Written 调用 write_completed**，ShellEnded 保留 shell 并返回 pump，以 completion_seen 阻止后续写入，同时继续有界交付尾部 Data/Exited。中断的普通写前缀不重放，未完成的协议写也不会误记为完成。r1 失败的既有 tail 测试在 bridge 组中通过，并额外 **5/5 次单独通过**；原有最终 OO 写完成/失败专项仍通过。

3. **APP-BRIDGE-R1-003 / P2 — CLOSED。** `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/terminal_zmodem.rs:360`，以及 443–475、697–706 行。取消成功提交后进入 Cancelling，重复和保留回调不能再次提交，按钮禁用；Review/Progress/InputRejected/ActionRejected 均不能复活 Cancelling/Ended 状态。两项新增实际控制回归通过：一次取消提交、三类终态后的迟到拒绝/进度/审核都保留原回执且不重建输入封锁。测试位置 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/terminal_zmodem_tests.rs:451`。

4. **APP-BRIDGE-R1-004 / P3 — CLOSED。** `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/terminal_zmodem.rs:310`，显示在 710–727 行。VerifiedSave 独立保留原始 UUID、路径、字节数、SHA-256；后续 offer、Finished 与收起卡片不会删除它，下一卡片仍能显示。最近 32 项有明确双语说明，重复通知不挤掉原记录。两项新增回归通过，覆盖同 poll 多事件、跨 offer/Finished/收起、后续传输及 35→32 边界，测试位置 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/terminal_zmodem_tests.rs:561`。主动收起后卡片隐藏属于显式用户操作，历史仍保留。

**独立执行与来源核对**

父稳定副本 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/stable-tests/app-r2` 的 SHA-256 为 `8afd5978df434493edfb24f6b50bb5c3343d747245e1bdc7da04edb165368222`；与元数据一致。实际 `--list` 输出与库存逐字相同，**1227 个测试名**，也与元数据数组逐名匹配。相对 r1 新增十个 GPUI 测试名均包含在实际执行的 Zmodem 19 项中。元数据来源为父 `app-zmodem-r2` 构建，冻结为 r2。**reviewer 未独立重编译，也未运行 Cargo、fmt、Clippy 或完整 workspace gate。**

| 实际有界执行 | 结果 | 退出码 |
| --- | --- | --- |
| app-r2 zmodem --test-threads=1 | 19 通过 / 0 失败 | 0 |
| app-r2 ssh_bridge::tests:: --test-threads=1 | 6 通过 / 0 失败 | 0 |
| 原失败 tail 用例 --exact，额外重复 5 次 | 每次 1 通过 / 0 失败 | 每次 0 |

两组重叠三个 bridge 测试，**去重 22 项全部通过**；含重叠与额外重复共 **30 次执行全部通过**，不能称为 30 个独立用例。日志及命令保存在 `focused-execution-r2.json` 与七份专项日志；二进制/库存核对见 `app-prebuilt-verification-r2.json`，新增十项清单见 `report.json`。

额外检查 publication_running 的新增 GPUI 回归通过：active 释放后，真实共享 publication latch 仍为 true 时，多次点击风险确认都不能确认未知结果或放行关闭；实际 worker latch 清零后还须重新完成两次确认。未知 epoch 与独立未解决集合未削弱。现有 endpoint/OSC、精确输入交接、未知结果跨成功、最终 OO 写 guard 均通过。本次长审核测试还包含 32 份完整保存回执，跨 Frost/Graphite/Lavender、明暗、双语共 12 组合验证最后记录可滚动到达、动作固定且确认/取消始终可达。这些为 GPUI 测试证据，**不代表真实桌面原生通过**。

**互操作与总验收边界**

父方当前 macOS `real-pty-r6` 真实结果为 **6/7、wrapper exit 1 / test binary exit 101**；reviewer 只读核对 `real-pty-r6/result.json`、测试汇总及运行回执，未独立重跑。失败项是 `real_sz_sends_binary_unicode_content_over_crc32`。按父最新观测，macOS lrzsz 0.13.1 下 FileSaved 内容完整、ZFIN 有效、peer exit 0，但没有 OO，故 **仍不能接受 direct-PTY 全互操作**。Linux 最近获告知 r3 仅 4/7，也没有获得 acceptance。旧 r5 7/7 和 pipe 7/7 均不能替代本轮候选结果，原失败材料继续保留。

父正在相同冻结运行 `full-gate-r2`；本报告不声明其已完成或通过。产品总目标、FILE-05、三平台原生交互、IME、辅助技术、包和发行验收均未签署。session 安全发布/协议完整审查及缺 OO 的根因处理不由本次 app 差分批准覆盖。

r1 报告和失败日志保持原件。本轮只写入 `review-app-bridge-r2/` 报告、专项日志及隔离测试临时文件；未修改源码、Git、共享 target 或执行父方资源清理。最终建议是 **接受上述 app 差分源码修复；继续保留完整门禁及 direct-PTY 互操作 HOLD**。
