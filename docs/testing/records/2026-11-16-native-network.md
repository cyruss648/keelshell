# 原生流程与真实 IP 网络实验 — 2026-11-16

状态：`FINAL_GATE_PASS / NON_AUTHOR_SOURCE_SCOPES_APPROVED / SCOPED_MACOS_NATIVE_PASS`。基线为本地
`5a6b30a2b9cca6d89aea551409738145608ac689`，只本地提交，不 push。
本批按独立实现并行、集中集成验证修复、非作者复核的节奏进行。

## 范围与执行方式

父负责当前 macOS arm64 原生窗口、三主题与审核式 SSH 路径比较流程；
隔离作者负责可重复的 Linux IP 延迟、丢包实验工具。
唯一有效 Cargo target 仍为 `work/recovery-batch-integration-20261115/target`，
归属保持父 checkout，作者不执行 Cargo。

原生测试使用暂存包、独立 `KEELSHELL_DATA_DIR`、私有临时密钥、
自有 localhost SSH 服务。测试包不安装、不替换日常应用。
辅助工具的操作按明确 PID 限定到本批测试应用；
屏幕与辅助功能权限在开始时现场读取。

网络实验要求在自有容器的隔离 network namespace 配置 kernel qdisc，
读回配置与计数。流延迟、应用字节丢弃或协议 mock 不记为 IP 丢包实验。
所有前提失败、未知结果和清理失败保留原始状态。

## 开始时记录（历史原件保留）

- 原生 AX 辅助程序编译 `exit 0 / reaped / 2.056s`。
- 本轮官方依赖在线检查 45 秒超时，`exit -15 / reaped`；
  不签署全部依赖最新，没有因此修改 lock。
- 上一批有效 GUI/MCP 暂存为本批 `package-r1`；修后正常构建与原生流程见下文。

## 原生发现与修复

基线实际建立 direct SSH 与 SFTP；经跳板流程停在目标凭据面板后，
返回 `session is closed`。定位为每 200 ms 的窗口清理只从终端计算根，
遗漏尚未创建终端的当前已认证上游。新真实 peer 用例通过显式清理 tick
稳定复现红结果。当前将待完成上游作为根，复用现有传递依赖闭包；
完成、取消和最后 tab 释放继续回收。

基线指纹在截图可见，但不在 AX 树；新渲染回归取得真实红证据，
随后补上指纹、变更警告、路线和路径审核/结果的可读角色与同源文本。
首轮新增测试有 integer ElementId 类型编译错误，原件保持，
不得与随后两个行为失败混为成功。设计见
[ADR 0142](../../adr/0142-pending-route-ownership-and-readable-reviews.md)。

`affected-green-r1` 实际 `exit 0 / reaped / 15.973s`，相关专项 `19/19`。
新非作者 app 范围源码批准、开放 P0–P3 为 0，
独立执行父预构建同一 `19/19 / 5.078s`，没有独立重编译。
r1 报告签署时最终门禁与原生证据复核待定，原报告保留条件，最终绑定见下文。

修后 `native-build-r2` 为正常 GUI/MCP 构建，
实际 `exit 0 / reaped / 28.966s`，暂存包和 macOS 原生结构检查通过。
`native-r2` 使用新私有状态和 localhost 服务，按独立生成的公钥指纹
实际核对 direct、jump、routed target；目标认证界面有意停留 0.85 秒
（实际回执 0.946 秒），随后成功建立两个会话。真实窗口按生产周期运行；
不将时间换算成逐项观测的 tick 次数。

`native-flow-r2` 的非零退出保持：起因是对屏外路径按钮执行 AXPress
未展开；后续用限定到本测试窗口的真实滚动将按钮移入视口后继续，
不是放宽完成断言。手动接续后从真实界面预览、核对两条路线与精确命令，
确认并完成 18 样本；AX 中可读三组中位数、有符号差异、全部 18 个通过样本
和本次路线来源。详细截图及 AX 原件保存；不以 AX label 存在推断控件可见。

三主题实际切换并保留两个连接及完成结果，之后切英文，AX 树保留完整三组中位数
及 18 个通过样本。请求客户区 900×580，实际外框 900×612；有界滚动后的
`41-narrow-results-settled` 截图可见完成说明、第一组中位数及差异，较低的样本
与来源仍须滚动。`32`、`36` 截图主要可见来源尾部，不能声称全部结果同时可见。
恢复大窗口后停止 supervisor，`exit 0`，两个原端口关闭、私有目录 absent；
父另查 18 条原 kernel birth 身份，活跃匹配为 0。
`native-summary-r1.json` 保存分段结论。未执行原生“停止比较”，不以协议取消测试
代签该按钮，也不代签完整 VoiceOver 或 IME。

## 真实 Linux IP 实验

隔离作者的五个新文件按 SHA-256 导入，691 份原件逐字节核对一致；
作者 agent 已关闭，受管 worktree 路径及 Git 注册均 absent。
不切换父 Cargo target 归属，不重复编译同一传输 binary。
最终 `real-r5` 实际 `exit 0 / reaped / 31.709s`，五次 opt-in 测试通过：

| 场景 | 声明条件 | 样本 | A kernel drops 增量 | B kernel drops 增量 |
| --- | --- | ---: | ---: | ---: |
| baseline | 两侧清洁 | 18 | 0 | 0 |
| direct-impaired | A egress 20 ms / 5% 随机丢包 | 18 | 7 | 0 |
| relay-impaired | B egress 20 ms / 5% 随机丢包 | 18 | 0 | 6 |

这是同一逻辑目标、账户和 pin 的 Linux IP 层观察，不是生产性能承诺。
qdisc 按 namespace/owner 绑定，保留原始 netlink 字节与配置、计数；
计数单位为 skb，不宣称等于实际线上帧或测得百分比。
r1/r2 前置检查失败、r3 的历史 1% 成功、r4 配置 1% 但零实际 drop 的失败
均保留；r4 即使完成 54 样本也不签成功。SIGTERM 回执保留 `INTERRUPTED / 130`，
清理确认不改变其中断结果。作者专项 Python `22/22`；
作者交付时新的非作者网络范围审查与集中门禁尚在进行，最终状态见后文。

网络非作者 r1 为 `REQUEST_CHANGES_SCOPED`，开放 P2 一项、其他级别为 0：
超时或中断的创建客户端仍活着时，一次不存在查询可能提前签 absent；
停止 CLI 也不能证明 Podman 服务端已取消。独立的两个真实子进程 probe 复现，
没有运行真实晚到资源实验。父新增 deadline、interrupt 两个有界回归，
真实客户端加模拟独立延迟服务端；先 `2 run / 2 failures`，修后专项 `24/24`，
全 Python `227 run / 1 skipped`，`exit 0 / reaped / 9.423s`。
修复先停止/reap 客户端，资源清理前再做全部客户端屏障；创建状态未知时，
即使二次不存在或稍后删除了匹配资源，也保留 `MutationCompletionUnknown`
及 `CleanupUnconfirmed`。原件保持，不能将模拟延迟服务端说成真实 Podman 超时验收。

修后真实 `real-r6`：runner `exit 0 / 34.907s`，外层 recorded
`exit 0 / reaped / 34.975s`，五次 opt-in、三场景各 18 样本通过。
baseline drops 为 `0/0`；direct-impaired 为 `4/0`；relay-impaired 为 `0/11`。
这些是整个调用窗口的 qdisc skb 增量，不做逐样本归因。
9 个自有资源按 ID/名称删除后复查，96 个清理前客户端与最终全部 164 个客户端
均停止/reap，私有目录删除，输入逐字节前后一致，`cleanup.confirmed=true`。
网络非作者的 r6 复核在 r2 原件保留，后续残余修复另记 r3。

非作者 r2 继续发现同一 P2 的非零或信号退出残余：wait 正常返回错误码时，
创建仍被写成 `COMPLETED`。两个独立真实客户端 probe 复现。
父先停止并实际回收 `full-gate-r2`（`exit -15 / reaped / 404.37s`），
3 条 birth 身份无活匹配，精确 birth/uid 匹配的中断临时目录经无打开句柄检查后回收；
保留日志与退出原件。另补 `125` 和 `SIGTERM/-15` 两项真实子进程回归，
先 `2 run / 2 failures`（`netem-client-exit-red-r1`），修后专项 `26/26`；
仅 `rc0` 的创建请求记为 `COMPLETED`，其他退出一律保持 `UNKNOWN`。
模拟延迟服务端的范围不变，不能签署真实 Podman 晚到创建验收。
app 非作者 r2 已批准七个 app 文件、开放 P0–P3 为 0，已关闭。
报告签署时门禁待定保持；其 298 个 app Rust 输入在后续 r3 中不变，
由父将最终门禁和网络范围复核绑定到最终批次，不改写原报告。

最终真实 `real-r7` 使用 r3 冻结：runner `exit 0 / 35.456s`，
recorded `exit 0 / reaped / 35.523s`，五次 opt-in、54 样本通过。
baseline drops `0/0`、direct-impaired `7/0`、relay-impaired `0/8`；
前后输入一致，9 资源、96 个清理前客户端和最终 164 个客户端清理确认。
父另查 18 个 ID/名称均不存在、164 条 birth 身份无活匹配、6 个 gate 端口关闭，
私有目录 absent，共享 base image 保留（`resources-final-r2.json`）。

网络非作者 r3 `APPROVE_SCOPED_SOURCE`，唯一 P2 及其残余关闭，
开放 P0/P1/P2/P3 均为 0。独立旧 r2 红回归 `2/2` 预期失败、最终专项 `26/26`；
r7 的 12 份 raw qdisc 独立解码及 249 项核验通过，独立现场读回同上述资源、进程
和端口。没有重新运行真实 lab、Cargo 或 Rust 预构建，不宣称独立重编译。
两个 reviewer 均已关闭。其签署时最终全仓门禁仍 PENDING，由父绑定后续终态；
r1/r2 的 REQUEST_CHANGES、旧 gate 中断与失败原件保持。

## 集中门禁与冻结

父全仓冻结 r1 包含 699 项 non-doc 源码、配置及资产，
集合 SHA-256 为 `deba86e1554f9a5b1c4b6b2b649d441aef3fbd57dd9e47114cb250de8a9ef7d8`。
相较上一批冻结额外覆盖工作流、打包脚本、根级说明等输入；
两个数目不直接用作新增功能或用例计数。
`full-gate-r1` 实际 `exit 0 / reaped / 705.126s`：普通 Rust
`3105/0/40 ignored`、rustdoc `14/0`、工程 Python `225 run / 1 skipped`、
helper Python `25/25`，格式、严格 Clippy、x.y 策略与三组 AI controller 通过。
两个新默认 Rust 测试名在实际全门禁日志中逐项通过；另增强既有路径本地化用例。

最终 r2 为相同 699 项，集合 SHA-256
`4ba8a00cbce305bd7d4935a158104c89ceb8313e8c2d2431bd43fa1513fca2c4`。
相对 r1 恰好 `ssh_path_netem_lab.py` 及其测试两项变化，其余 697 项不变。
受影响 Python 全套已重跑；仓库要求最终冻结一次完整成功门禁，
因此另执行 `full-gate-r2`，该轮因上文残余修复实际停止，不改签成功。
最终 r3 仍为 699 项，集合 SHA-256
`f435b9fa78f9a40bc2962fa3043b983cf5818e1ab4e34db5deae5960dfc50e21`，
相对 r2 仍恰好上述两项 Python 变化。
最终 `full-gate-r3` 实际 `exit 0 / reaped / 701.952s`：
普通 Rust `3105/0/40 ignored`、rustdoc `14/0`、工程 Python
`229 run / 1 skipped`、helper Python `25/25`；格式、严格 Clippy、
x.y 策略与三组 AI controller 通过。两个新默认 Rust 名在实际运行中逐项通过，
最终工程完整路径与逐文件 SHA-256 零漂移。
`final-gate-summary-r2.json` 绑定最终事实，旧 r1 汇总保持历史采集状态。
`review-resolution-r1.json` 将原报告条件、不变 app 输入、网络最终窄差分与 gate r3
合并为本批限定结论，没有将被中止的 gate r2 改签成功。

原件封存至[持久证据](2026-11-16-native-network-evidence/README.md)，
共 1422 份、`64,975,404` 原始字节，gzip 后 `46,813,810` 字节逐项解压一致。
首轮导出因 helper 的字面标记误触自身扫描而退出，保留回执与旧 helper；
1418 个未完成导出先与原文核验后重建，原件未删，最终完整导出成功。
正式本地功能提交前已核对 Git index/blob，源码提交为
`6699148b0fd9b5455c69a2ae6b9187ecb1cc3633`；
[提交绑定](2026-11-16-native-network-evidence/commit-input-binding.json)及
[清理后绑定](2026-11-16-native-network-evidence/post-cleanup-binding.json)
核验 699 项工程输入与 1422 份原件、当前文件零 mismatch。
随后删除本批 1435 个临时文件，逻辑 `781,265,332` 字节；
[最终清理](2026-11-16-native-network-evidence/cleanup-final.json)
确认批次目录 absent、正常 GUI/MCP 摘要不变、唯一 target/共享 base image 保留，
作者 worktree 与匹配分支 absent，三个 agent 已关闭，没有新建用户 chat。
后续仅补文档回执，不重跑相同工程输入的门禁；没有 push。
原始自动流程失败、红回归、非作者 r1/r2、零 drop 失败及中断始终保留。
本批限定结论不代签三平台完整原生、完整输入法/辅助技术、签名发行、
生产性能或整个产品目标。
