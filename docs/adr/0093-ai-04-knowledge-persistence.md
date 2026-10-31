# ADR 0093：AI-04 单条本地知识显式持久化

**状态：** Accepted for the bounded first persistence slice
**日期：** 2026-10-25

## Context

AI-04 已能把当前 SSH session 的明确捕获内容转换为可审核预览。预览通过内容闸门后，还需要一个明确的本地保存动作；这个动作必须保留来源身份和审核时的 session/source revision，不能把未知结果、敏感排除项或整段终端输出写入普通应用状态。

StateStore 已提供带文件锁和 optimistic snapshot 的原子保存。应用需要把保存请求放到后台执行器，并区分已知冲突、已知失败和目录 durability 未确认，避免把一次可能已经替换成功的写入盲目重放。

## Decision

- 使用独立的 `SavedKnowledgeEntry` 持久化边界，不给 `KnowledgeDocument` 增加通用反序列化。wire 结构只包含 entry ID、session source binding、session identity、capture type、capture time、已审核的 title、summary 和 `Completed`/`Failed` outcome。
- 当前切片只接受一个完整文档中的一个 `Session` capture。文档必须通过 `Persistable` 闸门、没有 exclusion；unknown、not started、cancelled、敏感内容和混合文档整体拒绝保存。`File`、`Workflow`、`Monitor` 与 `Manual` 来源不能跨过该边界。
- StateStore 的 `AppState.knowledge_entries` 最多保存 256 条。旧状态缺少该字段时按空列表读取；写入前和读取后都执行受控 wire 校验。相同 entry ID 的完全相同 receipt 是幂等操作，不同内容的同 ID 会被拒绝。
- Workspace 只接受当前 `review_id` 的 Ready 预览，并再次检查 immutable capture、source ID/revision、session identity、capture time 和当前 panel capture。StateStore 保存使用当前 AppState snapshot；所有文件 I/O 运行在后台 executor。
- `Conflict` 表示当前候选没有被采用，保留预览供用户重新加载配置后重新审核；`Durability` 表示文件替换已发生但目录 durability 未确认，UI 进入 `Unknown`，禁用自动重试。未知结果不能由按钮或后台任务重放。
- Conflict 或 Unknown 状态提供一个明确的 `Reload and review again` 动作。该动作在后台调用 `StateStore::load`，成功后更新 Workspace 的配置快照并清除旧 preview/review ID；用户必须重新生成预览并再次确认。读取失败保持知识保存阻断，只允许再次读取，不会把旧 snapshot 变回 `Ready`。
- 显式保存只写本地 StateStore 的知识条目字段；原始终端文本、命令、输出、凭据、session secret、host password 和 API key 不进入普通 metadata。知识条目不会自动发送给模型。

## Consequences

用户现在可以在完成/失败审核后保存一条来源可追溯的本地知识条目；重启后 StateStore 可以重新加载它，且 stale writer 不会覆盖其他状态。未知、敏感和不完整结果仍然只保留安全的审核状态。

本 ADR 不实现条目编辑、删除、搜索 UI、批量管理、知识库分类、模型发送或重启后的 session 恢复。reload/reconcile 仍只处理当前 StateStore 快照，不提供条目合并或自动重试。跨平台原生桌面、发布安装和真实客户 SSH 验收仍需单独验证。
