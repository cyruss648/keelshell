# ADR 0119：连接库选中层级与主动作

- 日期：2026-11-07
- 状态：已实现；定向 GPUI 回归和完整工程门禁通过；独立复核与当前源码原生桌面验收待完成
- 范围：`keelshell-app` 连接库表格、窄窗口卡片和连接行动作
- 关联：[当前交接](../HANDOFF.md)、[开发路线](../ROADMAP.md)

## 背景

连接库已经支持全部连接、收藏、最近连接、回收站、文件夹筛选、批量审核和双语
可访问标签，但行选中状态主要由左侧选择按钮表达。用户选择多个连接后，需要
从选择计数和小按钮判断当前目标；表格中的“连接”也和编辑、移动、复制、回收
站处于相同的弱化视觉层级，降低了常用入口的辨识度。

本轮只调整连接库的视觉层级和无障碍状态，不改变连接实体、筛选、批量审核、
持久化、SSH 生命周期或凭据边界。

## 决策

### 1. 选中状态提升到整行

`connection_library_row` 先从 `library_selection` 取得当前连接的 `selected`
快照，并同时用于：

- 行的 `aria-selected` 语义；
- 行的 `surface_active` 背景；
- 主题 `accent` 左边界；
- 选择按钮的选中状态和双语动作标签。

非选中行保留交替底色，并使用 `surface_hover` 提供指针反馈。宽表格和窄卡片
共用同一行渲染路径，因此选中反馈不会随窗口断点消失。

### 2. Connect 是连接库的 primary 主动作

活动连接库中的 Connect 使用 GPUI Kit 的 `primary` 变体，颜色由当前注册主题
的 `Palette::primary`、hover、active 和 foreground token 提供。Edit、Move、
Copy 和 Trash 继续使用次级弱化动作，以保持一个明确的主要视觉入口。回收站
仍只显示 Restore 和永久删除，不出现 Connect。

### 3. 继续使用 registry/token，不按主题 ID 分支

行视图只消费 `Palette` 语义色和 GPUI Kit 组件变体，不检查 Frost、Graphite、
Lavender 的主题 ID。未来新增主题只需补齐注册定义和 token；不会复制连接库
视图或为单一主题增加条件分支。

## 取舍与限制

整行选中语义使用 GPUI headless accessibility snapshot 验证；headless 测试
不会读取最终像素，因此不能单独证明三套主题在真实桌面上的颜色对比和视觉
观感。当前源码尚未为本片重新构建 macOS 包，也未执行 Windows/Linux 原生
窗口、IME、VoiceOver 或真实 SSH 长流程观察。

## 验证

最终源码和测试实现提交为
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9`。连接库定向测试为
`27 passed / 0 failed`，selected-row 精确回归为 `1 passed / 0 failed`，
覆盖宽表格、窄卡片、双语、回收站、批量选择、动作可达性和 selected-row
无障碍语义。完整 `python3 scripts/check.py` 退出 `0`，实际完成 Python
`131 run / 1 skipped`、app `1089 passed / 2 ignored`、workspace 其它测试、
严格 Clippy、doctests、依赖策略和三组 AI controller。此前测试曾把
observation 的可访问标签误当成可见按钮文本，修正后的断言已在精确回归和完整
门禁中通过。
