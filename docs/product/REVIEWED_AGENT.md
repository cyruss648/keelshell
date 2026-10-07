# 审核式 Agent

状态：2026-10-07 epoch3 已通过新的非作者限定复核，已与 MCP 生命周期修复精确导入主工作副本。前两次冻结版本的权威退休和停止传播 P1 及原始失败保留；组合工程门禁、精确提交 CI 和新原生验收另见[整合记录](../testing/records/2026-10-07-reviewed-ai-mcp-main-integration.md)。本指南说明应用内工作流，不改变 [KeelShell 对外 MCP 服务](EXTERNAL_MCP.md) 的方向，也不增加通用第三方 MCP 客户端。

## 使用流程

1. 在远程终端明确选择文本或当前屏幕，捕获目标主机及这一条真实 SSH 会话。在 AI 助手选择现有 API、Claude Code 或 Codex CLI 命名配置，然后切换到 **Agent**。更改问题、配置或所选上下文会停止旧任务。活跃任务的问题保持只读，可选择和复制；固定操作区的“编辑问题并停止”先撤销旧授权，再将问题滚入视口并恢复编辑。
2. 选择上限：3 回合／2 操作、6 回合／4 操作（默认）或 12 回合／8 操作。回合指实际发送的一次推理请求；拒绝的操作仍计入操作额度。整段历史最多 64 KiB，不通过删掉旧结果或秘密截断来继续运行。
3. 点击预览，检查目的服务、完整脱敏请求、工作目录及所选证据，再明确发送。每一轮都需要这一步；结果不会自动交给模型。
4. 模型每轮只能提出一个远程命令、一个 SFTP 常规文件读取、一个已有文件的完整替换，或结束并给出结论。界面显示真实收到的决策和步骤，模型生成的计划文本不能成为执行记录。
5. 检查固定的目标、理由及完整内容。批准只授权这个操作；拒绝不会执行。文件替换先明确读取原文用于桌面审核，再批准写入。原文不会因此隐式发送到模型。文本支持两轴滚动，批准／拒绝与停止位于固定操作区。
6. 查看确认结果，再审核下一轮完整请求。命令的非零退出码仍作为已确认的执行结果显示；超时、连接变化或取消后不确定的操作显示未知，并停止这一轮任务。模型结论保持为模型文本，不能当作系统已验证的事实。

## 目标与操作边界

目标包含捕获的终端身份和实际 `SshSession` 连接句柄。切换活动页不会把任务改到另一连接；重连、原页结束、替换 SSH 句柄或保存路线／主机信任变化会撤销任务。新会话需要重新明确选择并审核，不回放旧审批。桌面逐次复核原连接及当前路线，模型不能自行指定另一条会话。

关闭捕获页签、生产重连移除旧连接及终端发布结束通知时，在当前前台回合同步撤销后台令牌；周期巡检仅作补充。后台持有原SSH句柄不代表目标仍授权。已经派发的命令／写入结果保守显示未知，迟到成功不能恢复旧任务或把它绑定到替代页签。

后台还绑定开始时捕获的同一条 SSH typed lifecycle 源。原始源结束或关闭就拒绝写授权，并取消等待中的后台操作，即使前台轮询和观察回调延迟；新连接不能替换这条源。界面的结束状态及未知步骤稍后由真实前台回调发布。已经发往远端的操作无法据此承诺回滚。

点击停止、两种重新选择上下文、编辑问题或改变配置／凭据，在当前交互回合同步撤销这一个run的后台令牌。排队通知负责清理桌面owner，不能独自承担撤权。暂停在写发布之前的工作会重新检查取消令牌；已经发送的远程操作依然可能有影响。旧run的迟到清理、捕获上下文或成功结果不能改变新run，也不能恢复旧授权。

提案有效期为 120 秒，单个远程操作有 30 秒总期限，不因观察到进度延长。批准在 UI 线程消费一次，网络 I/O 在后台运行。停止会撤销后续准入；已发出的命令不能保证远端回滚，因此未知结果禁止自动重试。文件写入复用现有 SFTP 原子替换、共享排他所有权、旧内容复核、未知修改隔离与完整读回；普通 Agent 审批不能解除其它任务的隔离。

文件路径必须是明确审核的绝对 canonical POSIX 路径；拒绝 `..`、重复斜杠、根目录和路径别名。只读完整常规 UTF-8 文件，单个文件／命令输出最多 32 KiB。替换仅适用于已有常规文件，展示原文与完整新内容，不创建新文件、不递归操作。命令保持精确字节，不根据“只读”名称自动批准；隐藏控制符在审核显示中转义，实际字节不被悄悄改写。远端 Windows 路径与更广工具范围保持未完成。

## 推理后端

API 的 Chat Completions、Responses 和 Messages 沿用现有配置、凭据引用、请求头、代理及推理选项。每轮使用严格、拒绝未知字段的 JSON 决策协议；不兼容响应会停止，不猜测或拼接成可执行动作。

Claude Code 和 Codex CLI 使用已准入的无工具单轮适配器。**Agent 的回合及 SSH 工具由 KeelShell 编排**；供应商 CLI 的项目 hooks、MCP、插件、技能与执行工具保持关闭，并继续检查实际版本、协议、环境及工作目录。该流程不是从提示词推断本地 CLI 被隔离，也不开放自动工具批准。当前精确版本及 API 密钥／订阅登录边界见[本地智能体](LOCAL_AGENTS.md)和[工作目录](LOCAL_AGENT_WORKING_DIRECTORY.md)。CLI 每轮启动自己的有界进程，由既有所有权与清理契约管理；不复用任意用户会话。

已核对[Claude Code 程序化接口](https://code.claude.com/docs/en/headless)及[Codex 非交互接口](https://learn.chatgpt.com/docs/non-interactive-mode)：前者的许可模式不能单独作为禁止所有读取的证明；因此本候选复用已验证的工具关闭适配器，在 KeelShell 内建立人工审批边界。供应商账户、模型质量、真实 CLI 新工作流及跨平台桌面需要各自原生证据。

## Evidence and English usage

Select the SSH context explicitly, choose a shared API or local CLI profile, and switch to Agent. Review every complete inference request before sending. Each validated round proposes one action or a final summary. Approve or reject each exact action separately; file replacement requires retrieving and reviewing the complete original before write approval. Review the desktop result before preparing the next request. Stopping or losing the captured session ends the run; unknown remote outcomes cannot be retried automatically.

Closing the captured tab, replacing its connection during reconnect, or observing its terminal end synchronously revokes the backend token in that foreground turn. Periodic checks are supplementary. A retained SSH handle grants no continuing authority, and a late success cannot revive an unknown retired run. Earlier frozen epochs were blocked by independently reproduced retirement and Stop propagation defects. Epoch3 passed a fresh scoped non-author review and was imported into the main working copy; combined gates, exact-commit CI and fresh native acceptance have their own [integration record](../testing/records/2026-10-07-reviewed-ai-mcp-main-integration.md).

Backend authorization also retains the original typed SSH lifecycle receiver. A raw end or closed producer denies write authorization and cancels the pending backend operation without waiting for foreground polling. Desktop state and the unknown outcome are published by queued foreground callbacks. This does not promise rollback of an action already sent to the peer.

Stop, context selection and configuration or credential changes synchronously cancel the exact run's backend token before queued ownership cleanup. While a run is active the question is read-only and remains selectable and copyable. The fixed **Stop and edit question** action cancels first, then reveals, unlocks and focuses the editor. Obsolete events cannot retire a newer run. This protects admission after invalidation; it does not promise remote rollback.

The application coordinates a real finite inference/action/result loop. Supplier CLI tools remain disabled. There is no third-party MCP client: KeelShell's external MCP server remains a separate capability for external agents. Controlled HTTP/GPUI/SSH/SFTP tests prove their recorded fixture scope only; they do not prove supplier-cloud, native desktop or Windows/Linux acceptance. See [ADR 0073](../adr/0073-reviewed-agent-workflow.md) and the [author test record](../testing/records/2026-10-07-reviewed-agent-workflow.md).
