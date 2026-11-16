# 2026-11-16 隧道面板配置重新加载

本批从干净的 `c81f718ccc3ee5d6a8cce6b7988b1cbcab8ea694` 开始，源码与功能独立
复核均已限定批准，集中完整门禁已通过；原生桌面未验。当前 checkout 连续使用
原 Cargo target，归属未切换，不 clean 或使用其他作者的构建产物。

## 范围与计划

- 面板按钮、epoch／pending 绑定与草稿保留。
- Workspace 真实订阅、后台 StateStore 读取和跨窗口配置门禁。
- 冲突恢复、读取失败重试、现有／新面板 busy、旧审核与 retired owner 回归。
- 原有 SSH 和活动隧道保持；三主题使用同一语义组件路径。
- 必要专项、非作者代码与功能复核，修复后一次最终完整工程门禁。

设计见 [ADR 0151](../../adr/0151-tunnel-configuration-reload.md)。

## 官方依赖查询与锁文件

第一轮官方 crates.io 查询完成于 `2026-11-16T15:02:12Z`，113 个声明／使用中
两条 russh 直接边报告 `direct_lock_not_latest`：`0.65.1` → `0.65.2`。
原失败保持；`cargo update -p russh --precise 0.65.2` 实际退出 0，
同时解析 pkcs1 `0.8.0`、rsa `0.10.0`、ssh-key `0.7.0-rc.12`。
manifest 继续使用 `0.65`。更新后的官方复查与实际编译／回归结果见下文。

完整原件存入[本批证据目录](2026-11-16-tunnel-configuration-reload-evidence/README.md)；
交付前核对全部摘要并回收自有运行副本。

第二轮官方查询于 `2026-11-16T15:08:12Z` 通过全部 113 声明／使用。
传输专项实际 `0/reaped/31.156s`，session library `288 passed / 0 failed /
4 ignored`；真实 OpenSSH `17 passed / 0 failed / 0 ignored`，
外层 `0/reaped/11.868s`，内层 11.777s。临时目录已移除、记录的原进程身份均已停止，
用户 SSH 配置未修改。这里的 session 计数来自实际运行，不补造独立 `--list`。

## 首轮集成与修正

所有原失败保留，不用后续通过覆盖：

| 原运行 | 实际结果／修正 |
| --- | --- |
| app inventory r1 | 退出 101；测试模块 glob 导入了同名测试宏，递归展开。按仓库既有做法显式导入 core test，未提高 recursion limit。 |
| app inventory r2 | 退出 0，1364 唯一用例名，其中 21 新名；一个未使用测试导入随后移除。 |
| panel reload r1 | `13 passed / 1 failed`，失败为 runtime 根未注册测试观测 ID；补充与编辑／审核视图相同的根 ID，无布局或权限变化。 |
| tunnel integration r1 | `60 passed / 3 failed`；三失败均在 GPUI deferred event 交付前断言 Workspace busy。改为在生产订阅之后观察实际准入，仍分别要求真实受理或拒绝。 |
| workspace reload r2 | `7 passed / 0 failed / 0 ignored`；两个未使用测试 closure 绑定随后移除。 |
| app inventory r3 | 退出 0，1364 唯一名、21 新名；当前编译无 Rust 源码警告，链接器既有大 section 提示单独保留。 |

三主题 × 明暗 × 双语 × 900/1400 宽度的 24 组几何检查已在修订后的 tunnel
运行中通过。它验证 headless 绘制边界、AX 名称、固定操作区与反馈行，不是实际
macOS／Windows／Linux 桌面观察。另有真实回环 listener 保留与独立停止审核。

当前 r2 冻结含 721 输入，排除 docs 与设计台账；聚合
`442393e019c51a19b8fe49d5040813e44a06b5a77cc3a7eb4c9b162701a979f2`。
精确 r3 app 测试二进制 `273,326,016` 字节，SHA-256
`1f3ac5387026afaad1d36717faaa3a976e179503562aea04bea2537e5a7cef6e`；
新非作者源码和功能复核已完成；该冻结上的最终完整门禁结果见下文。

## 独立非作者复核

- **Socrates：限定 APPROVE，P0/P1/P2/P3 = 0/0/0/0。**
  静态复核六个 production 文件和 Cargo.lock，7 项输入与 r2 冻结一致。
  15 个变化 package 条目的缓存归档 checksum／manifest 一致，3,530 条锁依赖边
  唯一解析。没有运行 Cargo／测试或修改 target；没有将此结论扩大为平台编译。
  [原始报告](2026-11-16-tunnel-configuration-reload-evidence/originals/source-review-r1.md)
  SHA-256 为 `dd873661f3bc5cbf30a10c3e65a8f4cddbd307a1d1009e98f345459996bc41cb`。
- **Ptolemy：限定 APPROVE，P0/P1/P2/P3 = 0/0/0/0。**
  独立核对全部 721 输入及父编译二进制摘要，1364 个库存名逐名／顺序相等，
  直接运行 `63 passed / 0 failed / 0 ignored`，包括 21 新例和 42 原回归；
  运行声明、唯一逐名结果和汇总相等，实际 reap，无 timeout。
  这是独立执行已校验的父编译产物，没有独立编译，也不是原生桌面。
  [原始报告](2026-11-16-tunnel-configuration-reload-evidence/originals/function-review-r1.md)
  SHA-256 为 `e37a8f098dba06cc6f6f08bc5e5bb72add870319409951179ea97378a03283b5`。

两位均已收取并关闭，作者 Rawls 也已关闭。未创建用户 chat、worktree、分支或容器；
真实 OpenSSH 自有 fixture 进程与临时密钥目录已回收。上述不同执行不累计为不同用例。

## 最终完整门禁与正常程序

`python3 -B scripts/check.py` 实际 `0/reaped/834.958s`，无 timeout，
终态 direct reaped、active processes 0、pipes drained、errors 空。

| 范围 | 实际结果 |
| --- | --- |
| 格式、x.y 政策、严格 workspace/all-target Clippy | 通过 |
| 普通 Rust/libtest | 92 套件，`3246 passed / 0 failed / 48 ignored` |
| rustdoc | `14 passed / 0 failed / 0 ignored`，保留一个 crate 的多个实际子运行 |
| app | `1362 passed / 0 failed / 2 ignored`；1364 独立库存与唯一逐名执行相等，21 新例全部 ok |
| Python 主 discovery | `293 run / 2 skipped`，成功 |
| Rust 集成调用的 archive helper Python | 单列 `30 run`，成功，不混入主 discovery |
| 无 libtest harness 的两个 workspace controller | 各自明确成功终态，不伪填普通测试数 |
| readiness／observer／small-stack controller | 三个实际命令均通过 |
| 输入读回 | 721 项零漂移，未切换 target 归属、未 clean |

证据派生器前三次诊断和当时的解析器／成员表均保留：r1 未识别 Cargo
`Running tests/`；r2 尚未区分无 libtest harness 的 controller 与多子运行 rustdoc；
r3 将后续 archive helper 的 30 项误并入主 discovery 核对。
r4 对实际原件分别解析后通过，普通 Rust、rustdoc、主 Python 和 helper 的计数
各自保留。修订只在 docs 的证据派生器中，没有重放测试或改变工程输入。

正常 GUI／MCP release build 为 `0/reaped/183.646s`。提交前 r1 打包与本机结构检查
均退出 0，确认 arm64、Info.plist 与 minos 15.0；没有启动、安装、签名或公证。
该提交前包绑定上述冻结输入，`build.commit=null`，不假称它来自尚未存在的 Git 提交；
提交后以相同二进制摘要另行生成精确提交标识的正常包。

- GUI SHA-256：`0db8640a0939a51be9d343221d69f6cb0a60321a0ee95089e3c273721f15de3f`
- MCP SHA-256：`bb415f3ae3ae7ca62bdb78abe752b50f5a3a796dc65ec99490f54ee28cb3f693`

76 成员、1,854,302 字节原件已按摘要封存；`verify-sealed-r1`
实际 `0/reaped/0.154s`，721 输入零 mismatch，1364 app 名逐项相等、21 新名全部 ok。
重派生没有执行测试或修改 target。原 parser 诊断、父／非作者执行和原生锁屏事实均保留。

随后增强为从精确 Git 提交核对全部成员与工程输入；派生器变化后当前封存仍为
76 成员、1,854,528 字节。原始日志按目录属性关闭换行转换及 diff 空白 lint，
补齐默认忽略的日志后，在首次 push 前完成本地修订；没有强推。

## 实际提交推送与包

工程 `6f28f5c424752a221746fbfb621e319c20ff38b9` 已普通推送，
`2026-11-16T15:56:52Z` 三方读回一致且工作区干净。
76 原件与 721 输入全部从该提交的真实 Git blob 验证通过，
`0/reaped/7.350s`，零 mismatch，测试重放 0。

精确标识该提交的正常包位于本地
`work/tunnel-configuration-reload-20261116/package-r1/KeelShell.app`；
程序摘要与上面的提交前包相同，全体包成员逐项核验，打包／结构检查再次退出 0。
原 `build.commit=null` 的提交前包没有被追溯修改，而由新目录替代。

[30 成员交付补充](2026-11-16-tunnel-configuration-reload-evidence/delivery/README.md)
保存实际提交／push／三方读回、精确包 manifest、CI 运行中快照及旧资源原件。

交付文档 `e56f3cf7bcd621cbce37ee9b112b0bf3755b98aa` 已普通推送。
在全部对应原件与包 manifest 核对真实 Git blob 后，实际回收 115 文件、
115,495,894 字节，包括本批运行副本、提交前重复包、被新包替代的旧 publication 包
及上一活动目录。新正常包六文件前后摘要相等，共享 target 的目录身份不变，
没有删除第三方缓存或其他任务资源；三位自有 agent 已关闭。
[逐项清理回执](2026-11-16-tunnel-configuration-reload-evidence/delivery/owned-cleanup-r1.json)
保留每项路径、摘要、字节及 Git 原件依据。旧活动目录已经不存在，后续只使用本批
正常包和 docs 封存；活动目录仅保留必要包、工装和最后交付／CI 观察回执。
[后续原件](2026-11-16-tunnel-configuration-reload-evidence/delivery/followup/README.md)
记录清理后主封存再次验证 `0/reaped/0.155s`，以及原生通道再次报告锁屏、
GUI 操作／截图仍为 0；没有重跑宿主完整门禁。

## 精确 CI 终态补充

精确 Quality `43901863960` 已 completed/success，watch 已实际
`0/reaped/1258.244s`；Windows `133420285732`、Ubuntu `133420286130`、
macOS `133420286347` 的完整 gate 均 success，macOS／Ubuntu OpenSSH step
success、Windows 该 step skipped。

| 平台 | 普通 Rust passed / failed / ignored | app passed / failed / ignored | Python run / skipped | rustdoc passed |
| --- | --- | --- | --- | --- |
| Windows | `3233 / 0 / 46` | `1351 / 0 / 2` | `293 / 34` | 14 |
| macOS | `3246 / 0 / 48` | `1362 / 0 / 2` | `293 / 2` | 14 |
| Ubuntu | `3239 / 0 / 49` | `1363 / 0 / 2` | `293 / 2` | 14 |

每平台 92 个普通套件的声明／唯一结果／摘要匹配。CI app 无独立 `--list`，
Python 无逐名输出；宿主独立库存不能移签给 CI。
[25 成员终态封存](2026-11-16-tunnel-configuration-reload-evidence/ci-completion/README.md)
实际验证 `0/reaped/7.658s`，从真实 Git blob 核对原 721 输入，零 mismatch。
两次前缀／转义派生诊断和原日志保持，没有重放测试。

## 尚未验证

正常包结构检查不代表 GUI 已启动，也不代表 Windows／Linux 或六目标发行通过。
本轮 `cua.getState` 再次明确报告 Mac 锁屏且自动解锁失败，GUI 操作与截图均为 0。
前批源码、正常包与 CI 成绩不移签为本批。
