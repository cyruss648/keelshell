# 连接库选中层级与主动作回归 — 2026-11-07

## 范围

本片验证连接库在宽表格和窄卡片中的选中反馈、Connect 主动作层级、行内动作
可达性和双语无障碍语义。源码范围：

- `crates/keelshell-app/src/workspace/library_view.rs`
- `crates/keelshell-app/src/workspace/tests/library.rs`

最终实现提交：
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9`。

## 实现结果

- 连接行从 `library_selection` 读取一次 `selected` 快照；
- 选中行公开 `aria-selected`，使用主题化 `surface_active` 和 `accent` 左边界；
- 非选中行保留交替底色，并在悬停时使用 `surface_hover`；
- 宽表格和窄卡片继续使用相同选中行路径；
- 活动连接库中的 Connect 使用 GPUI Kit `primary` 变体；
- 回收站仍只提供 Restore 和永久删除；
- 主题视图没有按主题 ID 分支，所有颜色来自当前 `Palette`。

## 定向验证

第一次运行定向连接库测试时，已有 26 项通过，新增测试失败。失败原因是测试
把 GPUI observation 的 `label()`（可访问标签）误当成按钮可见文本，实际返回：

```text
left:  Some("取消选择: 已选主机 · operator@selected.invalid:22")
right: Some("☑")
```

失败发生在测试代码的断言层，没有产生源码行为失败。修正为验证双语完整可访问
标签、行的 selected 语义和主动作可达性后，精确重跑与完整定向模块均通过。
随后又修复了选中行悬停覆盖 active 状态和 `Role::Group` 不属于原生
selectable item 语义的问题，最终提交统一使用 `Role::List` / `Role::ListItem`
以及未选中行的 `surface_hover`。

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `git diff --check` | passed |
| `cargo test -p keelshell-app --locked 'workspace::tests::library::selected_library_row_exposes_state_and_keeps_primary_action_reachable' -- --nocapture --test-threads=1` | 1 passed |
| `cargo test -p keelshell-app --locked 'workspace::tests::library::' -- --nocapture --test-threads=1` | 27 passed / 0 failed |
| `cargo clippy -p keelshell-app --locked --bin keelshell-app --no-deps -- -D warnings` | passed |
| `cargo test -p keelshell-app --locked -- --list` | 1091 tests / 0 benchmarks |
| `python3 scripts/check.py` | exit 0 |

完整门禁实际完成 Python `131 run / 1 skipped`、app `1089 passed / 2 ignored`，
workspace 其它 crate 与集成测试、严格 workspace Clippy、doctests、依赖版本
策略和 readiness/observer/small-stack AI controller。构建期间保留既有 macOS
linker warning：`__eh_frame section too large`；没有导致命令失败。

## 验收边界

本片只形成源码和 headless GPUI 证据。`package-r10` 等此前包不包含最终提交
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9`，不能代签本片的真实桌面颜色或
按钮样式。当前源码的 macOS 包、
Windows/Linux 原生窗口、完整键盘/IME/VoiceOver、真实 SSH/Podman 长流程和
外部 AI provider 仍需独立验收。

## 独立复核

新的独立严格只读复核以源码提交
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9` 为基线完成，复核 agent 未编辑、
提交、推送或清理工作树。结果如下：

| 严重级别 | 数量 |
| --- | ---: |
| P0 | 0 |
| P1 | 0 |
| P2 | 0 |
| P3 | 0 |

复核确认选中行的 `surface_active`/`accent` 与未选中行的 `surface_hover` 不互相
覆盖，容器/行使用 `Role::List` / `Role::ListItem` 及完整 set position/size
语义，宽表格与窄卡片共用 `connection_library_row`，Connect 使用 primary，
主题只读取 registry/Palette/token。独立执行的精确测试为：

```text
cargo test -p keelshell-app --locked --offline \
  workspace::tests::library::selected_library_row_exposes_state_and_keeps_primary_action_reachable \
  -- --exact --test-threads=1
```

结果为 `1 passed / 0 failed / 1090 filtered out`。复核未覆盖全量工程门禁、
原生 macOS/Windows/Linux、真实 VoiceOver/IME/键盘焦点、真实连接流程和像素/
对比度验收。范围内 merge recommendation：`APPROVE`。
