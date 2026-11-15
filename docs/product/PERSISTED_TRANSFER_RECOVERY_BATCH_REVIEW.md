# 持久化传输恢复批量整理 / Persisted transfer recovery batch review

状态：已集成，最终作者门禁与独立修复复核通过；完整结果与原生/网络边界见
[集中验证记录](../testing/records/2026-11-15-transfer-recovery-batch-integration.md)。
最初实现专项见
[测试记录](../testing/records/2026-11-15-persisted-transfer-recovery-batch-review.md)。

## 入口与顺序

在 Files 的传输队列详情内，恢复记录支持独立复选框与“已选择”数量。
“建立审核队列”按当前恢复列表顺序保存选中记录的 ID；点击选择的先后顺序、
UUID 的排序不会改变队列顺序。队列只存在于当前面板内存中，绑定当前
SSH session；它没有任何传输或连接权限。

“逐条进入审核”每次只取队首一条，重新取得这条记录的排他 claim，并复用
原有的 Inspect → Review → Confirm 流程。保存连接必须匹配；未绑定的记录
先要求用户确认当前 SSH 主机，再开始当前会话的只读计划。审核通过后仍需
单独确认本条传输。进入下一条必须再次点击，不会自动连接、检查下一条或执行。

工作线程创建失败或传输队列已满时，当前记录回到待审核队首，其余顺序不变；
未启动的 claim 释放。单条队列也保留当前会话归属。用户重新进入本条时必须
完成新的检查、审核和明确确认，失败不会自动重试或启动后面的任务。

取消审核、取消待审核队列、离开详情、导航撤销、会话退休、记录消失或过期
回调均撤销旧待审核状态。已经明确确认并发出的传输继续使用原任务的所有权、
取消与结果记录，不会因整理队列而被重放。

## 批量丢弃

“批量丢弃”先显示选中的完整记录列表及本地元数据删除说明；
只有明确点击原有审核区的“确认”才开始。审核操作保留完整记录快照，
后台按列表顺序逐条 `claim → load → 比较审核快照 → remove_with_claim`。
即使 ID 相同，只要方向、路径、绑定、进度或其他元数据变化就拒绝删除，
保留实际记录并报告。不会连接 SSH 或删除本地/远端传输文件；
每条 claim 在该条处理结束时释放。

成功项从恢复列表移除；Busy、记录消失、读取/写入失败或 worker 错误不会
触发无条件删除或自动重试。未处理记录保留，界面重新选中仍存在的失败项并
报告确认删除/未确认删除数量及逐项错误；用户可以稍后重新选择和确认。
全部完成、部分失败和整体 worker 结果未知均重新读取真实 store；未知写结果
不等同于“全部保留”，列表以重新读取结果为准，失败不会触发自动重试。

## 呈现与实现边界

- 霜白、曜石、雾紫共用现有 registry、Palette 和语义 token；无主题 ID 业务分支。
- 中英文按钮、说明与可访问性名称随当前语言绘制；操作条允许换行，详情使用
  已有滚动容器，审核确认/取消继续位于固定审核区。
- 队列不持久化 session、claim、审核 token、凭据或 worker；跨 crate 接口和依赖不变。
- GPUI headless 与本地 SSH fixture 的结果不替代三平台原生、真实客户 SSH/SFTP、
  Podman、IME 或完整辅助技术验收。

## English

Select persisted recovery records in Transfer queue, then build a queue in the
current list order. Selection order grants no authority. Review one by one
claims only the next record and reuses the existing Inspect → Review → Confirm
flow, including current SSH host/connection binding and a fresh read-only plan.
Every next record requires another explicit action.

If the transfer worker cannot start or the transfer queue is full, the current
record returns to the review queue head without reordering the other records.
The unstarted claim is released and the current session binding is retained,
including for a single-record queue. Retrying requires a fresh review and
explicit confirmation; neither the current nor a later record starts automatically.

Cancellation, navigation, leaving the recovery details, retired sessions,
missing records and stale callbacks invalidate pending review state.
Confirmed transfers retain their original ownership and honest outcomes.

Discard selected first presents an explicit confirmation of the complete local
metadata snapshot. A background worker independently claims and reloads each
record, compares every persisted field with the reviewed snapshot, and only then
removes it. Changed records, Busy and errors retain unprocessed records and report
individual results. Every outcome reloads the authoritative store, including
unknown overall results after a partial write. Uncertain removals are reported
as unconfirmed; there is no unconditional removal, automatic retry, connection or
file transfer.
