# 本地知识库

AI 助手中的“本地知识库”展示用户明确审核并保存过的终端知识条目。打开、搜索和编辑使用本机配置快照，不调用模型，也不读取新的终端内容。

## 使用

1. 在 AI 助手中打开“本地知识库”，按标题、摘要或来源名称搜索。搜索不区分大小写，支持中文，最多 512 UTF-8 字节。
2. 选择条目查看来源、捕获时间、原结果和摘要。只有标题和摘要可以编辑；条目、来源及会话身份、来源版本、捕获类型、时间和原结果保持不变。
3. 点击“预览修改”，核对原内容和将要保存的完整内容，再点击“确认保存修改”。多行草稿会合并为空格，预览展示实际保存的摘要。标题和原始摘要各最多 16 KiB；空内容、敏感标记及不支持的控制字符被拒绝。
4. 删除需要“审核删除”及“确认删除条目”两步。取消审核不写入配置。删除仅从当前知识库移除条目，配置备份可能保留历史副本；这不是安全擦除。
5. 列表中的管理选择用于批量删除，最多选择 32 条。它与 Ask 附件和编辑器选择独立；搜索会隐藏未匹配条目，但顶部计数会显示隐藏的已选条目。点击“审核删除”后，审核正文逐条展示 UUID、完整标题和摘要、来源、来源 UUID、来源版本、原会话、捕获类型、时间和原结果，再点击一次确认。

审核绑定完整原条目、配置快照、草稿文字、管理选择版本和单次审核身份。管理选择变化、修改输入、替换审核或收到新的配置快照后，旧按钮不能批准新内容。批量删除先在内存中校验整组选中的完整原条目，再通过一次 StateStore 保存；任一条目缺失、内容变化、身份重复或候选配置无效，整批不写入。保存期间禁止编辑与重复提交。磁盘操作通过 Workspace 后台 executor 和统一 StateStore 保存队列执行。

保存失败后需要明确重新加载并重新审核，配置冲突及写入耐久性未知均不会自动重放。重新加载失败时修改仍被阻止；成功后更新配置消费者并清除旧审核，用户重新选择条目。

## 范围

本切片提供本地列表、字面搜索、单条编辑和单条或批量删除。知识上下文发送给模型、分类、跨机器同步、重建原 SSH 会话和自动执行 runbook 不属于该切片。批量删除只改变当前本地知识库记录，不删除配置备份中的历史副本，也不改变终端或 SSH 会话。终端来源是用户确认过的事实记录，并不保证外部事实仍然成立。

后续的[显式知识上下文](AI_KNOWLEDGE_CONTEXT.md)接入了逐条加入 Ask、请求预览和明确发送；
本地浏览及选择本身不会发送资料，编辑、删除或重新加载会使相关旧审核失效。

源码和测试结果见[测试记录](../testing/records/2026-11-04-ai-knowledge-library.md)。GPUI 受控测试、真实 StateStore 读写、macOS 桌面及 Windows/Linux 桌面各有独立证据边界。

## English

Open **Local knowledge library** in the AI assistant to browse explicitly reviewed and saved terminal captures. Literal, case-insensitive search matches titles, summaries and source labels, including Chinese text. The query is limited to 512 UTF-8 bytes. These actions use the local configuration snapshot and do not contact a model or collect terminal content.

Select an entry, edit its title or summary, and review the complete current and replacement content before confirming. Multiline drafts become a single summary with spaces; the preview shows exactly what will be saved. Each raw content field is bounded to 16 KiB. Empty text, sensitive markers and unsupported control data are rejected. Entry and source identities, source revision, session binding, capture kind, timestamp and original outcome remain immutable.

Deletion has a separate review and confirmation. The management selection supports up to 32 entries and is independent from Ask attachments and the editor. Search can hide selected entries while the management bar reports the hidden count. The full review shows every selected UUID, title, summary, source identity, session binding, capture kind, timestamp and original outcome before one confirmation. Cancelling a review writes nothing. Deletion removes the current library record; configuration backups may retain historical copies, so this is not secure erasure.

Each approval binds the complete original entry set, configuration snapshot, draft text, management-selection revision and one review identity. Stale controls cannot approve a newer review. Batch candidates validate every original before changing memory, preserving survivor order and using one background StateStore write. Any conflict, duplicate identity or invalid candidate rejects the whole batch. Workspace serializes background StateStore writes. A failed save requires an explicit reload and fresh review; conflicts and unknown durability outcomes are never replayed automatically.

Model context sending, categories, cross-device knowledge synchronization, restoration of the original SSH session and automatic runbook execution remain outside this slice. Controlled GPUI tests and local storage tests do not establish native acceptance on all three platforms.

The subsequent [explicit knowledge context flow](AI_KNOWLEDGE_CONTEXT.md) adds per-entry
Ask selection, request preview and explicit sending. Browsing and selecting remain local;
editing, deletion or reloading revokes the affected review.
