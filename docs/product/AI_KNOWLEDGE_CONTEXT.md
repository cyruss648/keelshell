# 显式保存知识上下文

本切片将本地知识库中已经保存的条目接入 **Ask** 的逐次请求审核。它复用 API 和本地 CLI 的 `ContextDraft`、脱敏、预算、预览和明确发送链路；源码实现不代表真实模型或原生桌面已验收。

## 使用

1. 在 AI 助手选择“选择本地知识”或打开“本地知识库”。搜索、浏览、加入和移除只使用本机已经加载的知识快照，不读取原远端、不恢复 SSH，也不联系模型或启动 CLI。
2. 用“加入 Ask”逐条选择。列表里的标题仍可打开本地审核编辑；未保存的编辑稿不能代替已保存条目加入请求。编辑已选条目会移除该条选择并撤销相关请求审核。
3. 返回 AI 助手查看所选标题、原来源、日期和原结果。可逐条“移除”或“清空知识选择”。最多选择 8 条，序列化后的完整记录合计最多 32 KiB；该限制包含来源元数据。
4. 输入问题，选择 API 或本地推理配置，点击“预览请求”。实际载荷包含历史条目的原 ID、来源种类/ID/版本/标签、原会话 ID、捕获类型、时间、结果、标题与摘要。资料仍是不可信数据；日期与结果不表示当前主机状况。当前凭据脱敏后才展示实际载荷。
5. 核对目标、完整 JSON、脱敏与省略计数，再点击固定在底部的“确认发送此请求”。API 发送该正文；本地 CLI 使用既有预览中的确切 stdin 和受限 Ask 配置。发送不自动恢复来源会话，不执行来源文字或模型输出中的命令。

Ask 的现有输入预算仍为 16 KiB，API 配置声明的上下文上限可进一步降低可用容量。**含保存知识的请求不接受任何上下文截断**：超预算时不产生可发送预览，需减少条目、终端内容或问题。32 KiB 的选择上限不是模型可发送容量。空选择不增加知识字段或占位记录。

最终请求还会在预览前检查已知凭据，包括非当前配置的凭据和 JSON 转义后仍可从知识字段还原的值。若正文仍包含该值，整次准备会拒绝，不展示或发送该请求。可移除相关条目后重新预览；原保存内容、来源身份、日期和结果不会因此被改写。

## 生命周期

- 选择仅保存在当前助手内存中；普通视图、主题和语言切换保留选择，改变这些显示设置不发送资料。
- 所选条目被编辑、删除、出现保存失败或明确重新加载后，旧审核失效。重新加载或配置恢复清空知识选择，即使加载后的条目字节恰好相同，也需重新选择。
- 每次预览绑定知识选择版本和独立审核身份；旧的绘制按钮不能批准后来的请求。发送前再次同步检查当前记录与编辑状态，后台准备和迟到的回复也受相同版本约束。
- 移除或清空选择会请求取消在途请求，并丢弃迟到的回复。已发送内容可能已被模型服务处理，取消不代表服务端撤回。
- 配置、凭据和原 live SSH 授权的既有撤销路径继续生效。来源会话结束不抹去已保存的历史资料；这些资料不会赋予新的命令目标或 SSH 授权。
- 此切片只在 Ask 使用保存知识。切到 Agent 后仍有知识选择时，准备请求会明确阻止；可返回 Ask 或清空选择。没有自动按主机匹配、自动检索、通用 MCP 客户端或后台知识发送。

与[本地知识库管理](AI_KNOWLEDGE_LIBRARY.md)的本地增删改范围分别记录。设计见 [ADR 0098](../adr/0098-explicit-saved-knowledge-context.md)，验证状态见[本轮记录](../testing/records/2026-11-05-ai-knowledge-context.md)。

## English

Saved local knowledge can be explicitly attached to **Ask**. Open **Choose saved knowledge** or **Local knowledge library**, select **Add to Ask**, then return to the assistant. Browsing and selection use the loaded local snapshot only: they do not contact a provider, start a CLI, fetch source content, or restore SSH.

Select up to eight entries and 32 KiB of complete serialized records, including source metadata. Remove individual entries or clear the selection. Unsaved editor drafts cannot replace saved text. The assistant shows historical source, date and outcome; the exact request contains all original saved metadata and reviewed content as untrusted historical data.

Choose an API or existing local CLI profile, enter a question, and **Preview request**. Current credentials and existing sensitive-content policies apply before preview. The existing Ask input budget is 16 KiB; a declared API context window may reduce it further. A request containing saved knowledge is rejected if any context would be truncated. Reduce the selection, terminal context or question, then review again. An empty selection adds no knowledge placeholders.

Before a preview is admitted, the final payload is also checked against known credentials from inactive configurations. If a credential survives literal redaction through JSON escaping in a saved field, preparation fails without displaying or sending that request. Remove the affected attachment and preview again. This refusal does not rewrite the saved original or its historical identity, date or outcome.

Only **Send this exact request** consumes the preview. API requests send the reviewed body; local inference uses its existing reviewed stdin and Ask isolation path. Saved history does not establish current host state, authorize a connection or execute commands. Historical source identity, date and outcome stay unchanged in storage.

Selections survive ordinary view, language and theme changes, but never restart as implicit permission. Changed/deleted records, edited selected drafts, uncertain library writes, explicit reloads and configuration recovery revoke old review. Reload/recovery clears selection even for identical records. A request binds the selection revision and a unique review identity; stale buttons and delayed background results cannot approve or restore a newer request.

Removing or clearing selection requests cancellation and rejects late replies. Already transmitted data may have been processed by the service. Existing provider, credential and live-session invalidation remains in effect. Historical saved records may remain after their source SSH session ends, without creating a new live command target.

Saved attachments are bounded to Ask in this slice; Agent preparation refuses a nonempty selection. Automatic host matching, background retrieval, generic MCP clients, automatic actions, live-provider verification and complete native acceptance remain outside this implementation.
