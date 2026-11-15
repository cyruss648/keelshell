# 2026-11-15 审核式 PTY Zmodem 候选证据

状态：`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_APPROVED / DIRECT_PTY_ACCEPTANCE_OPEN`。
本批只有本地交付；完整产品、FILE-05/M4 和三平台原生验收继续开放。
说明见[集中记录](../2026-11-15-reviewed-pty-zmodem.md)。

## 验证与源码

- [最终冻结](frozen-inputs-r3.json)：570 个 Rust/Cargo/Python 输入与 12 个门禁脚本，
  SHA-256 `14b353ecbf84d9684b20dd7bafa0ffc090644b452fe199e4d32f1c2745e8e882`。
- [最终门禁回执](logs/full-gate-r3.json)与[摘要](validation-summary-r3.json)：
  `exit 0 / 613.125s / reaped`，普通 Rust `2876/0/34 ignored`、rustdoc `14/0`、
  Python `141 run / 1 skipped`；fmt、严格 Clippy、x.y 策略、三组 controller 通过。
  [完整日志](logs/full-gate-r3.log.gz)解压后 SHA-256 为
  `2970e3d39c8d8644a45a7357849f291188ab015389e2d92f41fdcbf9057f15dd`。
- [新增用例绑定](new-case-binding-r3.json)：129 个默认用例在最终产物库存中逐名
  存在并实际通过，已经包含于普通总数。专项、reviewer 重复执行和 ignored
  real-peer 用例不叠加；完整库存另见各 `*-inventory-r3.txt`。
- [正常程序构建](normal-artifacts-r1.json)：arm64 GUI/MCP 产物、大小与 SHA；
  只证明正常原生二进制构建，不代表桌面交互或安装。
- 源码提交之后由 `commit-input-binding.json` 绑定 Git blob、冻结与当前字节，
  HANDOFF 和 ROADMAP 记录准确本地 source SHA。

## 非作者复核

[core/session 报告](review-core-session-r2/report.md)、
[app/bridge 报告](review-app-bridge-r2/report.md)与
[r3 属性差分批准](review-app-bridge-r2/r3-delta-approval.json)
均为限定候选源码批准，零开放 P0/P1/P2/P3。四项 P2 和一项 P3 已经修复并关闭。
reviewer 使用逐名库存、摘要核对过的父预构建稳定副本，未独立重编译；
其实际专项及限制保留在原报告中。

原报告签署时完整 gate 尚在运行，没有改写该历史状态。
[父后续条件核验](final-review-conditions-r3.json)记录相同冻结的完整成功门禁、
129 项实际通过绑定和正常程序摘要核对，满足其工程条件。原 notice、失败和
HOLD 均保留。

## 互操作与开放边界

当前本机直接 PTY [r6](real-pty-r6/result.json)为 **6/7**；
Debian [r2](linux-cycle-r2/result.json)、[r3](linux-cycle-r3/result.json)
分别 **5/7、4/7**。部分下载已有完整 FileSaved、CRC 有效 ZFIN 和 peer
退出 0，但缺最终 OO，因此没有 Finished，直接 PTY 继续 HOLD/PARTIAL。
根因未确认，没有通过退出码或单文件保存补签批次握手。

旧本机 r5 `7/7` 保留历史身份。Debian pipe 对照 `7/7` 和
[最终 relay 完整周期](linux-final-relay-r1/result.json)的 `17/17` archive/ACL、
独立 peer reference、应用 relay `7/7` 及 owned cleanup 只签其有限范围；
不代签直接 PTY、Linux Rust 本地文件后端或 Linux 桌面。

Windows 安全文件后端显式 Unsupported；Python 3.8 只有语法检查。
三平台真实桌面、IME/完整辅助技术、签名、安装与发行验收继续开放。
工具链见 [toolchain-r1.json](toolchain-r1.json)。三轮全量在线依赖复查超时；
新增 ACL 单包已经检查，不能据此声称全部依赖确认最新。

## 无损保存与清理

[archive-manifest.json](archive-manifest.json)保存 747 份原件的原文 SHA、字节数、
持久目标、持久 SHA 与逐字无损核验结果。日志 deterministic gzip，其余文本原样
保留；helper Python 改为 `.py.txt`，临时二进制和测试数据排除。
manifest SHA-256 为
`2de4a6e9a79744962bbbcbfd874fe75f20f7d291e54f4adbf4cf330a95090d3f`。
`linux-author/` 保留 Linux 实现来源的必要 receipt、peer 来源与许可证。
原报告中的旧绝对路径是原始执行位置；工作目录清理后，按本目录相同后缀查找，
日志增加 `.gz` 后缀，不修改原始报告链接或签署事实。

[实现清理回执](implementation-cleanup-r2.json)记录三份受管实现 worktree 和本地
分支已回收、七个 agent 已关闭、会话归档及各 fixture 的真实退出/清理状态。
早期受管归档保护拒绝和 fixture 失败保留。最终清理以
[cleanup-final.json](cleanup-final.json)记录本批重复运行目录、稳定测试副本和临时
GPL peer 的删除，共 769 个文件、654,860,244 字节；
保留唯一有效 Cargo target、其他任务 WIP、工作树与容器。
