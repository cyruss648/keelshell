# 命名隧道 owner 与恢复卡片测试记录 — 2026-11-14

实现源码 exact SHA：

- owner/reconnect：`90fbca73fc52e528d82b23cdd3d57d125c72458d`
- 恢复卡片视觉与可达性：`1c8df52`

本记录使用当前 checkout 的专用 target：

`work/tunnel-owner-ui-polish-20261114/target`

日志目录：

`work/tunnel-owner-ui-polish-20261114/logs/`

## 结论

作者 focused 验证通过，当前状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_PENDING`。
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

## 作者完整门禁

最终命令为：

`CARGO_TARGET_DIR=work/tunnel-owner-ui-polish-20261114/full-target python3 scripts/check.py`

源码门禁绑定的实现基线为 `36a1fcd`；本轮文档只补充门禁、复核和交接证据，不改变
产品源码。最终日志见
`work/tunnel-owner-ui-polish-20261114/logs/full-gate-final-rerun.log`，末尾
`[full-gate-exit] 0`。实际记录包括：

- 依赖版本策略通过；
- Python 工程检查 `131` 项运行、`1` 项跳过，exit `0`；
- `cargo fmt --all --check` 通过；
- workspace 严格 Clippy 命令完成；
- app workspace 测试 `1155 passed / 0 failed / 2 ignored`；
- core 单元测试 `214 passed / 0 failed / 0 ignored`，其余 integration test
  result 均为 `0 failed`；
- AI 本地 agent process、readiness、observer 和 controller 场景均报告
  assertions/scenario groups passed，未调用供应商 CLI 或模型。

第一次完整门禁日志
`work/tunnel-owner-ui-polish-20261114/logs/full-gate-final.log` 曾在
`local_agent_process --controller-small-stack` 中因 `EmptyReply` 返回 exit `1`；
同一场景单独复跑 exit `0`，随后完整门禁 rerun exit `0`。该时序失败没有修改
源码。早先基线门禁日志
`work/tunnel-owner-ui-polish-20261114/logs/full-gate-36a1fcd.log` 也保留。
最终文档工作树另行执行 `git diff --check` 和
`cargo fmt --all -- --check`。

## 独立严格只读复核

独立 reviewer 固定 exact SHA
`36a1fcdd679b81dc4cc774a3efa7a2ceb330d1ed`，使用独立 target
`/tmp/keelshell-review-36a1fcd-target`，原始报告见
[独立复核原文](2026-11-14-named-tunnel-owner-and-recovery-card-independent-review-raw.md)。
复核结论为 `REQUEST_CHANGES`，仅有一个 P3 文档质量问题：

- P0 `0`、P1 `0`、P2 `0`、P3 `1`；
- 两份本记录与 ADR 文件末尾存在多余空行，触发
  `git diff --check 540f9fc^ HEAD` exit `2`；
- 没有发现运行时、状态、安全或 UX 缺陷；
- core `tunnel_runtime` `10/10`、`reconnect` `18/18`、profile-sync `7/7`
  与 `11/11`，app `tunnel_rule_tests` `19/19`、`tunnels::rules::tests`
  `18/18`，严格 Clippy 均 exit `0`；
- app 测试清单为 `1159` 项，其中 `1157` 项可运行、`2` 项 ignored。

作者已在复核后删除两处多余空行，并把当前 HEAD 改为现场读取，把门禁证据明确
绑定到源码实现基线，避免把某个文档提交误写成永远有效的当前 HEAD。该文档修正
不改变产品源码；原生窗口、真实 SSH、Podman、完整 IME/VoiceOver/NVDA、签名、
公证、安装升级和发布验收仍为未验证边界。

## 未验证边界

本轮没有运行 macOS 真实窗口、Windows/Linux 原生窗口、真实客户 SSH、Podman
多节点转发、断电恢复、跨进程旧 listener 探测、完整 IME/VoiceOver/NVDA、
签名、公证、安装升级、发布流程或业务验收。
