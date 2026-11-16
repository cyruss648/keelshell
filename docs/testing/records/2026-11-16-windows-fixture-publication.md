# Windows 进程清理测试的发布完成屏障

## 原始 Windows 运行

Quality `43873806902` 绑定源码
`95813a76f0ec9b59fd043cd982e33cdc01dd69f8`。
Windows job `133321520130` 已 failure：

| 范围 | 实际结果 |
| --- | --- |
| filesystem | `70 passed / 0 failed / 0 ignored / 0.81s` |
| core archive | `41 passed / 0 failed / 0 ignored / 17.55s`，41 名库存与执行逐项相等 |
| 完整 session library | `329 passed / 0 failed / 3 ignored / 10.89s`，332 名库存与执行逐项相等 |
| 其中 Windows Zmodem | 45 名：`43 passed / 0 failed / 2 ignored`；两项新名与空父目录替换例逐名 ok |
| 完整 gate | 停在 Python discovery：290 run／1 failure／34 skipped；全量 Rust 尚未运行 |

Python 失败为
`test_real_parent_failure_keeps_exit7_even_with_descendant_pipe_timeout`。
原身份观察阶段是 `reading_publication`，错误为 `PermissionError`、
`errno=13`、`winerror=None`，`ack_created=false`、最终记录和发布标记均已存在。
控制器为 `observed_exit=None/controller_exit=125/owned_preparation_failed`；
终态仍记录 direct reaped、active processes 0、pipes drained。
不把此结果写成父进程实际退出 7 或完整 Windows gate 通过。

这关闭了上一批新 Zmodem 断言的 Windows 执行缺口，尚未关闭整个 Windows gate、
桌面或三平台验收。macOS／Ubuntu 在本次记录初稿时仍运行，终态另行收取。

## 测试工装修订

本批只有 `scripts/test_windows_native_crash.py` 一个工程文件变化，
应用、Rust、Cargo manifest／lock、进程所有权实现均无变化。
719 输入冻结 SHA-256：
`1a516c0280487e62157ad3ee470206ef91c12a08501f12214ef7348c688cfc3a`；
与 `95813a7` 冻结比较另外 718 项逐字节相等。
完整关闭／rename 后的发布标记现在是读取身份记录的必要条件。
JSON、PID、原始句柄、Job／group、ACK 顺序、短执行预算及清理断言保持，
见 [ADR 0150](../../adr/0150-fixture-publication-readiness.md)。

原日志不能独立定位哪个系统调用产生读错误；“观察者与发布操作重叠”是源码时序
分析。本批以明确的生产者完成标记消除提前读取，没有用宽泛捕错、任意重试或加时掩盖失败。

## 实际宿主验证

| 命令／范围 | 结果 |
| --- | --- |
| 专项 `test_windows_native_crash.py -v` | `0/reaped/6.268s`；72 run／2 skipped |
| 集中限定 gate | `0/reaped/15.369s`；完整 Python discovery 293 run／2 skipped |
| 工程检查 | fmt、严格 workspace all-target Clippy、x.y 政策、diff 检查全部通过 |
| 旧读取负例 | 单项退出 1，精确 PermissionError；工程改动 0 |
| 吞错负例 | 单项退出 1，精确 `PermissionError not raised`；工程改动 0 |
| 输入读回 | 719 项零漂移；生产改动 0 |

两项负例控制器退出 0 表示两次预期失败确已发生，不表示负例通过。
Windows share=0 文件句柄反例与原 native Job barrier 在本宿主均 skipped；
新候选 Windows 实际执行仍为 0。

本批没有重复全量 Rust 与三组 Rust controller。其输入与上一批完整宿主门禁相同，
上一批 `3225/0/48 ignored`、rustdoc `14/0/0`、app `1341/0/2 ignored`
仍只按原 958 冻结签署；本批以 718 项字节绑定和当前 Python／工程专项接续。
同一 checkout／target 归属未切换，没有 clean，也没有并行占用 Cargo 槽。
新非作者 Boyle 完成限定代码／功能复核，APPROVE，P0/P1/P2/P3 全 0。
独立单文件专项退出 0：72 run／70 passed／2 skipped／0 failures/errors，
unittest 6.134s、外层 6.203s，实际 waited。exit7 控制保留
`observed_exit=controller_exit=7/timed_out=true`；staged 控制保留空／部分观测、
身份先于 ACK 和完整清理。额外静态编译两组生成 helper 路径变体。
719 冻结及 718 未变输入独立匹配；初始辅助路径检索诊断按原报告保留。
未运行 Cargo／写 target／查询 CI，Windows 实际执行 0，已收取并关闭。
作者与非作者专项是相同用例的不同运行，不累计为不同测试。
后续精确 Windows CI 仍须实际执行新反例和原七类 tree 场景。

## 接续与原件

[25 成员封存](2026-11-16-windows-fixture-publication-evidence/README.md)已完成：
current 验证 `0/reaped/0.277s`，719 工程输入零 mismatch，测试重放 0。
非作者原件、作者负例、原 Windows 失败均保持原文，不把原失败覆盖成后续通过。

父先收齐原 `43873806902` 的三平台终态，避免新工程 push 取消仍有价值的运行。
限定门禁及新非作者复核完成后，封存输入、源码快照、负例、实际命令与原失败，
普通提交推送并读回本地／tracking／远端完整 SHA。
不清除原失败、未知结果、隔离或清理保留事实；不将本片写成完整产品验收。
