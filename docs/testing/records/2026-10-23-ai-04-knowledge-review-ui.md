# AI-04 本地知识预览审核 UI 测试记录 — 2026-10-23

## 范围

本记录覆盖 `keelshell-app` AI 助手中的显式本地知识预览入口，以及它与 `keelshell-core::ai_knowledge` 的边界。测试使用 GPUI Kit 受控窗口和内存领域对象，不启动模型、网络或命令执行。

## 已验证行为

- 没有当前 session、host 或上下文时，预览入口不可用。
- 用户显式捕获终端内容后，可以生成来源绑定的知识文档和 Markdown 预览。
- 预览显示来源标签、来源 revision、来源类别、条目和排除项。
- `Persistable` 结果显示本地内容闸门通过，但保存按钮仍为禁用，因为 StateStore 接入尚未实现。
- 敏感内容通过领域层变成 `DoNotPersist`，Markdown 不包含敏感正文，保存按钮保持禁用。
- 同一个 session 的重复选择保持 source identity；切换 session 时由 UI 生成新的 source identity。
- 预览不会自动发送到模型，也不会执行任何 action。

## 命令与结果

```text
cargo fmt --all -- --check
cargo check -p keelshell-app --locked --offline
cargo test -p keelshell-app --locked --offline explicit_terminal_context_builds_reviewable_knowledge_preview_and_keeps_source_binding -- --nocapture
```

结果：格式检查通过；应用编译通过；新增 GPUI 测试 1 项通过。编译输出保留已有传递依赖 `block 0.1.6` 的 future-incompatibility 提示。

## 未完成边界

本记录不证明知识持久化、条目编辑/删除、模型发送、真实供应商、跨平台原生窗口、IME、VoiceOver 或 Finder 拖放验收。AI-04 后续必须继续保留“预览 → 明确保存/发送”的人工审核边界，并在 StateStore 接入时绑定 snapshot/revision 和未知写入结果。
