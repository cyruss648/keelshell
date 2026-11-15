# 服务器归档 session / app 独立复核最终报告 r3

结论：**APPROVE（本分工，最终冻结 r5）**。

当前开放发现：**P0 = 0，P1 = 0，P2 = 0，P3 = 0**。
本分工累计发现 P2 **1**，即 `review-session-ui/notice-P2-001.md` 的非 Unix
fixture 编译 lint 问题，现已关闭。core reviewer 的 inherited ACL P2 是另一
分工的发现，已由其 `review-core/report-r2.md` 关闭，不重复计入本分工。

本报告解除 report-r1、report-r2 及中间 addendum-r3 的条件性 HOLD。
**原报告、addendum 和 notice 均保留原文，不覆盖历史结论；最终状态以本报告及
`completion-r3.json` 为准。** 已满足本分工最终合并条件，支持将受审 r5
生产功能及必要测试修复纳入本批本地功能提交。

## 最终来源与绑定

Checkout：
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。

基线 HEAD：`9112e232b6f0e022d299cba9e8498088c77b3d3f`。
候选是该基线加未提交冻结，不以 HEAD 单独表示产物来源。

```text
work/server-archive-20261115/frozen-inputs-r5.json
SHA-256 30770f74d710b231936e8537d5ce4082798e68a2894aa1ad7e5b9110afa08f92
```

最后独立 binding 时间：**2026-11-15 07:00:58 UTC / 15:00:58 Asia/Shanghai**。
**553 工程输入（包含 Python）和 10 脚本全部零 mismatch**。
详见 `final-binding-r3.json`；此前 gate 结束后的读回还保存在
`final-source-state-r5.json`。

r4→r5 实际仅有两个 `#[cfg(test)]` 模块内的测试文件变化：

| 文件 | r5 SHA-256 | 精确差分 |
| --- | --- | --- |
| `crates/keelshell-app/src/files/server_archive_fixture.rs` | `4df7cfa53766a05799ec0f198eccacccd8f586795ab8355853e400dbc2d35b69` | 三个 Unix caller 专用方法及其 `Checked` 导入匹配 `#[cfg(unix)]`；拆开导入以保留跨平台 `filesystem`。 |
| `crates/keelshell-app/src/files/test_server.rs` | `b954f6c7593ed6efa3622a3f10326943888152bc2b5a33c5658ca91ba2412a63` | `Server.server_archives` 测试控制字段及初始化匹配 `#[cfg(unix)]`；Peer 路径保留。 |

机械反向还原后，两文件 SHA 分别精确等于 r4 的
`d3a784673c23245d77b174efba01d282db37f6ae2e2a1bcd17ce6dae41d25e9c` 和
`bbc22f6f4e6989c5977072f6d9c88e0ef905e98b27299fd8b9c9e0ef0f4147a0`。
**所有生产、core 和脚本输入不变，Unix 有效代码不变。**
差分及快照为 `cfg-fix-r4-r5.patch`、`cfg-fix-snapshots-r5/`、
`cfg-fix-verification-r5.json`。

## 本分工 P2 的关闭

触发：r4 的 `allow`、`inputs`、`hold_next` 方法在所有平台参与测试编译，
但其 caller 仅在 Unix 编译；`pub(in crate::files)` 不会豁免未使用 API。
非 Unix strict Clippy 的 `-D warnings` 会因 `dead_code` 阻塞。父的
`visibility-probe-r1.json` 在 macOS rustc 的同模块可见性最小 probe 中实际
exit 1，报告 `method allow is never used`。这是语义复现，不是 Windows 编译。

最终修复的绝对位置：

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive_fixture.rs:2`：
  `Checked` 导入仅 Unix 编译，避免方法排除后产生 `unused_imports`。
- 同文件 **33、47、52** 行：三个方法各自匹配 `#[cfg(unix)]`。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/test_server.rs:175`、
  **262** 行：Server 的测试控制字段及初始化条件一致。
- 同文件 **61、117、122、216、217、242** 行：Peer 字段、lookup/serve、
  Fixture 创建、clone 和 Peer 初始化保持跨平台；没有整体排除模块或放宽 lint。

关闭依据为**完整 cfg 差分复核、两个文件精确反向 SHA、最终 r5 的完整成功
门禁及结束绑定零差异**。最终 gate 同时确认 Unix 测试路径未被修复破坏。
本项没有未修复的源码问题，验证状态已关闭；实际 Windows 编译仍未执行。

## 最终父门禁及产物

下列均为父实际执行，reviewer 只读核验；不计为 reviewer 执行。

| 证据 | 最终核验结果 |
| --- | --- |
| `logs/full-gate-r4.json` / `.log` | `python3 -B scripts/check.py`，实际 wait/reap，**exit 0 / 603.804s / 无超时**。最终日志 SHA：`c2e1396e5805919abe1a5343f705f9006dcdacd274b3ef1a80ba5385050652c9`，与回执和 summary 一致。 |
| 原始 gate 的逐 suite 结果 | reviewer 从原 log 重新求和：普通 Rust **2747 passed / 0 failed / 27 ignored**；doctest **14 passed / 0 failed / 0 ignored**，匹配 `validation-summary-r4.json`。Python **131 run / 1 skipped / 0 failed**。fmt、严格 Clippy、workspace 和三个 controller 专项均完成。 |
| 新增具体用例 | 原 gate 的 `test … ok` 行逐名包含 app **7**、session lib **11**、session SSH **40**、core **14**，共 **72** 个；全部存在，不用总数替代库存核验。这 72 项已包含在门禁结果内，不额外累加。 |
| 最终 app binary | 实际 SHA 匹配 `app-binary-r5.json`：`b8f4a55037feb8126cf70ea31240bd6b18604f385f23447f607ac438b09361ca`；来源为 full-gate-r4、绑定 r5。 |
| 最终 app inventory | `app-inventory-r5.txt` **1208 项**，与 r4 父清单及 reviewer 现场 r4 `--list` 输出逐字节一致。SHA：`c8a63f9415cb332f52ef18c9eb1b0c7623831462bcafccb351e6a6d59789ab34`。本次没有再次启动 binary。 |
| 真实 OpenSSH-r2 | **15/15**；回执记录 owned processes stopped、temporary directory removed。r4→r5 的 session/helper/core 生产及 session 测试输入不变，因此保留其实际互操作证据，无需重复运行。 |
| 正常 GUI/MCP build | `cargo build -p keelshell-app -p keelshell-mcp --locked`，**exit 0 / 8.215s**；`normal-build-r1.json` 和 log SHA 一致。两产物实际 SHA 匹配 `normal-artifacts-r1.json`，其 r4→r5 test-only 绑定经独立核对成立。 |
| core reviewer | `review-core/report-r2.md` **APPROVE**，历史 inherited ACL P2 关闭。r5 的全部 core 输入与其 r4 受审输入相同，最终完整 gate 条件现已满足。 |

正常 GUI SHA：
`fef16361ec31e0998f0f533f68ccdcbb76858dd38970a0d8cc9abadd641041b9`。
正常 MCP SHA：
`cf89dbc2b7fb77958d16c225d7d94c536970aca4c3f7675e544696a8a5811c88`。
详细证据为 `normal-build-verification-r5.json`、
`parent-openssh-r2-verification.json` 和 `final-binding-r3.json`。

未使用旧 full-gate-r3 的成功替代最终 r5 门禁；旧失败/停止回执全部保留。

## reviewer 的实际执行和边界

reviewer 此前独立执行的是已核验 SHA/库存的 **r4 稳定预构建**，没有独立
重编译：session **40/40**、session lib **11/11**、app 新专项 **7/7**、
共用归档回归 **9/9**，合计 **67 passed / 0 failed / 0 ignored**。
新增 cfg 修复后按用户要求**没有重复执行这 67 项，r5 reviewer 新执行数为 0**。

共同 cwd 为本 checkout；实际执行命令如下，完整 stdout/JSON 保存在本目录，
由 wrapper 在启动前核验 SHA，并以自有 process group 设置期限及回收：

```text
python3 work/server-archive-20261115/review-session-ui/run-prebuilt.py session-binary-r4 session-focused-r4 180 --test-threads=4
python3 work/server-archive-20261115/review-session-ui/run-prebuilt.py session-lib-binary-r4 session-lib-focused-r4 180 sftp::server_archive::tests:: --test-threads=4
python3 work/server-archive-20261115/review-session-ui/run-prebuilt.py app-binary-r4 app-focused-r4 240 files::transfer_tests::server_archive_tests:: --test-threads=4
python3 work/server-archive-20261115/review-session-ui/run-prebuilt.py app-binary-r4 app-shared-archive-r4 240 files::transfer_tests::archive_tests:: --test-threads=4
```

三次现场库存命令不计测试执行；旧 r1 的 47 项不重复累加。父的 72 个新增用例、
2747 项门禁和 OpenSSH 15 项也不加入 reviewer 的 67 项。
差分脚本的两次摘要转换错误及随后成功结果保留在
`cfg-fix-verification-r5.json`，没有启动工程测试，也没有工程输入 mismatch。

最后 binding 确认七个自有测试/库存进程组全部不存在，临时目录为空；两个
有界只读观察进程也已 wait/reap。未回收或终止父侧进程。
reviewer 没有运行 Cargo、格式化/修改工程源码、改变共享 target、commit、
push、cleanup 或安装；所有写入仅在本 review 目录。

最后确认原报告摘要不变：

```text
report-r1.md  280aa9f4c43d26109d8440f18c6b414d6bf3d8fbcd9cae8d7553e89a5e7df87e
report-r2.md  3f85a2197d95466f2e680ea3622b50354ec7563979ae344f2adbf449b23ad85d
```

**合并意见：APPROVE 受审最终 r5；本分工无剩余阻塞，条件性 HOLD 已解除。**
此意见不代签 Windows 实编译、Linux 文件系统权限实测、三平台原生桌面交互、
IME/辅助技术、发行打包或安装。预构建、headless GPUI、Mac 最小 probe、
正常 GUI/MCP 编译和 localhost OpenSSH 的证据边界保持独立。
