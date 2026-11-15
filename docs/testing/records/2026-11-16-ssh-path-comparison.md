# 2026-11-16 — 审核式 SSH 路径比较

状态：`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_SCOPES_APPROVED`。
基线 `162d883460a98cd03056731fd8d4be76a41d5c15`。完整产品总目标保持进行中。

## 范围与节奏

传输作者：成功认证时封存的不可变路线与身份、固定内存测速和协议回归。
实验作者：真实 target/jump OpenSSH、可重复流延迟/限速 A/B、进程清理和中继指南。
父：原生监控入口、两个真实连接与选择 revision 的授权、迟到撤销、UI 测试和集成。
两个作者在受管 worktree 开发，不抢父 Cargo target，不重复跑完整门禁。
集成后集中测试、修复、新非作者复核；只本地提交。

设计与验收边界见 [ADR 0141](../../adr/0141-reviewed-ssh-path-comparison.md)。
原始失败、冻结、门禁、联调和限定报告从
[持久证据入口](2026-11-16-ssh-path-comparison-evidence/README.md)接续。

## 环境与当前验证

当前实查 Rust/Cargo 均 `1.100.0`，沿用环境 stable，无 toolchain pin 或新 MSRV。
有效 target 仍为 `work/recovery-batch-integration-20261115/target`，未切换源码归属。
不新增依赖。官方最新依赖检查 `dependencies-live-r1` 总预算 45 秒超时，
实际 `exit -15 / reaped`，不能签全部依赖为最新；失败原件保留。

最终候选 r3 为 669 项完整工程输入，包含 Cargo、所有 crates/assets/scripts 输入
以及被应用编入的 CHANGELOG。集合 SHA-256 为
`066d7835e75d09895ae7a97f5b83863904a8dcbb0419bb1d8e865f89b7a047e0`。
相对 r2 修改五项、增加一个明确的 decoded-message 协议单元模块，663 项不变。
所有 Cargo 均在父工作树执行，未切换 target 归属；作者不运行 Cargo。

## 专项与失败修复

最终 `focused-tests-r3` 实际 `exit 0 / reaped / 33.558s`：

| 范围 | 实际结果 | 覆盖 |
| --- | --- | --- |
| app | 11/11 | 两条真实会话绑定、预览零探测、ABA、关闭非活动候选、重连/路线撤销、worker 取消与迟到、未知清理、三主题双语、900×580 工作区、实际绘制文本 |
| session lib | 4/4 | 完整三轮中位数、缺失/重复/失败/零值、真实固定 Python 子进程、decoded-message 重复控制 |
| 加密 loopback SSH | 11/11 | 实际递归跳板与两种显式代理、目标/路径资格、18 个交替样本、恶意输出/EOF/exit/CLOSE、背压、取消/迟到及共用父/独立兄弟存活 |
| Python runner | 16/16 | 真实 localhost 字节流、限速/缓冲/half-close、精确目标约束、严格报告、库存和按身份清理 |

这 26 个默认 Rust 用例与 16 个 Python 用例分别计数，不把重复执行或多个参数
算成额外测试。三项 OpenSSH opt-in 测试默认 ignored，真实运行见下节。

`focused-build-r1`–`r4` 保留真实编译失败：新增可选连接元数据的旧 fixture 漏参、
proxy fixture 私有类型、GPUI 属性宏的 glob 导入解析、重复导入和 Kit 测试 API
不匹配。修正后 r5 no-run 成功；没有提高 recursion_limit 掩盖宏解析错误。
测试链接保留 `__eh_frame section too large` 的 linker warning，不签“零 warning”。

`focused-tests-r1` app 为 `9 passed / 2 failed`：真实关闭 tab 已完成连接关闭，
重复关闭返回 SendError；现仅在实际观察 `is_closed()` 时接受重复关闭。普通
字符串没有进入 Kit 测试文字观察，现让 renderer 与观察共用同一 SharedString，
正常构建不插入测试元素。r2 app `11/11`，三个 session lib 用例通过，
loopback 为 `10 passed / 1 failed`，失败保留。

r2 最后失败是误导的 DuplicateAck fixture：锁定 russh 的公开服务端 API 第一次
响应后清除 wants_reply，第二次调用没有第二个 ACK。真实生产代码正确接受了
实际发送的合法序列，不能将 fault 名称当作 wire 证据。现移除这个伪 wire fault，
在 decoded-message 边界先证明合法完整序列成功，再注入重复 ACK/EOF/exit，
三种操作均必须保持 InvalidSequence，不能被合法尾部恢复；没有放宽生产校验。
其余公开 API 可发出的恶意序列继续经过实际加密连接。

非作者提出的显示精度问题也已修正：以整数 Duration 的绝对差和 10µs 比较，
只在显示时生成有符号 B − A。实际绘制测试独立断言双语字面文本、相等、±9µs
不足与 ±10µs 显示边界，不用同一个生产 helper 生成期望。

## 最终真实 OpenSSH 实验

最终 `openssh-path-lab-r2` 使用 r3 稳定 `ssh_path_interop` 副本，实际
`exit 0 / reaped / 23.030s`。库存三个精确 opt-in 测试全部核验；基线执行三项，
A 受损与 B 受损各执行一项，共五次 Rust 测试进程。三场景各 18 个按实际顺序
保留的 Passed 样本、Complete、无未知清理，合计 9 MiB 合成载荷，额外包括 SSH、
固定命令与存活检查开销。基线另含预取消、同路线拒绝、错误 target/jump pin。

| 场景 | A gate | B gate |
| --- | --- | --- |
| baseline | 无附加等待 | 无附加等待 |
| direct-impaired | 每次写前 20ms + chunk/524288 秒 | 无附加等待 |
| relay-impaired | 无附加等待 | 每次写前 20ms + chunk/524288 秒 |

gate 每方向独立最多读取 16 KiB，字节原样转发并尊重背压。A→target，
B→jump→同 target；target/jump 各自 pin、独立临时 host/client key，jump 限定
唯一转发目的且禁止 exec/PTY。所有 listener 都是受限 localhost。
每场景原件包含 conditions、完整审核、18 样本、字节数、失败/状态和中位数；
差值为有符号 B−A，不命名普遍“加速”。原 r1 `19.228s / exit 0` 绑定 r2，
保留其原身份，不改签最终 r3。

两轮 cleanup 各有 23 个顶层 owned 子进程真实 wait/reap、无未知 ancestry、
临时私有目录已删除。父补充按 kernel birth identity 读回合计 197 条身份全 absent，
16 次 endpoint 探测均无监听，两个私有目录 absent。见
`openssh-final-readback-r1.json`；不依据进程号复用或自行发送信号猜测完成。

实验作者只探查三个独立 Podman 容器；现有镜像缺 `tc`/sshd/Python 组合，
三个容器均已删除且 exists 返回 1。未安装工具或修改任何任务的 qdisc，
未删除共享镜像或其他任务资源。**IP netem / 随机丢包为 HOLD / NOT_TESTED**。

## 工程门禁、复核与交付

最终 `full-gate-r1` 已实际 `exit 0 / reaped / 698.757s`：
普通 Rust `3103 passed / 0 failed / 40 ignored`、rustdoc `14/0`；
工程 Python `203 run / 1 skipped`、helper Python `25/25`。
格式、严格 workspace/all-targets Clippy、x.y 策略和三组 AI controller 全通过。
26 个新增默认 Rust 名在完整门禁中逐项为 ok，已包含在总数；门禁后 669 项工程
输入零 mismatch。分类与逐名明细保留在 `final-gate-summary-r1.json`。

`normal-build-r1` 实际 `exit 0 / reaped / 11.374s`：正常 GUI 与 MCP 均为
arm64 Mach-O；GUI fresh=false 表示本次实际重编译，MCP fresh=true 是同一父工作树
未切换归属的有效缓存，不宣称 MCP 本次重编译。产物摘要、真实路径和 build-finished
成功见 `normal-build-readback-r1.json`，这些不签署系统桌面验收。

非作者 app 最终 `report-final-r3` 批准原范围，两个 P2 与一个 P3 已关闭，
开放 P0–P3 均为 0。它独立核对 669 项、1271 项 app 库存，并执行 11/11 专项；
使用父预构建，无独立重编译。原报告签署时完整 gate 仍在运行，父已在相同冻结
读回成功终态，满足其工程条件，原报告不改写。app reviewer 已实际关闭。
传输/runner 最终 `report-final-r3` 也批准原范围：一个 P1、两个 P2 全部关闭，
开放 P0–P3 均为 0。独立执行最终 15/15 Rust、16/16 Python、9/9 固定程序、
3/3 真实 alarm 和 1/1 自有身份清理专项；这些分开计数，不与父门禁叠加。
它独立审计最终 54 个 ordered samples、条件、中位数、来源和两轮全部清理；
读取父 reap 回执与自己实际 wait 的范围明确区分。没有独立重编译、重跑完整
OpenSSH lab 或门禁。原报告签署时正常 build 待父收口，父现已确认其成功终态。

两个新非作者范围累计关闭一个 P1、四个 P2 和一个 P3，当前均无开放 P0–P3；
两名 reviewer 均已实际关闭。`final-review-resolution-r1.json` 保留原报告摘要、
关闭回执及相同冻结的门禁/正常构建对应关系，不改写旧失败或原签署条件。
两个作者 worktree、三个自有 Podman 探查容器均再次确认 absent，没有新建用户 chat。
后续只形成本地功能提交；无损封存与 Git index/blob 读回完成后再删除重复副本。

## 保持开放的边界

原生 macOS/Windows/Linux 桌面、键盘、IME、辅助技术及发行均未由本批签署。
受控 TCP 字节流延迟/限速不等于 IP netem、随机丢包或链路带宽。
NET-03/M4 的完整网络收益和其余产品范围继续开放。
整次六十秒绝对期限已源码复核，但没有强制耗尽六十秒的端到端专项；
八秒 sample 与远端程序 alarm 分别有实际执行证据。重复 ACK 只到 decoded-message
边界，不能写成真实加密 wire 注入验收。
