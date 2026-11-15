# 2026-11-15 审核式服务器归档集中验证

状态：`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。最终 r5 对应完整 gate r4
退出 `0`、`603.804s`；两个非作者最终 APPROVE，零开放 P0–P3，两项历史 P2 均已
关闭。以下保留各阶段失败、被终止门禁和原始条件性结论，不沿用上一批结果。
仅本地提交，不推送。

源码功能提交：`2726b6466e68508037ff81b1daf01fcf2c91b6e5`。
[提交输入绑定](2026-11-15-server-archive-evidence/commit-input-binding.json)
逐项核对 Git blob、最终冻结和当前 `553 + 10` 文件，零 mismatch。

基线为 `9112e232b6f0e022d299cba9e8498088c77b3d3f`。
实现按 core/Python、session 和原生 UI 拆分；两个实现 agent 不执行 Cargo，
父独占当前主树的 `work/recovery-batch-integration-20261115/target`。
没有切换该 target 的源码归属。运行回执、日志与原始失败暂存
`work/server-archive-20261115/`，交付前需持久归档并清理重复临时目录。

## 必须验证

- 完整只读命令审核之后才扫描；完整成员、命令、标准输入审核之后才打包。
- 同一主机文字或 token 不能替代实际 SSH 连接；取消、草稿/选择变化和迟到结果
  不能产生新授权。审核正文完整可滚动，确认/取消固定可达。
- ZIP/tar.gz 均在服务器实际压缩，源漂移和非法成员被拒绝，已有目标保留。
- 私有 staging、新目标、源读资源和未知结果的隔离不被重复执行或成功提示绕过。
- 完整读回、摘要与清单核对之后才显示完成；失败、超时、取消不伪造回滚。
- 固定冻结包含 Rust/Cargo 及 crate 内嵌 Python，清单核对具体新增测试名称。
- 集中 fmt、严格 Clippy、依赖策略、workspace/rustdoc/controller；
  实际 OpenSSH 与新的非作者源码/功能复核单独记实。

原生三平台、IME、完整辅助技术、Windows 安全解包、Zmodem、中继及发行验收仍开放。

## 已执行专项与失败保留

- core `core-r2` 实际 `14/14`，包含隔离进程执行内嵌 Python、两种真实格式与既有
  codec 校验。`core-r1` 的 SHA2 0.11 `LowerHex` 编译失败保留；修正为逐字节
  小写十六进制，没有更改依赖。
- session `session-r3` 实际 `40/40`，覆盖真实辅助程序、精确 stdin/EOF、组合输出
  预算、统一期限、取消/迟到 open、同端点真实连接替换、三类资源冲突、未知结果和
  完整读回。`session-r1` 的同类编译失败和 `session-r2` 的 `26/14` 保留；
  后者是新测试错误地将「无隔离记录」预期为空列表，现按已有 API 合同断言
  精确的 `Invalid("no matching unknown transfer destination")`，没有放宽生产检查。
- app `app-r4` 实际 `7/7`，使用真实 GPUI window/context 和两次确认控件；
  受控 SSH 只执行精确注册的实际辅助程序，实际输出再镜像到测试 SFTP。
  三主题 × 明暗 × 双语包含在该组中，不另叠加数量。`app-r1/r2` 的夹具访问域
  编译失败、`app-r3` 的 `2/5` 保留；修正测试工具对新 disclosure 动作的定位，
  不跳过真实点击。
- `openssh-r1` 为真实 localhost OpenSSH `15/15`，新用例同时覆盖 ZIP/tar.gz、
  Unicode/引号/shell 字符文件名、目录和空目录、预取消、完整清单/字节与临时
  路径消失。回执确认 owned 进程已停止、临时目录已删除，不代签原生桌面交互。
- `full-gate-r1` 在 Clippy 阶段退出 `1`、已 wait/reap；新 app 测试夹具三处
  `expect` 违反仓库 lint，改用已有 `Checked` 测试断言接口。新冻结 r3 只有
  该测试夹具相对 r2 变化，生产输入相同。

冻结 r3 包含 `553` 个 Rust/Cargo/Python 工程输入及 `10` 个门禁脚本。
core/session/app 已实际核对的预构建测试清单分别为 `14/40/1208` 项；
两位新的非作者 reviewer 按相关冻结和二进制摘要独立复核，不运行 Cargo，
其预构建执行不得写成独立重编译。最终数量与评审收口见本文末尾。

本机实际读回 Rust/Cargo `1.100.0`、active 为
`stable-aarch64-apple-darwin (default)`；本批没有新增依赖、toolchain pin 或
`rust-version`。两次官方在线依赖检查均在 `60s` 被有界 wrapper 终止，
没有版本报告；不能声称最新版本已经全部核验。Python 3.8 只做语法合同检查，
本机实际进程为 Python 3.15.2，不代签 Python 3.8 的运行验收。

## 非作者发现与最终候选

core 首轮 reviewer 在真实 macOS 继承 ACL 环境发现 P2-001：仅检查
`0700/0600` 会使新 stage/payload 继承额外 ACL。收到 notice 后实际停止并
wait/reap `full-gate-r2`，回执 exit `-15`、`96.247s`；原始失败冻结、notice、
report-r1 和权限复现均保留，不能写成门禁通过。

真实红回归 `acl-red-r1` 在 ZIP/tar.gz 都观察到源内容写入前的额外 ACL。
修复仅处理新建对象的 fd：Darwin 清空并读回 extended ACL，Linux 清除并
复核已知 POSIX access/default xattr，其他 ACL 模型、缺失接口及失败均拒绝。
源、父目录和已有目标权限不修改；目录复核与归档读回重复检查 ACL。
新增真实回归还覆盖设置/查询 fd 错误、打包中途给 stage/payload 添加 ACL，
确认没有成功回执且未知对象保留。`acl-green-r1` 与包含新回归的
`core-r3/r4` 均通过。Python 3.8 grammar 检查单独记录，不是旧运行时验收。

最终冻结 r4 的 `553 + 10` 输入 SHA256 为
`c01ae7a78099b4805bff413a795176e0aa7732871bf6a909cd83fff6277c5eda`。
实际专项为 core `14/14`、session lib `11/11`、session SSH `40/40`、
app `7/7`；真实 OpenSSH r2 `15/15`，自有进程和临时目录回收。
为避免后续 Cargo 替换 reviewer 正在核验的程序，父将最终 binary 做独立稳定
副本，逐字核对源/副本 SHA 和实际库存，再交付非作者执行。它们仍是父预构建
程序，不能写成 reviewer 独立重编译；该阶段尚未取得完整 gate r3 和最终差分终态。

## 跨平台测试条件修正

r4 冻结的 `full-gate-r3` 实际退出 `0`、`629.306s`；普通 Rust 为
`2747/0/27 ignored`，rustdoc `14/0`，Python `131 run / 1 skipped`，
格式、严格 Clippy、依赖策略和三组 AI controller 全部通过。
正常 macOS arm64 GUI/MCP `normal-build-r1` 也退出 `0`，未启动或安装应用。

收口静态检查另确认测试夹具的 Unix caller 与声明条件不一致。父在本机以
相同 ancestor 可见性运行 `rustc --emit=metadata -D dead_code` 最小 probe，
真实 exit `1`；这是编译器 lint 语义证据，不是 Windows target 的编译。
给三个仅 Unix 方法、其 `Checked` 导入，以及只在 Unix 测试读取的
`Server.server_archives` 字段/初始化添加相同条件；跨平台 `Peer`、`lookup`、
`serve` 保留，不扩大可见性或放宽 lint。发现及关闭由 session/UI reviewer
的新 notice 与后续差分报告分别保存。

r5 相对 r4 只改变 `server_archive_fixture.rs` 和 `test_server.rs` 两个
`#[cfg(test)]` 子模块文件。所有 core、生产输入及脚本逐字相同，真实 OpenSSH r2、
正常程序构建和 core reviewer r2 的原范围仍有效；不重复这三项验证。
最终冻结 r5 SHA256 为
`30770f74d710b231936e8537d5ce4082798e68a2894aa1ad7e5b9110afa08f92`，
仍为 `553 + 10` 项。为满足最终冻结对应完整成功门禁要求，另行执行
`full-gate-r4`；r3 原成功回执不改写或与 r4 合并计数。

## 最终结果与证据

最终 `full-gate-r4` 已实际 wait/reap，exit `0`、`603.804s`、未超时。
原始日志 SHA256 为
`c2e1396e5805919abe1a5343f705f9006dcdacd274b3ef1a80ba5385050652c9`。
最终 `553` 个 Rust/Cargo/Python 输入及 `10` 个脚本与 r5 冻结逐项相同，
工程库存零缺失、零额外项。最终 app `--list` 为 `1208` 项，与 r4 库存逐字相同。

| 范围 | passed | failed | ignored |
| --- | ---: | ---: | ---: |
| keelshell-ai 普通用例 | 194 | 0 | 5 |
| keelshell-app 普通用例 | 1206 | 0 | 2 |
| keelshell-core 普通用例 | 738 | 0 | 0 |
| keelshell-mcp 普通用例 | 77 | 0 | 0 |
| keelshell-session 普通用例 | 532 | 0 | 20 |
| 普通 Rust 合计 | 2747 | 0 | 27 |
| workspace rustdoc | 14 | 0 | 0 |

Python 为 `131 run / 130 passed / 1 skipped / 0 failed`。fmt、workspace all-targets
严格 Clippy、`x.y` 依赖策略和 readiness/observer/small-stack controller 均通过。
新增默认 `72` 项为 core `14`、session lib `11`、session SSH `40`、GPUI `7`，
已逐名核对最终日志并包含在普通总数；不叠加专项、reviewer、内部 Python 或单独执行
的 `15` 项 ignored OpenSSH。

core reviewer 最终 `report-r2.md` 为 APPROVE、ACL P2 关闭；session/UI 最终
`report-r3.md` 为 APPROVE、Unix cfg P2 关闭并核验最终 gate、freeze 和全部 72 个
用例。两者 P0–P3 均为零。core 独立执行父预构建 `14/14`，另有 14 个探针组
（6 组真实 Darwin、8 组模拟分支），单独可见 Darwin 风险测试 `3/3`；不累加进
Rust 数量。session/UI 独立预构建执行 `40 + 11 + 7 + 9 = 67/67`，未在最终
两个 cfg 测试文件修正后重复计数。两者均未独立重编译；最终已关闭。

原始记录、稳定产物摘要、库存、失败、探针、冻结和源码绑定统一归档到
[持久证据](2026-11-15-server-archive-evidence/README.md)。
正常 GUI/MCP 构建和真实 OpenSSH 的生产输入在 r4→r5 中相同；未重新标注为最终
原生交互。实际 Windows 编译、Linux 文件系统权限和 default ACL、Python 3.8
运行、三平台原生 UI、IME/辅助技术、安装和发行验收仍开放，FILE-05/M4 与产品
总目标继续进行。本机编译器 probe 或模拟 syscall 返回不能替这些边界补签。

248 份原件已按 manifest 逐字或无损压缩校验，随后在本地源码提交和绑定后清理
本批自有运行目录、稳定预构建副本和重复日志，约 332 MB。所有实现/复核 agent
已关闭，两个实现 worktree 已受管归档且本地分支不存在，自有 OpenSSH 回收，
未创建容器。保留唯一有效 target、其他任务 WIP 与受保护 worktree；
最终事实见[清理回执](2026-11-15-server-archive-evidence/cleanup-final.json)，
原阶段性 pending 观察保留，不覆盖。
