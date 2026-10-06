# API 推理与采样设置 / API inference and sampling

修复候选已通过新的独立源码与受控行为复审，现已主线整合；组合门禁及原生验收按实际结果另记。设计见 [ADR 0054](../adr/0054-ai-inference-options.md)，作者验证见 [测试记录](../testing/records/2026-10-06-ai-inference-options.md)。

## 配置方式

在 AI 配置中选择一个 API 配置及准确的模型 ID，再设置“当前模型的推理与采样”。默认选择“服务端默认（省略）”，温度和 Top P 留空；这样不会发送这些字段。选择努力值、自适应思考、明确关闭思考或旧版预算，表示已核对这个模型支持该选项。使用采样时还需显式点击“已核对当前模型支持采样”。模型发现只提供模型 ID，不保证参数能力。

- Chat Completions 的努力值映射到 `reasoning_effort`；Responses 映射到 `reasoning.effort`。可选项属于协议词汇，当前模型可能只接受其中一部分。
- Messages 的努力值映射到 `output_config.effort`；思考模式独立映射到 `thinking.type`。可以组合自适应思考 + `medium`，或者在明确支持的旧模型中组合手动预算 + 努力值。努力值和思考模式各有“默认（省略）”，清除其中一项会保留另一项。某些新模型会拒绝关闭思考或特定组合，需核对文档。旧版思考预算只适用于支持手动思考的模型，最低 1024 Token，并严格小于输出上限。输出未填写时有效上限为 4096。新模型可能拒绝旧版预算；不要仅凭模型名称推断支持。
- 温度范围 0–2（Messages 为 0–1），Top P 范围 0–1，最多三位小数。只填写一个采样字段；空白是省略，`0` 是明确发送零。Messages 新模型通常不支持可调采样，只有核对文档后的兼容旧模型才能选择。
- 本适配器拒绝采样与启用的推理同时使用；服务端默认、OpenAI `none` 或 Messages 努力值省略且思考模式省略/明确关闭时例外。这个限制会显示为配置错误，不会删除用户字段后偷偷发送。

旧版本已存的单一努力值、思考开关或预算保持原语义，编辑后采用组合表示。每个命名配置按模型独立记忆设置和无效草稿；只更改努力值不会清除无效的手动预算草稿。切换模型、配置或语言不会让上一模型的值进入新模型；无效草稿必须先修正才能应用或测试。修改地址、协议或后端会清除推理/采样声明，以免将旧服务能力带给新目标。本地 Claude Code、Codex CLI 不使用这些 API 控制。

应用和磁盘保存前都会检查已知临时秘密，包括非当前配置保留的值及未选模型的设置。所选温度/Top P 同时检查持久化千分位整数与实际请求小数，所选手动预算检查 Token 整数；碰撞时拒绝保存，原磁盘状态保持。

助手审核显示最终完整 JSON，人工发送的正文与审核一致。选项修改使旧审核失效，并取消旧请求的本地等待。测试连接在明确点击后将同一组选项用于固定检查问题，可能产生服务用量；发现模型不发送它们。声明错误或服务端拒绝后保留失败，不更改参数重试。努力值不等于精确推理 Token 数，也不保证回答质量。

## English

Select a named API profile and an exact model ID, then use **Inference and sampling for this model**. Provider default and blank sampling fields omit the corresponding wire fields. Choosing a reasoning setting declares that you checked this model's documented support; sampling additionally needs the explicit support button. Discovery supplies IDs, not capability certification.

Chat and Responses use different effort fields. Messages independently composes optional output effort and a thinking mode (omitted, adaptive, disabled or legacy manual budget); clearing one preserves the other. Existing single-field choices retain their previous wire semantics. Some newer models reject disabled thinking or particular combinations; manual budgets need at least 1024 tokens and room below the effective output ceiling. Sampling has exact thousandth precision, explicit zero differs from omission, and temperature/Top P cannot both be configured. The adapter rejects mixing sampling with active reasoning. New Messages models may reject legacy budgets or adjustable sampling.

Drafts and metadata are scoped by profile and model. An effort edit cannot replace an invalid manual-budget draft with its previous valid number. Endpoint, protocol or backend changes clear declarations. Invalid text cannot cause a request to fall back to a previous valid value. Local CLI backends do not borrow these API fields. Ask displays the exact immutable JSON for review; the fixed connectivity test uses the same mapper, while model discovery remains independent. Remote rejection never changes the approved body or triggers an automatic retry.

Apply and disk dispatch both check known ephemeral secrets from all profiles against every stored model selection. Selected sampling numbers are checked as persisted integer thousandths and actual wire JSON numbers; selected manual budgets are checked as integer tokens. A collision refuses persistence and preserves the existing disk state. Fixed schema tags and existing numeric limits retain their prior handling.

Current controlled tests are not cloud-model or Windows/Linux native acceptance. No automatic terminal action is introduced.
