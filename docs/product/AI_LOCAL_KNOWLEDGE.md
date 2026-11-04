# 本地知识与 AI 复用

KeelShell 的本地知识功能把用户明确选择的终端内容整理为带来源的知识条目。它服务于后续的排查复盘、运行手册和模型上下文选择；任何内容都要经过用户审核，终端输出不会因为出现在会话中就自动进入知识库。

## 当前流程

1. 在 AI 助手中选择“所选文本”或“当前屏幕”。Workspace 将捕获内容绑定到当前 SSH session、source identity、source revision、capture time 和 capture type。
2. 点击“生成本地知识预览”。初始结果是 `Unknown`，只显示安全的来源和状态信息。
3. 用户明确点击“确认已完成”或“确认已失败”。只有明确结果且内容闸门通过时，预览才变成 `Persistable`。
4. 用户点击“保存到本机知识库”。Workspace 再次检查 review ID、session、source revision 和当前 capture，然后把一个条目交给后台 StateStore 保存。
5. 保存成功后按钮显示“已保存”。StateStore 冲突会保留审核内容；写入结果未知时显示 Unknown，并要求重新加载后重新审核，系统不会自动重放。
6. 在冲突或未知写入结果下，用户可以明确点击“重新加载并重新审核”。Workspace 在后台读取最新 StateStore，清除旧 preview 和 review ID；读取成功后必须重新生成预览并再次确认，旧保存请求不会被重放。读取失败时保存继续阻止，只允许再次执行读取动作。

## 保存内容

每条记录保存独立 entry ID、来源 UUID 和 revision、session identity、捕获类型、捕获时间、审核后的标题、摘要以及 completed/failed 结果。当前切片一次只保存一个完整 session capture，最多 256 条。

只保存用户明确捕获并审核的标题与摘要，不自动采集终端历史。摘要可能包含用户选择的命令或输出；命中敏感标记的内容被阻止，不写入普通 StateStore。未知、未开始、取消、敏感或混合文档整体不能保存；这些被排除的内容在 Markdown 预览中只保留安全状态和来源标记。

## 当前范围与后续范围

已实现单条目的显式保存、受控 wire 反序列化、StateStore 乐观冲突、旧状态兼容、幂等 receipt、敏感/unknown 阻断、Conflict 后受控 reload/reconcile 和 Workspace 到 StateStore 的后台写入链。reload 会更新当前配置快照并撤销旧知识审核，不包含自动保存或自动重放。

[本地知识库](AI_KNOWLEDGE_LIBRARY.md)已增加列表、字面搜索、标题/摘要编辑和单条删除。修改需要预览与确认，保留不可变来源元数据，绑定完整旧条目和配置快照；失败后显式重新加载，不重放旧审核。工程和原生验证状态见[本轮记录](../testing/records/2026-11-04-ai-knowledge-library.md)。

分类、批量管理、知识内容的显式模型发送、发送前再次脱敏与上下文选择，以及完整 macOS、Windows、Linux 原生矩阵仍待完成。当前实现不自动恢复旧 SSH session 或重放任何知识保存请求。
