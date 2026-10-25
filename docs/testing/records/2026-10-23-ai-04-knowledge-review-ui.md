# AI-04 本地知识预览审核 UI 测试记录 — 2026-10-23

> 本记录是 2026-10-23 的 UI 预览快照；其中“保存按钮禁用”和“未覆盖 Workspace capture”只描述该日期的范围。2026-10-25 的单条持久化和完整 capture/save 集成证据见[后续记录](2026-10-25-ai-04-knowledge-persistence.md)。

## 范围

本记录覆盖 `keelshell-app` AI 助手中的显式本地知识预览入口，以及它与 `keelshell-core::ai_knowledge` 的边界。测试使用 GPUI Kit 受控窗口和内存领域对象，不启动模型、网络或命令执行。

## 已验证行为

- 没有当前 session、host 或上下文时，预览入口不可用。
- 用户显式捕获终端内容后，可以生成来源绑定的 `Unknown` 知识文档和 Markdown 预览；真实 GPUI 按钮确认完成后才生成条目。
- 预览显示来源标签、捕获类型、session identity、来源 revision、捕获时间、条目和排除项。
- 未确认的未知结果显示 `DoNotPersist`；只有用户明确确认完成或失败且通过敏感内容闸门后才显示 `Persistable`。保存按钮仍为禁用，因为 StateStore 接入尚未实现。
- 敏感内容通过领域层变成 `DoNotPersist`，Markdown 不包含敏感正文，保存按钮保持禁用。
- 生产代码路径会为同一个 session 的重复捕获保持 source identity，并让 source revision 随捕获递增；切换 session 时由 workspace 绑定新的 source identity。当前专项 GPUI 测试直接驱动 `AssistantPanel::set_context`，没有真实点击 `context-selection` 或 `context-screen`，也没有覆盖完整的 workspace capture event 订阅路径。
- 真实 `knowledge-confirm-completed` 按钮交互会把未知预览转换为可审核的完成事实；敏感内容仍由领域层阻止，Markdown 不包含敏感正文。
- 预览不会自动发送到模型，也不会执行任何 action。

## 命令与结果

```text
cargo fmt --all -- --check
cargo check -p keelshell-app --locked --offline
cargo clippy -p keelshell-app --all-targets --all-features --locked --offline -- -D warnings
cargo test -p keelshell-app --locked --offline explicit_terminal_context_builds_reviewable_knowledge_preview_and_keeps_source_binding -- --nocapture
cargo test -p keelshell-ai --locked --offline spaced_api_key_marker_is_redacted -- --nocapture
```

结果：格式检查通过；应用编译通过；严格 Clippy 退出 0；新增 GPUI 测试 1 项通过；普通 AI Redactor 的带空格 `api key` 回归测试 1 项通过。编译输出保留已有传递依赖 `block 0.1.6` 的 future-incompatibility 提示。

## 未完成边界

本记录不证明完整 workspace capture 集成测试、知识持久化、条目编辑/删除、模型发送、真实供应商、跨平台原生窗口、IME、VoiceOver 或 Finder 拖放验收。普通 AI Redactor 与知识预览共用带空格 `api key` 的敏感标记回归，但本记录仍不宣称覆盖所有秘密格式。AI-04 后续必须继续保留“预览 → 明确保存/发送”的人工审核边界，并在 StateStore 接入时绑定 snapshot/revision 和未知写入结果。
