# 未连接面板的主题画布 — 2026-11-06

基线为已推送main的`56d5440e4fa260f53ee6ea404fee9223f41d2188`，
当前分支`feature/pane-surface-polish-20261106`。

## 变更与状态

离线恢复原件中，浅色未连接面板的外层仍使用终端背景和终端内边距，
形成明显的黑色宽边。恢复标签改用既有语义画布，状态正文继续使用原组件间距，
标题/选中边界沿用当前Palette。三主题共用同一状态判断，无主题ID分支、新字体或颜色常量。

画布的生产改动仅`workspace/pane_view.rs`，8行增加、3行删除；
本切片另组合诊断与输入选择修订，工程检查针对全部组合输入。
单文件`rustfmt --edition 2024 --config skip_children=true`与`git diff --check`
均退出0。冻结前后摘要与patch保存在
`work/pane-surface-polish-20261106/candidate.json`、`applied-r1.json`。

父顺序使用共享target、2 jobs和四测试线程。旧`full-gate-r3`通过，
但之后另改搜索输入选择，不代签新源码；
新`full-gate-r4`于17:43:55–17:53:01 UTC通过，父exec35977已实际wait、exit0/reaped。
2,239 ordinary/23 ignored、app973/2 ignored、14 rustdoc、Python131 run/1 skip，
格式、严格Clippy、依赖策略及追加控制器通过。
新`build-r1`于17:43:00–17:43:14 UTC退出0，父exec12666已实际wait/reap。
525项构建/门禁输入与当时源码逐摘要相同；五个包成员及41assets已核对。
新macOS开发包程序SHA-256：
`f9b5ac99e109c16de69e497dfa32f3fb47056ff0ea9e1e456764900e251a51b3`。
包位于`work/pane-surface-polish-20261106/native/package-r1/`，未安装、签名或发布。

## 新包的实际原生观察

复用原已审查的离线恢复监督器，一次启动run
`5a650e67ced3483c9cb71122d65756a1`，不调用restart。
原件位于`work/input-and-layout-readiness-20261105/restore-native-preparation/runs/`
下本UUID的`captures/`；本轮绑定回执另在
`work/pane-surface-polish-20261106/native/`。
合成种子包含可用未连接、未保存占位、缺失连接三态，不是已连接主机数据。

在同一真实macOS窗口取得36对2880×1866 AX/JPEG：

| 语言 | 家族 | 显式明暗 | 画布原件 |
| --- | --- | --- | --- |
| English | Frost、Graphite、Lavender | Light、Dark各一次 | 每格未保存/缺失各一对，共12对 |
| 简体中文 | 霜白、曜石、雾紫 | 浅色、深色各一次 | 每格未保存/缺失各一对，共12对 |
| 对应设置 | 三家族、两语言 | 每次显式核对明暗 | 另12对，不混计为画布 |

实际观察的24个画布中，两叶四周采用对应画布颜色，未露出旧终端底色的宽边。
标签、分隔线、状态和操作文字可见；缺失及未保存占位均没有连接按钮，
可用未连接态保留显式连接动作。没有点击Connect、输入命令或发起诊断。
AX语义与图像各自保留，不能以AX代替颜色或以图像代替选择状态。
本轮固定同一窗口几何，不签紧凑尺寸、输入/IME、VoiceOver或Windows/Linux桌面。
监督器不是网络审计器，也不据此声称零网络。

本轮app PID99109经正常UI Quit退出0/reaped；随后监督器在
`WAITING_RESTART/launches=1`收到stop，父exec89824已实际wait、exit0/reaped。
终态`STOPPED/success=true/launches=1`，私有目录删除，已观察子进程和监督器PID均不存在，
清理在原600秒总TTL内完成。没有app-2，不签重启持久化。
桌面另有原组合开发包进程PID89730，本轮按新包完整路径选择并核对合成窗口，
没有操作或清理该已有进程。

新非作者`surfaces-native-r1`已批准限定的macOS画布原件与映射，
`APPROVE_SCOPED_MACOS_CANVAS_NATIVE_MAPPING`，开放P0/P1/P2/P3均为零；
Hallmark finish仅对本次画布映射为ship。`native_acceptance=NOT_PROVEN`
继续表示完整原生产品验收未完成，不否认以上实际子集。

之后的搜索按钮焦点修订只改变`terminal.rs`及其测试，没有改变画布生产路径、
主题token或assets。新`build-r2`于18:19:44–18:19:50 UTC退出0/reaped，
父exec78221已实际wait；525项输入与新门禁r5输入一致。
`full-gate-r5`于18:20:25–18:29:38 UTC退出0/reaped，父exec80111已实际wait；
2,242 ordinary/23 ignored、app976/2 ignored、14 rustdoc、Python131 run/1 skip，
格式/严格Clippy/依赖策略及追加控制器通过，525输入保持。
新包`native/package-r2/`程序SHA-256为
`a5261fab757cd5bef412a0be5b79769a827e1fc8aa4f9982341ef8c4f1c3aa29`，
五成员和41assets已核对，未安装、签名或发布，也未作本轮原生运行。
原36对捕获保持签r1包，不代签r2的搜索按钮修复或原生输入。
