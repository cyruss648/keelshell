# 工具栏远程上下文布局修复记录 — 2026-11-07

## 范围

本轮修复工作区顶部工具栏在窄窗口中的远程上下文显示，并收口 1280px 断点的
动作组布局：

- `crates/keelshell-app/src/workspace/view.rs`
- `crates/keelshell-app/src/workspace/tests/workbench_layout.rs`
- `docs/adr/0115-workbench-visual-rhythm-and-context.md`

用户解锁 macOS 后的原生观察发现，工具栏分组在当前窗口宽度下会把“远程工作区”
上下文压成逐字竖排。该观察被视为实现缺陷，修复前不写成通过。

## 修复合同

- 紧凑工具栏（`width <= 1280px`）为远程上下文保留 112px 最小宽度；
- 宽工具栏仍允许上下文收缩，避免品牌区和完整动作组在 1280px 断点溢出；
- 上下文标题和当前主机值使用 `truncate()`，保持单行、隐藏溢出并显示省略号；
- 1280px 保留品牌区，同时使用紧凑动作标签；低于 1280px 隐藏品牌区；
- 视图不按主题 ID 分支，仍只消费注册表和语义 token。

## 回归与失败证据

初始布局修复为上下文设置 168px 固定最小宽度后，完整 app 测试发现真实副作用：

```text
cargo test -p keelshell-app --locked --offline -- --test-threads=4
```

结果为 `1086 passed / 1 failed / 2 ignored`。失败用例是
`workspace::tests::update_panel_close_has_a_localized_name_and_closes_the_real_panel`，
`about-updates` 在 1280px 工具栏尾部不可见。该失败促成了“紧凑动作与品牌显示
分离”的断点修正。

随后把 1280px 加入工具栏顺序回归时，旧的非紧凑动作布局再次暴露设置组越界：

```text
En/1280: workspace-settings-actions escapes toolbar
```

最终改为 1280px 保留品牌、但使用紧凑动作标签，且只在紧凑布局上设置上下文最小
宽度。精确回归结果：

| 检查 | 结果 |
| --- | --- |
| `toolbar_groups_keep_action_order_and_accessible_names_at_workbench_widths` | 1 passed |
| `workbench_regions_fit_all_registered_themes_without_changing_drafts` | 1 passed |
| `update_panel_close_has_a_localized_name_and_closes_the_real_panel` | 1 passed |
| `cargo test -p keelshell-app --locked --offline -- --test-threads=4`（断点修正前） | 1087 passed / 2 ignored |
| `cargo test -p keelshell-app --locked --offline toolbar_groups...`（加入 1280 后首次） | 0 passed / 1 failed，失败事实已保留 |

上述最终修复后的完整 app 和工程门禁需要在当前源码候选上重新执行；本记录不把
断点修正前的 `1087/2` 结果升级为最终候选证明。

## 边界

这些检查证明 GPUI Kit 受控布局、主题矩阵和可访问名称回归；macOS 当前源码包的
真实窗口截图、Windows/Linux 原生桌面、IME、VoiceOver、真实 SSH/Podman 联调和
外部 AI provider 仍需分别形成证据。
