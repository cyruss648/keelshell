# 开发路线 — 2026-11-08

完整产品目标保持进行中。精确SHA、执行会话和Cargo归属从[当前交接](HANDOFF.md)接续；
[历史路线](history/2026-11-06-roadmap-before-editor-traceroute.md)保留上轮冻结状态。

## 2026-11-08 工作区窗口切换器

工作区顶部新增主题化的“工作区 N”入口，持续显示当前原生窗口组数量；展开后列出
每个窗口在进程生命周期内稳定的序号、会话数和当前主机或“快速连接”空态。点击条目会激活对应真实
`AnyWindowHandle`，不会迁移 SSH、终端草稿、AI 审核或远程面板。新建窗口继续复用
既有配置闸门和八窗口上限。

定向 GPUI 回归已通过：窗口入口 `3/3`、全部窗口生命周期 `19/19`、modal/focus
`12/12`、工作区布局 `8/8`、主题 `9/9`；窗口列表覆盖中文/英文和 Frost「霜白」、Graphite「曜石」、
Lavender「雾紫」三套主题。当前修复还覆盖统一 modal scope、Dialog 语义、全屏输入
隔离、focus trap、Escape 关闭和工作区焦点恢复。详细设计见
[ADR 0123](adr/0123-workspace-window-switcher.md)，产品说明见
[工作区窗口切换器](product/WORKSPACE_WINDOW_SWITCHER.md)，测试证据见
[测试记录](testing/records/2026-11-08-workspace-window-switcher.md)。

本片的完整工程门禁需要在当前修复候选上重新执行；独立只读复核、当前源码精确包、真实 macOS
多窗口、Windows/Linux 原生窗口、IME、VoiceOver、真实 SSH/Podman 和外部 AI provider
仍待后续收口。

## 2026-11-08 本片交付状态

本片已经补齐连续无换行保存与
`merge-adopt → merge-save → late newer draft/readback` 组合回归；生产代码没有修改。
完整工程门禁第二轮 `python3 scripts/check.py` 退出 `0`：Python
`131 run / 1 skipped`，workspace Rust `2,437 passed / 23 ignored`，app
`1,115 passed / 2 ignored`，14 项 doctest、严格 Clippy、依赖策略和三个 AI
controller 均通过；`cargo fmt --all -- --check` 与 `git diff --check` 也退出 `0`。

第一轮完整门禁在 Python `131 run / 1 skipped` 后于格式阶段退出 `1`，原因是新增
断言未按 `cargo fmt` 展开；格式化后重新执行并通过。该失败事实和第二轮完整回执
已记录在[组合回归测试记录](testing/records/2026-11-08-editor-format-merge-combinations.md)。

源码实现已固定为
`9134cfaa771057a0f246ead47479be3706248415`。首次独立严格只读复核以该 SHA 为基线
返回 `REQUEST CHANGES`：P0/P1/P3 为 `0`，唯一 P2 是提交后的状态文案仍写成待提交；
本次文档修订已将固定 SHA、复核结果和剩余交付边界写实。后续复核确认该 P2 关闭后，
第二次独立复核已返回 `APPROVE`，P0/P1/P2/P3 均为 `0`。下一步生成绑定最终
文档提交 SHA 的精确包并执行有限 macOS smoke。Windows/Linux 原生、IME、VoiceOver、
真实 SSH/Podman 长流程和外部 AI provider 继续保持未验证边界。

## 2026-11-08 内联合并审核视口

内联三方合并审核已收口为固定 `220 px` 卡片：正文拥有独立滚动句柄，动作区固定为
两行，展开、采用和关闭动作保持在正文滚动之外。正文滚轮由带 `review_id` 的处理器
显式限幅并阻止外层文件工具区移动；冲突切换、方案选择和完整版本切换都会复位正文
滚动。旧 GPUI 帧 listener 在审核替换后失败关闭，不能改变新审核。

当前定向回归已通过：stale frame `1/1`、conflict merge `27/27`、review viewport
`12/12`、editor format `13/13`；格式和差异检查通过。随后按共享 target 规则失效并
重编五个项目包，完整 `python3 scripts/check.py` 退出 `0`：Python `131 run /
1 skipped`，workspace Rust 测试 `2,435 passed / 23 ignored`，app 为
`1,113 passed / 2 ignored`，14 项 doctest、依赖策略、严格 workspace Clippy 和
三个 AI controller 专项全部通过。首次视口回归的失败原因、首轮 Clippy 修复和完整
门禁证据已保存在[测试记录](testing/records/2026-11-08-inline-merge-review-viewport.md)；
设计见 [ADR 0121](adr/0121-inline-merge-review-viewport.md)。

本片源码提交、SHA 回读、exact package 和 macOS 主题/双语短验已完成；独立复核因
agent 未返回可审计报告而保持 `NON_AUTHOR_REVIEW_UNAVAILABLE`。连续无换行保存、
late newer draft/readback 组合、Windows/Linux 原生窗口、IME、VoiceOver、真实
SSH/Podman 和外部 AI provider 继续保持未验证边界。

## 2026-11-08 编辑器格式与合并迟到读回组合回归

新增两个真实 GPUI/SFTP 回归，分别覆盖无换行文件连续保存的 LF 策略保持，以及
合并采用后保存发布期间产生较新草稿时的完整读回和下一次保存基线重建：

- `editor_format_tests`：`14 passed / 0 failed / 1103 filtered out`；
- `conflict_merge_tests`：`29 passed / 0 failed / 1088 filtered out`；
- 两个新增用例精确运行均为 `1 passed / 0 failed / 1116 filtered out`；
- `rustfmt` 与 `git diff --check` 均退出 `0`。

本片没有修改生产代码；产品合同、设计决策和失败证据分别见
[远程文本格式](product/REMOTE_TEXT_FORMAT.md)、
[远程文本合并](product/TEXT_CONFLICT_MERGE.md)、
[ADR 0122](adr/0122-editor-format-and-late-readback-regressions.md) 和
[测试记录](testing/records/2026-11-08-editor-format-merge-combinations.md)。
首次接入时的闭包上下文编译错误和并行工作树覆盖测试文件的事实已保留，修复后
重新确认测试清单包含两个新增用例。完整工程门禁已经通过；独立复核、精确包和
原生验收仍待本片后续步骤。文档修订后的第二次独立复核已返回 `APPROVE`，
P0/P1/P2/P3 均为 `0`。

## 2026-11-08 快速操作中心

快速操作中心已完成 Workspace 生产布局和第一批动作接入：纯动作目录提供双语
标题、关键词、稳定排序、平台快捷键提示和会话上下文过滤；Workspace 复用既有
连接、会话、工具面板、批量命令、依赖工作流、AI 和设置入口。一次性快速连接、
主机监控面板路由和首末/唯一会话边界也已接入。模态根、搜索输入、结果列表和
选中状态共用三套主题的语义 token，新增主题只扩展 registry/Palette/token。

当前定向 GPUI 回归为 `16 passed / 0 failed / 1091 filtered out`，覆盖
`Command/Ctrl+K`、双语、Escape 焦点恢复、Enter 连接管理器路径、一次性快速连接、
主机监控路由、会话边界和 `900×580` 内容边界。随后完整
`python3 scripts/check.py` 已退出 `0`：依赖策略、Python `131 run / 1 skipped`、
格式、workspace 严格 Clippy、workspace tests、doctests 和 readiness/
observer/small-stack AI controller 均通过。workspace 测试中 app 单元为
`1105 passed / 2 ignored`，core、mcp、session 的单元与集成测试均通过；带有
Podman、OpenSSH、真实 Linux 采样等显式环境要求的可选测试继续保持 ignored。
源码提交
`c13724a9eab9df3d97fdb3ed1eaf25b5dcfabffa` 已推送，local、tracking 和 remote
SHA 已读回一致。新的独立严格只读复核已完成，P0/P1/P2/P3 均为 `0`，范围内建议为
`APPROVE`；
新建窗口、分屏、布局保存、原生多平台输入/辅助技术、真实 SSH/Podman 和外部
AI provider 保持后续验收项。当前只剩文档收口提交。设计见
[ADR 0120](adr/0120-command-palette.md)，实现说明见
[快速操作中心](product/COMMAND_PALETTE.md)，测试回执见
[快速操作中心记录](testing/records/2026-11-08-command-palette.md)。

## 2026-11-07 连接库选中层级与主动作

连接库视觉层级已完成一片源码候选并提交为
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9`。宽表格和窄卡片共用
selected-row 反馈：行公开 `aria-selected`，使用当前主题的 `surface_active`
背景和 `accent` 左边界，悬停使用 `surface_hover`。活动连接库中的 Connect
改为 GPUI Kit `primary`，编辑、移动、复制和回收站动作继续保持次级层级；回收站
仍不会暴露连接入口。视图继续只消费 registry/Palette token，没有主题 ID 分支，
未来新增主题只扩展注册定义。

定向连接库回归为 `27 passed / 0 failed`，selected-row 精确回归为
`1 passed / 0 failed`，应用严格 Clippy、格式和差异检查通过。完整
`python3 scripts/check.py` 退出 `0`，实际完成 Python `131 run / 1 skipped`、
app `1089 passed / 2 ignored`、workspace 其它测试、doctests、依赖策略和三组 AI
controller。首次新增测试曾把可访问标签误当可见文本而失败，已保留失败事实并
精确重跑通过；随后还收敛了 selectable role 与选中行悬停状态。记录见
[连接库视觉层级回执](testing/records/2026-11-07-connection-library-visual-hierarchy.md)，
设计决策见[ADR 0119](adr/0119-connection-library-visual-hierarchy.md)。

独立严格只读复核已以最终源码提交
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9` 为基线完成，P0/P1/P2/P3 均为
`0`，merge recommendation 为 `APPROVE`；复核实际执行 selected-row 精确回归
`1 passed / 0 failed / 1090 filtered out`。此前包不包含最终提交
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9`，不能代签本片真实
桌面颜色。macOS 当前源码包、Windows/Linux 原生窗口、完整键盘/IME/VoiceOver、
真实 SSH/Podman 长流程和外部 AI provider 继续保持独立验收边界。

## 2026-11-07 exact package smoke 与复核边界

当前主题选择器源码提交 `37b8eaf7d65fe31133c5e5058848341c50c8a820` 已重新构建
精确 macOS 开发包 `work/native-cua-20261107/package-r10/KeelShell.app`，
结构检查退出 0，manifest 绑定该 SHA。解锁桌面短验确认 Frost、Graphite、
Lavender 三套主题可以切换，System/Light/Dark 控件和 AX 文案可读，空连接库返回
正常；exact package 进程已清理。

本次仅覆盖主题弹窗和空工作区短流程，未覆盖真实 SSH、分屏、900×580、IME、
VoiceOver、签名/安装和 Windows/Linux。针对 exact source 的三次新独立复核尝试
均未在有界等待内返回正文，当前状态为 `NON_AUTHOR_REVIEW_UNAVAILABLE`；不能把
本片标成非作者批准。详细包和 smoke 证据见
[主题选择器记录](testing/records/2026-11-07-theme-picker-layout.md)。

## 2026-11-07 主题选择器可见性与交互回归

主题选择器已完成一片可验收的视觉和布局修复：主题卡片目录独立滚动，明暗模式
控制固定在弹窗可见区，三套内置主题 Frost「霜白」、Graphite「曜石」、
Lavender「雾紫」均可选择。预览和按钮继续由主题 registry 与语义 token 驱动，
以后新增主题只扩展注册定义，不增加业务视图分支。

主题模块定向回归为 9/9，磁盘监控主题联动为 1/1，格式、差异检查和应用严格
Clippy 通过。完整 `python3 scripts/check.py` 退出 0，Python 为 131 run / 1
skipped，完整 app 测试为 1088 passed / 2 ignored；其余 workspace tests、
doctests、依赖策略和三组 AI controller 也通过。详细证据见
[主题选择器记录](testing/records/2026-11-07-theme-picker-layout.md)。

本片仍不代签 macOS/Windows/Linux 原生窗口、900×580 accessibility frame、
IME、VoiceOver、安装包和真实外部 provider；这些范围继续按平台记录单独验收。

## 2026-11-07 Reef / reef-template 参考边界

状态：**文档切片已完成，可独立合并**。

新增 [ADR 0118](adr/0118-reef-reference-boundary.md)，记录本轮对指定 Reef
checkout（`75ccbf5d864fadb543ecfa62fc9871c7cff04153`）和 reef-template checkout
（`03bd71c096fdd941e389584c44bdcb40dfed0cdf`）的只读检查。证据覆盖：

- Reef 的 resolver 3、workspace package/依赖集中声明、`reef-compose` feature
  装配、`application` 的 owner/启动失败回滚/join/shutdown report，以及
  Core 的配置 loader 和 revision snapshot；
- reef-template 的 workspace 继承、单一 `crates/app`、公共/dev/release 配置、
  `build.rs`/入口、模板生成限制和工程同步文档；
- Reef checkout 检查时存在一处预先存在的 `Cargo.toml` 修改，模板 checkout
  干净；两者都没有被本轮修改或清理。

后续 KeelShell 只采用这些工程原则，继续保持 `keelshell-app`、
`keelshell-core`、`keelshell-session`、`keelshell-ai` 的桌面边界。服务端
runtime、私有 Git 依赖、内网 URL、HTTP/MQ/DB/telemetry 等 Reef 中间件、
模板 Liquid 和服务部署约定均不进入桌面产品。若未来需要真实 Reef 接入，先
创建新的 ADR，补齐公开依赖、版本 revision、配置路径、生命周期和跨平台验收。

## 2026-11-07 会话、连接库与 AI 助手界面增强

本片把连接库范围反馈、会话条键盘导航和 AI 审核阶段统一到可扩展的工作台表面：
连接库区分范围与空态，会话条显示数量并提供平台快捷键，AI 助手显示“捕获、提问、
预览、发送、审核回复、保存知识”六阶段。普通回复到达后自动显露审核区，长请求
仍在正文滚动区保留完整预览。

定向结果为连接库 26/26、AI 6/6、会话条 5/5、快捷键 1/1，长 MCP、Agent 编辑
问题和知识保存回归各 1/1，`cargo check`、格式和差异检查通过。首轮 AI 回复可达性
测试失败后补充响应锚点，精确重跑 2/2；Agent 编辑问题曾暴露旧布局滚动，最终在
render 后按实际几何修复。完整工程门禁 `python3 scripts/check.py` 已退出 0；
初始精确提交 `dd6c88b0710bb56ca785eeeb83ee0835a1b77ede` 已推送；随后窄栏修正
`58fd2d248302958675b0652b7bc15b8aee77b580` 也已推送，新的独立严格只读复核
返回 P0/P1/P2/P3 均为 0 并建议 `APPROVE`。当前源码的 `package-r7` 已按共享
target 规则重新构建、精确绑定并通过 macOS 静态结构检查；解锁后的短范围 smoke
确认六段工作流完整换行，AI 助手侧栏和 AX 元素可见。完整原生桌面证据仍待执行。设计见
[ADR 0117](adr/0117-session-navigation-and-assistant-workflow-surface.md)，
回执见[本轮记录](testing/records/2026-11-07-session-library-ai-ui-polish.md)。

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

## 2026-11-07 工具栏远程上下文布局修复

工作区顶部工具栏已修复紧凑窗口中远程上下文逐字竖排的问题，并收口 1280px
断点：1280px 保留品牌区但采用紧凑动作标签，低于 1280px 隐藏品牌区；上下文
标题和当前主机值使用单行省略，紧凑布局保留 112px 最小宽度。布局回归覆盖
900/1280/1440 三档、中英文、三主题矩阵中的既有工作区场景，完整 app 为
1087 passed / 2 ignored，完整 `scripts/check.py` 退出 0。

源码提交为 `2d05525c00d21455420d25cce675bacdedf8a73a`。当前 macOS 包
`work/native-cua-20261107/package-r9/KeelShell.app` 已做结构检查和默认窗口短验；
紧凑尺寸因 GPUI accessibility frame 限制仍未形成原生通过证据。当前 exact tip
的独立只读复核已返回 P0/P1/P2/P3 均为 0、范围内 `APPROVE`；该复核没有代替
作者完整工程门禁或原生验收。Windows/Linux 原生桌面、完整输入/辅助技术矩阵、
当前包真实 SSH/Podman 长流程和外部 AI provider 仍开放。
测试证据见[工具栏上下文布局修复记录](testing/records/2026-11-07-toolbar-context-layout-repair.md)。

## 当前并行开发

| 优先级 | 用户功能 | 真实状态与下一步 |
| --- | --- | --- |
| P1 | [远程文本格式](product/REMOTE_TEXT_FORMAT.md) | r3三项连续编辑P2关闭；新增无换行连续保存 GPUI/SFTP 回归 `14/14`；父543输入门禁/包通过；native-r1限定批准macOS原生BOM/CRLF取消与确认，完整编辑器验收HOLD |
| P2 | [审核式有界traceroute](product/TRACEROUTE_DIAGNOSTICS.md) | stderr精确版本合同与最终门禁通过；同包真实SSH UI各审核确认UDPv4/v6一次，均一跳回复；原生ICMP/取消和完整平台另验 |
| P1 | 独立工作窗口 | 源码已导入当前分支；空白新窗、各窗独立会话/审核、安全关闭、共享 StateStore 协调、worker 回收与 loopback SSH 定向测试通过；完整工程门禁通过，独立复核和真实多窗口验收待完成 |
| P1 | 编辑器动作可达性 | inline editor 与三方合并正文独立滚动、底部动作固定、手工输入最小高度修复已导入；视口专项和本片 `merge-adopt` / `merge-save` / late newer 回归通过；完整工程门禁、独立复核和真实原生复验待完成 |

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
