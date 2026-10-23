# ADR 0091：AI-04 本地知识与运行手册的领域边界

**状态：** Accepted for the core domain slice
**日期：** 2026-10-23

## Context

AI-04 需要把用户明确挑选的、已经得到已知结果的事实整理成可复用的本地知识。现有工作流和批量审计记录是任务历史，它们描述执行过程和状态，不自动代表用户认可的知识，也不能因为存在审计正文就进入知识库。

该能力必须先建立在 UI、模型调用和持久化之前的领域边界上。它需要能够表达来源、来源版本、时间线、脱敏摘要、运行手册步骤和本地搜索，同时把未知结果、未完成结果及可能的秘密明确隔离。领域服务不能联网、执行命令或写文件。

## Decision

在 `keelshell-core::ai_knowledge` 中提供纯内存模型和服务：

- `SourceBinding` 使用来源类别、非空 UUID、非零 revision 和经过敏感标记检查的显示标签绑定事实来源。
- `SelectedFact` 只能由用户显式选择的构造函数创建，`KnowledgeSelection` 再施加数量和总字节上限。模块不实现、也不提供从 `WorkflowAuditRecord` 或其他任务审计正文转换的接口。
- 只有 `Completed` 和已知 `Failed` 事实能够形成内容摘要。未知、未开始和取消的事实只留下来源绑定、时间戳和稳定状态标记，不保留标题或正文。
- 来源标签命中 `Authorization`、Bearer、API key、token、密码、secret、私钥等保守敏感标记时，在构造 `SourceBinding` 阶段直接拒绝；标题或摘要命中这些标记时，不保留原文，生成 `SensitiveContent` 的时间线标记并将文档标为 `DoNotPersist`。
- 所有排除项只保存安全的 `SourceBinding`、来源时间戳和 `KnowledgeExclusionReason`；它们不保存原始标题、正文、命令、输出或凭据值。
- `KnowledgeService` 生成来源绑定摘要、时间线、描述性 runbook Markdown 和有界本地字符串搜索。runbook 的 action 是说明性文本，模块没有命令执行接口。
- Markdown 产物携带 `SaveDisposition`。即使产物可以预览，存在排除项时也必须显示为不可保存；本切片不提供保存、发送或模型调用。
- 领域产物只提供 `Serialize` 用于受控预览；本切片不提供通用 `Deserialize`，避免外部数据绕过来源、敏感内容和 `DoNotPersist` 不变量。

## Consequences

该切片可被后续 UI 用于“选择 → 预览 → 明确保存/发送”的审核流，并能在 UI 接入前独立验证秘密和未知结果不会进入内容。没有文件写入和网络依赖，单元测试可以完全使用内存数据。

摘要脱敏采用保守排除而不是猜测性替换：含敏感标记的事实不会进入摘要正文。标记规则不是秘密检测的完备证明，后续 UI 仍须展示完整预览、允许用户拒绝，并在发送前复用 AI 上下文脱敏边界。

本 ADR 不宣称 AI-04 已接入 `keelshell-app`、`keelshell-ai` provider、StateStore、UI 审核、保存或发送流程，也不构成跨平台原生验收。
