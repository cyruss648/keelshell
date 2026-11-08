# KeelShell 当前交接 — 2026-11-08

先读[AGENTS](../AGENTS.md)、本页、[路线图](ROADMAP.md)及
[共享 Cargo 指南](testing/SHARED_TARGET.md)。时间使用 Asia/Shanghai，
原始回执 UTC 保持。上一阶段完整过程保存在
[原生收口前交接](history/2026-11-06-handoff-before-editor-native-closeout.md)。

## 2026-11-08 当前片：工作区窗口切换器

当前源码候选在既有多窗口生命周期上增加窗口切换器。`AppWindows` 注册表为
每个窗口保存轻量 `WindowSummary`，`Workspace` 顶部显示窗口数量，展开列表显示
会话数和当前主机/快速连接空态，点击后激活真实原生窗口。窗口摘要不共享 SSH、
终端、文件、AI 或远程面板实体；新建窗口仍经过原配置闸门、维护状态检查和八窗口
上限。窗口序号由进程生命周期内的单调计数分配，关闭前置窗口后保留后续窗口序号。

源码已固定提交为
`373d4157ae816a29a74aa72cf490f688e3563457`。定向结果已通过：窗口入口 `3/3`、
全部窗口生命周期 `19/19`、modal/focus `12/12`、工作区布局 `8/8`、
主题 `9/9`；新增中文/英文与 Frost「霜白」、Graphite「曜石」、Lavender「雾紫」
组合回归，英文无障碍标签使用英文标点。当前未提交修复还将窗口列表接入统一
modal scope，增加 Dialog 语义、全屏输入隔离、focus trap、Escape 关闭和工作区焦点
恢复。设计、产品和测试记录分别见
[ADR 0123](adr/0123-workspace-window-switcher.md)、
[工作区窗口切换器](product/WORKSPACE_WINDOW_SWITCHER.md) 和
[测试记录](testing/records/2026-11-08-workspace-window-switcher.md)。

当前定向结果使用重新编译的 `1,121` 项 app 测试清单。源码提交后的完整
`python3 scripts/check.py` 已使用命令会话 `63031` 退出 `0`：Python
`131 run / 1 skipped`，app `1,119 passed / 2 ignored`，workspace 其余 Rust
测试、14 项 doctest、严格 Clippy、依赖策略和 readiness/observer/small-stack 三组
AI controller 均通过；`__eh_frame section too large` 仍只是链接器 warning。
当前正在以该 SHA 做独立严格只读复核；复核通过后重新构建绑定当前 SHA 的 macOS 包
并执行有限原生多窗口观察。旧 `package-r4` 只代签主题/语言短验，不代签本片新增
窗口切换器。

## 2026-11-08 当前片：编辑器格式与合并迟到读回组合回归收口

本片在源码基线
`1d0898025bcd714550de8d702d5c8026242844bf`
上新增两个真实 GPUI/SFTP 组合回归，生产代码没有修改：

- `independent_editor_format_no_newline_keeps_lf_policy_across_empty_and_plain_saves`
  验证无换行远端文件首次插入 LF 后，连续保存带换行、空内容、再次无换行和后续
  带换行内容仍保留 LF 插入策略，不增加 BOM 或 CRLF；
- `merge_adopt_save_keeps_late_newer_draft_and_rebases_next_save` 验证合并采用、完整
  保存审核、发布期间较新草稿、成功目标读回、新编辑基线和下一次 CRLF 保存的
  完整时序。

完整工程门禁第二轮 `python3 scripts/check.py` 已退出 `0`：

- Python：`131 run / 1 skipped`；
- workspace Rust：`2,437 passed / 23 ignored`；
- `keelshell-app`：`1,115 passed / 2 ignored`；
- 14 项 doctest、依赖版本策略、严格 workspace Clippy 和 readiness/observer/
  small-stack AI controller 全部通过；
- `cargo fmt --all -- --check` 与 `git diff --check` 均退出 `0`。

第一轮完整门禁已在 Python `131 run / 1 skipped` 后于格式阶段退出 `1`，原因是新增
断言尚未按 `cargo fmt` 展开；格式化后重新启动完整门禁并通过。失败事实和最终回执
保存在[组合回归测试记录](testing/records/2026-11-08-editor-format-merge-combinations.md)。

源码实现已固定提交为
`9134cfaa771057a0f246ead47479be3706248415`。独立严格只读复核已以该 SHA 为基线
完成两轮检查：首轮发现并由本次文档修订关闭一个 P2 状态文案问题；第二轮已返回
`APPROVE`，P0/P1/P2/P3 均为 `0`。精确 macOS 包、local/tracking/remote SHA
回读、Windows/Linux 原生窗口、IME、VoiceOver、真实 SSH/Podman 长流程和外部 AI
provider 仍未验收；既有旧包不能代签本片新增测试。

## 2026-11-08 本片源码与 macOS 包收口

本片源码提交为 `e5891d1c8101c00944979ec2761718d376ae60d5`，已推送到
`feature/main-visual-integration-20261106`；local、tracking、remote 三者已读回
一致。完整工程门禁已在该源码候选上通过：Python `131 run / 1 skipped`，
workspace Rust `2,435 passed / 23 ignored`，app `1,113 passed / 2 ignored`，
14 项 doctest、依赖策略、严格 Clippy 和三个 AI controller 均通过。

当前 exact package 为
`work/native-cua-20261107/package-r11/KeelShell.app`，manifest 的
`build.commit` 绑定上述源码 SHA，GUI/MCP 为 arm64 Mach-O，实际最低版本为
macOS `15.0`。manifest、GUI、MCP SHA-256 分别为：

- `54ed0e165a27c760f4672673bbde16d3572efaff815b50b6b7c629927b49f8d6`
- `c6afd91a3cdb219a3974c92b90f0d2310d70c16a45ce1efc068bffe8e22cc139`
- `94eaf83f8bbb23ae4db7cc190c9c06332ddbc19b4bf9246dcdf35f43b3645d8f`

已解锁 macOS 桌面短验确认默认工作台、连接库空态、三套主题 Frost「霜白」、
Graphite「曜石」、Lavender「雾紫」和中英文切换可见；测试使用隔离数据目录，
没有安装或替换已有应用，进程与隔离数据已回收。

源码提交后的两次独立只读复核尝试均未返回可审计正文：第一次重复轮询旧会话后
被系统停止，第二次隔离 agent 在中断请求后仍无报告。当前必须记录
`NON_AUTHOR_REVIEW_UNAVAILABLE`，不能把作者门禁或原生短验写成非作者批准。

## 2026-11-08 内联合并审核视口候选

当前分支 `feature/main-visual-integration-20261106` 的源码基线为
`ed75c9802509b54f4c57dc38f8573fe0f43aeee1`，本片的四个源码变更文件
`files/merge.rs`、`files/review_viewport_tests.rs`、`files/transfer_tests.rs` 和
`files/conflict_merge_tests.rs` 已随源码提交
`e5891d1c8101c00944979ec2761718d376ae60d5` 收口。它们将内联三方合并审核收口为固定
`220 px` 卡片；
正文独立滚动，动作区两行固定，展开按钮移入动作栏。正文滚轮通过 review ID 绑定的
处理器显式更新并截断 `body_scroll`，旧帧和新审核之间失败关闭；切换冲突、完整版本
和冲突选择方案会复位正文滚动。

截至本交接片，专项结果为：

- stale replacement frame：`1 passed / 0 failed / 1114 filtered out`；
- conflict merge：`27 passed / 0 failed / 1088 filtered out`；
- review viewport：`12 passed / 0 failed / 1103 filtered out`；
- editor format：`13 passed / 0 failed / 1102 filtered out`；
- `cargo fmt --all -- --check` 与 `git diff --check`：退出 `0`。

本片随后按[共享 target 指南](testing/SHARED_TARGET.md)失效五个项目包并完成真实
重编。第一次重编在严格 Clippy 阶段发现并修复了未使用构造器、测试专用清理函数、
`then_some` 风格和未使用测试参数；最终 `python3 scripts/check.py` 退出 `0`：
Python `131 run / 1 skipped`，workspace Rust 测试 `2,435 passed / 23 ignored`，
其中 app 为 `1,113 passed / 2 ignored`，14 项 doctest、严格 Clippy、依赖策略和
readiness/observer/small-stack controller 全部通过。完整证据保存在[本片测试记录]
(testing/records/2026-11-08-inline-merge-review-viewport.md)；设计决策见
[ADR 0121](adr/0121-inline-merge-review-viewport.md)。首次视口回归曾因外层尾部裁掉
正文命中区域、并因 stop propagation 阻止正文默认滚动而失败，修复后精确重跑通过。

后续工作：按共享 target 规则重新执行完整工程门禁，固定本片源码并取得可审计的
独立严格只读复核，再生成绑定新源码的精确包。Windows/Linux 原生窗口、IME、
VoiceOver、真实 SSH/Podman 长流程和外部 AI provider 仍未验收。

## 2026-11-08 编辑器格式与合并迟到读回组合回归

源码实现已固定在
`9134cfaa771057a0f246ead47479be3706248415`，本片包含两个真实 GPUI/SFTP 回归：

- `independent_editor_format_no_newline_keeps_lf_policy_across_empty_and_plain_saves`
  从无换行远端文件开始，验证首次 Enter 使用 LF，连续保存空内容和无换行内容后仍
  保留 LF，后续编辑不增加 BOM 或 CRLF，并核对四份原始备份；
- `merge_adopt_save_keeps_late_newer_draft_and_rebases_next_save` 验证读取合并、
  Adopt、完整 Save review、发布期间较新草稿、成功读回、新基线和下一次保存的
  精确 CRLF 字节。

定向结果为：编辑器格式 `14 passed / 0 failed / 1103 filtered out`，冲突合并
`29 passed / 0 failed / 1088 filtered out`，两个新增用例精确运行均为
`1 passed / 0 failed / 1116 filtered out`。格式、差异检查已通过。生产代码没有
修改；产品、ADR 和失败证据见
[组合回归测试记录](testing/records/2026-11-08-editor-format-merge-combinations.md)
和 [ADR 0122](adr/0122-editor-format-and-late-readback-regressions.md)。

首次接入的测试上下文编译错误已修复，并重新确认新增用例出现在当前测试清单中。
并行切片曾覆盖第一个测试文件的未提交内容，已按返回差分重新应用并重跑；这一
事实保留在测试记录中。第二轮完整工程门禁已通过；首次独立复核的唯一 P2 是提交
后状态文案未同步，本次文档修订已补齐固定 SHA 和复核状态。精确新包和原生验收仍
按上方当前交接顺序待执行。文档修订后的第二次独立复核已确认 P2 关闭并给出
`APPROVE`。

## 2026-11-08 快速操作中心生产布局与键盘收口

本片源码已提交为
`c13724a9eab9df3d97fdb3ed1eaf25b5dcfabffa`，并推送到
`origin/feature/main-visual-integration-20261106`；源码提交的 local、tracking、
remote SHA 已读回一致。该提交继续完成 Workspace 快速操作中心。动作目录仍由纯
Rust 元数据、双语标题、
关键词、平台快捷键和会话上下文过滤组成；Workspace 只负责模态生命周期与已有
动作映射。当前生产布局已收敛为单一真实根节点：`command-palette-modal` 覆盖
模态内容区，`command-palette-card` 承载搜索、结果滚动区和计数栏。结果行有稳定
尺寸、列表语义、选中状态和主题 token，新增主题仍只扩展 registry/Palette/token。

`Command/Ctrl+K` 已接入现有 Workspace action binding；输入框、方向键、Enter、
鼠标点击和 Escape 共用现有 modal/focus 机制。Enter 通过模态根的捕获键盘路径
触发现有 Workspace 流程，关闭后恢复工作台焦点。快速连接现在进入真正的一次性
连接模态，主机监控会选择 Monitor 导航面板；快速操作中心不直接执行 SSH、文件、
AI provider 或持久化写入。

该源码提交的定向 GPUI 回归为 `16 passed / 0 failed / 1091 filtered out`，
覆盖纯动作目录、双语、焦点恢复、连接管理器 Enter 路由、一次性快速连接、主机
监控路由、首末会话边界、平台快捷键和 `900×580` 卡片/结果区边界。此前失败是
关闭后对已卸载元素调用 `find`，已改为 GPUI 的 `try_find(...).is_none()` 语义；
补充主机监控回归时还曾因测试夹具没有挂载 `monitor-scroll` 节点失败，随后改为
验证稳定的生产 `navigation-monitor` 语义。测试专用渲染分支和诊断 `eprintln!`
已删除。随后完整
`python3 scripts/check.py` 已退出 `0`：依赖策略、Python `131 run / 1 skipped`、
格式、workspace 严格 Clippy、workspace tests、doctests 和 readiness/
observer/small-stack AI controller 均通过。workspace app 单元测试为
`1105 passed / 2 ignored`，core、mcp、session 的单元与集成测试均通过；显式依赖
Podman、OpenSSH 或真实 Linux 采样的可选测试仍按边界保持 ignored。当前只剩固定
源码交付已完成；新的原生多平台、真实 SSH/Podman 与外部
AI provider 验收不由本片 headless 门禁代签。

新的独立严格只读复核已针对该源码提交完成，P0/P1/P2/P3 均为 `0`，范围内
merge recommendation 为 `APPROVE`；复核工作树保持未修改。文档收口提交会继续
保留本片完整门禁、精确重跑和源码 SHA 证据。

本次完整门禁首轮曾在 Python 阶段因 4 个既有
`test_windows_native_crash.PreparationTreeTests` 用例的 `124` 超时退出而退出
`1`；精确重跑这 4 项为 `4 passed / 0 failed`，随后完整门禁再次执行并退出 `0`。
该失败与精确复跑均保留在[测试记录](testing/records/2026-11-08-command-palette.md)中。

当前边界：macOS/Windows/Linux 原生窗口、完整 IME/VoiceOver、真实 SSH/Podman
长流程、外部 AI provider 和当前源码包验收仍不由 headless GPUI 回归代签。新建
窗口、分屏和布局保存暂时继续使用工具栏入口，后续加入动作目录只需扩展纯动作
注册和 Workspace 映射。详细记录见[快速操作中心测试记录](testing/records/2026-11-08-command-palette.md)、
[产品说明](product/COMMAND_PALETTE.md)和[ADR 0120](adr/0120-command-palette.md)。

## 2026-11-07 连接库视觉层级与主动作候选

本片最终源码已提交为
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9`，当前连接库行在宽表格和窄卡片中统一表达
selected 状态：行公开 `aria-selected`，使用 `Palette.surface_active` 和
`Palette.accent` 左边界，非选中悬停使用 `surface_hover`。活动连接库中的
Connect 使用 GPUI Kit `primary` 变体，Edit/Move/Copy/Trash 保持次级动作；
回收站继续只提供 Restore 和永久删除。没有增加主题 ID 分支，Frost「霜白」、
Graphite「曜石」、Lavender「雾紫」继续通过 registry/Palette/token 驱动，新增
主题只扩展注册定义。

验证已完成：连接库定向 `27 passed / 0 failed`，selected-row 精确回归
`1 passed / 0 failed`，app 严格 Clippy、格式和差异检查通过；完整
`python3 scripts/check.py` 退出 `0`，Python `131 run / 1 skipped`、
app `1089 passed / 2 ignored`，workspace 其它测试、doctests、依赖策略和三组
AI controller 通过。首次新增测试错误地把可访问标签当成可见文本的失败已保留，
并已修正为验证可访问标签与语义状态；最终提交还收敛了 selectable role 和选中
行悬停状态。设计见[ADR 0119](adr/0119-connection-library-visual-hierarchy.md)，
回执见[连接库视觉层级记录](testing/records/2026-11-07-connection-library-visual-hierarchy.md)。

新的独立严格只读复核已经以
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9` 为代码基线完成，P0/P1/P2/P3 均为
`0`，merge recommendation 为 `APPROVE`；复核实际执行 selected-row 精确回归
`1 passed / 0 failed / 1090 filtered out`，并核对了行状态、原生 selectable
语义、宽窄布局复用、主动作层级和主题 token 解耦。
当前包不包含 `48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9`，因此不能把既有 macOS 包写成本片原生通过；当前源码
macOS 包、Windows/Linux 原生窗口、完整键盘/IME/VoiceOver、真实 SSH/Podman
长流程和外部 AI provider 仍待独立验收。

## 2026-11-07 主题选择器 exact package 与复核状态

主题选择器源码已提交并推送到
`feature/main-visual-integration-20261106`，源码 SHA 为
`37b8eaf7d65fe31133c5e5058848341c50c8a820`；local、tracking 和远端 SHA 已读回
一致。该提交已通过完整工程门禁：Python `131 run / 1 skipped`，app
`1088 passed / 2 ignored`，workspace Clippy、Rust tests、doctests、依赖策略和
三组 AI controller 均通过。

当前 exact source 已重新构建
`work/native-cua-20261107/package-r10/KeelShell.app`，bundle 结构检查退出 0，
manifest 精确绑定上述 SHA。解锁 macOS 短验确认三套主题均可切换，明暗模式按钮
保持可见，空连接库和主题对话框 AX 元素可读；测试进程已按 PID 读回并回收。该
smoke 仍不覆盖真实 SSH、分屏、900×580 accessibility frame、IME、VoiceOver、
签名、公证、安装、Windows/Linux 原生桌面。

针对该 exact source 已启动三次新的独立严格只读复核，但均在有界等待内没有返回
可审计正文，随后停止；没有任何复核任务修改、提交、推送或清理工作树。当前状态
保持 `NON_AUTHOR_REVIEW_UNAVAILABLE`，不能把本片写成非作者批准。包、smoke 和
复核边界见[主题选择器记录](testing/records/2026-11-07-theme-picker-layout.md)。

## 2026-11-07 主题选择器布局与可见性修复

本片继续推进用户选定的三套主题方向：Frost「霜白」、Graphite「曜石」、
Lavender「雾紫」全部保留在项目中，并通过 registry、语义 Palette、组件形状、
Chrome/Typography token 解耦。主题卡片目录现在拥有独立滚动区域，明暗模式控制
固定在弹窗可见动作区；主题卡片的工作台预览和真实生产按钮均保持注册表驱动，
后续新增主题只需添加主题定义和 token。

定向验证已通过：主题模块 9/9、磁盘监控主题联动 1/1、格式、差异检查和应用
严格 Clippy 均通过。随后 `python3 scripts/check.py` 退出 0，Python
`131 run / 1 skipped`，完整 app 测试为 `1088 passed / 2 ignored`，workspace
其余测试、doctests、依赖版本策略和 readiness/observer/small-stack AI
controller 均通过。既有 `__eh_frame section too large` linker warning 保留，
不影响退出码。详细证据见[主题选择器记录](testing/records/2026-11-07-theme-picker-layout.md)。

本片当前待执行交付顺序仍是：固定源码提交后启动新的独立严格只读复核，按
P0/P1/P2/P3 返回路径/行号、真实测试计数、限制和 merge recommendation；复核
通过后推送分支并读回本地、tracking、远端 SHA。macOS/Windows/Linux 原生窗口、
900×580 accessibility frame、IME、VoiceOver、安装包和真实外部 provider 继续
保持各自独立验收边界。

## 2026-11-07 工具栏远程上下文布局修复

本轮修复了 macOS 短验发现的真实布局缺陷：紧凑工具栏中的“远程工作区”上下文
在窄窗口会被压成逐字竖排。源码提交
`2d05525c00d21455420d25cce675bacdedf8a73a` 已在本地完成，包含上下文最小宽度、
标题和值的单行省略、1280px 品牌/紧凑动作断点拆分、1280px 布局回归以及开发机
绝对路径记录修复。

当前验证结果：

- `keelshell-app` 定向布局、主题和更新面板回归均通过；
- 当前源码 `cargo test -p keelshell-app --locked --offline -- --test-threads=4`
  为 `1087 passed / 2 ignored`；
- `python3 scripts/check.py` 退出 `0`，Python 131 passed / 1 skipped、格式、
  严格 workspace Clippy、workspace tests、doctests、依赖策略和三组 AI controller
  均通过；linker `__eh_frame section too large` warning 仍按历史边界保留；
- 当前精确 macOS 包为 `work/native-cua-20261107/package-r9/KeelShell.app`，
  manifest 绑定源码 `2d05525...`，包结构检查退出 `0`；manifest/app/MCP SHA-256
  分别为 `8cceb76d...`、`524ae37f...`、`94eaf83f...`；
- 解锁后的 macOS 短验启动当前包，观察到工具栏 `Remote workspace` 单行显示，
  主题对话框可打开/关闭，进程和 `data-r9` 已清理。外部 900×580 调整仍受 GPUI
  无边框窗口 accessibility frame 限制，本次不把紧凑尺寸原生验收写成通过。

当前 exact tip 已完成一次独立只读复核，复核结果为 P0/P1/P2/P3 均为 0，
范围内 merge recommendation 为 `APPROVE`。复核实际验证了工作区布局、工具栏
断点、上下文最小宽度、双语 AX 标签、格式和差异检查；它没有把作者的完整工程
门禁或 macOS 原生短验重复计入独立证据。此前 exact HEAD 复核提出的窄布局和绝对
路径问题已由本轮处理。Windows/Linux 原生窗口、完整键盘/IME/VoiceOver、真实
SSH/Podman 当前包联调和外部 AI provider 仍保持开放。

## 2026-11-07 Reef / reef-template 参考边界

本次新增文档切片 [ADR 0118](adr/0118-reef-reference-boundary.md)，把 Reef 与
reef-template 的工程参考价值固定为 workspace resolver 3、workspace 版本/元数据
继承、配置分层、显式生命周期与 shutdown、owner 归属和证据管理。KeelShell
继续使用自己的 GPUI Kit 原生桌面分层，不引入 Reef 依赖。

本轮只读检查记录了 Reef revision
`75ccbf5d864fadb543ecfa62fc9871c7cff04153`；该 checkout 在检查时已有一处
`Cargo.toml` 工作树修改。检查覆盖 Reef workspace、`reef-compose` 的 feature
装配和 `application` 生命周期实现、Core 配置读取，以及模板的
`crates/app` 继承、`conf/` 三层配置、入口和工程同步文档。reef-template
revision 为 `03bd71c096fdd941e389584c44bdcb40dfed0cdf`，检查时工作树干净。
两处源仓库均未被本任务修改、提交或清理。

边界已明确：不复制 server runtime、全局 Compose、HTTP/MQ/DB/telemetry 等
服务端中间件，不引入私有 Git 依赖、内网 URL、模板 Liquid 或服务部署约定。
本轮只做 Markdown 文档和差异验证，未把静态源码检查写成 Reef 最新版本或
KeelShell 原生/远端功能验收。未来任何真实 Reef 接入都必须另建 ADR，固定
可公开依赖、API revision、配置、owner 和平台验收。

## 2026-11-07 会话、连接库与 AI 助手界面增强

本片已提交并推送到 `feature/main-visual-integration-20261106`，精确提交为
`dd6c88b0710bb56ca785eeeb83ee0835a1b77ede`；当前工作树、tracking 和远端 SHA
一致且工作树干净。
本片包含连接库范围标题与空态、会话条数量与双语 AX、上一个/下一个会话快捷键，
以及 AI 助手的六阶段工作流轨道。AI 回复到达后会自动滚动到回复审核区，避免
长上下文把审核入口推到首屏之外；知识保存动作固定在助手底部审核栏，Agent 停止
并编辑问题会在最新布局完成后按实际几何显露问题输入框。

定向验证已通过：连接库 26/26、AI 助手 6/6、会话条 5/5，快捷键专项 1/1，
`cargo check -p keelshell-app --locked`、格式和差异检查通过；长 MCP、Agent 编辑
问题和知识保存回归均为 1/1。AI 定向测试首轮曾因回复区域不可见失败，补充响应
滚动锚点后精确重跑为 2/2；Agent 编辑问题第一次和一次旧布局修补也曾失败，最终
改为 render 后 `on_prepaint` 几何滚动并精确重跑为 1/1。所有失败事实保留在
[本轮测试记录](testing/records/2026-11-07-session-library-ai-ui-polish.md)。

设计决策见 [ADR 0117](adr/0117-session-navigation-and-assistant-workflow-surface.md)；
产品流程见 [AI 助手审核工作流](product/AI_ASSISTANT_WORKFLOW.md)。Frost「霜白」、
Graphite「曜石」、Lavender「雾紫」继续由主题 registry、Palette、组件形状、
Chrome token 和 Typography token 提供，视图不按主题 ID 分支，新增主题只需扩展
注册定义和语义 token。

完整 `python3 scripts/check.py` 已重新执行并退出 0，包含 Python 131/1 skipped、
格式、严格 workspace Clippy、workspace tests、doctests、依赖策略和三组 AI
controller。当前提交已完成新的独立严格只读复核：P0/P1/P2/P3 均为 0，
复核建议为 `APPROVE`；复核使用两个定向用例，各 1 passed，保留
`__eh_frame section too large` linker warning 及共享 target 只读限制。当前精确
源码已重新失效构建并生成 `package-r6`，manifest 绑定 `dd6c88b...`，结构检查
退出 0；解锁后的 macOS 短验确认 Frost、Graphite、Lavender 三套主题均可切换，
AI 助手侧栏和其 AX 元素可见，运行进程已关闭。Windows/Linux 原生桌面、完整键盘
焦点、IME、VoiceOver、当前源码包的长流程、Podman/SSH 真实联调和外部 AI provider
仍不能由本轮 headless 或短范围 smoke 代签。

## 2026-11-07 AI 助手窄栏工作流修正

在上述初始提交之后，`58fd2d248302958675b0652b7bc15b8aee77b580` 修正了 AI
助手窄栏工作流轨道：六个阶段以完整阶段项换行，分隔符附着前一个阶段项，
当前宽度可以直接看到“保存知识”，不再依赖横向滚动。新增 360×760 中英文 GPUI
布局测试，确认六个阶段均在轨道边界内可见。

该提交的定向验证为窄栏专项 1/1、assistant 专项 7/7、app Clippy 和
`cargo check -p keelshell-app --locked` 通过；新的独立严格只读复核返回
P0/P1/P2/P3 均为 0，并建议 `APPROVE`。复核确认 1087 项 app 测试库存，
知识审核 footer 相关 3 项与窄栏专项合计 4/4 通过；首次不匹配过滤器的
0/1087 结果没有计入验收。

当前精确源码包为 `work/native-cua-20261107/package-r7/KeelShell.app`，
manifest 绑定 `58fd2d2`，结构检查退出 0；macOS 短验观察到六段轨道完整换行
为两行，助手 AX 元素可读，测试进程已关闭。该提交与文档提交均已推送，最终
HEAD/tracking/远端 SHA 已读回一致。跨平台原生桌面、完整键盘焦点、
IME、VoiceOver、Podman/SSH 真实联调和外部 AI provider 仍保持开放。

## 2026-11-07 连接身份栏最终候选

本轮实现已完成源码与测试修复，当前候选包含 pane 连接身份栏、恢复状态语义
修正、主题 typography token 以及 live IPv6 端点显示修复。源码仍保持 UI、core、
session、AI 分层：

- 每个终端 pane 都显示会话标签、真实 SSH 用户/主机/端口、`SSH / UTF-8`
  和带文字的生命周期状态；
- 单 pane、横向/纵向分屏和恢复占位共用同一身份栏渲染路径；
- 未保存的一次性连接显示 `未保存连接 / Unsaved connection`，连接档案缺失
  显示 `配置已移除 / Profile unavailable`，两者使用不同语义色；
- Frost、Graphite、Lavender 仍全部可选，主题由 registry、Palette、组件形状、
  chrome token 和 typography token 组成，新增主题只需添加注册定义；
- pane 标签区域已收紧身份栏宽度，保留分屏标签与关闭动作的点击命中空间。

当前源码定向回归和完整工程门禁均通过：Python 131 passed/1 skipped，
`keelshell-app` 1079 passed/2 ignored，core 197 passed，session 434 passed/16
ignored；workspace integration、MCP、SSH loopback、transfer、doctest、readiness、
observer 和 small-stack AI controller 也通过。严格 workspace Clippy、格式和
差异检查通过。app 单线程与最终并行重跑、session 四线程重跑均达到相同计数。
首轮完整门禁中新增测试曾触发 `needless_borrow`，已修复并保留失败事实；当前还
不能把该候选写成已获独立复核批准。

IPv6 P2 已由源码提交 `b2f875458e42cf374cb3d388d808e986a415c2cd` 修复：初始连接
和重连路径统一调用 `jump_host_picker::endpoint`，身份栏显示
`operator@[2001:db8::8]:2202`；helper 定向测试 1/1，完整工程门禁再次退出 `0`。
该提交之后的文档更新只记录证据和状态，不改变这次包构建的 Rust 输入。

`package-r3` 仍保留为源码提交 `2589d3b` 的详细人工 GUI 观察包；
当前源码精确包为 `work/native-cua-20261107/package-r5/KeelShell.app`，manifest
绑定 `b2f875458e42cf374cb3d388d808e986a415c2cd`，结构检查退出 `0`，并完成了
当前包的短范围主题/双语 native smoke check。两份包的证据边界在测试记录中分开。

上一轮固定提交 `2589d3b` 的独立报告提出的证据问题已由 `9d02440` 及后续文档
提交修复：Hallmark preflight 已指向当前 pane 记录，ADR 计数已同步，旧包补齐
源码提交、构建退出码和 manifest/app/MCP hash。固定 `9d02440` 的旧复核正文曾
另外指出 IPv6 显示与包 provenance 两项问题；前者已由 `b2f8754` 修复，后者由
`package-r5` 的精确 manifest 绑定收口。针对当前 exact source `b2f8754` 新开的
两次独立只读复核任务均在有界等待内没有返回可审计正文，随后已停止；它们没有
修改、提交、推送或清理目标工作树。因此当前状态是
`NON_AUTHOR_REVIEW_UNAVAILABLE`，不能把旧报告、代理启动或超时写成当前提交批准。

完整 app 并行负载下，route/split 夹具曾在 10 秒 UI 就绪等待内超时；当时
gateway 和 target 都已认证，跳板通道及 SSH 会话仍存活，`connect_route` 仍在
进行。由于生产 SSH 操作期限为 15 秒，测试就绪等待改为有界 20 秒，route
server session 改为有界 120 秒以覆盖完整 split/close 循环。生产 reconnect、
SSH 生命周期、命令审核和 AI 权限没有改动。session 还曾出现一次 traceroute
夹具抖动（129 passed/1 failed/4 ignored，`traceroute_hop_limit` 实际得到
`timeout`）；精确重跑、四线程重跑和最终 workspace 门禁均通过，失败事实已保留。

旧提交 `d9072ed` 的 Avicenna 只读报告发现 1 个 P1 与 3 个 P2；P1 已由
`1e63165` 修复，三个 P2 已在当前源码中关闭。该旧报告不替代当前提交的独立
复核；当前 exact source 的两次新复核没有形成正文，后续维护仍必须固定最新提交
重新审查并返回 P0/P1/P2/P3、路径/行号、真实计数、限制和 merge recommendation。

macOS 原生包的详细人工观察绑定源码提交 `2589d3b` 的
`work/native-cua-20261107/package-r3/KeelShell.app` 与隔离数据目录
`work/native-cua-20261107/data-r3`，回环端点为 `hdh@127.0.0.1:22222`；已观察
单 pane、横/纵分屏、合并、关闭面板、三主题、双语、主题切换后的身份栏与会话标签、
真实 SSH 连接、断开和恢复状态。当前源码的 `package-r5` 已完成结构检查和短范围
主题/双语 smoke check；900×580 调整因 GPUI 无边框窗口 accessibility frame 异常
仍未通过。Windows/Linux 桌面、
完整键盘焦点、IME/VoiceOver 矩阵和 Podman 真实联调仍未验证。Headless GPUI
测试不能代替这些边界。

源码设计决策和测试证据见
[ADR 0116](adr/0116-pane-connection-identity-bar.md) 与
[测试记录](testing/records/2026-11-07-pane-connection-identity-bar.md)。

## 2026-11-07 连接身份栏继续推进（历史状态）

上一片视觉节奏收口已由 `e3226e7097b72f6927fbc38f05a94a365d8269d7` 提交并推送，
本地、tracking 和远端分支已读回一致。当前候选把每个终端 pane 的顶部统一成
会话标签、连接身份和 pane 比例动作组成的紧凑 header：

- 真实端点来自 Workspace 的远端快照，恢复标签从当前连接档案生成，缺失档案明确
  显示不可用，不生成示例主机；
- SSH / UTF-8 与状态 badge 分开显示，状态文字与语义色同时保留；
- 身份区具有双语 AX 标签，单 pane 与分屏都使用同一渲染路径；
- 继续使用 Frost、Graphite、Lavender 的 Palette、组件 token 和
  `ChromeTokens::pane_header_height`，没有主题 ID 业务分支。

源码、设计决策和测试记录分别见 `crates/keelshell-app/src/workspace/pane_view.rs`、
[ADR 0116](adr/0116-pane-connection-identity-bar.md) 和
[测试记录](testing/records/2026-11-07-pane-connection-identity-bar.md)。本段保留
提交前的历史状态；最终候选的门禁结果以上方记录为准。

该实现继续保持 UI、core、session、AI 分层，阻塞 I/O 不进入 UI 线程，连接身份栏
不改变 SSH 生命周期、命令审核或 AI 权限。真实 macOS/Windows/Linux 桌面、主题
切换截图、IME、键盘和 VoiceOver 仍不能用 headless GPUI 测试代签。

## 2026-11-07 继续推进回执

本次继续推进的视觉增量尚未提交时已完成定向回归和完整工程门禁。源码新增
`ChromeTokens` 并接入主题注册表，工作区宽窗增加品牌与远程工作台上下文，
命令栏增加审核目标徽章，执行按钮使用 primary 语义，会话标签补充
`surface_active`/`surface_hover`/按下层级。新增设计决策见
[ADR 0115](adr/0115-workbench-visual-rhythm-and-context.md)，测试回执见
[2026-11-07 视觉记录](testing/records/2026-11-07-workbench-visual-refinement.md)。
三套主题仍通过 registry、Palette、组件形状和 chrome token 扩展，业务视图不
按主题 ID 分支；900px 以下隐藏品牌区以保留紧凑窗口操作空间。

定向设计测试 4/4、工作区三主题/双明暗/双语/三尺寸布局 1/1、长标题会话
1/1、900×580 双语工具栏 1/1 通过。首次 app crate 全量运行出现既有 reconnect
分屏夹具 1 项失败（1074/1/2），精确重跑该用例 1/1 通过；随后完整
`python3 scripts/check.py` 退出 0，实际完成 Python 131/1 skip、app
1075/2 ignored、workspace 其余测试、严格 Clippy、rustdoc、依赖策略和三组
AI controller。该 linker 警告和首次 app 全量失败均保留，不伪装成无失败历史。

独立视觉/产品只读复核已返回：未发现 P0，确认当前主题注册表和本轮视觉增量
方向可维护；下一片优先建议为所有终端 pane 增加持续的连接身份栏，并随后处理
顶部工具栏分组、AI 面板窄窗比例和会话标签键盘/AX 语义。该报告不替代源码
独立非作者复核，后者仍在等待。

原生 CUA 通道仍需再次确认桌面解锁后才能观察当前工作树；在此之前 macOS
多窗口、主题切换、键盘/IME/VoiceOver 和当前视觉增量的实际截图继续保持
`NOT_RUN`，不以 GPUI headless 测试代签。

当前源码提交 `ac397c87f3fc49e1e1f940ecd99fab20566c7a23` 已于
2026-11-06 22:07:13 UTC 推送到
`origin/feature/main-visual-integration-20261106`，本地、tracking 和远端
分支均已读回同一 SHA；`main` 未改变。

在没有源码变更的前提下，继续执行了三组定向回归：`app_windows::tests`
5/5、`workspace::tests::windows` 14/14、`files::transfer_tests::review_viewport_tests`
11/11。应用使用隔离 `KEELSHELL_DATA_DIR` 完成编译并启动，随后停止并清理临时目录；
这只证明启动路径可执行，不替代桌面交互验收。

文档提交后的第一次完整 `python3 scripts/check.py` 在
`keelshell-ai --test local_agent_process -- --controller-small-stack` 处遇到
`EmptyReply` 并退出 101；精确重跑同一控制器通过，随后第二次完整门禁退出 0。
第二次结果保持 Python 131 run / 1 skipped、workspace 2,397 ordinary / 23 ignored、
11 rustdoc、严格 workspace Clippy、依赖策略和三组 AI controller 全通过。第一次
失败和精确复跑成功均保留为测试事实，不将第一次失败隐藏成从未发生。

本轮再次尝试非作者严格只读复核。默认 agent 在有界等待内没有报告；备用供应商
返回 API-Key 限制 403；本地 Codex agent 在工具调用循环中未到达审查正文。三次
尝试均没有修改工作树、提交、推送或清理任何项目资源，当前状态保持
`NON_AUTHOR_REVIEW_UNAVAILABLE`，不能写成独立批准。

CUA 桌面通道连续返回 macOS 仍锁定，未能观察多窗口、Quit/Install 聚合审核、
主题切换、`merge-adopt`、`merge-save`、late-save 或连续无换行保存；当前原生范围
保持 `NOT_RUN`。下一次桌面可用后，先运行这些受控场景，再更新本页和对应测试记录。

## 当前集成：主题与工作区外壳视觉

本分支 `feature/main-visual-integration-20261106` 已将 `main` 的最新监控、知识分类、
远端文本格式和 traceroute 能力，与视觉分支的三主题语义 token 和工作区外壳修整合并。
集成源码提交为 `26e11c5`，文档编号清理提交为 `ecdc8c1`，进度记录提交为
`2cca8cd`；继续工作前应按交付回执重新读取本地、tracking 和
`origin/feature/main-visual-integration-20261106` 的实际 HEAD，后续文档修订可能
继续推进分支 SHA。
视觉增量覆盖顶部工具栏、连接导航选择器、会话栏、底部工具栏和状态栏：统一使用
`chrome`、`surface`、`surface_hover`、`surface_active`、`toolbar`、`focus` 和
`accent` 语义层级，活动会话以边界与底部强调线表达当前目标，不改变会话实体、焦点、
审核、网络、持久化或权限语义。

集成后复核发现监控面板文件系统容量条仍使用硬编码 `0x448eaa`，导致资源条不随
主题明暗变体切换。修复提交 `ef110d2` 增加 `Palette.resource`，为三套主题的六个
变体定义资源色，容量条读取当前 `visual.resource`，并以设计测试校验资源色与进度
轨道的至少 3:1 对比度。修后定向设计 4/4、磁盘相关 10/10、独立磁盘主题回归 1/1、
严格 Clippy 和完整 `python3 scripts/check.py` 均退出 0。随后尝试启动新的非作者只读
复核，但默认复核任务未在有界等待内返回报告，备用模型因 API-Key 权限返回 403；
没有任何复核任务修改工作树。父级对固定提交 `052eebc` 的限定只读核对未发现新的
P0/P1/P2/P3，当前状态为
`LIMITED_PARENT_REVIEW / APPROVE_SCOPED_WITH_REVIEW_SERVICE_LIMITATION`，不把它写成
非作者子代理已批准。

霜白、曜石、雾紫仍由注册表、Palette 和组件形状扩展；业务视图不按主题 ID 分支，
新增主题只需要扩展主题注册表、语义 Palette 和组件形状。视觉来源切片的定向门禁、
焦点环实际 alpha 合成对比度检查和第二轮独立复核均已通过；集成后的
`python3 scripts/check.py` 已重新执行并退出 0，包含 Python 131 run / 1 skipped、
workspace tests、格式、workspace Clippy、依赖策略和三组 AI controller 门禁。
集成后的 design 4 项和 workbench layout 5 项也再次通过；独立集成复核固定在
`26e11c5`；`ecdc8c1` 解决 ADR 编号冲突，`2cca8cd` 记录集成门禁和当前
交付边界，后续文档提交只推进状态记录，不改变源码输入。
原生包、Windows/Linux 桌面、紧凑窗口、输入法和辅助技术仍保持各自未完成边界。
实现和视觉来源记录见 [工作区外壳 ADR](adr/0113-workbench-chrome-hierarchy.md)
与 [视觉测试记录](testing/records/2026-11-06-workbench-chrome-polish.md)。

## 接续片：独立工作窗口与编辑器动作可达性

2026-11-07 已从受保护的隔离源码副本导入两项下一片实现到当前集成分支：

- `AppWindows` 为同一 `StateStore` 路径提供进程内窗口注册、共享更新服务和
  `ConfigurationGate`。新窗口通过真实 `gpui_kit::open_window` 创建，从空工作区
  开始；每个 `Workspace` 独立持有 SSH、终端输入、草稿、AI 审核、远程面板和
  transport owner，窗口最多八个。
- 配置写入使用 `WriteLease` 绑定 snapshot revision、epoch 和 owner。兄弟窗口
  收到变更后只更新主题/语言并撤销旧配置审核，保留 live SSH 与本地草稿，显式
  reload 后才能继续配置操作。关闭窗口先审核精确会话/草稿/格式快照，再按 owner
  有界停止和 join worker；未知清理结果保留 owner，不自动重试。
- inline editor 与三方合并手工输入的布局修复已导入：正文和完整版本独立滚动，
  底部审核动作固定在可见动作区，900×580 下手工输入保留可达最小高度。

当前 app 测试清单为 1,075 项、2 项 ignored；配置闸门 4、窗口行为 12、真实
loopback SSH 窗口清理 1、worker 回收 3、编辑器视口 11，以及本轮 macOS
临时目录根路径回归 11 项均通过。第一次 workspace 重跑暴露两个并行传输夹具
直接使用 macOS `/var` symlink 临时路径的问题，已让这两个夹具使用
canonicalized temporary root；产品的静态 symlink ancestor fail-closed 规则保持不变。
随后完整 `python3 scripts/check.py` 实际退出 0，包含 Python 131 run / 1 skipped、
workspace 2,397 ordinary / 23 ignored、11 rustdoc、workspace strict Clippy、
依赖策略和三组 AI controller 门禁。独立非作者复核、开发包、真实 macOS 多窗口/
编辑器采用保存，以及 Windows/Linux 原生验收仍待完成。正式边界见
[ADR 0114](adr/0114-independent-workspace-windows.md) 和
[本轮记录](testing/records/2026-11-07-multiwindow-editor-layout.md)。
本轮两次新开的有界只读复核 agent 均未返回可审计报告，已停止且没有改动工作树；
因此不能把本批写成已获独立批准。父级只读差分检查发现并修正了新增的 worker
stderr 输出，未发现新的 P0/P1；该检查不替代非作者复核报告。

## 已交付基线

本批基线为已交付的 `fa7c05914e0eefc0f24ab32f8af690bdc50a63ed`，
包括按项监控能力与审核式知识分类。536 输入 fresh 完整门禁为
2313 ordinary / 23 ignored、14 rustdoc、Python 131 run / 1 skipped；
限定新包原生与正式文档已有各自非作者批准。
精确 main / Quality / push / attempt 1 的 run `42153758152` 三平台成功，
候选各 204 名通过。原观察器外层 91099 实际 wait、exit 2 / reaped；
原 Linux 附件 GET 失败保留，原期限内一次固定附件补取与两平台各 11 项 OpenSSH
由 `current-ci-r2` 限定批准，不重启或重跑旧 run。
见[该批记录](testing/records/2026-11-06-monitor-knowledge-categories.md)。

## 本批：文本格式与路径探测

整合工作树分支 `feature/editor-traceroute-20261106`，基线 fa7c059。
最终 r3 为 543 项工程输入、21 个源码路径，冻结文件 SHA256：
`1b5c7cf42d8a4d9ede5de6fcca29caf11da5daeaef59fc8e580ea4a1172590bd`。
原件入口 `work/next-batch-20261106/` 被 Git 忽略，不表示公开附件。

- UTF-8 BOM 与 LF/CRLF 保持：25 个新增用例实际通过，三项连续编辑 P2
  已在独立 r3 差分复审中关闭。备份 ACK、目标发布 ACK 与目标完整读回分别处理。
- 审核式有界 traceroute：17 个新增 Rust 用例实际通过；真实工具版本输出在 stderr，
  r3 修正精确准入合同，14 项 Python 与 10 个错误流负例通过。
  原 r2 工程通过但真实版本拒绝的失败保留。
- 最终完整门禁 `full-gate-r3` 外层 5546 实际 wait、exit 0 / reaped，
  575.972 秒：2355 ordinary / 23 ignored，含 app 1047 / 2 ignored；
  14 rustdoc，Python 131 run = 130 passed + 1 skipped。
  42 新增与基线 204 个具体名均实际通过，543 输入前后不变。
- 采用 `full-gate-r3-summary-r2.json` 与 `full-gate-r3-exec-readback-r2.json`；
  首次派生摘要误留旧日志信息的原件保持，修正未重跑门禁。
- 正常程序构建、macOS 打包和静态检查均 exit 0 / reaped；新包 6 成员、41 资产读回。
  app SHA256 为 `9e83d83af253ffe3c3aba29997450ea75f4c70a6dbe9311e61469228bd0158b9`，
  MCP 为 `e29f58d15f0c26c6634033f2eb0efed62b5b71dc37fcbfbab6bbbc9d28ed78a6`。
  Gauss 的 `combined-r3` 源码、`combined-r4` 门禁/新包增量复核均限定批准，
  开放 P0–P3 为 0；不签原生或未来 CI。
- 本批官方依赖检查 34 包 / 104 要求通过，manifest / lock 未改变；
  直接 registry 要求保持 `x.y`，不固定 Rust toolchain。

### 限定原生

runtime-r3 owner 为 `01e8d214a619471b8ad93786b6083afe`。
Curie `report-r3` 仅批准准备；父真实运行取得 38 组 AX/JPEG。
BOM/CRLF 完整审核取消后字节/stat/目录不变；重新审核确认一次后，
app 自己完整读回，独立观察目标 29 字节与唯一原备份 22 字节。
真实 SSH UI 中 UDP IPv4 / IPv6 各审核确认一次，均返回目的地址一跳回复。

inline 编辑器底部动作可达性待修；合并仅准备草稿和一次固定竞争写，
`merge-adopt` / `merge-save`、late-save 和连续零换行保存保持 NOT_RUN。
不能用源码/夹具观察替代这些原生结论。Gauss 的 `native-r1` 已批准观察子集与
基础设施收口，建议允许当前 543 输入批次限定交付；新增 P2 `NR1-P2-01`
保持 OPEN，完整原生继续 HOLD。报告 SHA256：
`9ce082e4be171262753f750ae4f931f37ca8be61cdeb71b985bc3960c3e07f70`。

外层 28510 由父实际 wait、exit 0 / reaped，797.330164 / 900 秒，没有重启或延期。
app exit 0 / reaped，sshd -9 / reaped，容器 exit 0；
该 owner 的 container / network / image / private 已清理，共享 base / VM / 第三方 target
保留。原 `cleanup.json` 的阶段性 false 与后续 `editor-cleanup.json` 的 true 分别保留。
旧 runtime-r2 因 0700 目录先 chown 后写 seed 失败，app 未启动，外层 49450
实际 wait、exit 1 / reaped。Curie 于 05:13:12 UTC 另编号完成旧 image/private
补充清理，7 条命令实际 wait/reaped、0.298/30 秒，精确身份与 absent 已读回；
旧 HOLD / false、失败和 12 项受保护原件不变，不回填成功。

完整来源与失败见[组合记录](testing/records/2026-11-06-editor-traceroute-integration.md)，
原生范围与原件见[原生记录](testing/records/2026-11-06-editor-traceroute-native.md)。

## 并行下一片与 Cargo 归属

2026-11-06 04:38:48 UTC 父已将共享 Cargo 槽释放给 **Dewey 的 multiwindow 隔离源码**；
原件 `work/next-batch-20261106/cargo-slot-release-r1.json`。
父只消费不可变 `package-macos-r1`，不得用当前 target 字节重新签旧包。
其他角色不能并行 Cargo / clean。归属切换先只使五个项目包失效，保留第三方缓存。

| 切片 | 当前范围与状态 |
| --- | --- |
| 独立工作窗口 | 源码已导入当前分支并通过定向窗口、worker 和 loopback SSH 测试；workspace 全量门禁通过，独立复核和真实多窗口验收待完成 |
| 编辑器动作可达性 | 源码布局修复已导入并通过 11 项视口专项；`merge-adopt` / `merge-save`、late-save 和连续无换行保存的真实原生复验待完成 |

作者隔离副本没有 `.git`，不修改父正式源码、文档或 Git。
已合并分支及时清理；受管建树已两次超时，旧 worktree 的其他任务/primary 保护保持，
不用 shell 绕过。失败和不可变原件保留。

## 当前候选接续顺序

1. 提交当前源码、测试和文档候选。
2. 固定新提交启动独立严格只读复核，返回 P0/P1/P2/P3、路径/行号、真实测试计数、
   限制和 merge recommendation；发现 P1/P2 时修复并重新门禁、重新复核。
3. 复核通过后推送当前功能分支，读回本地、tracking 和远端 SHA。
4. 使用已解锁 macOS 桌面完成三主题、900×580、横/纵分屏、连接中断、键盘/AX
   观察；Windows/Linux 原生和 Podman 真实联调继续分别验收。

## 历史交付顺序（2026-11-06 交接片）

提交与 CI 身份不能在同一提交正文中自引用。执行下列步骤前先读取实时 Git 和
`work/next-batch-20261106/delivery-r1.json`、`delivery-exec-readback-r1.json`
及 `current-ci-preparation/` 的实际回执；已有实际完成记录的动作不重放。

1. 原生非作者报告已收取；正式文档、链接及差分由非作者作最终限定复核。
2. 提交本批既有 543 输入，普通快进推送 main，读回本地/跟踪/远端 SHA。
3. 用已封存 `current-ci-preparation/launch_exact.py --sha <新的精确 main SHA>`
   启动一次新观察器；首次构造开始原 5400 秒时钟，不能重启、延时或重跑。
   248 候选的平台预期为 macOS 248 / Linux 247 / Windows 246。
   新 SHA / run / exec / deadline 尚未绑定，不把旧 fa7 CI 当本批结果。
4. 当前两项源码候选完成新的非作者审后，统一 fresh 完整门禁，构建新包及真实原生验收。

## 持续约束与开放边界

仅管理远程 SSH；本地终端、RDP、串口不在范围。UI / core / session / AI 分层，
阻塞 I/O 不进 UI 线程。霜白、曜石、雾紫全部保留，通过注册表和语义 token 扩展，
遵循[原生设计合同](design/HALLMARK_NATIVE.md)。
AI 上下文与命令各自显式审核，模型不能自主执行；主机密钥变化 fail closed，
凭据不进普通配置或日志。

Windows/Linux 真实桌面、完整主题/尺寸/键盘/IME/辅助技术、多窗口的完整原生矩阵、
供应商账户/外部 MCP、签名安装仍开放。用户已授权自主开发、提交推送 main、
标签触发 Release 和本地 Podman 联调；不含覆盖安装或遥测。产品总目标保持进行中。
