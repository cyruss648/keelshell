# KeelShell AI 审核命令保存为片段切片最终严格只读复核

复核日期：2026-11-14（Asia/Shanghai）

复核 worktree：`/Users/hdh/.codex/worktrees/91d7/keelshell`

复核提交：`db808e0b45a788f4744acf2fe98cde7bae7b2f0a`

实现基线：`b89445604b4ce44fc5a80ff26605f6e6b9039d57`

复核 target：`/Users/hdh/.codex/worktrees/91d7/keelshell/work/review-final-db808e0/target`

原始日志目录：`/Users/hdh/.codex/worktrees/91d7/keelshell/work/review-final-db808e0/logs`

复核范围严格只读。第一次命令为 `git rev-parse HEAD`，返回值与指定 exact SHA 完全一致。没有切换 checkout、修改 tracked/untracked 项目文件、格式化写回、暂存、提交、推送、合并、删除或清理既有资源。仅创建了被 `.gitignore` 覆盖的 `work/review-final-db808e0/` 证据目录。

## 结论

结论：`REQUEST_CHANGES`

P0：0

P1：0

P2：1

P3：0

唯一 finding 是片段编辑器的名称、说明和标签输入没有显式无障碍名称；这不影响审核安全边界或持久化机密隔离，但使关键编辑流程的屏幕阅读器/AccessKit 识别不可靠，因此当前不批准。

## 代码与行为复核

### AI 审核事件

`AssistantPanel::save_suggestion` 先从当前 response target 构造 `CommandProposal`，随后调用 `review(Duration::from_secs(120))`，再调用 `ReviewTicket::into_suggestion`，最后进入 `review_and_emit_snippet_save`。诊断步骤的 `save_diagnostic_step` 同样先通过 `DiagnosticPlan::review_step`、`into_proposal` 和 `CommandProposal::review`，再复用相同的保存事件路径。

保存事件带有完整的 command、原始 target、session identity 和 review UUID。`pending_snippet_save` 同时保存这些值，`consume_snippet_save_review` 要求四者与当前 response target、host、session 完全相等，并在成功消费后清空 pending 状态；重复消费因此失败。response、context、session、profile/configuration 变化也会清除 pending review。

`Workspace` 的 `AssistantEvent::SaveSnippet` 分支只调用 `handle_ai_snippet_save`。该路径先做 live admission，再消费 AssistantPanel 的一次性 pending review，成功后只创建锁定的 `SnippetEditor` 草稿；没有直接调用 `StateStore`。真正的写入发生在编辑器 `Save` 事件之后的 Workspace `persist` 后台任务中。

### Workspace 二次准入

打开草稿时，`admit_ai_snippet_target` 检查 configuration gate、保存/重载/恢复状态、活动 modal、当前活动 tab、terminal identity、terminal open 状态和 `remote_hosts` target。编辑器提交保存和 re-review 时，`admit_ai_snippet_binding` 再次检查 configuration gate、恢复/关闭状态、SnippetEditor modal scope、绑定的 terminal EntityId、session identity、terminal open 状态和 target。

关闭活动 tab、重连挂起、配置失效、窗口关闭审核和其他 modal 替换路径会撤销 AI panel 的 session/review 状态；迟到、退休、已关闭、目标切换和重复事件均不能创建或保存片段。保存冲突或失败不会关闭编辑器，也不会执行命令。

### SnippetEditor

`SnippetEditor::new_reviewed_draft` 保存审核正文快照并禁用 command textarea。`save` 在保存前再次比较正文快照；名称、说明和标签仍可编辑。AI 草稿第一次保存点击只设置确认态，第二次才发出 `SnippetEditorEvent::Save`。修改命令必须通过“取消并重新审核命令”离开当前流程。

StateStore 保存由 Workspace 的后台 executor 执行，UI 线程只构造候选状态并派发任务。冲突和其他失败会将完整草稿和双语错误交回编辑器；成功才移除编辑器。保存和取消路径均没有向 SSH pane 写入命令，专项测试也核对了两个夹具 pane 的写入为空。

### 持久化与分层

保存结果使用普通 `keelshell_core::Snippet`，其字段只有 UUID、name、command、parameterized、description 和 tags。session ID、terminal EntityId、target/host、AI profile/provider/model、review UUID/ticket、凭据和秘密元数据只存在于进程内 binding/event，不进入 `Snippet` 或 StateStore。`keelshell-core` 的 Snippet 验证和 StateStore revision/conflict 机制继续负责领域边界。

UI、Workspace、core 和 AI 的职责保持分离；阻塞 `StateStore::save` 在后台 executor 执行。新增的公开编辑器 API 和事件有 rustdoc；本次源代码、格式和严格 Clippy 检查通过。片段视图使用 Palette/registry/token 取得颜色，没有按 Frost/Graphite/Lavender theme ID 分支。表单滚动区和 fixed footer 的结构存在，命令正文有明确的 accessibility ID/ARIA label。

## Findings

### P2-01：片段元数据输入缺少显式无障碍名称

- 严重级别：P2
- 文件：`/Users/hdh/.codex/worktrees/91d7/keelshell/crates/keelshell-app/src/snippet_editor/view.rs:83`
- 同一问题：`/Users/hdh/.codex/worktrees/91d7/keelshell/crates/keelshell-app/src/snippet_editor/view.rs:92`、`/Users/hdh/.codex/worktrees/91d7/keelshell/crates/keelshell-app/src/snippet_editor/view.rs:101`
- 触发条件：用户在 AI 审核片段编辑器中使用屏幕阅读器或 AccessKit 焦点进入“名称”“说明”“标签”三个输入框。
- 证据：这三个 `Input` 只有 `.id(...)` 和 `.disabled(...)`；可见文字由外层 sibling `div` 的 `label(...)` 绘制，没有 `.aria_label(...)` 或与输入建立的语义 label 关联。相邻命令输入在 `/Users/hdh/.codex/worktrees/91d7/keelshell/crates/keelshell-app/src/snippet_editor/view.rs:161-163` 明确设置了 `.accessibility_id("snippet-command")` 和 `.aria_label(...)`，说明当前实现已经有可用的显式做法，但元数据输入没有采用。
- 影响：辅助技术不能可靠得到这三个输入的稳定、双语字段名称，可能只能读出通用输入角色、placeholder 或不完整上下文；用户难以区分名称、说明和标签，违背本切片要求的 accessibility 覆盖。现有 GPUI 回归只检查元素可见、命令 ARIA label、按钮可达和主题/语言矩阵，没有检查这三个输入的可访问名称。
- 建议修复：为三个 `Input` 分别增加稳定 `accessibility_id` 与双语 `aria_label`，并补充中英文回归断言。

## 实际验证命令与计数

所有 Cargo 命令均使用独立 target `work/review-final-db808e0/target`、`--locked --offline`；完整输出保存在上述日志目录。

1. `CARGO_TARGET_DIR=work/review-final-db808e0/target cargo test -p keelshell-app --bin keelshell-app --tests ai_snippet_save --locked --offline -- --nocapture --test-threads=1`
   - exit 0
   - `6 passed / 0 failed / 0 ignored / 1149 filtered`
   - 日志：`ai_snippet_save.log`

2. `CARGO_TARGET_DIR=work/review-final-db808e0/target cargo test -p keelshell-app --bin keelshell-app --tests snippet_editor --locked --offline -- --nocapture --test-threads=1`
   - exit 0
   - `11 passed / 0 failed / 0 ignored / 1144 filtered`
   - 日志：`snippet_editor.log`

3. `CARGO_TARGET_DIR=work/review-final-db808e0/target cargo test -p keelshell-app --bin keelshell-app --tests command_review_target --locked --offline -- --nocapture --test-threads=1`
   - exit 0
   - `6 passed / 0 failed / 0 ignored / 1149 filtered`
   - 日志：`command_review_target.log`

4. `CARGO_TARGET_DIR=work/review-final-db808e0/target cargo test -p keelshell-core --locked --offline -- --nocapture --test-threads=1`
   - exit 0
   - core 普通 unit/integration test binaries 合计 `634 passed / 0 failed / 0 ignored`
   - 日志：`core_all.log`

5. `CARGO_TARGET_DIR=work/review-final-db808e0/target cargo test -p keelshell-core --doc --locked --offline`
   - exit 0
   - `7 passed / 0 failed / 0 ignored`
   - 日志：`core_doctest.log`

6. `cargo fmt --all -- --check`
   - exit 0
   - 日志：`fmt.log`

7. `CARGO_TARGET_DIR=work/review-final-db808e0/target cargo clippy --workspace --all-targets --locked --offline -- -D warnings`
   - exit 0
   - 日志：`clippy.log`

8. `git diff --check b89445604b4ce44fc5a80ff26605f6e6b9039d57 db808e0b45a788f4744acf2fe98cde7bae7b2f0a`
   - exit 0，未输出 whitespace error
   - 日志：`diff-check.log`

app 测试链接阶段出现已知 macOS linker warning：`__eh_frame section too large (max 16MB) to encode dwarf unwind offsets in compact unwind table`。warning 未改变任何命令的 exit code。

## 未验证边界

本次独立复核没有执行完整 `scripts/check.py`，也没有把作者门禁结果代签为独立 full gate。没有执行真实 macOS 窗口级验收、Windows/Linux 原生桌面、IME、VoiceOver/NVDA 或其他完整辅助技术矩阵；没有执行真实客户 SSH、主机密钥变化现场、Podman、外部 AI provider、跨进程 StateStore、断电耐久性、签名、公证、安装升级、发布制品或 CI runner 验收。

定向 GPUI 测试中的 SSH pane 是确定性进程内夹具，用于验证事件、目标身份、写入副作用和 UI 状态；它不等于真实客户连接或原生桌面验收。作者记录中的 `2499 passed / 23 ignored`、Python controller、完整门禁及其他原生边界仍属于作者证据，不在本次独立复核计数中。
