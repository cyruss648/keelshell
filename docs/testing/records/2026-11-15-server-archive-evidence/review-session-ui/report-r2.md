# 服务器归档 session / app 非作者独立复核 r2

结论：**HOLD（本分工源码及 r4 独立专项通过；等待父最终门禁终态）**。

本分工发现计数：**P0 = 0，P1 = 0，P2 = 0，P3 = 0**。没有新增
`notice-*.md`。独立执行最终 r4 预构建产物共 **67 passed / 0 failed /
0 ignored**；本 reviewer 没有运行 Cargo，也没有独立重编译。

当前签署范围是 session SSH/SFTP 服务器归档、app 两次审核、权限授权及
UI 生命周期。core helper 内部审查及 Darwin inherited ACL P2-001 的关闭由
`review-core` 负责，不把该项计为本 reviewer 的新发现，也不独立代签其修复。
本报告保留原 `report-r1.md`；r1 是旧候选的历史证据，不能替代本轮执行。

记录日期：2026-11-15，Asia/Shanghai。执行回执使用 UTC。

## 候选来源和差分

Checkout：
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。

HEAD 实际读回为 `9112e232b6f0e022d299cba9e8498088c77b3d3f`；审查对象为
该基线加未提交冻结，不能用 HEAD 单独描述最终产物。

最终冻结 `work/server-archive-20261115/frozen-inputs-r4.json` 的 SHA-256：

```text
c01ae7a78099b4805bff413a795176e0aa7732871bf6a909cd83fff6277c5eda
```

2026-11-15 06:36:06 UTC 重新计算 **553 工程输入和 10 脚本，零 mismatch**。
冻结包含 Python。最终关闭前还会再次核对输入；详细结果保存在
`inventory-source-verification-r4.json` 及最终状态回执。

r3→r4 实际只有四个工程输入变化，脚本无变化：

| 输入 | 本轮结论 |
| --- | --- |
| `crates/keelshell-app/src/files/server_archive.rs` | 仅两次审核中各一条中英文平台前提，共四条文案；机械反向还原后 SHA 精确等于 r3。 |
| `crates/keelshell-core/src/server_archive.rs` | 仅两处 rustdoc：远端平台收窄为 macOS/Linux，以及在写入前清除并验证继承 ACL；去掉 rustdoc 后 Rust 正文逐字相同，无接口或调用合同变化。 |
| `crates/keelshell-core/src/server_archive.py` | helper ACL/平台能力修复；内部正确性由 core reviewer 复核，本分工核对其调用合同和实际 session/app 集成。 |
| `crates/keelshell-core/tests/server_archive.rs` | core 修复回归；由 core reviewer 负责。 |

app/session 共 **402** 个冻结输入，除上述一个 app 文案文件外，其余 **401**
个与 r3 完全相同。`server_archive.rs`、mutation admission、specialized exec、
session fixture、OpenSSH 新例以及 app worker/lifecycle 均无新生产逻辑差分。
证据为 `r4-delta-verification-r1.json`、`final-delta-scope-r4.json` 和
`core-interface-delta-r4.patch`。

四条新前提位于以下绝对位置，均明确为 remote macOS/Linux、Python 3.8+、
可核验的私有文件权限：

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive.rs:88`
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive.rs:93`
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive.rs:191`
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive.rs:196`

没有保留允许任意 POSIX 远端的旧提示，也没有新增平台 fallback 或授权分支。

## 源码审查结论

本轮延续 r1 全量源码复核，并确认最终输入中以下不变量没有改变。以下均为
当前 checkout 的绝对路径及一基行号，表示审查证据，非缺陷。

| 检查 | 位置与结果 |
| --- | --- |
| 真实连接和 stale authority | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/server_archive.rs:330` 按实际 SSH connection 比较；`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive.rs:124`、`:133` 把浏览 revision、目录/选择、格式、名称、host 和实际连接一起绑定。相同 host/token 的替换连接不能继承审核。 |
| 两次完整确认 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive.rs:44`、`:101`、`:187`：先单独确认完整只读命令，再以新 review ID 审核完整成员、摘要、final、stage、写命令和精确 UTF-8 stdin；第一次确认不授予打包权限。 |
| 名称/选择变化及迟到回包 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1454`、`:1770`、`:1803`、`:1937` 重新核对 operation/binding；`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/local_browser.rs:96`、`:115` 撤销旧审核。迟到扫描、旧确认或取消 listener 不会建立新权限。 |
| EXEC ack、stdin、EOF、组合预算 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:798`：OPEN 后重新 admission/revalidation，EXEC 派发前设置 pending，收到 Success 后才输入；输入/输出并发，写完显式 EOF，共用 stdout/stderr 上限，要求完整 exit/EOF/CLOSE。 |
| 单一 deadline | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/server_archive.rs:222`：同一期限覆盖 admission、exec、stdin、解析及完整 SFTP 读回，不因阶段切换延长。 |
| source/final/stage reservation | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/mutation.rs:245` 建立源树读 claim 和 final/stage 写 claim；`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/server_archive.rs:235`、`:252` 持有并在派发前重验至最终验证完成。 |
| dispatch/drop/quarantine/readback | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/server_archive.rs:272`、`:321`：完整成功前不清 pending；派发后取消、超时、拒绝、drop 及读回失败隔离两个写目标。SFTP 验证完整字节、长度、attrs、SHA、codec 成员及 stage 缺席。 |
| 旧隔离 owner 保留 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1483`、`:1698`、`:1778`：PackServerArchive 接入 previous-owner 路径，worker 启动失败、成功或已知失败恢复旧 owner，未知结果保留当前 close hold。 |
| worker 取消语义 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/worker.rs:414`、`:878`：session 自己处理 pack 的取消和未知写结果，外层不会先 drop 已派发操作并误报普通取消。 |
| 主题、双语及固定动作 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive.rs:210`、`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive.rs:234` 使用 registry/Palette/组件 token；`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/review.rs:147`、`:488` 保留完整滚动正文和固定确认/取消。 |
| 实际 helper fixture / OpenSSH 新例 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/support/server_archive_fixture.rs:377`、`:533` 在受控环境执行精确 helper；`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/openssh_interop.rs:18` 覆盖真实两 codec、特殊名称、预取消、完整成员和 stage 消失。 |

没有把应用内 reservation、SFTP attrs 或 SSH channel close 解释为全服务器
文件系统锁、远程进程已终止、自动回滚或 crash durability 保证。

## 稳定预构建绑定和独立执行

三份父 metadata 均绑定 `frozen-inputs-r4.json`，binary 位于本批独立
`prebuilt-r4/` 快照，避免后续唯一 Cargo owner 替换共享路径产物。reviewer
实际重新计算快照 SHA；没有使用已失配的旧 app metadata，也没有绕过摘要门。

| 快照 | binary SHA-256 | 现场库存数 / inventory SHA-256 |
| --- | --- | --- |
| `prebuilt-r4/session` | `23475667fa92ce812723f0024f613ed04850a74115148ced8b72be0c0b763366` | 40 / `4e283e8cfd84788f9c76b5ffe3c3ec6dbd84a70aeb1fe0d62787f95f4bede784` |
| `prebuilt-r4/session-lib` | `70ca84e15aacdabbcf4afd5e7553d8c016b9894bef60dfd20edcf66c0b58044f` | 158 / `c578f8f4b1650165992a270b457add59df888af336b4d45a7d22f60dcd862849` |
| `prebuilt-r4/app` | `8b249ab269786157589fc6ffa58cd294f3c9de59dd991c0bb1518f499b559a4c` | 1208 / `c8a63f9415cb332f52ef18c9eb1b0c7623831462bcafccb351e6a6d59789ab34` |

三次直接 `--list` 的退出均为 0，输出与父 inventory **逐字节相同**。
不仅核对总数：session 的 40 项、app 新增 7 项、共用归档 9 项及 session-lib
新增 11 项具体名字均已核对；11 项和 7 项逐名对应当前源码。完整名字保存在
`prebuilt-verification-r4.json`、`inventory-source-verification-r4.json`
及三份现场 inventory log。

session-lib 新 11 项的模块均为 `sftp::server_archive::tests::`：

```text
server_archive_published_file_stamp_accepts_private_complete_regular_file
server_archive_published_file_stamp_rejects_receipt_length_mismatch
server_archive_published_file_stamp_rejects_size_over_encoded_cap
server_archive_published_file_stamp_rejects_symlinks_even_with_correct_size
server_archive_published_file_stamp_rejects_unexpected_public_permissions
server_archive_published_file_stamp_rejects_unknown_size
server_archive_receipt_rejects_empty_stdout_even_with_zero_exit
server_archive_receipt_rejects_nonempty_stderr_even_with_zero_exit
server_archive_receipt_requires_an_explicit_successful_exit
server_archive_root_sibling_stage_has_no_double_separator
server_archive_stage_is_an_exact_uuid_sibling_before_review
```

实际执行命令如下；共同 cwd 为前述 checkout。每个命令由本目录
`run-prebuilt.py` 启动，先核验 SHA，TMPDIR/TMP/TEMP 均指定本目录 `tmp/`，
库存期限 30s，session/session-lib 180s，app 两组 240s。超时有自有 process
group TERM/KILL 和直接子进程 wait/reap。

```text
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/prebuilt-r4/session --list
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/prebuilt-r4/session-lib --list
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/prebuilt-r4/app --list

/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/prebuilt-r4/session --test-threads=4
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/prebuilt-r4/session-lib sftp::server_archive::tests:: --test-threads=4
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/prebuilt-r4/app files::transfer_tests::server_archive_tests:: --test-threads=4
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/prebuilt-r4/app files::transfer_tests::archive_tests:: --test-threads=4
```

| 独立执行 | 退出 | passed / failed / ignored / filtered | wrapper 耗时 | reviewer 证据 |
| --- | --- | --- | --- | --- |
| session 集成 | 0 | **40 / 0 / 0 / 0** | 1.751s | `session-focused-r4.json` / `.log` |
| session lib 新单测 | 0 | **11 / 0 / 0 / 147** | 0.006s | `session-lib-focused-r4.json` / `.log` |
| app 新专项 | 0 | **7 / 0 / 0 / 1201** | 4.903s | `app-focused-r4.json` / `.log` |
| 共用归档必要回归 | 0 | **9 / 0 / 0 / 1199** | 2.001s | `app-shared-archive-r4.json` / `.log` |

本轮独立实际执行合计 **67 项**，不叠加旧 r1 的 47 项或父的运行。三次库存
命令不计测试执行。app 的真实两次确认同时覆盖 ZIP/tar.gz；主题用例遍历
三 family、明暗和双语，矩阵不另外增加 Rust 测试数量。共用归档组包含
`archive_pending_write_cancel_retains_close_hold_across_another_verified_upload`。

旧 `app-shared-archive-refused-r1.json` 继续保留：当时因 SHA 不符，binary
未启动，0 项执行。本轮使用稳定 r4 app 快照完成 9/9，不改写旧拒绝回执。
`independent-r4-summary.json` 记录逐个实际通过名、日志 SHA 和运行回执。

2026-11-15 06:38:26 UTC，7 个自有运行/库存进程组全部不存在，`tmp/` 无遗留。
没有超时，也未回收、终止或清理父侧进程/数据。

## 父证据及合并边界

已读取父 **OpenSSH-r2** 的 `logs/openssh-r2.json`、
`openssh-r2/result.json` 和 `tests.log`：在最终 r4 冻结后执行，退出 0，
真实 localhost OpenSSH **15 passed / 0 failed / 0 ignored**，包含新
`openssh_server_archive_packs_remote_sources_with_exact_manifest_and_readback`。
回执记录 owned processes stopped、temporary directory removed；
wrapper 为 6.660s，实际测试 log 为 5.19s。该项属于父的执行，不加入本 reviewer
67 项，也未复用旧 OpenSSH-r1。

父 full-gate-r3 尚待终态。已读到本轮 Python tests、fmt 和严格 Clippy 通过，
workspace tests 仍在进行；没有根据中间 log 宣称整条 gate 已通过。旧
full-gate-r1 Clippy 失败、full-gate-r2 exit -15 继续作为历史证据保留，
不用于代签最终候选。

core reviewer 正在独立核验 ACL 修复；本报告不以父 core14/14 替代其关闭意见。
正常 GUI/MCP 构建由父提供回执，本 reviewer 不运行 Cargo。

本 reviewer 没有修改/格式化工程源码、commit、push、cleanup、安装或改变共享
target；写入仅在 `work/server-archive-20261115/review-session-ui/`。已遵循
AGENTS、RTK、HANDOFF 顶部、STRATEGY、ADR0135 和 SHARED_TARGET 要求。
原 `report-r1.md` SHA-256 仍为：

```text
280aa9f4c43d26109d8440f18c6b414d6bf3d8fbcd9cae8d7553e89a5e7df87e
```

预构建、headless GPUI、受控 SSH、localhost OpenSSH 或普通编译均不能代签
macOS/Windows/Linux 三平台原生桌面交互、IME、辅助技术、发行安装或真实客户
环境验收。新的远端平台前提也不表示已独立实测 Linux、Python 3.8 或其他文件
系统权限实现。

**合并意见：本分工源码及 r4 独立专项无阻塞项；等待同一冻结的 full-gate-r3
终态、正常 GUI/MCP 构建及 core reviewer ACL 关闭后再解除最终合并 HOLD。**
