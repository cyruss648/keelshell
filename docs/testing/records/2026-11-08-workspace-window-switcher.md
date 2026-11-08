# 工作区窗口切换器测试记录

日期：2026-11-08（Asia/Shanghai）
范围：工作区窗口入口、窗口摘要、原生窗口激活、modal/focus、主题/语言和配置闸门
状态：**作者完整工程门禁通过；独立复核与原生多窗口待收口**

## 实现范围

本片在既有 `AppWindows` 与 `Workspace` 生命周期上增加窗口切换器：

- 注册表保存每个窗口的轻量展示摘要；
- 顶部工具栏显示窗口总数；
- 弹出列表显示窗口序号、会话数、当前活动主机或快速连接；
- 点击条目激活对应 `AnyWindowHandle`；
- “新建窗口”复用既有创建路径和配置闸门；
- 窗口列表纳入统一 modal scope，使用 Dialog 语义、全屏输入遮罩和 focus trap；
- Escape、关闭按钮和窗口选择共用关闭入口，关闭后恢复当前工作区焦点；
- 主题和中英文由现有 registry、Palette、组件 token 和 i18n 路径提供。

没有共享 SSH session、TerminalView、草稿、AI 审核或远程面板实体。

## 定向结果

当前源码候选按共享 target 归属切换规则重新生成了 `1,121 tests` 的清单，并确认
modal/focus 回归实际出现在当前测试二进制中。
此前基线在共享 target 归属切换后生成了 `1,120 tests` 的清单；本次加入 modal/focus
回归后，已再次按 `docs/testing/SHARED_TARGET.md` 失效
`keelshell-app`、`keelshell-core`、`keelshell-session`、`keelshell-ai` 和
`keelshell-mcp` 项目包并重新编译，当前清单和测试输出如下。

实际执行：

| 命令范围 | 结果 |
| --- | --- |
| `workspace::tests::windows` | `19 passed / 0 failed / 1102 filtered out` |
| `workspace_window_switcher_uses_modal_scope_and_escape_restores_surface` | `1 passed / 0 failed / 1120 filtered out` |
| `workspace::modal_scope` | `11 passed / 0 failed / 1110 filtered out` |
| `workspace::tests::workbench_layout` | `8 passed / 0 failed / 1113 filtered out` |
| `workspace::tests::themes` | `9 passed / 0 failed / 1112 filtered out` |
| `cargo fmt --all -- --check` | 通过 |
| `git diff --check` | 通过 |

## 作者完整门禁

源码提交 `373d4157ae816a29a74aa72cf490f688e3563457` 的
`python3 scripts/check.py` 使用命令会话 `63031` 退出 `0`。实际结果为：

- Python：`131 run / 1 skipped`；
- `keelshell-app`：`1,119 passed / 2 ignored`，当前 app 测试清单为 `1,121`；
- workspace 其余 Rust 单元/集成测试全部通过；
- 14 项 doctest 全部通过；
- 严格 workspace Clippy、直接依赖版本策略和 readiness/observer/small-stack 三组
  AI controller 全部通过。

输出中的 `__eh_frame section too large` 是既有链接器 warning，未形成失败。该门禁
不代签独立只读复核、当前源码新包或原生多窗口验收。

新增回归覆盖：

1. 两个窗口中一个有两个会话、一个为空时，切换器显示准确摘要；
2. 点击空窗口条目后，`active_window` 切换到对应真实窗口，原窗口弹出层关闭；
3. Frost、Graphite、Lavender 与中文/英文组合下切换器可渲染，列表不越过窗口底部；
4. 配置写 lease 占用时，新建窗口按钮禁用。
5. 关闭第一个窗口后，后续窗口仍显示进程生命周期内分配的序号 `2`、`3`，不会把
   后续窗口重编号为 `1`、`2`。
6. 窗口列表以 Dialog 角色出现，Tab 焦点保持在列表模态内，Escape 关闭后恢复工作区
   焦点。

## 边界

本记录没有代签独立只读复核、当前源码新包、真实 macOS 多窗口桌面观察、Windows/Linux
原生桌面、多显示器几何、IME、VoiceOver、真实 SSH/Podman 长流程或外部 AI provider。
旧 `package-r4` 只属于
[主题与语言短验](2026-11-08-package-r4-theme-language-smoke.md)，不能代签本片新增
窗口切换器或本次 modal/focus 修复。
