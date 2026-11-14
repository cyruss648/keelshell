# 命名隧道 owner 与恢复卡片最终独立复核原文 — 2026-11-14

复核 exact SHA：

`79c9090a7754b049f5a91fe3b71f96e15710d66b`

独立源码 checkout：

`/private/tmp/keelshell-review-79c9090a`

独立 Cargo target：

`/private/tmp/keelshell-review-79c9090a-target`

## 结论

`APPROVE`

- P0：`0`
- P1：`0`
- P2：`0`
- P3：`0`

源码 provenance 一致：`36a1fcd` 是 `79c9090` 的祖先，
`git diff 36a1fcd..79c9090` 只有文档文件；非文档源码差异检查 exit `0`。
共享 checkout 和独立复核 checkout 均 clean。

## 文档与门禁证据

- 当前交接不再把旧提交写成当前 HEAD，要求现场执行 `git rev-parse HEAD`；
- 完整门禁证据明确绑定到 `36a1fcd` 源码实现基线；
- 保留了第一次 `EmptyReply` 失败、focused rerun 通过和完整 rerun exit `0`
  的完整链路；
- 之前的 EOF 空白问题已消失，`git diff --check 540f9fc^..79c9090` 和
  `git diff --check 36a1fcd^..79c9090` 均 exit `0`；
- 历史 P3 原始报告仍作为历史证据保存，没有被表述成当前 HEAD 状态。

## 独立 focused 验证

- core `tunnel_runtime`：`10 passed / 0 failed`
- core `reconnect`：`18 passed / 0 failed`
- app `tunnel_rule_tests`：`19 passed / 0 failed / 1138 filtered`
- app `tunnels::rules::tests`：`18 passed / 0 failed / 1139 filtered`
- 更宽的 app `tunnels` filter：`23 passed / 0 failed / 1134 filtered`
- app 测试清单：`1157` 项，exit `0`
- core/app 严格 Clippy：exit `0`
- `cargo fmt --all -- --check`：exit `0`
- 基线与最终差异检查：exit `0`

实现复核确认：会话替换时 owner 轮换；恢复确认绑定 token、当前 owner、状态
revision 和候选内容；挂起时清理旧审核并保留旧 worker 的清理责任；停止审核绑定
token、owner、规则 ID 和 worker 集合；恢复卡片使用稳定 ID、Group/Status AX 语义、
显式 Inspect/Review/Discard 和只读详情。

## 未验证边界

macOS 真实窗口、Windows/Linux 原生窗口、真实客户 SSH、Podman 多节点转发、断电
恢复、跨进程 stale-listener、完整 IME/VoiceOver/NVDA、签名、公证、安装升级、
发布执行和业务验收仍未验证。作者完整 `scripts/check.py` 没有由该 reviewer
独立重跑，而是检查保存日志与独立 focused 证据。
