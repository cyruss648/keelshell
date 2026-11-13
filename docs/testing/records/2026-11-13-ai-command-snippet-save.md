# AI 审核命令保存为片段测试记录

日期：2026-11-13（Asia/Shanghai）

基线：`e1d410d2d61753f2e90296981199ed2b4b3d3703`

状态：**专项 GPUI/纯逻辑与 focused 静态门禁通过；提交后的独立复核、原生平台和真实 provider/SSH 验收未覆盖**

## 范围

本片把助手中的普通命令建议和诊断计划步骤接入既有命令片段编辑器。来源保留原始
命令、目标、会话、review ID 和 120 秒单调审核时间。编辑器可以修改名称、说明、标签、
命令和参数模式；保存事件重新检查来源与当前活动终端后，沿用
`AppState::insert_snippet` 和既有 StateStore 持久化。

保存不会填入命令栏、向终端发送字节或执行命令。命令被编辑时，来源横幅显示原审核
只覆盖原始命令并提示重新审核。来源横幅、AI 命令预览和命令库预览使用现有 typography
token，没有改主题系统或增加依赖。

## 定向结果

| 命令 | 结果 |
| --- | --- |
| `cargo check -p keelshell-app --locked` | 通过 |
| `cargo check -p keelshell-app --tests --locked` | 通过 |
| `cargo test -p keelshell-app --locked ai_command_snippets -- --nocapture` | `3 passed / 0 failed / 1122 filtered out` |
| `cargo test -p keelshell-app --locked reviewed_ai_source_validation_is_bounded_and_monotonic -- --nocapture` | `1 passed / 0 failed / 1124 filtered out` |
| `cargo fmt --all` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy -p keelshell-app --all-targets --locked -- -D warnings` | 通过 |
| `cargo test -p keelshell-app --locked workspace::tests::ai_command_review_target -- --nocapture` | `1 passed / 0 failed / 1124 filtered out` |
| `git diff --check` | 通过（提交前复核） |

`cargo check -p keelshell-app --locked` 和
`cargo check -p keelshell-app --tests --locked` 在最终事件载荷修订后均通过。严格
Clippy 首轮发现 `SnippetEditorEvent::Save` 的完整 `Snippet` 使 `Save/Cancel` 触发
`large_enum_variant`；将保存载荷装箱为 `Box<Snippet>` 后重新执行 Clippy 与测试，
行为和计数保持通过。GPUI 测试链接阶段仍打印既有
`__eh_frame section too large` linker warning，未影响退出码。

新增 GPUI 回归覆盖：

1. AI 回复中的“保存为命令片段”按钮可见，点击后打开既有编辑器；
2. 编辑器显示原始目标与会话，并保留原始命令；
3. 修改命令后横幅显示原审核范围和重新审核提示；
4. 活动会话变化后保存被阻止，编辑器和完整草稿保留，StateStore 片段快照不变；
5. 重复片段 ID 显示失败详情，既有片段仍保持不变，终端没有收到写入。

纯逻辑回归覆盖有效来源、空命令、危险控制字符、空目标、非法会话、nil review ID
和过期审核时间。既有片段编辑器 GPUI 回归继续覆盖空命令、危险控制字符和失败后
草稿恢复。

## 修订事实

首轮 GPUI 执行发现来源横幅和错误状态节点没有稳定的测试/无障碍标签；实现已补充
原始目标、审核状态和错误文本的 `aria_label`，随后三个 GPUI 用例全部通过。测试夹具
还修正了默认命令片段快照的期望，避免把内置片段误判为本片写入。

## 未覆盖边界

- 本片没有把审核来源持久化到 JSON，也没有为编辑后的命令自动生成第二张审核票据；
  保存后使用命令片段仍经过现有命令栏审核。
- 没有等待真实 120 秒 UI 定时器；过期分支使用单调 `Instant` 纯逻辑测试覆盖。
- 没有调用真实 AI provider、真实 SSH 主机、真实远端执行或客户数据。
- 没有完成 macOS/Windows/Linux 原生窗口、输入法、VoiceOver、签名、安装和发布包验收。
- 本记录的 focused 结果不替代完整 `scripts/check.py`、独立只读复核或原生平台验收。
