# ADR 0128：AI 审核命令保存为片段草稿

日期：2026-11-14。产品实现基线为 exact HEAD
`b89445604b4ce44fc5a80ff26605f6e6b9039d57`，主体实现提交为
`9b831103aae5563a5450561988d668d2e77a19dd`，无障碍修复提交与当前最终实现
SHA 为 `50ca0ead2fead7b0e92679ee9d531604ae040772`。当前状态为
`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。

## 背景

AI 助手已经能够在当前 SSH 会话中捕获上下文、提取 shell 命令、绑定目标并通过
现有的 `CommandProposal::review` 审核。用户还需要把经过审核的命令沉淀为普通本地
片段，以便以后通过片段搜索和命令建议复用。这个沉淀动作不能把 AI 响应、SSH
会话或审核凭据直接带入持久化层，也不能把“保存片段”变成执行命令的快捷路径。

## 决策

### 1. AssistantPanel 只发出一次性审核事件

建议卡片和诊断计划步骤都新增“审核并保存为片段”入口。入口先构造带有原始命令、
目标字符串和 session identity 的 `CommandProposal`，调用现有
`CommandProposal::review`，再通过 review ticket 提取精确命令正文。成功后由
`AssistantPanel` 发出 `AssistantEvent::SaveSnippet`，事件携带：

- 命令正文；
- 原始 target；
- 原始 session/terminal identity；
- 一次性 `review_id`。

AssistantPanel 只登记 pending review 并发送事件，不直接写 `StateStore`。同一
review 只能在当前 response、target 和 session 上被精确消费一次；响应变更、会话
失效或重复消费都会使候选失效。

### 2. Workspace 是第二次授权边界

Workspace 收到事件后重新确认：

- 当前活动标签仍是同一个 SSH terminal Entity；
- 当前 terminal 仍处于打开状态，远端 target 没有切换；
- configuration gate 没有进入保存、重新加载、恢复、失效或关闭边界；
- 当前 modal scope 仍可打开 SnippetEditor。

任一条件失败时，Workspace 发送双语错误、清除 AssistantPanel 的 pending review
并丢弃保存候选，不创建片段。打开编辑器后，保存和重新审核入口再次检查相同的
target/session/configuration/modal binding。迟到事件、旧 terminal、关闭标签和被
替换的操作范围都 fail closed。

### 3. 审核命令正文在片段编辑器中保持锁定

`SnippetEditor::new_reviewed_draft` 复用现有编辑器和 fixed footer/scroll 布局，
但将已审核命令正文设为 disabled，并保留审核时的原始正文快照。

> reviewed command body locked until explicit re-review

这是本流程的不变量：名称、说明和标签可以编辑；审核命令正文不能通过编辑器
直接修改后保存。若未来内部状态与审核快照不一致，保存拒绝并显示双语错误。用户
必须点击“取消并重新审核命令”，退出当前 AI 保存流程，把命令放回命令栏的现有
审核入口；这个入口不会执行命令。普通片段编辑器仍沿用原有可编辑行为。

AI 片段保存按钮第一次点击只进入二次确认状态，第二次明确点击后才发出
`SnippetEditorEvent::Save`。成功后 Workspace 才调用现有后台 `persist`，复用
`AfterSave::SnippetSaved`、StateStore revision 检查和冲突处理。StateStore 冲突、
I/O 失败或未知结果都保留完整草稿和错误，只有成功保存才关闭编辑器。

### 4. 持久化只保存普通片段

保存结果使用现有 `keelshell_core::Snippet`，只包含片段本身的 UUID、名称、说明、
标签、命令正文和参数化设置。以下内容只存在于进程内的事件或 Workspace binding，
不写入 `Snippet` 或 StateStore：

- session ID、terminal Entity identity 和 target/host；
- AI profile、provider、模型或请求上下文；
- `review_id`、review ticket 和其他审核状态；
- 凭据、主密码、API key 或其他秘密。

编辑器明确提示片段会以明文保存在本机，用户不要保存密码、API key 或其他秘密。
取消不会执行命令，保存也不会执行命令；保存成功后片段只进入本地搜索和命令建议
来源。

### 5. 主题、语言和可达性

新增文案使用中文/英文翻译，编辑器继续使用 fixed footer、独立滚动表单、
`aria-label` 命令输入、名称/说明/标签输入和稳定 test/accessibility hooks。
Frost、Graphite、Lavender 三套主题均从设计 registry、Palette 和组件 token
取得颜色与形状；业务代码不按 theme ID 分支。新增主题时只需扩展主题注册表和
语义 token。GPUI 回归会在中文和英文下按精确可访问性名称查找三个元数据输入。

## 分层与并发

`keelshell-ai` 负责命令提取、目标绑定和 review ticket；`AssistantPanel` 负责
AI 事件；`Workspace` 负责二次 admission、编辑器生命周期和后台 StateStore
persist；`SnippetEditor` 负责草稿校验、锁定和用户确认；`keelshell-core` 的
片段领域模型继续负责本地片段验证和搜索输入。阻塞 StateStore I/O 仍由已有
Workspace background executor 执行，UI 线程不直接做磁盘写入。

## 验证与边界

作者在主体实现提交 `9b831103aae5563a5450561988d668d2e77a19dd` 上使用独立
worktree `/Users/hdh/.codex/worktrees/ai-command-snippet-save-20261114/keelshell`
和专用 target `work/ai-command-snippet-save-20261114/target` 完成 focused
回归、格式和严格 Clippy；全量门禁使用
`work/ai-command-snippet-save-20261114/full-target`，日志在
`work/ai-command-snippet-save-20261114/logs/full-gate-9b83110.log`。

作者 focused 回归为 AI 片段 `6 passed`、SnippetEditor `11 passed`、Assistant
review `6 passed`；core 全包测试为 `634 passed / 0 failed`。全量
`python3 scripts/check.py` 实际退出 `0`，Python 为 `131 run / 1 skipped`，
workspace 为 `2499 passed / 0 failed / 23 ignored`，workspace Clippy、fmt、
依赖策略和三个 AI controller 均通过；单独 workspace doctest 为
`14 passed / 0 failed`，其中 2 项 compile-fail。

无障碍修复后的最终实现 SHA `50ca0ead2fead7b0e92679ee9d531604ae040772`
重新运行 focused 回归：
SnippetEditor `12 passed`、AI 片段 `6 passed`、Assistant review `6 passed`，
fmt、app tests 严格 Clippy 和 diff check 均通过。第一次独立复核发现名称、说明、
标签输入没有显式无障碍名称，修复后第二次独立复核固定该 SHA 返回 `APPROVE`，
P0/P1/P2/P3 均为 `0`；完整原始报告见
[独立复核原文](../testing/records/2026-11-14-ai-reviewed-snippet-save-independent-review-raw.md)。
本 ADR 不把 GPUI
headless 回归写成原生验收，也不把真实客户 SSH、Podman、外部 AI provider、
macOS/Windows/Linux 原生窗口、IME、辅助技术、签名、公证、安装升级或发布验收
写成已通过。
