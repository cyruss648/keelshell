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

断点修正后的最终完整 app 重跑为 `1087 passed / 2 ignored`；随后当前源码上的
`python3 scripts/check.py` 退出 `0`，Python 131 passed / 1 skipped、格式、严格
workspace Clippy、workspace tests、doctests、依赖策略和 readiness/observer/
small-stack AI controller 均通过。workspace 测试保留已有 linker
`__eh_frame section too large` warning；没有把它写成测试失败。

## 当前精确包与原生短验

提交 `2d05525c00d21455420d25cce675bacdedf8a73a` 的 macOS release 构建退出 `0`，
生成包 `work/native-cua-20261107/package-r9/KeelShell.app`。包结构检查：

```text
python3 packaging/inspect_native.py \
  --platform macos \
  --target aarch64-apple-darwin \
  --stage work/native-cua-20261107/package-r9
```

退出码为 `0`，manifest 绑定 exact source commit，三项 SHA-256 为：

- manifest：`8cceb76d2807683fc5bd264743834b110f2c94781ff6dffe59ea972b858eb378`
- app：`524ae37ff3e45694072a906421edb5a2e575da2566a1ca48f237787cecb1d06a`
- MCP：`94eaf83f8bbb23ae4db7cc190c9c06332ddbc19b4bf9246dcdf35f43b3645d8f`

在解锁后的 macOS 桌面启动当前包，观察到 `Remote workspace` 在默认窗口保持单行，
主题与外观对话框可以打开和关闭，测试进程退出且 `data-r9` 已删除。尝试通过
系统窗口调整 API 设置 900×580 时，GPUI 无边框窗口 accessibility frame 没有
接受调整，截图仍为默认窗口尺寸；因此当前包没有获得 900×580 原生通过。

## 独立复核状态

当前 exact source `2d05525c00d21455420d25cce675bacdedf8a73a` 已完成独立只读复核。
复核结果为 P0/P1/P2/P3 均为 `0`，范围内 merge recommendation 为 `APPROVE`。
实际检查包含：

- `workbench_regions_fit_all_registered_themes_without_changing_drafts`：1 passed；
- `toolbar_groups_keep_action_order_and_accessible_names_at_workbench_widths`：1 passed；
- `cargo fmt --all -- --check`：exit 0；
- `git diff --check` 与 `git diff --check HEAD^ HEAD`：exit 0。

复核确认了 900/1280/1440 断点、112px 紧凑上下文、上下文两行的单行省略、
品牌区显示规则和双语可访问名称。复核没有把作者的完整工程门禁、当前包原生短验、
Windows/Linux 桌面、IME、VoiceOver 或外部 provider 联调重复计入独立证据。

## 边界

这些检查证明 GPUI Kit 受控布局、主题矩阵和可访问名称回归；macOS 当前源码包的
真实窗口截图、Windows/Linux 原生桌面、IME、VoiceOver、真实 SSH/Podman 联调和
外部 AI provider 仍需分别形成证据。
