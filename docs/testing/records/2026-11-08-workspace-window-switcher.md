# 工作区窗口切换器测试记录

日期：2026-11-08（Asia/Shanghai）  
范围：工作区窗口入口、窗口摘要、原生窗口激活、主题/语言和配置闸门  
状态：**源码定向通过；作者完整工程门禁通过；原生多窗口待复验**

## 实现范围

本片在既有 `AppWindows` 与 `Workspace` 生命周期上增加窗口切换器：

- 注册表保存每个窗口的轻量展示摘要；
- 顶部工具栏显示窗口总数；
- 弹出列表显示窗口序号、会话数、当前活动主机或快速连接；
- 点击条目激活对应 `AnyWindowHandle`；
- “新建窗口”复用既有创建路径和配置闸门；
- 主题和中英文由现有 registry、Palette、组件 token 和 i18n 路径提供。

没有共享 SSH session、TerminalView、草稿、AI 审核或远程面板实体。

## 定向结果

在共享 target 归属切换后，按 `docs/testing/SHARED_TARGET.md` 失效
`keelshell-app`、`keelshell-core`、`keelshell-session`、`keelshell-ai` 和
`keelshell-mcp` 项目包，再重新生成测试清单。当前清单包含 `1,119 tests`，并确认
新增的两个测试名存在。

实际执行：

| 命令范围 | 结果 |
| --- | --- |
| `workspace::tests::windows::workspace_window_entry` | `2 passed / 0 failed / 1117 filtered out` |
| `workspace::tests::windows` | `17 passed / 0 failed / 1102 filtered out` |
| `workspace::tests::workbench_layout` | `8 passed / 0 failed / 1111 filtered out` |
| `workspace::tests::themes` | `9 passed / 0 failed / 1110 filtered out` |
| `cargo fmt --all -- --check` | 通过 |
| `git diff --check` | 通过 |

## 作者完整门禁

`python3 scripts/check.py` 使用命令会话 `65860` 完成并退出 `0`。门禁中的 Python
检查为 `131 tests run / 1 skipped`；workspace Rust 单元测试、集成测试、doctest、
严格 Clippy、依赖版本策略和 AI controller/readiness/small-stack 检查均通过。
共享 target 按 `docs/testing/SHARED_TARGET.md` 先失效本项目包后重新编译。输出中出现
的 `__eh_frame section too large` 是链接器 warning，未导致门禁失败。

新增回归覆盖：

1. 两个窗口中一个有两个会话、一个为空时，切换器显示准确摘要；
2. 点击空窗口条目后，`active_window` 切换到对应真实窗口，原窗口弹出层关闭；
3. Frost、Graphite、Lavender 与中文/英文组合下切换器可渲染，列表不越过窗口底部；
4. 配置写 lease 占用时，新建窗口按钮禁用。

## 边界

本记录没有代签独立只读复核、当前源码新包、真实 macOS 多窗口桌面观察、Windows/Linux
原生桌面、多显示器几何、IME、VoiceOver、真实 SSH/Podman 长流程或外部 AI provider。
旧 `package-r4` 只属于
[主题与语言短验](2026-11-08-package-r4-theme-language-smoke.md)，不能代签本片新增
窗口切换器。
