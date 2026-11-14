# 命名隧道 owner 与恢复卡片测试记录 — 2026-11-14

实现源码 exact SHA：

- owner/reconnect：`90fbca73fc52e528d82b23cdd3d57d125c72458d`
- 恢复卡片视觉与可达性：`1c8df52`

本记录使用当前 checkout 的专用 target：

`work/tunnel-owner-ui-polish-20261114/target`

日志目录：

`work/tunnel-owner-ui-polish-20261114/logs/`

## 结论

作者 focused 验证通过，当前状态为 `AUTHOR_FOCUSED_PASS / NON_AUTHOR_REVIEW_PENDING`。
本记录没有把 headless GPUI、loopback fixture 或本地 focused gate 写成原生桌面、
真实客户 SSH、Podman、外部 AI provider 或发布验收。

## 实际命令与结果

所有 Cargo 命令使用 Rust/Cargo `1.100.0`、`--locked --offline` 和上述专用
target；所有测试均由当前源码重新编译的测试二进制执行。

| 验证 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | exit `0` |
| `git diff --check` | exit `0` |
| app `tunnel_rule_tests` | `19 passed / 0 failed / 1138 filtered` |
| app `tunnels::rules::tests` | `18 passed / 0 failed / 1139 filtered` |
| core `reconnect` | `18 passed / 0 failed` |
| core `tunnel_runtime` | `10 passed / 0 failed` |
| core strict Clippy | exit `0` |
| app strict Clippy | exit `0` |

新增或补强的回归包括：

- SSH 会话替换后 `TunnelRuntimeOwner` 轮换，旧停止审核被清空，原 worker 仍由
  原 panel 负责有界清理；
- recovery plan 的旧 owner 不能通过 `confirm_for`；
- 恢复区域、恢复卡片和状态 callout 的 `Group`/`Status` role、双语 AX label、
  稳定 ID 和状态/动作几何可达性；
- 规则变化、当前连接不匹配、显式 Inspect/Review/Confirm 和 Discard 的已有
  恢复回归继续通过；
- focused 测试中包含已提交的命名规则管理回归和 owner replacement 回归。

app 链接阶段出现已知 warning：
`__eh_frame section too large (max 16MB) to encode dwarf unwind offsets in compact unwind table`。
相关命令 exit `0`，该 warning 没有被写成失败。

## 未验证边界

本轮没有运行 macOS 真实窗口、Windows/Linux 原生窗口、真实客户 SSH、Podman
多节点转发、断电恢复、跨进程旧 listener 探测、完整 IME/VoiceOver/NVDA、
签名、公证、安装升级、发布流程或业务验收。完整工程门禁尚未在本轮最终源码
上重复运行；后续复核必须固定当前 exact SHA 并独立建立 target。

