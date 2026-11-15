# 2026-11-15 审核式 PTY Zmodem 集中验证

状态：`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_APPROVED / DIRECT_PTY_ACCEPTANCE_OPEN`。
基线：`721bad2157da8d3a0bc11a1eab5ac59b6616aa9c`。
本批只本地提交，不 push；完整产品、FILE-05/M4 保持进行中。

## 输入与职责

core 协议、session 文件审核及 Linux fixture 在独立受管 worktree 并行编写；
父集成原生 UI、单一 PTY 写入通道并独占当前主树的 Cargo target：
`work/recovery-batch-integration-20261115/target`。本批没有切换 target 源码归属。
后续两个修复 agent 只写互不重叠的 UI 与 session 文件，不执行 Cargo。
所有实现 agent 已关闭。新的两名非作者 reviewer 仅审查源码并运行核对过的
父预构建稳定副本，没有独立重编译，也没有修改共享 target。

最终 r3 冻结包含 570 个 Rust/Cargo/Python 工程输入与 12 个门禁脚本，摘要为
`14b353ecbf84d9684b20dd7bafa0ffc090644b452fe199e4d32f1c2745e8e882`。
r1→r2 修复六个 Rust 文件；r2→r3 只为本来已被外层 cfg(test) 引入的
`zmodem/local/tests.rs` 增加文件级 `#![cfg(test)]`，不改变生产输入或架构 guard。
原始冻结、差分、日志、review 与二进制逐名库存无损保存到
[持久证据目录](2026-11-15-reviewed-pty-zmodem-evidence/README.md)。
日志以 deterministic gzip 保存，原文及压缩文件分别记录摘要并逐字解压核验；
临时二进制和临时测试数据不属于持久证据。

## 作者检查与失败保留

当前已执行并实际退出 0：

| 检查 | 结果 |
| --- | --- |
| core 独立协议专项 | 65/65 |
| session lib Zmodem（含五项 publication hold 新回归） | 27/27 |
| session controller 集成 | 18/18 |
| app Zmodem（含十项 UI 新回归） | 19/19 |
| 完整 SSH bridge | 6/6 |
| remote-only 架构 guard 修后专项 | 5/5 |
| 最终完整 gate r3 | exit 0 / reaped，613.125s |
| 普通 Rust | 2876 passed / 0 failed / 34 ignored |
| rustdoc | 14 passed / 0 failed |
| Python | 141 run / 1 skipped |
| fmt、workspace all-targets strict Clippy、x.y 策略、三组 AI controller | 全部通过 |

app 两组专项有重叠，专项与完整门禁也不叠加计数。修后 app 完整逐名库存
为 1227 项，core Zmodem integration 为 65 项、session lib 为 185 项、
session Zmodem integration 为 18 项。已经从最终 gate 实际产物重新读取库存，
将 app `19`、core `65`、session lib `27`、session integration `18`，
合计 `129` 个本批新增默认用例逐名核对为在最终 gate 通过，零遗漏。
这些用例已经包含于普通总数；7 个 ignored real-peer 用例、专项或 reviewer 的
重复执行均不叠加。详见
[新增库存绑定](2026-11-15-reviewed-pty-zmodem-evidence/new-case-binding-r3.json)。

首次集中编译修复新版 SHA-256 返回类型、macOS FIFO 夹具、GPUI trait 与
测试宏问题。Clippy r1 的两处风格问题已修复。完整 gate r1 因开始修复真实
互操作及 reviewer 问题主动停止，实际退出 -15、360.050s；不能称通过。
完整 gate r2 实际退出 1、460.575s：Python、fmt、strict Clippy 及此前 Rust
suites 已运行，最终在 remote_only guard 报外置测试模块的文件级属性缺失。
补上明确 test-only 属性后，guard 5/5 和 fmt 通过。旧失败原件不覆盖。
最终命令为 `python3 -B scripts/check.py`，使用有界 wrapper，完整门禁 r3
已实际取回退出 `0`，外层执行 handle `27291` 已 reap，无遗留运行中的父测试。
原始日志 SHA-256 为
`2970e3d39c8d8644a45a7357849f291188ab015389e2d92f41fdcbf9057f15dd`；
门禁后 `570 + 12` 输入逐项与 r3 冻结相同，零 drift。包级普通结果为 AI
`194/0/5 ignored`、app `1225/0/2 ignored`、core `803/0/0`、MCP `77/0/0`、
session `577/0/27 ignored`，rustdoc 另外计算。
完整派生摘要见[验证汇总](2026-11-15-reviewed-pty-zmodem-evidence/validation-summary-r3.json)。

正常 GUI/MCP 构建 `normal-build-r1` 退出 `0`，9.632s，均为 arm64 Mach-O，
生产输入与最终候选相同。GUI SHA-256 为
`96b5133efc6463357184bfed3b72e604a8c33aa33a1ef6859bc38f5790927d99`，
MCP 为 `cf89dbc2b7fb77958d16c225d7d94c536970aca4c3f7675e544696a8a5811c88`。
此处没有新的真实桌面交互、安装或发行验收。

macOS ACL 的真实探针证明 `ls -e /dev/fd/N` 不能观察 retained vnode ACL。
生产代码已使用 macOS 目标限定的 `calcifer-macos-acl 0.1` 安全 API 直接处理
保留 fd，锁文件精确解析为 0.1.0、MIT、零传递依赖；Linux 使用 fd xattr。
继承 ACL、晚到 ACL 和 pathname 替换后仍检查 retained vnode 均有实际回归。
测试中的 chmod/mkfifo 仅位于明确 test-only 模块，生产端不调用本地命令。

## 非作者发现与修复

r1 共提出四项 P2、一项 P3，原始报告与 notice 保留：

- 首 offer 前结束没有 UI 卡片时漏发 ResumeInput：每个终态 UUID 都进入
  有界、去重、保留背压的交接队列，桥接只释放相同且已结束的 lease。
- 挂起写入时 shell completion 丢尾部输出：区分 Written/ShellEnded/Stopped，
  shell 结束后继续交付有序 Data/Exited，未完成 write 不得确认成功。
  旧稳定副本连续 5/5 失败；修后作者通过，reviewer 又重复 5/5 通过。
- 重复取消的迟到拒绝复活卡片：取消只提交一次，Cancelling/Ended 不接受旧回复
  重新激活，也不重放普通输入。
- 保存回执被新 offer/Finished 覆盖：独立保留最近 32 条完整路径、字节和摘要，
  完成、后续 offer 和收起后仍能核对；三主题、明暗、双语与固定 footer 有回归。
- 迟到旧 publication ticket 与新 success 的 pending 竞态：共享 Mutex 同步
  active ticket 数、sticky unknown 和 pending 发布。计数异常或 poison 保留 hold；
  backend reservation 在锁外处置，不清除 quarantine；五项新回归覆盖处置顺序、
  重复处置、poison 与两个有界并发场景。

两名非作者最终均对限定源码与预构建执行给出 APPROVE、零开放 P0–P3；
core/session 报告绑定 r3 全部 `582/582` 输入，app/bridge 由原 r2 报告及
r3 单文件属性差分批准共同绑定。core/session 独立执行 `65 + 27 + 18 = 110`
项；app/bridge 独立执行 `19 + 6`，重叠三项，去重 22 项；原 tail 失败另外重复
`5/5` 通过。没有独立重编译或独立全仓门禁。

原报告签署时最终门禁尚在运行；父后续核验相同冻结的成功终态及新增库存，
满足其工程条件。原报告与先前 notice、失败、HOLD 保持原件，不回写签署时事实。
原文见 [core/session](2026-11-15-reviewed-pty-zmodem-evidence/review-core-session-r2/report.md)、
[app/bridge](2026-11-15-reviewed-pty-zmodem-evidence/review-app-bridge-r2/report.md)及
[r3 差分批准](2026-11-15-reviewed-pty-zmodem-evidence/review-app-bridge-r2/r3-delta-approval.json)。
其结论不能授予产品、直接 PTY 或原生平台验收。没有更改模型上下文发送或命令审核边界。

## 独立 rz/sz 与真实 Linux

临时 macOS lrzsz 0.13.1 来自公开上游，无宿主安装、无应用源码捆绑或 peer patch。
实际生产 `start_shell` 请求 SSH raw PTY；用例覆盖 262156 字节、全部字节值、
Unicode 文件名、空文件、CRC16/32、已有目标保护和审核前取消。

本机 r1 5/7，r2/r3/r4 6/7；修复发送端把前进 ACK/重复 ZRPOS 误算为整批
重试预算的问题后，r5 曾 7/7。最终 r2 生产输入的 r6 为 **6/7、exit 1**：
CRC32 下载已经完整发布并读回，收到有效 ZFIN、peer 实际退出 0，但未收到 OO，
所以没有 Finished。不能用旧 r5 或重新挑一次成功来覆盖当前失败。

真实 Debian 12 arm64、Python 3.11.2、overlayfs、euid1000 上，Linux cycle r2
完成 17/17 归档/ACL 探针和两个原始独立 lrzsz 经 OpenSSH raw PTY 的对传
（262156 字节、退出 0/0、SHA-256 一致）。应用稳定 macOS test binary 对接
Debian lrzsz 0.12.21rc 的直接 PTY 结果 r2 为 5/7、诊断 r3 为 4/7；失败同样
是完整 FileSaved 后缺最终 OO。该现象不限于 Debian，确切根因未确认。

临时 `pipe_peer.py` 保持 Debian sz 程序不变，通过 pipe stdio 转发到真实 SSH
PTY。对照 `linux-pipe-diagnostic-r1` 为 7/7。最终相同生产输入的新稳定副本在
`linux-final-relay-r1` 完整 provisioning→17/17 archive/ACL→独立 peer reference
→应用 relay 7/7→cleanup 周期实际退出 0、95.217s。
这是有限 relay 对照，**不代签直接 PTY、Linux Rust 本地文件后端或 Linux 桌面**。
单文件完整读回与批次握手继续分别报告；缺 OO 时没有降低成功断言、伪造 Finished、
自动重试不确定写入或宣称远端删除/回滚。取消仅作两秒有界退出观察，再关闭自有
channel；不把取消认作对端退出 ACK。

Python 3.8 只有语法层检查，实际 runtime 不可用。fixture 原 linux-r1 的
Podman API/storage 故障保持 failed；本轮 cycle-r1 因本地 binary 准备错误而
中止，也保留原件。后续所有自有容器均以完整 ID、名称和两项 owner 标签核验
删除，container exists 为 exit 1；宿主自有进程退出、临时密钥移除已记录。
未重启 Podman VM、修复共享存储、prune 或操作其他任务容器。

## 工具链、依赖与资源

实测 rustc 1.100.0、cargo 1.100.0，stable-aarch64-apple-darwin 为活动默认。
无 toolchain pin 或新增 manifest MSRV。直接依赖保持 x.y，Cargo.lock 保留 patch。
本批全量在线依赖复查 45/90/50 秒三轮超时，没有完整最新版本报告；不能宣称
全部依赖已确认最新。新增 ACL package 的 registry 稳定版本与小型源码已检查。

session 实现 worktree 已受管归档。core 与 Linux fixture 曾被活动任务保护拒绝，
关闭实现 agent、保存并逐字核验 235 份必要 receipt/peer/license 后，后续受管
归档已经成功，两个路径均实际不存在。保护失败历史保留，没有 shell 绕过删除。
所有七个实现、修复及复核 agent 均已关闭；会话已归档，没有永久删除声明。
受管 worktree、分支及 fixture 清理原件见
[清理核验](2026-11-15-reviewed-pty-zmodem-evidence/implementation-cleanup-r2.json)。
747 份原件的原文/持久摘要、大小与逐字解压结果全部核验；所有非临时源证据均有
对应持久副本。之后删除本批两个重复运行目录，共 769 个文件、
654,860,244 字节，包含稳定测试副本及临时 GPL peer。
[最终清理](2026-11-15-reviewed-pty-zmodem-evidence/cleanup-final.json)
记录实际删除范围、路径不存在及有效 target 身份保持，保留其他任务 WIP 与受保护工作树。

Windows 安全文件后端与解包、直接 PTY 缺 OO 兼容、Linux 本地 Rust 后端、旧
Python runtime、中继性能收益、三平台真实桌面、IME/完整辅助技术、签名、安装
和发行仍开放。本次不把编译、headless、relay 或单个平台推导为完整产品完成。
