# 监控面板视觉层级与紧凑几何测试记录 — 2026-11-14

日期：2026-11-14（Asia/Shanghai）
源码起点：`31ca92b` 加本地监控面板改动；本记录随功能切片提交
设计决策：[ADR 0125](../../adr/0125-monitor-panel-visual-hierarchy.md)

## 本次范围

本片为主机监控 panel 增加控制区状态、资源指标卡片、网络接口卡片、监听端口
分组、文件系统分组、能力状态徽章和完整的双语 AccessKit 分组语义。窄 rail
中的磁盘设备选择器继续使用 160px 内层视口，设备行保持连续紧凑布局；能力状态
徽章与检测年龄位于详情列，长英文不会撑大整行。

本片只调整 UI、布局和测试可观测性，没有修改远端采样脚本、监控缓存、暂停/恢复、
进程审核、端口探测或磁盘 I/O 计算。

## 定向验证

在当前本地 worktree、Rust 1.100.0、锁定依赖和隔离 GPUI 测试数据下执行：

| 命令或范围 | 结果 |
| --- | --- |
| 磁盘末条滚动、选择和工作区边界回归 | `1 passed / 0 failed` |
| 能力状态、双语徽章、年龄和键盘可达性回归 | `1 passed / 0 failed` |
| `cargo test -p keelshell-app --locked --offline monitor::tests -- --test-threads=1` | `21 passed / 0 failed` |
| `cargo fmt --all` | 通过 |
| `git diff --check` | 通过 |

此前发现并修复的失败证据：

- 磁盘设备列表增加间距后，末条按钮的 bounds 只能部分落在 160px 内层视口，
  旧测试虽然看到 `visible()`，但点击中心落在裁剪区外；回归现在要求末条 bounds
  完整包含于 `monitor-disk-devices`。
- 能力徽章与长英文名称并排时，窄栏 flex 宽度争用会把第二行撑到数百像素；
  状态徽章改为详情列元信息后，900×580 双语主题矩阵恢复可滚动和可读。

## 门禁结果

本片完整工程门禁已经完成：

- `cargo clippy --workspace --all-targets --locked -- -D warnings`；
- `python3 scripts/check.py`，依赖版本策略、Python `131 run / 1 skipped`、格式、
  workspace 测试、三个 AI controller 均通过；
- workspace Rust 测试 `2454 passed / 0 failed / 23 ignored`；
- `keelshell-app` 测试 `1132 passed / 0 failed / 2 ignored`，测试清单为 `1134`
  项；本片新增监控层级回归已实际进入测试二进制；
- 14 个正常 doctest 和 2 个 compile-fail doctest 均通过。

仍需完成：

- 固定提交后的独立严格只读代码与功能复核；
- macOS、Windows、Linux 原生窗口，IME、VoiceOver/NVDA、真实远端监控和 Podman
  联调。

这些边界不能由 headless GPUI 测试或本地编译代签。

## 结论

监控面板视觉切片已经通过 21 项监控定向回归，两个由窄 rail 布局触发的真实
可达性问题也已由产品布局和回归合同共同关闭。完整工程门禁已通过，提交后的独立
复核与原生/真实远端边界仍保持开放。
