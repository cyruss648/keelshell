# ADR 0124：响应式 SSH 工作区层级

- 日期：2026-11-13
- 状态：已实现；focused GPUI 回归和 focused Clippy 通过；原生桌面验收仍待完成
- 范围：工作区顶部工具栏、会话条、连接导航和主机监控 rail
- 关联：[原生工作区](../product/NATIVE_WORKBENCH.md)、[测试记录](../testing/records/2026-11-13-responsive-workbench-polish.md)

## 背景

KeelShell 的工作区已经具备连接导航、SSH 会话、AI 助手、凭据库、配置恢复和
主机监控入口。窗口收窄后，这些入口共享顶部和会话 chrome 的有限横向空间；
完整文字会让动作组变密，AI 面板打开后也可能过度压缩终端。连接导航和主机监控
共用一个侧栏，但缺少持续可读的 panel header，用户需要依赖按钮文字确认当前层级。

本片只收紧工作区的响应式视觉层级。连接、终端、AI 审核、凭据、恢复、键盘动作
和已有状态实体继续由现有 Workspace 路径管理。

## 决定

### 极窄工作区保持内容可达

在 AI 助手打开后主工作区可能收窄到约 `240px`。连接库工具栏在低于
`360px` 的可用宽度时使用图标、完整双语 AccessKit label 和 tooltip，避免英文
动作文字把连接卡片推到窗口底部。快速连接区域在原有 `300px` 预算内允许纵向
滚动，保留输入框和连接动作。

主机监控的磁盘设备选择器使用 `160px` 的嵌套滚动视口，并保留右侧手势区域，
让滚轮在紧凑 rail 内可以继续传递给选择器；列表末项必须在受控 GPUI 几何中完整
落入选择器视口。

### 紧凑宽度保留动作发现性

在 `900×580` 及更宽的受控工作区中：

- 新建工作区窗口、凭据库和配置备份/恢复使用图标作为紧凑可见入口；
- AI 入口在紧凑工具栏显示 `Sparkles + AI`，完整的 `AI assistant` 仍作为
  AccessKit label 和 tooltip；
- 主题、凭据和恢复动作在隐藏可见文字后仍保留完整双语无障碍名称；
- 语言切换入口使用短的 `中/EN` 或 `EN/中` 文案，并保留完整切换语义；
- 新建工作区窗口属于会话条，因此它在 `session-strip` 内验证，而其余动作在
  `workspace-toolbar` 内验证。

这些变化只调整呈现和可见几何，不改变 action id、点击路由、键盘顺序、disabled
状态、modal scope 或焦点返回逻辑。

### AI 面板使用受控宽度预算

AI 列宽使用现有 GPUI `Pixels` 几何，在窗口宽度上取 `34%`，并限制在
`240px..=360px`。900px 窗口因此保留约 594px 的主工作区宽度；已有
`200px` 终端宽度和 `80px` 终端高度作为受控 GPUI 回归的最低可用预算。

### 导航 rail 增加结构 header

连接导航和主机监控继续使用同一个 rail，并增加：

- `workspace-navigation-rail`：`Role::Group`，双语 label 为“工作区导航”/
  `Workspace navigation`；
- `workspace-navigation-header`：使用 `ChromeTokens::pane_header_height`，
  根据活动 panel 显示“连接导航”/`Connection navigation` 或“主机监控”/
  `Host monitoring`；
- 连接导航在紧凑宽度使用 `236px`；
- 主机监控保留 `260px`，以满足现有 `MonitorPanel` 的根布局合同。

rail 内的连接树、监控内容、导航按钮和 focus handle 继续复用原有实现。切换
panel 只更新已有 `NavigationPanel` 状态，不创建新的连接、监控采样或 SSH 实体。

### 主题和依赖边界

工作区视图只消费现有 `Palette`、`ChromeTokens`、组件 token 和几何值，不按
Frost、Graphite 或 Lavender 的 theme id 分支。三套主题通过同一组视图和相同的
中英文回归。新增主题继续只扩展 registry、Palette、组件或 chrome 定义；本片不
引入依赖，直接依赖版本约束继续使用 `x.y`。

## 后果和边界

窄窗口的主要动作仍能通过图标、完整 AX label 和 tooltip 找到，AI 打开时终端
保留受控的可用宽度。导航 panel 的当前语义在视觉上和辅助技术树中都有稳定
位置。由于连接导航在 AI 打开且宽度低于 1280px 时仍按既有策略隐藏，用户通过
顶部导航入口重新显示它；现有状态和草稿不会被重建。

focused GPUI 测试证明受控渲染树中的 bounds、label、role 和 token 高度，不代替
macOS、Windows 或 Linux 原生窗口、IME、VoiceOver、真实终端输入和像素对比度
验收。详细命令和未覆盖边界见[测试记录](../testing/records/2026-11-13-responsive-workbench-polish.md)。

## 验证

`workbench_layout` 当前 focused 结果为 `10 passed / 0 failed / 1119 filtered
out`。新增回归覆盖 900×580 下三套注册表主题、明暗模式和中英文，检查关键动作
的可见 bounds、完整 AX label、AI 打开后的助手和终端预算，以及连接/监控 header
的 `Role::Group`、双语名称、`236px`/`260px` rail 宽度和
`ChromeTokens::pane_header_height`。`cargo clippy -p keelshell-app --locked
--offline --all-targets -- -D warnings` 已通过。

最终源码提交为
`43ae21ebc079dddd66245e36dd084d2c83c67c1a`；提交前以相同源码内容执行的完整
`python3 scripts/check.py` 返回 `0`。完整门禁结果和未覆盖的原生桌面边界见测试记录。
