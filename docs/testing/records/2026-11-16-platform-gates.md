# 三平台工程门禁修复 — 2026-11-16

状态：本机完整门禁通过，新真实 CI 待复验。源码基线为已推送的
`4fd193227bb9e5e5e4b49ad6f1503f10741f7383`；
分支为 `feature/editor-unicode-codec-20261114`。
用户要求及时提交并推送，替代历史离线规则。

## 已确认的失败

GitHub `cyruss648/keelshell` 的 Quality 工作流精确 push run
`43789081172 / attempt 1`，Windows job `133043263253` 失败。
Python `262 run / 2 failures / 20 errors / 15 skipped`；
门禁止于 Python，未进入 Rust 编译。

- 部分 POSIX 实验的真实子进程测试在 Windows 执行，触及不存在的
  `signal.SIGKILL` 与宿主不支持的进程清理能力，造成后续文件锁错误。
- `test_monitor_recovery_lab` 导入 `scripts/monitor_recovery/fixture.py`
  时找不到 POSIX 专有的 `fcntl`，导致整个测试模块未被发现。
- Windows 原生进程树测试
  `PreparationTreeTests.test_real_timeout_drains_inherited_descendant_pipes_and_forces_tree`
  在发布 PID 后未完成身份绑定，回执含 `owned_preparation_failed`。
  具体异常尚未保留，不能据此断定是时间预算或 Job 归属问题。
  该测试不可跳过；原始句柄、归属、终态和管道断言保持。

Windows 日志通过只读 `gh api` 捕获，raw 文件不直接渲染终端；
另存去除 ANSI 的阅读副本。原 raw 为 128,484 字节，SHA-256
`aff7bdd53c71669ce0b03337e17cf7ba3d3d5194fe2dcda9dedc8beeb2ad6609`，
采集 `exit 0 / reaped / 3.960s`。之前 CLI 拒绝终端转义的失败也保留。

同 run 的 macOS job `133043263469` 于 `2026-11-16T02:15:42Z` 返回失败。
Rust session 库 `287 passed / 1 failed / 4 ignored`，失败来自其调用的归档
Python 专项 `25 run / 2 errors`：直接等待已完成后，组观察的系统调用返回
非 ESRCH 的 OSError，`terminate_and_wait` 拒绝给出已确认清理的报告。
非作者以基线行号纠正了父最初“观察等满 0.2 秒”的解释：堆栈落在第 166 行
错误分支，期限耗尽实际为第 168 行。旧日志未保留 errno，具体错误尚不能确认。
日志不能证明当时尚存组内成员的具体身份或状态，不把孤儿 zombie 的推断写为事实。
macOS raw 为 610,991 字节，SHA-256
`ab1c140b7fd08017e8deaa24f14af0b443dd543d66c33283aa134a4efddb2666`，
采集 `exit 0 / reaped / 7.044s`。

同 run 的 Linux job `133043263463` 已于 `2026-11-16T02:18:15Z`
返回 `success`，完整日志已捕获并另存 ANSI 阅读副本。这只签署基线
`4fd1932` 的 Linux 工程检查和工作流中的 OpenSSH 检查，不代签本批新候选、
Linux 桌面或全部功能。

本机先补两个确定性时钟反例：组延后 0.35 秒才消失被旧上限提前拒绝；
leader 等待消耗 0.85 秒后重新开放整份组观察预算，实际总耗时 1.855 秒。
旧代码 `1 failure / 1 error` 已保留。候选改为一个绝对清理截止，
上限一秒，等待 leader 与观察组共用，不续期、不重复发送破坏性信号；
权限拒绝和最终无法确认仍抛出 `CleanupUnknown`。共享六秒工作截止已经计入
前面各命令清理，最后一份一秒清理仍留在现有十秒 transport 期限以内；
操作系统创建/调度不被声明为硬实时保证。真实 macOS CI 需要重新运行后才能签署。

非作者另指出：仅调整期限未覆盖上述 OSError 分支，以及旧真实后代测试在
leader 回收后可能用缓存 PGID 再次发送 SIGKILL。父保留原报告，继续补两个
EPERM 红例，旧候选 `1 failure / 1 error`；最终候选只在同一截止内重试
post-reap 的 EPERM 非破坏性观察，必须见到 ESRCH 才确认 absent。
持续 EPERM、持续存在、其他查询错误、首次 kill 拒绝和 wait 失败仍 UNKNOWN。
异常只带固定阶段与数字 errno，远端 main 仍不输出可采用报告或异常原文。
测试移除回收后的破坏性兜底，保留真实后代 absent 和 leader 已回收断言。
EPERM 是针对已知机制的候选覆盖，不能反推本次 CI 的实际 errno。

## 分工与验证边界

独立作者在受管隔离树修复 POSIX 实验的 Windows 导入/适用性。
父在当前 checkout 处理 Windows 原生进程树诊断。
Windows 能运行的纯合同、解析和模拟测试继续运行；只有真实宿主能力
不适用的测试允许精确声明跳过。Linux/macOS 实验的断言保持。

作者六文件最终字节已集成并逐项核对摘要。真实 macOS 专项 `127/127`；
Windows/FreeBSD 适用性模拟各 `127 run / 108 passed / 19 skipped`，
原件、修复前反例和完整测试库存已复制到父批次目录。作者没有运行 Cargo，
agent 已关闭，受管树经重新附着父 checkout 后成功归档，路径与 Git worktree
记录均 absent。初次因当前 primary 保护而拒绝的结果保留，未绕过保护删除。

父归档专项 `27/27` 通过；Windows 诊断模块本机 `69 run / 1 skipped`。
候选 r1 冻结 717 项，SHA-256
`6bf6b1abb5cfaaeada90b5eddb191c5d24ba963acb3e2c9eefa63b145b50afb5`。
session 包 r1 专项 `exit 0 / reaped / 221.500s`，绑定当时 archive 候选，
之后 EPERM 修订需要相应回归，不能移签。r2 仅另加 push 的 `docs/**`
忽略过滤，PR 与所有三平台任务保持；纯证据 push 不取消/重启代码验证。
新的非作者最终 r4 有条件源码批准，开放 P0–P3 均为 0；
两个历史 P2 在限定源码和回归范围关闭，原 notice 与纠正保留。

最终 archive r3 本机 `30/30`，实际 `exit 0 / reaped / 2.667s`。
中间 r2 `30 run / 1 failure` 原样保留：继承管道夹具在 0.15 秒内没有观察到
父工具的 exit 0，返回了清理造成的 -9；不能把后者当作原始 exit 0。
夹具改用既有生产一秒命令窗口、总耗时上界 2.5 秒，后代仍睡九秒，仍必须观察
工具 exit 0、管道超时、单次 kill-before-reap 和实际回收；生产时间参数没有
因这项夹具修订而增加。后续完整门禁继续覆盖该真实进程回归。

最终候选 r4 为 717 项，SHA-256
`04b232fb2095b490614d509d3545ea72afa65a998646300617df87f6af11180f`。
本机 Rust/Cargo 实测 `1.100.0`、默认 stable，没有新增依赖或 toolchain pin；
本批官方依赖新鲜度查询再次在 45.063 秒超时并实际回收，不签全部依赖最新。

## 最终门禁

`full-gate-r1` 实际 `exit 0 / reaped / 未超时 / 771.347454959s`：

- 普通 Rust `3210 passed / 0 failed / 48 ignored`。
- rustdoc `14 passed / 0 failed`。
- 工程 Python `290 run / 289 passed / 1 skipped`；archive helper `30/30`。
- fmt、严格 Clippy、x.y 政策及三组 AI controller 全部通过。
- 实际 app 程序库存 1,331 名，与完整门禁逐名相同；
  717 项完整工程路径、字节和摘要零漂移。

`final-review-gate-binding.json` 用同一集合满足非作者原签署中待定的完整门禁
条件，不改写原报告；真实平台条件仍待推送后的新 CI。
父 session 包历史专项 `714 passed / 41 ignored`、rustdoc `1/1`，
仅绑定原候选；最终 archive Rust 窄回归 `11/11` 内含最终 Python `30/30`。
打包工程检查 `57/57`；没有据此签署新的原生 GUI 或发行验收。

本批三个作者/调查/复核 agent 已关闭，作者受管树 absent。下一批命令框焦点的
三文件补丁和八项 GPUI 回归也已复制核对，作者关闭、受管树归档；其源码尚未
集成、编译、实际红绿或新非作者评审，不在本批冻结中。
原生反向 Tab 及完整平台/输入法/辅助技术状态继续保持原边界。

唯一 Cargo target 继续归父 checkout，无来源切换。
集中集成、专项修复、新非作者复核后运行最终门禁；
工程输入不变的旧完整门禁不重复执行。
Linux/macOS 同一 CI run 的终态继续查询，不把观察超时当作任务失败。
反向原生 Tab、三主题完整焦点矩阵、IME/辅助技术、三平台桌面和发行仍开放。

原件从[持久证据](2026-11-16-platform-gates-evidence/README.md)读取；
归档、Git blob 绑定和临时数据回收均以单列实际 JSON 回执为准。
