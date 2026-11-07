# ADR 0116：终端 pane 持续连接身份栏

- 日期：2026-11-07
- 状态：已实现，工程门禁通过；当前提交独立复核与原生桌面验收待完成
- 范围：`keelshell-app` 工作区外壳

## 背景

工作区在分屏时有 pane 标签头，单 pane 则直接进入终端画布。用户因此需要
回到全局会话栏才能确认当前终端对应的 SSH 端点；分屏后每个叶子也缺少持续
可见的连接身份和生命周期状态。终端输出本身不适合承担这类上下文，因为它
可能被远端内容覆盖，而且终端实体不拥有 Workspace 的连接档案与布局关系。

## 决策

所有已连接或已恢复的终端 pane 都渲染同一行 pane header。该行保留当前 pane
的会话标签和分屏比例动作，并增加一个紧凑的连接身份区：

- 端点使用 Workspace 已保存的远端快照，显示真实的用户、主机和端口；
- 启动中的会话、已连接会话、已断开会话和待恢复标签都同时显示文字与语义色；
- `SSH / UTF-8` 作为终端协议和编码元数据单独呈现，状态使用独立 badge；
- 恢复占位优先使用当前仍可用的连接档案生成端点，档案缺失时显示明确的
  “配置已移除 / Profile unavailable”，不制造示例主机；
- 端点、状态和整个身份区都有可访问标签，身份区不接收焦点，也不会改变 SSH
  输入焦点或命令审核目标。

状态只消费 `Workspace` 已有的 `remote_hosts`、恢复布局和 `TerminalView` 生命周期
快照。传输线程、SSH 会话、终端输入和 AI 审核模型不因此改变；阻塞 I/O 仍在
UI 线程之外。

## 主题与扩展边界

身份栏使用 `Palette` 的 `text`、`muted`、`success`、`warning`、`danger`、
`danger_surface` 和 `border`，高度使用 `ChromeTokens::pane_header_height`，
状态 badge 的圆角使用组件 token，端点字体使用主题 `TypographyTokens`。
Frost、Graphite、Lavender 共享同一视图实现，业务代码不按主题 ID 分支。以后
增加主题时，只需在主题 registry 中补齐语义 Palette、组件形状、chrome token
和 typography token；身份栏不需要增加条件分支。

## 验收

定向 GPUI 回归覆盖：

- 双语端点、协议、状态和 AX 标签；
- Frost、Graphite、Lavender 的明暗变体；
- 900×580、1280×840、1440×900 等工作区尺寸；
- 分屏布局、长标题、最小窗口和 SSH 输入不被布局控件消费。

这些检查证明源码布局和状态映射；本轮完整工程门禁也已通过，包括 Python
131/1 skip、app 1078/2 ignored、core 197、session 130/4 ignored、严格
workspace Clippy、workspace integration、MCP、SSH loopback、transfer、
doctest 和 AI controller。它们不能代替 macOS、Windows、Linux 原生桌面截图、
IME、VoiceOver 和真实 SSH 交互验收。当前原生边界继续记录在 `docs/HANDOFF.md`
与对应测试记录中。

