# 主题选择器可见性与交互回归 — 2026-11-07

## 状态

当前候选已完成主题选择器的布局修复、生产 GPUI 交互回归和完整工程门禁。
Frost「霜白」、Graphite「曜石」、Lavender「雾紫」三套主题都作为内置主题
提供给用户选择；主题视图继续由 registry、语义 `Palette`、组件形状和
Chrome/Typography token 驱动，新增主题只需增加注册定义和 token，不需要在
工作区视图中增加主题 ID 分支。

本记录只覆盖源码和 GPUI 测试证据。macOS/Windows/Linux 原生窗口、完整键盘和
辅助技术矩阵、安装包观察、真实 Podman/SSH 长流程和外部 AI provider 仍按各自
记录保持独立验收边界。

## 实现

- 主题目录成为独立的垂直滚动区域，主题卡片在窄窗口中可以完整换行；
- 明暗模式控制固定在主题弹窗的可见动作区，选择器滚动不会把
  「跟随系统 / 浅色 / 深色」按钮推到视口之外；
- 主题卡片继续显示工作台预览、侧边栏、工作区、终端和状态栏层级，并由注册表
  元数据生成；
- 主题族按钮直接作为稳定的交互命中目标，双语标签、选中状态和保存中的禁用
  状态保持在同一生产控件上；
- 磁盘监控场景切换主题后继续保留真实采样、终端 draft、监控选择和会话实体。

## 定向验证

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `git diff --check` | passed |
| `cargo test -p keelshell-app --locked 'workspace::tests::themes::' -- --nocapture --test-threads=1` | 9 passed / 1081 filtered |
| `cargo test -p keelshell-app --locked independent_disk_samples_survive_production_theme_buttons_and_focused_input -- --nocapture --test-threads=1` | 1 passed / 1089 filtered |
| `cargo clippy -p keelshell-app --locked --bin keelshell-app --no-deps -- -D warnings` | passed |
| `python3 scripts/check.py` | exit 0 |

完整门禁实际完成 Python `131 run / 1 skipped`、严格 workspace Clippy、workspace
Rust tests、doctests、依赖版本策略和 readiness/observer/small-stack AI
controller。app 测试在完整 workspace 门禁中为 `1088 passed / 2 ignored`。
测试期间保留既有 macOS linker `__eh_frame section too large` warning；它没有
导致命令失败。

## 未完成边界

本片没有把 GPUI headless 几何测试写成原生桌面验收。主题切换后的 macOS 窗口
截图、900×580 accessibility frame、Windows/Linux 原生窗口、IME、VoiceOver、
安装升级和真实外部 provider 请求仍需对应平台和运行环境的独立证据。
