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

原始终端全文、命令、输出、密码、私钥、session secret、API key 和其他敏感排除项不会写入普通 StateStore metadata。未知、未开始、取消、敏感或混合文档整体不能保存；Markdown 预览只保留安全状态和来源标记。

## 当前范围与后续范围

已实现单条目的显式保存、受控 wire 反序列化、StateStore 乐观冲突、旧状态兼容、幂等 receipt、敏感/unknown 阻断、Conflict 后受控 reload/reconcile 和 Workspace 到 StateStore 的后台写入链。reload 会更新当前配置快照并撤销旧知识审核，不包含自动保存或自动重放。

后续仍需设计并验证条目编辑、删除、搜索和分类 UI、批量管理、知识内容的显式模型发送、模型发送前的再次脱敏与上下文选择，以及 macOS、Windows、Linux 原生验收。当前实现也不自动恢复旧 SSH session 或重放任何知识保存请求。
