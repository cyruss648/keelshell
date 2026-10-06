# 任务记录 / Task history

在“依赖工作流”面板点击 **任务记录**，可查看本机已保存的任务结果。无需连接 SSH；选择一次运行，再阅读每个任务的状态。列表按记录写入顺序展示，使用 UTC 时间，系统时间倒退不会重新排序。

结果分别显示明确成功、非零退出失败、明确拒绝、结果未知、取消前未放行、依赖阻断、失败策略停止，以及启动复核或定时触发的过期/占用/授权失效。取消只表示观察到本地取消，不能证明已放行的远端进程停止。缺失或不匹配的完整回执不会被计为成功。

记录只有运行/任务/目标的 UUID、时间与固定状态。目标 UUID 可以是本次会话的临时身份，不会据此查找或连接新的主机。记录不含任务名称、命令正文、stdout/stderr、主机地址、凭据、用户参数或这些内容的摘要；含用户参数的工作流也只记录相同的非秘密结果。审核文本和输出仍然只在当前工作区内。

最多保留 100 次运行、每次最多 128 个任务，并限制累计任务数为 2048。超出预算时按写入顺序删除最旧的完整运行；例如 100 次各 128 个任务的运行最终保留最近 16 次。定时计划的每次已完成或明确未放行触发分别记录，遵守相同保留预算；等待中的触发不伪装成完成记录。

状态文件写入在后台完成。保存失败或被片段/凭据/配置同步模态窗口暂时阻塞时，结果显示 **未保存**。关闭模态窗口或点击 **重试保存记录** 可以再次保存元数据；不会重新执行任务、连接 SSH 或恢复计划。未保存记录只在本次进程中保留，退出应用前应确认显示 **已保存**。配置冲突或写入后持久性未知时，遵守应用现有的重新读取/重启提示，不能盲目覆盖别的配置。

待保存记录超过 68 条时暂不接受新的工作流确认，以便给一次最多 32 个触发的已审核计划预留空间。已经启用的有限计划不因此扩展或重放。待保存队列最多 100 条；异常超限会明确提示新结果未保存。

Open **Dependency workflow → Task history** to read locally saved results without an active SSH connection. Select a run to inspect its task statuses. UTC recording times are displayed in insertion order, even after a wall-clock rollback.

Confirmed success, nonzero exit, explicit rejection, unknown outcome, pre-admission cancellation, blocked prerequisites, failure-policy skips and known admission/schedule skips have distinct labels. Observing cancellation does not establish remote process termination. An incomplete or mismatched final receipt never proves success.

Records contain UUIDs, times and fixed statuses only. Captured target IDs may be ephemeral; they cannot resolve or connect to a replacement host. Task names, command text, stdout/stderr, endpoints, credentials, custom parameter values and their digests are excluded. Parameterized workflows use the same non-secret result projection. Review text and output remain transient.

Retention is bounded to 100 runs, 128 tasks per run and 2048 tasks overall. Old complete runs are removed in insertion order; 100 runs of 128 tasks leave the newest 16 runs. Every completed or conclusively never-admitted finite occurrence has its own record, subject to this retention policy. Waiting occurrences are not shown as completed work.

Saving runs on a background worker. A failed save or a snippet/vault/profile-sync modal lease leaves the result **Unsaved**. Closing the modal or selecting **Retry saving history** saves metadata only; it never executes, reconnects or restores a schedule. Unsaved records exist only in the current process, so check **Saved** before exiting. Follow the existing reload/restart guidance after configuration conflicts or uncertain post-replacement durability.

When more than 68 records await saving, new confirmations pause to reserve room for all 32 occurrences of an already reviewed finite schedule. The pending queue is capped at 100; unexpected overflow explicitly reports that the new result was not saved.

The isolated candidate's tests are recorded in [the verification record](../testing/records/2026-10-06-workflow-history.md), and the independently reviewed current-main combination is recorded in [the integration record](../testing/records/2026-10-06-task-audit-main-integration.md). Native desktop acceptance remains a separate gate.
