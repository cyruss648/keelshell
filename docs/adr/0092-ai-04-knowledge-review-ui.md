# ADR 0092：AI-04 本地知识预览审核入口

**状态：** Accepted for the first application UI slice
**日期：** 2026-10-23

## Context

AI-04 的 `keelshell-core` 领域模型已经能够把用户明确选择的事实转换为有界文档和 Markdown，但工作区还没有把它呈现给用户。直接把终端上下文写入知识库或发送给模型会绕过用户审核，也会把任务历史误认为知识来源。

## Decision

- AI 助手在用户明确捕获终端选择或当前可见终端内容后，提供“生成本地知识预览”入口。选择文本和可见屏幕分别标记为不同的捕获类型。
- 每次捕获都携带稳定的 session source identity、session 标识、捕获 revision 和捕获时间；预览使用这份不可变快照元数据，不使用普通 AI 请求 revision 或预览点击时间替代来源事实。
- 捕获后的初始结果始终是 `Unknown` 的 review-only 文档。用户在预览卡片中明确确认“已完成”或“已失败”后，UI 才构造 `SelectedFact::completed` 或 `SelectedFact::failed`；未确认的中间状态不能成为知识条目。
- 入口只使用当前助手绑定的 session、host 和文本；不从 `WorkflowAuditRecord` 自动导入，不在后台扫描终端或审计记录。
- UI 通过 `SourceBinding`、`SelectedFact`、`KnowledgeSelection` 和 `KnowledgeService` 生成领域文档，展示来源、捕获类型、revision、条目、排除项、`SaveDisposition` 和 Markdown。
- `DoNotPersist` 内容显示阻止原因，保存按钮保持禁用；`Persistable` 只代表本地内容闸门通过，不代表已经保存或允许自动发送。
- 当前切片不写 StateStore、不调用模型、不执行命令。保存和发送必须在后续切片中分别设计显式审核、上下文绑定和失败/未知结果语义。
- 同一个 SSH session 的重复捕获复用稳定 source identity，session 切换生成新的 source identity；捕获 revision 和时间戳随每次显式捕获重新生成。

## Consequences

用户现在可以在 AI 侧栏中看到可审查的本地知识草稿，同时能看到敏感或未知结果被排除的原因。领域层继续拥有最终内容闸门，UI 不复制敏感检测规则。

该入口是预览能力，不是持久化知识库或模型发送能力。StateStore、知识条目编辑/删除、明确保存、模型上下文复用和跨平台原生视觉验收仍然开放。
