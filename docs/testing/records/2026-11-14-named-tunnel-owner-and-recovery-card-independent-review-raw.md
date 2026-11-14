# 命名隧道 owner 与恢复卡片独立复核原文 — 2026-11-14

复核类型：独立、严格只读源码复核。reviewer 固定 exact SHA：

`36a1fcdd679b81dc4cc774a3efa7a2ceb330d1ed`

独立 target：

`/tmp/keelshell-review-36a1fcd-target`

## 原始结论

结论为 `REQUEST_CHANGES`，原因是一个 P3 仓库质量问题；没有发现 P0、P1 或
P2 功能、安全、状态或 UX 缺陷。

- P0：`0`
- P1：`0`
- P2：`0`
- P3：`1`

P3 证据：

- 文件：
  - `docs/adr/0129-named-tunnel-owner-and-recovery-card.md:73`
  - `docs/testing/records/2026-11-14-named-tunnel-owner-and-recovery-card.md:59`
- 触发命令：`git diff --check 540f9fc^ HEAD`
- 结果：exit `2`，两份新文档末尾报告 `new blank line at EOF`。
- 影响：没有运行时或安全影响，但当时 exact HEAD 没有通过仓库差异质量门禁。

作者在复核后删除两处多余空行；该修复只涉及文档。

## 独立验证结果

- `keelshell-core` `tunnel_runtime`：`10 passed / 0 failed`
- `keelshell-core` `reconnect`：`18 passed / 0 failed`
- `keelshell-core` profile-sync unit filter：`7 passed / 0 failed`
- `keelshell-core` `profile_sync_review`：`11 passed / 0 failed`
- `keelshell-app` `tunnel_rule_tests`：`19 passed / 0 failed / 1138 filtered`
- `keelshell-app` `tunnels::rules::tests`：`18 passed / 0 failed / 1139 filtered`
- 独立 tunnel lifecycle regression：`3` 个单测通过
- app 测试清单：`1159` 项，其中 `1157` 项可运行、`2` 项 ignored
- core/app 严格 Clippy：exit `0`
- `cargo fmt --all -- --check`：exit `0`
- 实现差异检查至 `1c8df52`：exit `0`

复核确认的关键边界包括：会话替换时轮换 owner、恢复与停止审核失效、
恢复入口使用带当前 owner 的 `confirm_for`、停止审核绑定 owner/token/rule/worker
集合、恢复卡片只读直到显式 Inspect/Review/Confirm，以及使用共享 palette 和
component token。

## 未验证边界

没有把 macOS/Windows/Linux 原生窗口、VoiceOver/NVDA、完整 IME、真实客户 SSH、
Podman、断电、跨进程 stale-listener、任意网络文件系统、多机 profile-sync 或
发布验收写成通过。完整 `scripts/check.py` 门禁不属于该 reviewer 的独立复核；
作者门禁日志单独保存在
`work/tunnel-owner-ui-polish-20261114/logs/full-gate-36a1fcd.log`。
