# 工作区视觉节奏与审核上下文测试记录 — 2026-11-07

## 范围

本轮验证 `ChromeTokens`、宽窗品牌区、当前主机上下文、命令审核目标徽章、
primary 执行动作以及会话标签的主题状态。源码范围：

- `crates/keelshell-app/src/design.rs`
- `crates/keelshell-app/src/design/registry.rs`
- `crates/keelshell-app/src/design/tests.rs`
- `crates/keelshell-app/src/workspace/view.rs`
- `crates/keelshell-app/src/workspace/pane_view.rs`
- `crates/keelshell-app/src/workspace/tests/workbench_layout.rs`

## 已执行

以下命令在当前工作树执行，均使用锁文件：

```text
cargo fmt --all
cargo test -p keelshell-app design::tests --locked
cargo test -p keelshell-app workbench_regions_fit_all_registered_themes_without_changing_drafts --locked
cargo test -p keelshell-app long_session_titles_keep_close_visible_in_both_languages_and_window_sizes --locked
cargo test -p keelshell-app minimum_window_keeps_bilingual_appearance_and_workspace_actions_visible --locked
```

结果：

- 主题设计测试：4 passed，1073 filtered
- 工作区三主题/双明暗/双语/三尺寸布局：1 passed，1076 filtered
- 长会话标题关闭按钮可达性：1 passed，1076 filtered
- 900×580 双语工具栏与会话条：1 passed，1076 filtered
- `git diff --check`：通过

布局测试额外确认：900px 宽度不渲染品牌区，1280px 和 1440px 宽度渲染品牌
区并保持在工具栏内；1280px 断点使用紧凑动作标签；审核目标徽章在所有矩阵
场景内可见。

随后执行：

```text
cargo test -p keelshell-app --locked
```

首次完整 app crate 结果为 `1074 passed; 1 failed; 2 ignored`。唯一失败是
`workspace::tests::routes::reconnect::split_preserves_the_selected_remote_tool_and_its_existing_panel`
在 `crates/keelshell-app/src/workspace/tests/routes/reconnect.rs:113` 的
“actual split control must create a peer” 断言；该失败涉及既有 reconnect/
分屏夹具，未触及本轮视觉文件。精确重跑：

```text
cargo test -p keelshell-app split_preserves_the_selected_remote_tool_and_its_existing_panel --locked
```

结果为 `1 passed; 1076 filtered out`。首次完整运行的失败回执保留，不把精确
重跑写成完整 app crate 已一次性全绿；最终结论以本轮完整 workspace 门禁为准。

随后执行完整工程门禁：

```text
python3 scripts/check.py
```

结果为退出码 `0`。本次门禁实际完成 Python `131 run / 1 skipped`、workspace
测试（app `1075 passed / 2 ignored`，其余 workspace crate 与集成测试通过）、
`cargo fmt --all -- --check`、严格 workspace Clippy、rustdoc、依赖版本策略和
readiness/observer/small-stack AI controller。app 测试期间保留了一个已有的
linker `__eh_frame` 警告；它没有转化为编译或测试失败。

## 待收口

源码工程门禁已完成。macOS 原生窗口、跨平台桌面、IME、VoiceOver 和真实
安装包观察不由这些 headless/GPUI 测试代替，继续保持 `NOT_RUN` 或既有
`HOLD`，直到获得对应平台的实际证据。

