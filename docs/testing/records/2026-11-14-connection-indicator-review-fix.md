# 连接状态灯独立审查修复 — 2026-11-14

## 问题与设计约束

首轮独立审查对本地提交 `98f8f9d0558098061563fcb9c24f500551fb361a`
返回 `REQUEST_CHANGES`，P0/P1/P3 均为 `0`，唯一 P2 是会话标签和工作区底部
状态灯把“无结束原因”当作已连接。恢复占位、Starting 和有 sibling 标签的空活动
面板都可能没有结束原因，成功色因此与真实状态矛盾。

修复把两个位置统一到 `Workspace::session_status_color`，读取当前终端与恢复
状态，不依赖翻译后的文字：

| 实际状态 | 主题语义色 |
| --- | --- |
| 没有活动终端，包括有 sibling 标签的空活动面板 | `muted` |
| 恢复的可用连接或未保存连接占位 | `warning` |
| 恢复的缺失连接 | `danger` |
| 非恢复占位，当前 `is_open()` 为真 | `success` |
| Starting、已结束或其他不可用终端 | `warning` |

只有当前就绪且允许输入的终端可呈现成功色，恢复占位不会获得连接能力。视图继续
只消费 registry/Palette 语义 token，不按主题 ID 分支；不修改 transport、
命令授权、恢复连接动作或持久化协议。

## 实际绘制回归

新增 `workspace/tests/connection_indicators.rs` 并在 app 测试模块注册：

- `connection_indicators_follow_lifecycle_and_active_tab_in_all_themes`：
  通过真实 watch 生命周期切换 Starting → Ready → Ended，在三套主题、
  明暗模式和中英文中检查两个标签与活动状态栏的颜色；通过真实 tab 点击验证
  活动目标切换，并确认没有产生终端 Write；
- `restored_and_empty_panes_never_paint_connected_indicators`：
  从真实持久化布局恢复 Available、Unbound、Missing 占位，切换活动标签并检查
  状态颜色；实际点击空面板，确认 sibling 标签仍在时底部状态灯使用中性色，
  不开始连接或创建 route。

断言从 `Window::painted_quads()` 读取与目标 indicator bounds 对应的真实背景，
不只验证测试专用属性、纯函数返回值或 AX label。此处是受控 GPUI 绘制证据，
没有把 headless quad 检查当作原生像素或辅助技术验收。

## 修复前后回执

所有命令通过本轮隔离官方 Cargo home、当前 Rust `1.100.0` 绝对工具链和已有
`target-r2` 串行执行；源码没有切换 checkout，正常源码增量重新编译，不借用
其他工作树的应用产物。执行原脚本和旧回执不覆盖。

| 日志前缀 | 实际命令 | exit | 结果 |
| --- | --- | --- | --- |
| `indicator-regression-before-fix` | `cargo test -p keelshell-app --locked --offline connection_indicators -- --test-threads=1` | `101` | `0 passed / 2 failed / 1134 filtered`，两项均捕获错误成功色 |
| `indicator-regression-after-fix` | 同上 | `0` | `2 passed / 0 failed / 1134 filtered` |
| `indicator-fix-format` | `cargo fmt --all` | `0` | 当前源码格式化 |

首轮编译和第二轮编译均重新构建 app；测试运行分别耗时 `0.15s` 和 `4.90s`，
各自完整命令耗时 `13.783s` 和 `14.786s`，实际 wait，无超时。链接器仍输出
既有 `__eh_frame section too large` warning，不影响这些命令的实际退出。

修复后二进制直接 `--list` 为 `1136` 项，包含上述两条具体新用例。为让新审查
与完整门禁并行，父线程复制一份预构建二进制并核对相同摘要：

```text
eaf386ff8bcedb7a649b0167ebf469730ee9b19be1ecc44efcaf755eac2a321c
```

复核临时复制件曾位于忽略目录 `work/dependency-refresh-20261114/review-fix/`；
复核结束后已删除，摘要、清单和全部回执保留。它不是独立重新编译。Cargo 输入
和三个 Rust 文件的冻结摘要保存为
`indicator-fix-inputs-r1.json`，基线为 `98f8f9d`。后续提交必须核对与冻结源码
逐字节一致；旧 `1134` 项清单与二进制摘要、旧完整门禁和首轮独立审查结论均保留各自
历史归属，不代签这份修复。

## 完整门禁与新的独立复核

作者完整 `python3 scripts/check.py` 已实际 wait 并退出 `0`。回执
`indicator-fix-full-gate-r1.exit.json` 记录 UTC `2026-11-14T01:45:02.775525Z`
开始，`01:54:42.137556Z` 完成，耗时 `579.351s`，未超时。真实结果为：

| 范围 | 结果 |
| --- | --- |
| 直接依赖 `x.y` 策略、fmt、workspace 严格 Clippy | 全部通过 |
| Python unittest | `131 run / 1 skipped`，即 `130 passed` |
| workspace ordinary Rust | `2442 passed / 0 failed / 23 ignored` |
| 其中 app | `1134 passed / 0 failed / 2 ignored`，清单 `1136` 项 |
| doctest | `14 passed / 0 failed`，其中含 `2` 项 compile-fail |
| readiness / observer / small-stack controller | 三组均通过 |

ordinary、app 子集、doctest 不重复相加；按原始日志分段的摘要保存在
`indicator-fix-test-summary-r1.json`。门禁完成后再次核对冻结的 Cargo 和 Rust
输入，字节未变化。

另一位新审查 agent 对基线 `98f8f9d` 加冻结输入返回
`APPROVE`，P0/P1/P2/P3 均为 `0`，首轮唯一 P2 对该输入关闭。5 个显式输入
的摘要在复核开始/结束均一致，补充核对的 519 个 Rust/Cargo 输入也无变化。
复核时修复提交尚不存在，不能声称该报告已审查过未来提交或最终文档快照；父线程
需在本地提交后单独核对 Git 对象与冻结源码的字节一致性。

审查独立执行作者预构建复制件，未独立重编译；测试、清单与差异检查共
8 条命令均 exit `0`，清单 `1136` 项。6 组测试实际为：

| 过滤范围 | 通过 | 失败 / 忽略 | 过滤 |
| --- | --- | --- | --- |
| `connection_indicators` | `2` | `0 / 0` | `1134` |
| `workbench_layout` | `12` | `0 / 0` | `1124` |
| `startup_restore` | `15` | `0 / 0` | `1121` |
| `design::tests` | `4` | `0 / 0` | `1132` |
| 静默 Ready/Ended 生命周期精确用例 | `1` | `0 / 0` | `1135` |
| `terminal::inactive_tests` | `2` | `0 / 0` | `1134` |

合计 `36 passed / 0 failed / 0 ignored`；每条测试使用独立临时数据、
单线程、`180s` 上限，均实际 wait。独立报告于 UTC `01:54:05Z` 完成，当时
作者完整门禁尚未结束；作者随后收取的 exit `0` 与非作者定向证据分别保留。
状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_FROZEN_SOURCE_REVIEW_APPROVED`，
提交绑定从[当前交接](../../HANDOFF.md)接续。

独立报告、源码快照、二进制清单和逐命令回执位于
`work/dependency-refresh-20261114/review-fix/`。两位本轮审查 agent 已关闭，
实现 worktree 已归档，复核临时二进制与 runtime 数据已清理，日志和失败原件
保留；当前有效 target 继续作为开发缓存，没有清理其他任务的工作树或 target。

公司内网暂不可用，只做本地提交，不执行 push。最近一次桌面观察实际读回锁屏，
原生主题与紧凑窗口保持 `HOLD`；Windows/Linux、IME、VoiceOver/NVDA、真实远端、
Podman、外部 provider、签名与安装未由本片验证。记录只描述工程和受控渲染范围。
