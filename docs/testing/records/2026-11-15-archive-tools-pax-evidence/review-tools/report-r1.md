KeelShell 服务端工具观察：非作者独立只读审查 r1

**结论：APPROVE，限本报告的 transport 与 GPUI UI 写集。P0 = 0，P1 = 0，P2 = 0，P3 = 0。**

没有发现需要修改的范围内问题。建议父线程在同一 r2 冻结的完整工程门禁成功后收口本切片，并继续明确保留原生平台验收边界。本报告不批准整批 PAX 生产实现，也不代表三平台原生操作、发布或安装验收通过。报告取证截止为 **2026-11-15 13:05:06 Asia/Shanghai（05:05:06 UTC）**；此时父线程 full gate r2 尚无终态退出回执。

**身份、冻结与只读边界**

- 工作目录始终显式指定为 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。
- 初始及最终 HEAD 均为 `e97705c3d773ea6919d76441685c5064e641ca86`。候选为该基线上的当前未提交文件；没有将候选冒称为已提交 SHA。
- 开始审查前读取 `AGENTS.md`、`/Users/hdh/.codex/RTK.md`、`docs/HANDOFF.md`、`docs/testing/SHARED_TARGET.md`；应用 `rust-best-practices`，检查所有权、错误传播、取消、资源回收、测试及公共 API 文档。
- 首次核验 r1 的 **538 个 Rust/Cargo 输入和 10 个脚本**，内容摘要全部匹配。r1 冻结 SHA256 为 `9153a94c3551829a4030e90f6e3ffc801815f4fb1ad392646bf868d032d87a93`。
- 用户通知父线程修改 PAX 测试字面量并生成 r2 后，独立核对 r1→r2：仅 `crates/keelshell-core/tests/archive_pax.rs` 的输入摘要变化，脚本全部相同，本报告 tools/UI 源码全部相同。
- 在内存中把当前测试文件的七处 `*b"xg"`、一处 `*b"05"` 反向替换为对应字节字符数组，重建字节的 SHA256 与 r1 冻结完全相同。因此确认这八处替换是该文件 r1→r2 的全部变化，未写回源码。
- 最终再次核验 r2 的 **538 个输入、10 个脚本**：**0 mismatch，0 新增工程输入，0 缺失工程输入**。r2 冻结 SHA256 为 `b648c546c07a09d0ef5abf0d2f461917528c265019a7afefe583abcab6b42353`。
- 另记录 14 个审查写集文件的 SHA256，包括未进入 Rust/Cargo 冻结的 ADR 0133 和产品文档；最终与该补充快照也全部相同。
- 没有编辑源码、测试或产品文档，没有格式化、Cargo、build、Clippy、提交、push、cleanup，也没有写入共享 target。reviewer 自己的脚本、报告、日志和测试临时目录仅写入本 `review-tools/` 目录。两次测试清单命令及两次专项命令均直接执行用户指定的父构建二进制。

初始的宽泛 Git `.rs` 清单额外列出了一个历史证据文件 `docs/testing/records/2026-11-15-transfer-recovery-batch-evidence/review-before-fix/worker_start_state_probe.rs`；它不是 crate 工程输入。按冻结的实际口径 `crates/**/*.rs` 加应用 Cargo manifests/lock 重新枚举后，工程清单恰为 538 项且无增减。初始记录保留，未通过覆盖旧证据消除该口径差异。

冻结证据均在本目录：

- [freeze-initial.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-tools/freeze-initial.json)
- [freeze-r2-first.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-tools/freeze-r2-first.json)
- [r1-r2-equivalence.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-tools/r1-r2-equivalence.json)
- [scoped-inputs.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-tools/scoped-inputs.json)
- [freeze-final-r2.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-tools/freeze-final-r2.json)

**审查范围与逐项证据**

完整检查新增 session `src/archive_tools.rs`、`tests/archive_tools.rs` 及 `src/lib.rs` 增量；app `files.rs`、`files/archive_inspection.rs`、`archive_inspection_tests.rs`、`archive_tools_fixture.rs`、`worker.rs`、`test_server.rs`、`transfer_tests.rs`、`archive.rs`；session `tests/openssh_interop.rs` 增量；ADR 0133 和 `ARCHIVE_TOOL_INSPECTION.md`。另外沿调用链只读检查共享 SSH channel owner、审核正文/footer、传输取消与关闭保护；没有扩展批准 PAX 生产解码实现。

| 核查项 | 绝对文件与起始行 | 结论与覆盖 |
| --- | --- | --- |
| 固定命令只观察七个工具，不运行所列工具 | [session archive_tools.rs:22](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_tools.rs:22)、[archive_tools.rs:132](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_tools.rs:132) | 命令是固定常量，无用户输入插值；循环体使用 `command -v "$ks_tool"`，其余为 shell 控制、赋值和 `printf` 输出。没有调用 tar/gzip/zip/unzip/python3/rz/sz 的执行语句。sentinel 保留 lookup 尾部换行供严格验证。 |
| 完整、严格且有界的响应 | [archive_tools.rs:167](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_tools.rs:167)、[archive_tools.rs:234](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_tools.rs:234)、[ssh.rs:740](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:740) | 要求零退出状态、空 stderr、准确版本帧、UTF-8、七项固定顺序和准确 NUL 终止；拒绝截断、额外字段、状态/路径矛盾、控制与危险显示字符。每路径最多 4096 字节，stdout/stderr 合计最多 32 KiB。Missing 与 Unverified、环境失败保持分离，不把失败生成七个缺失。 |
| 10 秒、取消及迟到 open 清理 | [archive_tools.rs:177](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_tools.rs:177)、[ssh/channel.rs:150](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh/channel.rs:150)、[ssh/channel.rs:199](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh/channel.rs:199) | 单一绝对期限包住 open、exec、响应和正常清理；入口、open 后、执行中与采用前检查取消，最终再检查超期/连接状态。future 被取消或丢弃后仍有独立 owner 接管迟到 channel；未确认 open 或 CLOSE 失败可关闭共享 transport。源码明确说明清理可晚于本地取消/期限，不能证明远端进程已经终止。 |
| 人工审核与实际连接身份 | [archive_inspection.rs:34](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_inspection.rs:34)、[files.rs:1450](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1450)、[worker.rs:390](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/worker.rs:390)、[ssh.rs:876](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/ssh.rs:876) | 审核消息含主机和完整固定命令；确认及回调检查 panel token、host 和 `same_connection`，worker 入场再检查真实连接所有权。旧绘制按钮也绑定 token/connection。probe 在 SFTP 初始化之前返回，所有网络操作在已有专属 transport worker。 |
| 旧成功失效、迟到结果不转移 | [archive_inspection.rs:81](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_inspection.rs:81)、[files.rs:1667](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1667)、[files.rs:1738](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1738) | 开始实际检测立即撤销旧 observation；worker 创建失败、运行失败、取消都有 failed 收口。身份替换不采用结果；suspend 撤销状态。每次检测都需新审核，没有后台刷新、持久缓存或结果到执行权限的转换。 |
| 原有恢复、未知 hold、比较与草稿 | [files.rs:1336](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1336)、[files.rs:1467](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1467)、[files.rs:1496](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1496)、[files.rs:1554](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1554)、[transfer.rs:186](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/transfer.rs:186) | 检测不提供 isolation target/recovery spec/transfer status；显式保留旧 recovery、comparison 与 transfer，`terminal_transfer_phase(false, …)` 不改写旧终态。未知隔离、未知 transfer 和 sync journal 的 close hold 继续存在；取消只改活动传输阶段。旧恢复审核权限按共享审核机制撤销，持久记录及恢复候选不删除。没有编辑器正文或源基线写入。此组合保留结论来自源码，六个新增 UI 专项未逐项构造这些旧状态。 |
| 原生控件、三主题、双语与完整固定 footer | [archive_inspection.rs:52](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_inspection.rs:52)、[archive_inspection.rs:131](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_inspection.rs:131)、[files/review.rs:147](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/review.rs:147)、[files/review.rs:488](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/review.rs:488)、[files.rs:2787](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:2787) | 使用现有真实 GPUI `FilesPanel`、Kit Button、Palette 与组件 token，无主题 ID 分支或新增颜色字面量。状态、工具详情和审核文案提供中英文；完整命令没有截断后执行。详情区高度上限 112 px；完整审核正文双轴滚动，确认/取消处于不参与正文滚动的固定 footer。 |
| 文档与公开 API | [session lib.rs:5](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/lib.rs:5)、[ADR 0133:1](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/adr/0133-reviewed-server-archive-tool-inspection.md:1)、[产品指南:1](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/product/ARCHIVE_TOOL_INSPECTION.md:1) | 公共模块/类型/API 有 rustdoc；typed SessionError 保留失败分类。ADR 与双语指南准确区分 PATH 观察、未找到、待确认、失败及实际功能可用；保留“实施/验证待完成”及原生边界，没有将当前结果写成服务端打包或 Zmodem 完成。 |

发现项为零，因此没有需要列出的缺陷触发条件、影响及修复建议，也没有自行修复代码。

**独立执行：四条二进制命令，22 个测试通过**

以下均为 reviewer 实际执行，cwd 为上述绝对工作目录。每条命令的原始输出和含完整 argv、PID、SHA、退出码、清单的 JSON 回执均保留在本目录。独立 runner 使用 `Popen(start_new_session=True)`，正常路径调用 `wait`；超时路径先对进程组 TERM、最多等 3 秒，再 KILL 并最多等 3 秒实际回收。运行预算设为 50 秒，留出回收余量，四条均未超时、未发信号且 `wait_reaped = true`。

```text
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-app/dd89ff795559e915/out/keelshell_app-dd89ff795559e915 --list
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-session/eb1796ec9fab9e9c/out/archive_tools-eb1796ec9fab9e9c --list
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-app/dd89ff795559e915/out/keelshell_app-dd89ff795559e915 files::transfer_tests::archive_inspection_tests:: --test-threads=4
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-session/eb1796ec9fab9e9c/out/archive_tools-eb1796ec9fab9e9c --test-threads=4
```

| 执行 | exit | 实际计数 | 外层实际耗时 | 回执 |
| --- | --- | --- | --- | --- |
| app `--list` | 0 | 全二进制 1201 个测试名；本片 6 个具体名称与当前源码逐一完全匹配 | 0.017 s | [app-list.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-tools/app-list.json) |
| session `--list` | 0 | 16 个具体名称与当前源码逐一完全匹配 | 0.006 s | [session-list.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-tools/session-list.json) |
| app 指定 filter、四线程 | 0 | **6 passed / 0 failed / 0 ignored / 1195 filtered**；通过名称与先前清单完全匹配 | 1.040 s | [app-tests.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-tools/app-tests.json)、[原始日志](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-tools/app-tests.log) |
| session integration、四线程 | 0 | **16 passed / 0 failed / 0 ignored / 0 filtered**；通过名称与先前清单完全匹配 | 10.072 s | [session-tests.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-tools/session-tests.json)、[原始日志](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-tools/session-tests.log) |

app 六个具体用例均经过 `--list` 核对后执行：

- `archive_tools_require_complete_review_and_run_once_without_sftp`
- `archive_tool_review_rejects_a_replaced_connection_without_token_change`
- `archive_tool_late_reply_cannot_attach_to_a_replacement_connection`
- `archive_tool_cancel_and_retry_need_separate_explicit_reviews`
- `archive_tool_failed_refresh_retires_the_previous_success`
- `archive_tool_observations_render_in_all_themes_and_languages_without_reprobing`

session 的 16 个具体用例见清单回执，实际包含完整固定命令逐字发送、缺失/待确认、每次新 channel、预先取消、迟到 open、丢弃 future、迟到结果、exec ACK 等待取消、ACK/保持 channel/未确认 open 的 10 秒期限、unsupported/rejected、畸形/stderr/缺失退出状态、输出洪泛、已关闭连接。

实际二进制 SHA256：

| 二进制 | SHA256 |
| --- | --- |
| app `keelshell_app-dd89ff795559e915` | `529b06bc710fb00ba38dae13f49b08deb264b4e5cb59a905e78bf2e1f608bf12` |
| session `archive_tools-eb1796ec9fab9e9c` | `1766d22c914348e630a92214bdad35c34027f2073f7eefd3a1b647a8dfc61a48` |

两者在清单、专项执行前后及最终冻结核验时摘要一致。**二进制由父线程构建，reviewer 没有独立重编译。** 测试名称、SHA 与源码冻结检查支持本次预编译专项的身份核验；它们不替代独立编译来源证明，也不授权重复使用未知来源的缓存产物。

**父线程证据与未验证范围**

- 只读核对父 `logs/full-gate-r1.json`：exit **1**，11.506 s；原始日志保留 `archive_pax.rs` 八处 `byte_char_slices`。已通过上述摘要和内存反向替换确认 r2 的八处等价修改，未将 r1 失败改写为通过。
- 截止取证时父 `logs/full-gate-r2.json` 尚不存在，原始日志仍有执行进展。因此 **本报告没有宣称 full gate r2 已通过**。父线程应待其实际退出并核对同一冻结后记录整批工程结论。
- 只读核对父 `logs/openssh-r1.json`：exit **0**，6.864 s；`openssh-r1/result.json` 为 `outcome = passed`、**14 tests**，含新增工具观察；清理回执记录 owned/observed processes stopped、temporary directory removed。这是父线程真实 localhost OpenSSH 证据，没有计入 reviewer 独立 22 项。
- 只读核对父 `logs/gui-mcp-build-r1.json`：exit **0**，7.580 s。正常 GUI/MCP build 成功独立于测试二进制；本 reviewer 未启动该正常程序、打包、安装或进行原生桌面交互。
- 独立 app 六项在 headless TestAppContext 中挂载生产 `FilesPanel`，基础窗口为 **1100×440**。三家族 × 浅/深 × 双语回归实际执行，但其断言主要核对控件可达、首末行存在及不重新发送请求；不证明全部行的实际屏幕可见性、完整 AX 文本或三平台视觉质量。
- 独立 session 16 项使用受控真实加密 loopback SSH peer，服务器返回固定 fixture 帧，不执行 probe shell；固定命令的行为另由本报告源码检查及父 OpenSSH 证据支撑。没有将 fixture 计为 reviewer 独立 OpenSSH 或生产主机验收。
- reviewer 未执行完整 gate、Cargo、fmt、Clippy、依赖策略、PAX 测试、额外原生检查或父完整门禁的重复运行。真实 macOS/Windows/Linux 桌面、900×580 组合布局、键盘/IME、VoiceOver/NVDA、签名公证、安装升级、服务端实际打包、版本/功能识别和 Zmodem 互通均不在本次独立执行证据内。

**最终 recommendation：APPROVE（本批服务端工具观察的限定源码与预编译专项审查）；P0/P1/P2/P3 = 0/0/0/0。整批收口仍需父 r2 完整门禁成功，原生验收按上述边界继续保持未验证。**
