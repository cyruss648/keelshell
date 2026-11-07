# 快速操作中心测试记录 — 2026-11-08

## 范围

本片验证 Workspace 快速操作中心的生产渲染路径、模态焦点边界、双语文案、
平台快捷键、Enter 动作路由和 `900×580` 内容几何。动作目录的纯 Rust 搜索、
排序、上限和上下文过滤继续由其模块单元测试覆盖。

源码工作树：`feature/main-visual-integration-20261106`

记录创建时的源码基线：`1239ce9077c3471eeb05d53a81a86973affd2e46`

源码实现已提交为
`c13724a9eab9df3d97fdb3ed1eaf25b5dcfabffa`，并推送到
`feature/main-visual-integration-20261106`；源码提交的 local、tracking 和
remote SHA 已读回一致。本测试记录的文档收口仍随本片最后一个文档提交完成。

## 变更

- 移除快速操作中心的 `#[cfg(test)]` 简化渲染分支，生产和测试共用同一套 GPUI
  布局。
- 用 `command-palette-modal` 作为唯一真实模态根节点，卡片、输入、结果滚动区
  和计数栏作为真实子树。
- 用模态根的捕获键盘路径处理方向键和 Enter，复用已有 Workspace 动作方法。
- 快速连接动作进入真正的一次性连接模态；主机监控动作显式切换到 Monitor 导航
  面板；方向会话动作按首个、末个和唯一会话边界过滤。
- 结果行从统一文字圆点改为 GPUI Kit 类别图标，并补充 `aria-selected`、位置/数量
  与活动后代语义。
- 删除 `SurfaceInputBoundary` 的临时诊断输出。
- 增加双语、Escape 焦点恢复、连接管理器 Enter 路由、一次性快速连接、主机监控
  路由、会话边界、平台快捷键和窄窗口几何回归。

## 已执行验证

命令：

```text
cargo fmt --all
cargo test -p keelshell-app --locked --offline \
  workspace::tests::command_palette -- --nocapture --test-threads=1
```

结果：`7 passed / 0 failed`（Workspace 快速操作中心 GPUI 回归）。

此前中间版本的同一过滤器曾为 4 项通过；补充一次性快速连接、主机监控和会话
边界回归后，纯动作目录与 Workspace 用例合并过滤的精确结果如下：

```text
cargo test -p keelshell-app --locked --offline command_palette \
  -- --nocapture --test-threads=1
```

结果：`16 passed / 0 failed / 1091 filtered out`。

覆盖的用例：

- `command_palette_is_bilingual_and_restores_focus_on_escape`
- `command_palette_enter_reuses_connection_manager_review_path`
- `command_palette_quick_connect_opens_the_one_time_connection_surface`
- `command_palette_host_monitor_selects_the_monitor_navigation_panel`
- `command_palette_session_actions_follow_first_and_last_tab_boundaries`
- `command_palette_opens_from_the_platform_shortcut`
- `command_palette_card_and_results_fit_the_narrow_modal_surface`

首次回归曾出现 `command_palette_is_bilingual_and_restores_focus_on_escape` 关闭
断言失败。原因是测试在模态已经卸载后调用 `window.find`；修复为
`window.try_find(...).is_none()` 后精确重跑通过。补充主机监控回归时，测试夹具还
曾因没有挂载 `monitor-scroll` 节点失败；随后改为验证稳定的生产
`navigation-monitor` 节点。两次失败事实均保留在本记录中。

## 完整工程门禁

命令：

```text
python3 scripts/check.py
```

结果：退出码 `0`。

已通过的阶段：

- 依赖版本策略；
- Python 回归：`131 run / 1 skipped`；
- `cargo fmt --all --check`；
- `cargo clippy --workspace --all-targets --locked -- -D warnings`；
- `cargo test --workspace --locked -- --test-threads=4`；
- `keelshell-ai` readiness-error、observer-io-error 和 controller-small-stack
  三组专项门禁。

workspace 测试证据中，`keelshell-app` 单元测试为 `1105 passed / 2 ignored`；
`keelshell-core`、`keelshell-mcp`、`keelshell-session` 的单元和集成测试组均以
退出码 `0` 完成，doctests 也全部通过。Podman、OpenSSH、真实 Linux 采样等需要
额外环境的测试保留其显式 `ignored` 边界，没有把它们写成已执行的真实联调。

完整门禁首轮在 Python 阶段退出 `1`，`test_windows_native_crash.PreparationTreeTests`
的 4 个既有进程控制用例在当次运行得到超时退出 `124`，因此没有进入 Cargo 阶段。
随后使用 `PYTHONPATH=scripts python3 -m unittest` 精确重跑这 4 项，结果为
`4 passed / 0 failed`；再次执行同一 `python3 scripts/check.py` 后，最终完整门禁
退出 `0`。首轮失败、精确重跑和最终成功均保留为测试事实，不把抖动隐藏成从未发生。

## 后续交付门禁

- 独立严格只读代码/功能复核已完成：P0/P1/P2/P3 均为 `0`，merge recommendation
  为 `APPROVE`（限快速操作中心范围）。复核没有修改、提交、推送或清理工作树；
  实际重跑 `cargo test -p keelshell-app --locked --offline command_palette
  -- --nocapture --test-threads=1` 得到 `16 passed / 1091 filtered out`，
  `git diff --check` 退出 `0`；
- 源码提交和 SHA 回读已完成；本记录随文档收口提交继续保留上述证据。

## 证据边界

本记录的 headless GPUI 回归不代签 macOS、Windows 或 Linux 原生窗口；不代签完整
IME、VoiceOver、真实 SSH/Podman 长流程、外部 AI provider、签名、公证、安装或
当前源码包。新建窗口、分屏和布局保存暂时继续使用现有工具栏入口，后续动作目录
扩展需要新增独立回归。
