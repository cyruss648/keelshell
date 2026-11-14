# 监控面板视觉层级与紧凑几何测试记录 — 2026-11-14

日期：2026-11-14（Asia/Shanghai）
源码起点：`31ca92b6b7e37680f0b7363f1bb42bd9fd9dd0e8`
功能提交及独立复核基线：`aab1718f6682a82860ded8f0510ab7ccb843a902`
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
- workspace ordinary Rust 测试 `2440 passed / 0 failed / 23 ignored`；
- `keelshell-app` 测试 `1132 passed / 0 failed / 2 ignored`，测试清单为 `1134`
  项；本片新增监控层级回归已实际进入测试二进制；
- 14 项 doctest 均通过，其中包含 2 项 compile-fail；不再额外相加。

作者完整门禁会话 `74916` 实际读回退出 `0`；该回执返回后再次轮询会得到
`Unknown process id`，不表示退出码丢失。作者原始日志的 SHA-256 为
`cb84412c38f2b7b4b0529bf9d0ed7cb0cbed426cac959cd8ef56441d1c832f5c`，
app 清单 SHA-256 为
`aa5e3c2c7f18b858007b53cd3d5c9a220115e56bdd2540f7201ac875975d663c`。
普通 Rust 测试与 doctest 通过数合计为 2454；首轮文档误将此合计写成普通测试数，
并重复列出 compile-fail。本次按日志分段纠正统计，原始结果未被覆盖。

## 独立复核

非作者 agent 对固定 SHA `aab1718f6682a82860ded8f0510ab7ccb843a902` 在独立
worktree 与独立 Cargo target 中只读复核，结论为 `APPROVE`，
P0/P1/P2/P3 均为 `0`。

| 独立命令或范围 | 实际结果 |
| --- | --- |
| app `--list` | `1134 tests`，退出 `0`，重新编译当前固定源码 |
| `monitor::tests` | `21 passed / 1113 filtered out` |
| 新增层级回归，`--exact` | `1 passed / 1133 filtered out` |
| 磁盘几何与末条选择回归，`--exact` | `1 passed / 1133 filtered out` |
| `workspace::tests::workbench_layout` | `12 passed / 1122 filtered out` |
| app `--all-targets` 严格 Clippy | 退出 `0` |
| fmt check 与固定提交差异检查 | 均退出 `0` |
| app 全量测试 | 主动中止，退出 `130`，不计为通过 |

第一次清单包装命令因 zsh 的保留变量 `status` 退出 `1`，当时 Cargo 尚未运行；
改用普通变量后成功。第一次复核报告前的重复等待被工具循环保护停止，随后恢复
同一 agent，读取实际回执并完成固定源码审查；循环停止本身不计为批准。
测试清单编译仍有既有 macOS `__eh_frame section too large` linker warning，
退出码为 `0`。独立复核未重跑 workspace 全量、doctest 或完整工程门禁，也未
独立证明作者全量计数。中止工具已返回退出 `130`，父级随后检查未发现该审查路径
或测试二进制的遗留进程；没有额外声称具备当时的 OS 进程树回收回执。

原始作者日志、清单和筛选后的独立工具回执保存在忽略目录
`work/monitor-panel-20261114/`。该目录用于本机审计，不作为跨平台原生验收证据。
审查 worktree 与独立 Cargo target 已在保存回执、关闭 reviewer、确认没有相关
遗留进程后清理，未修改其他工作树。

仍需完成：

- macOS、Windows、Linux 原生窗口，IME、VoiceOver/NVDA、真实远端监控和 Podman
  联调。

这些边界不能由 headless GPUI 测试或本地编译代签。
本片另外完成精确 macOS arm64 包静态检查；桌面通道读回锁屏，图像输入与输出
无法形成可靠对应，原生视觉仍为 `HOLD`，见
[原生观察记录](2026-11-14-monitor-panel-native-smoke.md)。

## 结论

监控面板视觉切片已经通过 21 项监控定向回归，两个由窄 rail 布局触发的真实
可达性问题也已由产品布局和回归合同共同关闭。完整工程门禁与固定源码独立复核已通过，
原生/真实远端边界仍保持开放。
