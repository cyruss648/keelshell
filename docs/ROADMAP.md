# 开发路线 — 2026-11-07

完整产品目标保持进行中。精确SHA、执行会话和Cargo归属从[当前交接](HANDOFF.md)接续；
[历史路线](history/2026-11-06-roadmap-before-editor-traceroute.md)保留上轮冻结状态。

## 2026-11-07 当前交付位置

当前候选已完成 pane 连接身份栏的源码、测试和文档收口准备。连接身份栏覆盖
真实 SSH 端点、协议/编码、生命周期状态、恢复占位和双语 AX；live IPv6 端点
已统一以括号包裹主机；未保存连接与配置缺失分别呈现，主题字体通过 registry
typography token 提供。三套内置主题 Frost、Graphite、Lavender 仍全部可选，
新增主题只扩展主题定义，不让业务视图按主题 ID 分支。

定向 GPUI 回归与完整工程门禁通过：Python 131/1 skip，app 1079/2 ignored，
core 197，session 434/16 ignored，workspace integration、MCP、SSH loopback、
transfer、doctest、readiness、observer、small-stack AI controller、严格
workspace Clippy、格式和差异检查均通过。首轮完整门禁曾因新增测试的
`needless_borrow` 失败，修复后的结果已重新执行并记录。

route/split 夹具在完整 app 并行负载下的首次 10 秒就绪等待曾早于生产 15 秒
SSH 操作期限结束；gateway、target 和跳板 SSH 会话当时仍存活。当前测试使用
有界 20 秒 UI 就绪等待，route server session 使用有界 120 秒上限，最终 app
单线程与并行重跑均为 1079 passed / 2 ignored。session 曾出现一次
`traceroute_hop_limit` 预期 hop-limit、实际 timeout 的负载抖动
（129 passed / 1 failed / 4 ignored），精确重跑、四线程重跑和最终 workspace
门禁通过，失败事实保留在测试记录中。

源码提交 `b2f875458e42cf374cb3d388d808e986a415c2cd` 已通过完整工程门禁并生成
精确来源绑定的 `package-r5`；旧 `package-r3` 继续承担详细 macOS 主题、双语、
横/纵分屏、合并、关闭面板、真实回环 SSH、连接中断和恢复场景人工观察。当前
`package-r5` 已完成结构检查和短范围主题/双语 smoke check。900×580 紧凑窗口因
无边框窗口 accessibility frame 异常保持未完成。下一步按交付顺序执行：固定
`b2f8754` 重新进行独立严格只读复核；本轮已启动两次 exact-tip 复核，但均未在
有界等待内形成可审计正文，状态保持 `NON_AUTHOR_REVIEW_UNAVAILABLE`。后续仍需
补一次有正文的复核，再更新合并建议。Windows/Linux 原生桌面、完整 IME/VoiceOver
和 Podman 真实联调仍保持开放。

上一片工作区视觉节奏收口已提交并推送，提交为
`e3226e7097b72f6927fbc38f05a94a365d8269d7`，本地、tracking 和远端 SHA 一致。
它包含主题 registry 的 chrome 几何 token、宽窗品牌与当前主机上下文、命令审核
目标、primary 执行动作和会话标签的主题化 hover/active 状态。

当前候选继续推进 pane 连接身份栏：所有终端 pane 在画布上方持续显示真实连接
主机、用户/端口、SSH/编码和连接状态，分屏时与会话标签和比例控制共享紧凑
header。实现只消费 Workspace 已有的连接/生命周期快照，不把传输状态塞进
TerminalView，并补齐中英文、三主题、最小窗口和焦点/AX 回归。设计和验证见
[ADR 0116](adr/0116-pane-connection-identity-bar.md) 与
[测试记录](testing/records/2026-11-07-pane-connection-identity-bar.md)。

当前身份栏和 IPv6 endpoint 修复的定向回归、完整 `python3 scripts/check.py`、
`package-r5` 结构检查及短范围 macOS smoke check 已通过；详细 SSH native 观察仍
归属于旧 `package-r3`，最新源码提交的独立非作者复核仍待完成。

旧固定提交的证据绑定问题已由 `9d02440` 及后续文档修订关闭；旧复核指出的
IPv6 live endpoint 问题已由 `b2f8754` 修复，当前包 provenance 已由 `package-r5`
精确 manifest 绑定。当前 exact-tip 复核仍是 `NON_AUTHOR_REVIEW_UNAVAILABLE`，
不能视为当前提交批准；后续必须固定 `b2f8754` 补一次有可审计正文的代码/功能
复核，再更新合并建议。

`e3226e7097b72f6927fbc38f05a94a365d8269d7` 已推送到
`feature/main-visual-integration-20261106`，本地、tracking 和远端 SHA 一致；
`main` 尚未推进。三组继续回归分别为 5/5、14/14 和 11/11 通过。独立非作者复核
仍因 agent 无报告、供应商 API-Key 403 及本地 agent 工具循环而不可用；真实 macOS
桌面通道仍报告锁屏，因此多窗口和编辑器原生操作保持 `NOT_RUN`。
文档提交后的首次完整门禁曾在 small-stack controller 收到 `EmptyReply` 退出 101；
精确重跑通过，第二次完整 `python3 scripts/check.py` 退出 0，保留首次失败证据。

## 当前集成

`feature/main-visual-integration-20261106` 已把 `main` 的最新功能基线与工作区
外壳视觉增量合并。集成后的 UI 使用 Frost、Graphite、Lavender 三套注册表主题和
语义 surface/chrome 层级，活动会话、连接导航、工具栏和状态栏共享同一套可扩展
组件 token。视觉来源的定向门禁、集成后的完整工程门禁和 design/workbench 定向回归
已经通过；独立非作者代码复核固定在源码提交 `26e11c5`，文档编号清理由 `ecdc8c1`
收口并已推送。开发包、原生桌面和跨平台运行仍需单独形成新的可交付证据。新主题
继续只扩展注册表、Palette 和组件形状，业务视图不增加主题 ID 分支。

复核发现监控文件系统容量条仍有硬编码颜色 `0x448eaa`。修复提交 `ef110d2` 增加
独立的 `Palette.resource`，六个主题变体均提供资源色，监控视图按当前主题读取该
token，并以轨道对比度测试防止回退到固定颜色。修后设计 4/4、磁盘相关 10/10、
独立主题回归 1/1、严格 Clippy 和完整 `scripts/check.py` 均通过；新的非作者只读
复核任务在有界等待内未返回，备用模型因 API-Key 权限返回 403；父级固定提交只读
核对未发现新的 P0/P1/P2/P3。当前采用
`LIMITED_PARENT_REVIEW / APPROVE_SCOPED_WITH_REVIEW_SERVICE_LIMITATION`，保留独立
子代理复核服务未形成报告的限制，不把它写成已获得非作者批准。

2026-11-07 的独立工作窗口与编辑器动作可达性候选已导入当前分支。新增实现通过
定向窗口、配置闸门、worker ownership、loopback SSH、编辑器视口和 11 项
临时目录根路径回归；完整 `scripts/check.py` 实际退出 0：workspace 2,397
ordinary / 23 ignored、11 rustdoc、Python 131 / 1 skipped，三组 AI controller
也通过。当前仍需新的严格只读独立复核和真实 macOS 多窗口/编辑器采用保存验收，
Windows/Linux 原生桌面验收继续单独保持开放。
两次新开的有界只读复核 agent 都未返回可审计报告，不能把当前候选标记为独立
批准；父级静态差分检查已修正新增的 worker stderr 输出，但不代替非作者复核。

## 最新交付

main已普通推送`fa7c05914e0eefc0f24ab32f8af690bdc50a63ed`，交付读回一致。
本批按项监控能力和审核式知识分类的536输入fresh完整门禁、开发包、
限定macOS A/B原件及正式文档均获各自非作者批准。
2313 ordinary/23 ignored、14 rustdoc、Python131/1 skip；28对原生AX/JPEG，
首轮HOLD/UNKNOWN保持，完整native finish开放。精确三平台CI均已成功；
唯一观察器exit2已实际wait/reaped；原Linux附件GET失败保留，新单次补充读回与
两平台各11项OpenSSH获独立`current-ci-r2`限定收口批准。三平台各204候选实际通过。
CI不代签桌面验收。见[本批记录](testing/records/2026-11-06-monitor-knowledge-categories.md)。

## 当前并行开发

| 优先级 | 用户功能 | 真实状态与下一步 |
| --- | --- | --- |
| P1 | [远程文本格式](product/REMOTE_TEXT_FORMAT.md) | r3三项连续编辑P2关闭；父543输入门禁/包通过；native-r1限定批准macOS原生BOM/CRLF取消与确认，完整编辑器验收HOLD |
| P2 | [审核式有界traceroute](product/TRACEROUTE_DIAGNOSTICS.md) | stderr精确版本合同与最终门禁通过；同包真实SSH UI各审核确认UDPv4/v6一次，均一跳回复；原生ICMP/取消和完整平台另验 |
| P1 | 独立工作窗口 | 源码已导入当前分支；空白新窗、各窗独立会话/审核、安全关闭、共享 StateStore 协调、worker 回收与 loopback SSH 定向测试通过；完整工程门禁通过，独立复核和真实多窗口验收待完成 |
| P1 | 编辑器动作可达性 | inline editor 与三方合并正文独立滚动、底部动作固定、手工输入最小高度修复已导入；11 项视口专项和完整工程门禁通过，`merge-adopt` / `merge-save`、late-save、连续无换行保存的真实原生复验待完成 |

当前候选已装配本轮源码和文档输入，五个项目 crate 已按共享 target 规则失效重编，
42 个新增具体测试名均在当前库存中。修复 macOS 临时目录根路径夹具后，完整工程
门禁实际退出 0：2,397 ordinary / 23 ignored、11 rustdoc、Python 131 / 1 skip，
三组 AI controller 和 workspace strict Clippy 均通过。
新包构建与macOS包检查完成，全部成员和41项资产已读回；
Gauss的combined-r3/r4已批准限定源码及门禁/新包；
新runtime-r3原生797.330164/900秒结束、外层28510实际wait0/reaped，
38组AX/JPEG及完整字节原件获native-r1限定批准；
新增P2 NR1-P2-01与完整native HOLD保持，下一片修复。
源代码通过、实际工具、原生、CI和发布安装分别验收，见[组合记录](testing/records/2026-11-06-editor-traceroute-integration.md)。

## 后续产品缺口

| 优先级 | 范围 | 当前边界与下一步 |
| --- | --- | --- |
| P1 | 工作区 | 已有分屏、比例、逐叶标签、显式保存和单窗恢复；多OS窗口、外框及完整关闭/重连审核矩阵仍开放 |
| P1 | 平台与发行 | 继续Windows/Linux真实桌面、完整几何/IME/辅助技术、六目标签名/公证和隔离安装回退；三平台CI不等于桌面验收 |
| P1 | 外部AI集成 | API/受控CLI Ask、逐步审核Agent及对外MCP已有实现；供应商账户/完整外部客户端、CLI登录身份复用的请求前隔离证明仍待验 |
| P2 | 日常文件操作 | 继续原生文件管理器拖放、更多文本编码、完整键盘/辅助技术及其他平台；现有字节与备份合同不扩大 |
| P2 | 监控与网络 | 按项能力与缓存已交付；继续df/ps缺失后恢复、ss可用、真实诊断错误/取消，以及远端macOS/Windows适配 |
| P2 | 持久化隧道 | 命名规则、三方向转发、流量与显式审核/停止已有实现；重启后的运行态恢复与重连owner完整矩阵开放 |
| P2 | 配置恢复/同步 | 已有审核恢复、回退与认证加密目录同步；跨机器挂载、断电耐久性及最终一致同步器待验 |
| P3 | 高级传输 | 审核式归档/路径穿越防护、rz/sz实机、用户中继与可重复收益验收另行实施 |
| P3 | AI知识复用 | 本地分类、显式Ask和审核式批量管理已交付；跨设备同步及更多复用待设计，不恢复旧SSH权限或自动执行runbook |

清理遵循资源所有权。已合并分支、无消费者自有容器/镜像及时清理；
受其他任务或primary保护的工作树保留，失败/UNKNOWN与不可变原件不删改。
产品能力从[能力清单](product/CAPABILITIES.md)及对应指南查询。
