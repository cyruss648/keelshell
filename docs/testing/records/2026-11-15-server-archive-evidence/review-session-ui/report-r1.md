# 服务器归档 session / app 非作者独立复核 r1

结论：**HOLD（验证证据待补齐；本分工未发现源码阻塞项）**。

P0 **0** / P1 **0** / P2 **0** / P3 **0**。
没有已确认的 P0–P2 源码发现，因此没有创建 `notice-*.md`。
本报告支持已经复核的 session/app 实现继续集成验证；目前不建议把整个功能
标记为最终可合并或完成，因为当前内嵌 helper 已偏离先前冻结，最终产物和门禁
尚待重新绑定。HOLD 是证据状态，不是未披露的缺陷计数。

记录时间：2026-11-15，Asia/Shanghai；下列执行回执保留 UTC。
我是本批新加入的非作者 reviewer，负责 session SSH/SFTP 服务器归档以及 app
审核、权限和 UI 生命周期。core helper/协议/codec 内部审查由另一 reviewer
负责；本报告只核对这些接口与 session/app 的调用合同。

## 审查基线和只读边界

Checkout：
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。

HEAD 实际读回：
`9112e232b6f0e022d299cba9e8498088c77b3d3f`。
审查对象是该基线加未提交的冻结工程输入，不能仅用 HEAD 表示候选来源。

已读取 AGENTS、`/Users/hdh/.codex/RTK.md`、HANDOFF 顶部、STRATEGY、
ADR 0135 和 SHARED_TARGET 指南。仅使用只读 Git 和源码读取；
没有运行 Cargo、rustfmt/Clippy、构建或安装，没有修改工程源码、commit、push、
清理或改变共享 target。所有 reviewer 写入仅在：

`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/review-session-ui/`。

已核验的冻结：

| 冻结 | SHA-256 | 现场核验 |
| --- | --- | --- |
| r2 | `9b01e8e98cfc3a2039cfe213ee2b1fff2f3e54e688f0cbdff19be4e384662719` | 2026-11-15 06:15:10 UTC，553 工程输入及 10 脚本零 mismatch |
| r3 | `af92acf079df41b4d4f12ef45af5abd49ba96c06d5275010cbc458e1fd6965cf` | 06:21:06 UTC，553/10 零 mismatch；r2→r3 仅一个 app 测试 fixture |
| 当前与 r3 | 以补齐的新冻结为准 | 06:26:48 UTC 读回两个 mismatch：`crates/keelshell-core/src/server_archive.py`、`crates/keelshell-core/tests/server_archive.rs` |

session binary 元数据绑定 r1；r1→r2 只有两个 app 测试输入变化，session 和
helper 输入当时一致。r2→r3 的 fixture 修改经机械反向还原后 SHA 精确等于 r2，
确认为三个 `.expect`→`.checked` 和 `Checked` 导入，生产输入没有变化。
随后发生的 helper 修改属于新的工程输入；旧 binary 的运行不代签该修改。

详细摘要证据为本目录的 `input-verification-r1.json`、
`r3-delta-verification-r1.json` 和 `source-scope-r1.json`。
完整受审 tracked diff 保存在 `reviewed-tracked-diff-r1.patch`；
新增 Rust 文件已逐一阅读全文，而非仅依赖 `git diff --stat`。

## 源码审查结果

以下位置均为 checkout 内绝对文件及一基行号；区间用于说明已核对的逻辑，
并不是发现项。

| 检查 | 位置和结论 |
| --- | --- |
| 实际连接身份 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/server_archive.rs:17`、`:203`、`:330`；计划保留实际 SSH 连接，执行按 `same_connection` 拒绝同端点新连接。 |
| app 当前授权 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive.rs:124`、`:133`、`:146`；绑定浏览 revision、目录、选择、格式、名称、host 和实际连接。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/worker.rs:390` 在打开 SFTP 前再次核对实际连接。 |
| 两次审核及准确内容 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive.rs:44`、`:101`、`:187`；第一次完整只读命令，第二次完整成员、大小、摘要、final、stage、写命令和原字节 UTF-8 stdin；不使用 lossy 转换。 |
| stale / cancel / late result | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1454`、`:1770`、`:1803`、`:1937`；当前 binding、operation ID、browser revision 和实际连接在启动和结果采用时分别检查。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/local_browser.rs:96`、`:115` 撤销草稿/选择变化的旧审核。 |
| 不复用第一次确认 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:2165`、`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/review.rs:89`；每个 proposal 使用新 review ID，旧确认或取消 listener 不能作用于新 pending。 |
| exec acknowledgement / input / EOF | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:792`；OPEN 后重新检查并运行 admission revalidation，派发前设置 pending；收到 Success 才发送 stdin，写完后 shutdown 发送 EOF；输入与输出并发，避免窗口背压遮蔽输出失败。 |
| 输出完整性和预算 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:869`、`:1188`；stdout/stderr 共用上限，拒绝重复 exit、EOF 后输出或不完整终态；要求明确 EOF、exit status 和 CLOSE。 |
| 单一期限和撤销 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/server_archive.rs:91`、`:146`、`:222`；planning/execution 分别建立固定 deadline。execution 的 admission、exec、stdin、输出、解析、读回、codec 验证和 stage 检查共用期限，不因阶段切换续期。 |
| 资源 reservation | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/mutation.rs:245`；源树读 claim，final 和 stage 两个写 claim。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/server_archive.rs:235` 持有 scope 至全部读回完成；`:252` 派发前重验。 |
| dispatch / pending / drop | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/server_archive.rs:321`、`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/mutation.rs:70`；只有完整成功才清 pending，派发后错误、超时、取消和 future drop 都保留两个写隔离；源读资源不会误变成未知写记录。 |
| final 和 stage 读回 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/server_archive.rs:272`、`:475`；要求私有普通文件、准确长度、前后路径/句柄 attrs、EOF、关闭成功、整体 SHA 和完整成员匹配，最后确认 stage 不存在及父路径观察仍一致。未发现自动删除、重试或覆写分支。 |
| 保留旧隔离 owner | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1483`、`:1698`、`:1778`；PackServerArchive 接入共用 previous-owner 路径，worker 创建失败、成功及已知失败恢复旧 owner，未知结果保留当前 close hold；完成新的归档不会清空更早的关闭保护。 |
| 取消后的实际结果 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/worker.rs:414`、`:878`；pack 属于由 session 自己管理取消和未知结果的分支，外层不会抢先 drop action 并把已派发写误报为普通取消。 |
| 主题、双语、固定动作 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive.rs:210`、`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive.rs:234`；使用 registry/Palette/组件 token，无按 theme ID 的业务分支。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/review.rs:147`、`:488` 保留完整滚动正文及固定确认/取消。 |
| 实际 fixture 和 OpenSSH 新例 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/support/server_archive_fixture.rs:377`、`:533`；正常路径实际执行精确注册的 helper，故障注入边界明确，进程有时限。app fixture 也执行实际 helper，再把发布文件镜像到受控 SFTP。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/openssh_interop.rs:18` 新增真实两 codec、特殊文件名、预取消、完整成员和 stage 消失检查。 |

没有把 SFTP attrs 观察、应用内 reservation 或 SSH channel close 当作服务器
文件系统锁、远程进程终止、回滚或 crash durability 证明。

## reviewer 实际执行

全部 binary 由父预构建；**没有 reviewer 独立重编译**。
运行前按元数据核验 binary SHA，现场 `--list` 输出与父 inventory 逐字节相同；
不仅核对总数，也核对本批具体测试名。

| 产物 | binary SHA-256 | inventory SHA-256 |
| --- | --- | --- |
| session-r1，40 项 | `1cada206d038ee0f0bc370911e33ffb50b4c81ca76d38e9d02a18780ae94832b` | `4e283e8cfd84788f9c76b5ffe3c3ec6dbd84a70aeb1fe0d62787f95f4bede784` |
| app-r2，全清单 1208 项 | `14d3cf04556dbf81c321e59e3d2988a7216129d36b1c43c426b0c3f97af2a981` | `c8a63f9415cb332f52ef18c9eb1b0c7623831462bcafccb351e6a6d59789ab34` |

共同 cwd 为上述 checkout。实际 binary 命令如下，均使用本目录有界
`run-prebuilt.py` 启动并保存独立 JSON/log，临时数据仅位于本目录 `tmp/`。

```text
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-session/cb81b114512a82d8/out/server_archive-cb81b114512a82d8 --list

/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-app/dd89ff795559e915/out/keelshell_app-dd89ff795559e915 --list

/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-session/cb81b114512a82d8/out/server_archive-cb81b114512a82d8 --test-threads=4

/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-app/dd89ff795559e915/out/keelshell_app-dd89ff795559e915 server_archive --test-threads=4
```

| reviewer 命令 | 退出 | 实际测试数和结果 | 证据 |
| --- | --- | --- | --- |
| session `--list` | 0 | 库存 40，不计测试执行 | `session-inventory-r1.json` / `.log` |
| app `--list` | 0 | 库存 1208，新增 7 项均存在，不计测试执行 | `app-inventory-r1.json` / `.log` |
| session 集成 | 0 | **40 passed / 0 failed / 0 ignored**，1.728s | `session-focused-r1.json` / `.log` |
| app 新专项 | 0 | **7 passed / 0 failed / 0 ignored / 1201 filtered**，4.461s | `app-focused-r1.json` / `.log` |

独立实际执行共 **47 项**，无重复叠加父运行。
app 7 项中真实两次确认用例同时执行 ZIP/tar.gz；主题用例遍历三 family、
明暗和双语，这些矩阵不是额外测试数。

随后尝试在同一 app metadata 上执行共用 `archive_tests` 生命周期回归：

```text
python3 work/server-archive-20261115/review-session-ui/run-prebuilt.py app-binary-r2 app-shared-archive-r1 240 files::transfer_tests::archive_tests:: --test-threads=4
```

摘要门返回 `binary SHA mismatch`，wrapper 退出 **1**，**binary 没有启动，
实际测试数 0**。共享路径的产物已在父门禁中被替换；未绕过摘要门，也未覆盖
已有成功日志。见 `app-shared-archive-refused-r1.json`。

各测试命令有 180/240s 的外部期限，超时按自有 process group TERM/KILL/reap；
库存命令 30s。所有实际运行均在期限内退出，无超时。`process-audit-r1.json`
读回四个自有进程组均已不存在，本目录 `tmp/` 无遗留文件。未清理他人数据。

## 父证据、剩余边界和合并意见

已独立读取父 `work/server-archive-20261115/openssh-r1/result.json` 和
`tests.log`：真实 localhost OpenSSH **15/15**，包含新服务器归档测试；
回执记录 owned processes stopped、temporary directory removed。这是父的实际
运行，未写成 reviewer 执行或三平台原生验收。

父 full-gate-r1 在 Clippy 失败，原件保留；r3 仅修正测试 fixture 的三个
deny-expect。现场读取 `full-gate-r2.json` 为 **exit -15**、96.247s，
不视为通过。该回执终止于 2026-11-15 06:18:21 UTC；本报告不推断停止原因。

最终解除本报告 HOLD 所需的工程证据：

1. 当前 helper 修改完成后的新冻结，重新核对 553 工程输入/10 脚本及本分工差分；
   helper 内部修正由 core reviewer 完成复核。
2. 新冻结对应的 session lib binary/inventory，核对并独立执行新增 **11 项**
   单测；目前尚未取得这份元数据，不能宣称已经执行。
3. 当前 app binary 的新摘要/inventory，在有效产物上验证新 7 项及共用归档
   close-hold 回归。必要时重跑受 helper 修改影响的 session 集成。
4. 父对相同最终冻结完成成功 full gate 和正常 GUI/MCP 构建，并保留失败回执。

本 reviewer 不重复作者全仓门禁，不覆盖 core reviewer 的分工。预构建 binary、
headless GPUI、受控 SSH 和 localhost OpenSSH 均不能代签 macOS/Windows/Linux
原生桌面交互、IME、完整辅助技术、Windows 安全解包或发行安装。

**合并意见：HOLD 最终功能提交；本分工源码零发现，允许继续集成验证。**
当上述证据齐备且最终差分没有新增问题时，可将本分工结论更新为 APPROVE；
不得把旧 helper 的 47 项成功改写为新冻结的成功。
