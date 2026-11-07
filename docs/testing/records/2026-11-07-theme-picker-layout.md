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

## 精确 macOS 包与短范围 smoke

源码提交 `37b8eaf7d65fe31133c5e5058848341c50c8a820` 重新失效项目 crate 后构建
了 `work/native-cua-20261107/package-r10/KeelShell.app`。构建和结构检查均退出
0；`package-manifest.json` 的 `build.commit` 精确绑定该 SHA，目标为
`aarch64-apple-darwin`，最低 macOS 版本为 15.0，`installed=false`，
`signed_by_packaging_script=false`。manifest、GUI 和 MCP SHA-256 分别为：

- `7bbdfc0b066beeafc9190afac6ebce69c7474623f311ee33f09fcc6cae59c521`
- `2f8f0eae2258857e24baf0731fa015342671825ac4cec54b8687047f5d393a11`
- `94eaf83f8bbb23ae4db7cc190c9c06332ddbc19b4bf9246dcdf35f43b3645d8f`

已解锁 macOS 桌面从该 exact package 启动并观察到：主题对话框可打开和关闭，
Frost、Graphite、Lavender 三套主题可逐一应用，System/Light/Dark 控件始终可读，
当前主题与“应用推荐”状态会随选择更新，返回后的空连接库和主题入口仍可见。
测试结束后，本次 exact package 进程已回收。该 smoke 未执行真实 SSH、分屏、900×580
调整、IME、VoiceOver、签名、公证或安装。

## 独立复核状态

针对 exact source 的三次新独立严格只读复核尝试均在有界等待内没有返回可审计正文，
随后停止；它们没有编辑、提交、推送或清理工作树。当前状态保持
`NON_AUTHOR_REVIEW_UNAVAILABLE`，不能把作者定向/完整门禁或本次原生 smoke 写成
非作者批准。
