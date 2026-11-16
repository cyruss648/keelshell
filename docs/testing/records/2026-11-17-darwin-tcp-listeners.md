# 2026-11-17 远端 macOS TCP 监听端口

## 精确 CI 终态补充

工程 `bd198a8ef2578ea9b96e0a603cd705bb2eadca13` 的 Quality `43916079699`
已三平台 completed/success。Windows／macOS／Ubuntu 普通 Rust 为
`3259/0/49`、`3272/0/51`、`3265/0/52`，app 为 `1356/0/2`、
`1367/0/2`、`1368/0/2`，各 14 rustdoc。Python 各运行 293 项，
分别 34／2／2 skipped；macOS／Ubuntu 标准 OpenSSH success。
31 成员[终态封存](2026-11-17-darwin-tcp-listeners-evidence/ci-completion/README.md)
包含逐名派生和精确 Git 绑定，验证器 exit 0、零 mismatch、测试重放 0。
下方旧 in_progress 保留为捕获时点；终态不代签后续编辑器批、桌面或发行。

## 原批次记录

本批在 `8d95d71cfa08181d5d44b689c108edcad7d995c4` 上开发。
源码／专项、两位非作者复核、一次最终完整门禁和真实 SSH 互操作均已通过；
完整产品、原生桌面与发行仍开放。

## 实现与来源

沿用 session 的 `LinuxMonitor::listening_sockets()` 和 app 原有端口列表。
Linux 使用固定 ss，Darwin 使用已有绝对路径 `/usr/sbin/lsof`，只读当前账户
可见的 TCP LISTEN，独立于 Python 资源采集。严格解析字段、数值、地址与预算，
两平台均要求 start、真实工具 status、end；缺帧不能变成 Available 空集合。
Darwin UDP、主动 TCP 探测、进程核对／SIGTERM 保持关闭。

三主题及双语继续走既有 registry、Palette、Kit 组件和能力缓存；
主题／语言切换不重复采集，暂停 ABA 和迟到结果不覆盖新状态。
设计见 [ADR 0152](../../adr/0152-darwin-tcp-listeners.md)和
[原生组件合同](../../design/HALLMARK_NATIVE.md)。

最终 r4 冻结包含 724 输入，排除 docs／设计台账，包含两份 README：
`baae8aaab5903ad60509c23341b91a23553fac47ea7d1b0ff9ac993031c2e6ef`。
唯一 Cargo target 连续属于此 checkout，没有切换或重复 clean。
现场默认 Rust/Cargo 1.100.0、GPUI Kit 0.10；不 pin 工具链、不设 rust-version。
官方 crates.io 复查于 `2026-11-16T16:14:14Z` 完成，37 包／113 声明通过，
本批 Cargo manifests／lock 没有修改；这只是该时点查询。

## 保留的失败与修复

| 原运行 | 实际结果与处理 |
| --- | --- |
| app-darwin-r1 | exit 0，但过滤器未命中、0 tests，不算覆盖；修正过滤后 r2 为 `9/0/0`。 |
| openssh-darwin-r1 | `6 passed / 2 failed`；真实 lsof 将 IPv4-mapped IPv6 描述符显示为无括号 IPv4。保留观察地址，补自有 mapped listener 正例；r2、最终 r4 均 `9/0/0`。 |
| missing-frame-negative-r1 | 修前公开 SSH 入口反例 `0 passed / 1 failed`、exit 101；对应独立评审原 P2。修后完整平台帧解析拒绝空白、stderr-only、裸行及截断，合法空帧仍通过。 |
| session-monitor-r3 | `101 passed / 1 failed`、exit 101；仅旧静态断言仍要求 ss 直接 exit。改为检查真实 status 捕获和完整帧，行为断言未放宽；r4 `102/0/0`。 |
| gate 派生 r1 | 原日志预期的 ZipWriter 预算诊断与一个 libtest 结果同行，首解析为 40／实际 41 而拒绝。只修正派生器并保留诊断，原日志不变、测试不重放。 |

## 独立非作者复核

Ohm 最终 r4 源码 **APPROVE，P0/P1/P2/P3 = 0/0/0/0**。
原缺帧 P2 已关闭，检查了公开入口、固定脚本和 ForceCommand 精确 allowlist。
724 输入零漂移；没有 Cargo、测试、格式化或 target 写入。
原 `source-review-r1.md` 的 REQUEST_CHANGES 和失败保留。

Hegel 最终 r4 功能 **APPROVE，P0/P1/P2/P3 = 0/0/0/0**。
独立直接执行父编译产物：app monitor `75/0/0`、session monitor `102/0/0`，
共 177 项；库存分别 1369／313 唯一名，5＋21 个新名全部 ok。
执行前后 724 输入与产物摘要相同，actual wait/reap，无 Cargo 或 target 写入。
这是独立执行父预编译产物，不是独立编译或 native。
作者 Jason 和两位 reviewer 均已关闭，没有新建用户 chat、worktree 或分支。

## 最终完整工程门禁

`python3 -B scripts/check.py` 实际 **`0/reaped/794.954s`**，
无 timeout／interruption，direct reaped、active 0、pipes drained、errors 空。

| 范围 | 实际结果 |
| --- | --- |
| x.y 依赖政策、fmt、严格 workspace/all-target Clippy | 全部通过 |
| 普通 Rust/libtest | 92 套件，`3272 passed / 0 failed / 51 ignored` |
| rustdoc | `14 passed / 0 failed / 0 ignored`，多个真实子运行分别保留 |
| app | `1367 passed / 0 failed / 2 ignored`；准确 gate 二进制的 1369 库存名与逐名执行相等 |
| 新增默认测试 | app 5＋session 21，共 26 个具体名全部 ok；包含在上述计数内 |
| Python 主 discovery | `293 run / 2 skipped`，成功 |
| Rust 调用的 archive helper Python | 另列 `30 run`，成功 |
| workspace 两个无 libtest harness controller | 各自实际成功终态，不填普通测试数 |
| readiness／observer／small-stack controller | 三个后续命令均通过 |
| 输入 | 724 项零漂移，共享 target 归属未切换 |

gate app 是 workspace feature 的独立产物，其 `--list` 来自同一个准确二进制；
不能把 reviewer 使用的单包产物身份移签到它。各 suite 的声明、唯一逐名结果和
汇总相等；专项／完整门禁不累计成不同用例。

## 真实 SSH、Linux 恢复与清理

- 最终真实 Mac localhost OpenSSH **`9/0/0`**，外层 `0/reaped/4.582s`。
  包括自有 IPv4、IPv6、mapped TCP listener，以及已连接端点排除；不保存整机端口
  或进程原文。r1、r2、r4 的自有 sshd、观察到的子进程与临时密钥目录均已停止／删除。
- 真实 Linux Podman 能力实验 **`2/0/0`**，result 为 PASSED。
  df／ps／ss 的缺失、不兼容、权限失败、负缓存、显式恢复和新连接均按实际状态断言；
  关闭的 SSH transport 不得误归为 command missing。
- 该实验的 `--source-sha` 参数记录基线 HEAD `8d95d71…`，不是尚未提交的 r4。
  `linux-recovery-binding-r4.json` 将实际 WIP 绑定到上述 724 冻结及新编译测试副本
  SHA-256 `b739c1028679af5718f6c87e312bb533675cf6712d9024fefb728c8214e3247a`。
- 本批容器、私有 network、派生镜像、私有运行目录和临时构建的基础镜像／中间
  parent 均已核对不存在；原公共 Ubuntu 镜像保留。自有残留容器 0。
  日志和稳定测试副本在精确 Git 原件核对后回收，不删除共享 target 或其他任务资源。

## 正常应用包与未验范围

正常 GUI／MCP release build **`0/reaped/151.594s`**；
提交前打包与结构检查均 exit 0，arm64、Info.plist 与 minos 15.0 相符。
提交前 `build.commit=null` 如实保留；后续精确提交包单独生成，不能追溯改写。

- GUI SHA-256：`f94ecc51f706b864bded6bb4eb04da9993b023feddb320260afc100d95c09c13`
- MCP SHA-256：`bb415f3ae3ae7ca62bdb78abe752b50f5a3a796dc65ec99490f54ee28cb3f693`

没有启动、安装、签名或公证。最新 native 可用性观察仍报告 Mac 锁屏且自动解锁
失败，GUI 操作／截图 0。真实 SSH 采集不等于 GPUI 桌面或目标 OS 原生验收。
远端 Windows、外部 AI/MCP、strict OO 独立 peer、完整主题／键盘／IME／辅助技术、
三平台桌面及六架构发行继续按路线推进。

## 封存与交付

[255 成员原件](2026-11-17-darwin-tcp-listeners-evidence/README.md)保留全部失败、
冻结、独立报告、原日志、来源绑定和清理回执，排除二进制、密钥与 token。
初次封存验证 `0/reaped/0.165s`、零 mismatch；验证器新增正常包断言后继续
按新回执核对，不重放测试。工程输入和当前 Git 提交绑定在实际提交后读回。

上一隧道源码 `6f28f5c` 的 Quality `43901863960` 已三平台成功；其
[25 成员终态](2026-11-16-tunnel-configuration-reload-evidence/ci-completion/README.md)
绑定原 721 输入，不能移签为本批。

工程 **`bd198a8ef2578ea9b96e0a603cd705bb2eadca13`** 已普通提交／推送，
push 实际 `0/reaped/4.760s`，`2026-11-16T17:14:08Z` 读回本地、tracking、
远端三方相同且工作区干净。没有强推或覆盖 origin/main 的独立文档。
255 原件和 724 输入从该提交真实 Git blob 验证 **`0/reaped/6.972s`**，
零 mismatch，测试重放 0。

精确标识该提交的正常包位于本地
`work/darwin-tcp-listeners-20261117/package-r1/KeelShell.app`，
完整文件与提交前 manifest 逐项相等，GUI／MCP 摘要同上；打包和结构检查均 exit 0。
原 `build.commit=null` 未改写，提交前重复包在核验后回收。

已实际回收本批与上一活动副本 **301 文件、88,633,128 字节**，
每项先匹配该提交 Git 中的原文或 manifest。包括稳定测试副本、提交前重复包和
旧 CI 运行副本；当前与上一正常包各六文件摘要前后相同，共享 target 的目录
身份不变。上一隧道活动目录仅保留正常包；原件从 docs 封存读取。

新 Quality **`43916079699`** 绑定本批工程；最新快照 Ubuntu `133470463980`、
macOS `133470464468`、Windows `133470464623` 均 in_progress，
尚无当前源码三平台终态。状态快照原文保留，没有活动 watcher，
没有把上一批成功或宿主门禁替代当前 CI 结果。

新非作者 Locke 对源提交时点的指定 11 项文档／封存限定 **APPROVE，P0–P3 全 0**；
两个文档验证器分别实际 `0/reaped/0.094828s`、`0/reaped/7.331875s`，
独立重算 92 套件、26 新例、主与 helper Python 及旧 CI，724 Git 输入零差异。
原计数／报告派生诊断保留，没有 Cargo、功能测试或 target 写入；
结论不覆盖父级此后的交付／清理补记。Locke 已关闭，本批四位自有 agent 均关闭。
[43 成员交付补充](2026-11-17-darwin-tcp-listeners-evidence/delivery/README.md)
保存原报告、实际推送、包身份、清理和 CI 捕获时点；补充提交并核对 Git 后
回收活动原件，不重复未变输入的宿主完整门禁。
