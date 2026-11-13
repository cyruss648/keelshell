# 响应式工作台视觉切片测试记录 — 2026-11-13

日期：2026-11-13（Asia/Shanghai）
基线：`e1d410d2d61753f2e90296981199ed2b4b3d3703`
范围：`keelshell-app` 工作区响应式 chrome、连接库、快速连接、主机监控 rail、GPUI 几何和辅助技术语义
状态：**focused 源码验证通过；原生桌面验收未执行**

## 实现范围

源码和测试只涉及：

- `crates/keelshell-app/src/workspace/view.rs`
- `crates/keelshell-app/src/workspace/library_view.rs`
- `crates/keelshell-app/src/monitor_disk.rs`
- `crates/keelshell-app/src/workspace/tests/workbench_layout.rs`
- `crates/keelshell-app/src/workspace/tests/independent_disk_monitor.rs`
- `crates/keelshell-app/src/workspace/tests/library.rs`
- `crates/keelshell-app/src/workspace/tests.rs`

本片使用现有 `Palette`、`ChromeTokens`、组件 token 和 registry 主题定义。没有
修改 `assistant.rs`、`files/**`、`snippet/**`、`session/core`，没有新增依赖，
依赖版本策略仍为 `x.y`。

实现内容：

- AI 列宽使用窗口宽度的 34%，限制在 `240px..=360px`；
- 紧凑连接导航使用 `236px`，主机监控保持 `260px`；
- 紧凑工具栏保留 AI、凭据库、配置恢复和主题图标入口；
- 新建工作区窗口在会话条保留图标入口；
- 关键动作保留完整双语 AccessKit label 和 tooltip；
- 导航 rail 使用 `Role::Group` 和 `Workspace navigation` 语义；
- 连接导航和主机监控使用 token 高度的双语 header。

## 门禁回归修复

完整工程门禁在 2026-11-13 首轮发现三项与本片响应式布局直接相关的回归：

- `900×580` 主机监控中，磁盘设备列表最后一行在固定高度内无法完整显露；
- `480px` 工作区打开 AI 后，连接库卡片因英文工具栏文字撑高而越出窗口；
- 首次启动测试把语言按钮的完整 AccessKit 名称误当成可见短标签。

修复后，磁盘设备选择器调整为 `160px` 的嵌套滚动视口，并在右侧保留窄手势
区域；极窄连接库将新建、导入、加密同步和导出动作收敛为图标并保留完整双语
AccessKit label/tooltip；快速连接卡片在固定的 `300px` 区域内允许纵向滚动。语言
回归改为核对按钮的完整辅助技术名称。

## 新增 focused 回归

`crates/keelshell-app/src/workspace/tests/workbench_layout.rs` 新增：

1. `compact_toolbar_keeps_critical_actions_discoverable_and_terminal_usable`
   - 运行 `900×580`；
   - 遍历 Frost、Graphite、Lavender，明暗模式和中文/英文；
   - 检查 `toggle-assistant`、`new-workspace-window`、`vault-settings`、
     `configuration-recovery` 的可见性、容器边界、最小可用 bounds 和完整 AX label；
   - 打开 AI 后检查助手列在 `240..=360px`，活动终端至少 `200px × 80px`，
     AI 入口仍可发现且 label 不变。
2. `navigation_headers_expose_panel_semantics_and_token_geometry`
   - 检查 `workspace-navigation-rail` 的 `Role::Group` 和
     `Workspace navigation` label；
   - 检查连接 header 的 `Connection navigation`/“连接导航” label、
     `236px` rail 和 `ChromeTokens::pane_header_height`；
   - 点击真实 `navigation-monitor` 后检查 `Host monitoring`/“主机监控”、
     `260px` rail 和同一 token 高度；
   - 同样遍历三套主题、明暗模式和中英文。

## 实际命令和结果

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo test -p keelshell-app --locked --offline workbench_layout -- --test-threads=1` | `10 passed / 0 failed / 1119 filtered out` |
| `cargo test -p keelshell-app --bin keelshell-app disk_monitor_minimum_workspace_languages_themes_scroll_and_selection_are_stable -- --test-threads=1` | `1 passed / 0 failed / 1128 filtered out` |
| `cargo test -p keelshell-app --bin keelshell-app independent_disk_samples_survive_production_theme_buttons_and_focused_input -- --test-threads=1` | `1 passed / 0 failed / 1128 filtered out` |
| `cargo test -p keelshell-app --bin keelshell-app compact_library_keeps_identity_and_actions_visible_with_assistant_open -- --test-threads=1` | `1 passed / 0 failed / 1128 filtered out` |
| `cargo test -p keelshell-app --bin keelshell-app first_launch_is_chinese_without_a_local_or_remote_terminal -- --test-threads=1` | `1 passed / 0 failed / 1128 filtered out` |
| `cargo test -p keelshell-app --bin keelshell-app design::tests -- --test-threads=1` | `4 passed / 0 failed / 1125 filtered out` |
| `cargo clippy -p keelshell-app --locked --offline --all-targets -- -D warnings` | 通过 |
| `git diff --check` | 通过 |

测试进程输出了既有 macOS 链接器 warning：`__eh_frame section too large`。它没有
转化为编译或测试失败。

## 未覆盖边界

本记录没有代替真实应用验收，以下仍需单独证据：

- 当前提交对应的 macOS 窗口截图、真实 900×580 调整和多窗口行为；
- Windows/Linux 原生桌面、输入法、完整键盘焦点和 VoiceOver/NVDA；
- 真实 SSH、Podman 主机监控和长时间 AI provider 流程；
- 最终安装包、签名、公证、启动和升级路径；
- 真实屏幕像素对比度、缩放比例、多显示器和系统字体差异。

## 交付边界

提交前再次执行 focused fmt/check/test/clippy，并以单个 feature commit 固定本片。
本记录只证明源码范围内的受控 GPUI 几何和可访问性回归；不把 headless 结果扩大为
原生桌面或完整产品验收。
