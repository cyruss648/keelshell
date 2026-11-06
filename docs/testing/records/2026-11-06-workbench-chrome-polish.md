# 2026-11-06 — 工作区外壳层级视觉修整

## 范围

本切片只调整高频工作区外壳的视觉层级：顶部工具栏、连接导航选择器、
会话栏、底部工具栏和状态栏。功能、会话实体、焦点路由、审核权限和持久化
协议保持现有实现。会话栏从 38px 调整为 40px，并将内边距/间距从 `px_1` /
`gap_1` 调整为 `px_2` / `gap_2`；900×580 下的整体控件可达性和最小窗口约束
仍由工作区专项测试覆盖。

活动会话现在使用当前主题的 `surface`、`accent` 边界和半粗底边；其他会话
使用 `canvas` 与普通边界。工作区外壳统一使用语义 `chrome`，连接导航选择器
与树内容形成轻微层级差。活动标题使用半粗字重。三主题继续通过注册表和
Palette 提供颜色，业务代码没有主题 ID 分支。用户可以在设置中选择霜白、
曜石或雾紫；新增主题只需注册新的 family 元数据、浅深 Palette 和组件形状，
不需要修改工作区业务视图。

## 主题 token 补全

集成后的非作者源码复核进一步指出，监控面板的文件系统容量条仍直接使用
`0x448eaa`，因此不会随 Frost、Graphite、Lavender 的明暗变体切换。修复提交
`ef110d2` 在 `Palette` 中增加独立的 `resource` 语义 token，为三套主题的六个
变体提供资源色，容量条改用当前 `visual.resource`；设计测试同时检查 token
非空以及资源色与进度轨道边界至少 3:1 的对比度。该修复不改变采样、容量计算、
滚动、选择或监控权限行为。

## 定向验证

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | exit 0 |
| `git diff --check` | exit 0 |
| `cargo test -p keelshell-app design::tests -- --test-threads=4` | exit 0；4 passed，1045 filtered；包含实际 50% alpha 合成焦点环和六个资源 token 的对比度 |
| `cargo test -p keelshell-app disk_ -- --test-threads=4` | exit 0；10 passed，1039 filtered；覆盖文件系统容量和磁盘采样相关回归 |
| `cargo test -p keelshell-app independent_disk -- --test-threads=4` | exit 0；1 passed，1048 filtered；覆盖主题切换后的独立磁盘采样 |
| `cargo test -p keelshell-app --locked workbench_layout -- --test-threads=4` | exit 0；5 passed，990 filtered |
| `cargo test -p keelshell-app --locked 'themes::' -- --test-threads=4` | exit 0；7 passed，988 filtered |
| `cargo test -p keelshell-app --locked workspace::tests::navigation -- --test-threads=4` | exit 0；21 passed，974 filtered |
| `cargo clippy -p keelshell-app --tests --all-targets -- -D warnings` | exit 0 |

已有工作区矩阵覆盖 900×580、1280×840、1440×900，三套主题、明暗模式和
中英文；本轮执行了工作区、主题和连接导航专项。

## 完整工程门禁

| 门禁 | 结果 |
| --- | --- |
| `python3 scripts/check.py` | exit 0 |
| Python 单元测试 | 131 passed，1 skipped |
| workspace Rust 普通测试 | 2,238 passed，23 ignored |
| rustdoc tests | 14 passed |
| workspace Clippy (`-D warnings`) | exit 0 |
| 依赖版本策略、格式、附加 controller | exit 0 |
| readiness / observer / small-stack controller | 全部通过；自有子进程均回收 |

完整门禁覆盖了本切片之外的现有代码，因此它证明当前工作树的工程回归
通过，不代表完整产品能力、发行包或三平台桌面验收已经完成。

## 主题 token 修复的复核状态

来源复核发现的容量条硬编码色已由 `ef110d2` 修复，修后作者定向测试、严格
Clippy 和完整 `python3 scripts/check.py` 均通过。新的非作者只读复核尚未完成；
在复核固定最终源码前，不把该 P2 写成已关闭。

## 复核修订

第一轮非作者复核发现浅色主题的普通焦点环会经过 GPUI Kit 50% alpha 合成，
原始 `focus` token 虽然满足未混合测试，却不足以保证实际绘制后的可见性。
本轮将霜白、曜石、雾紫浅色变体的 `focus` 调整为更深的语义颜色，并在
`design/tests.rs` 中按 Kit 的 alpha 规则对 `surface`、`chrome` 和 `sidebar`
逐项检查合成后的至少 3:1 对比度。该修订后的 design、工作区、主题、导航专项、
Clippy 和完整工程门禁均通过。

## 独立复核与原生边界

第一轮非作者只读复核发现的焦点环 P2 已按上节修复。第二轮非作者只读复核
重新检查了来源视觉链 `a54b978`（语义 token）与 `643248d`（工作区外壳），
确认三主题 token、
Kit 状态映射、900×580、会话权限不变性以及文档证据一致。限定复核运行
design 与 workbench_layout 两项短测试，均 exit 0；结论为
`P0=0, P1=0, P2=0, P3=0 / NO_BLOCKER / APPROVE`。

上述结论属于来源视觉切片；视觉源码通过 `26e11c5` 合入当前组合。
当前组合的完整工程门禁和交付边界以 [当前交接](../../HANDOFF.md) 为准，
本记录不把来源门禁扩大为当前组合的原生或发行验收。

标准开发包、macOS 原生截图、VoiceOver/IME、Windows/Linux 桌面和完整跨平台
发行验收仍需单独保留证据；本切片不把受控 GPUI 测试或已有原生包证据扩大为
这些边界的完成证明。

## Hallmark 记录

这是原生 Rust/GPUI app-screen 视觉调整，浏览器 `sloplint --render` 不适用。
设计依据和语义映射见 [ADR 0113](../../adr/0113-workbench-chrome-hierarchy.md)
及 [Hallmark 原生映射](../../design/HALLMARK_NATIVE.md)。
