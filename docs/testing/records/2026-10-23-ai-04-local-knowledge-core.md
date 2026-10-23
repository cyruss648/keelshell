# AI-04 本地知识领域切片测试记录

**日期：** 2026-10-23
**范围：** `keelshell-core::ai_knowledge` 纯函数模型与服务
**状态：** 本地领域测试记录；未接入 UI、持久化或发送流程

## 验证范围

- 用户显式选择的成功事实和已知失败事实形成来源绑定摘要。
- 时间线按 Unix 时间戳稳定排序，并保留来源 UUID、revision 和安全标签。
- unknown、not started、cancelled 只产生无正文状态标记并阻止保存，标题也不进入时间线 Markdown。
- `token`、带空格的 `api key` 等凭据标记、`Authorization`、私钥和相关敏感来源元数据不会出现在摘要或 Markdown 中；来源标签在构造阶段拒绝，标题／摘要命中时产物标记为 `DoNotPersist`。
- runbook 只生成说明性 Markdown，不执行 action，不联网，不写文件。
- 本地搜索有查询、输入、结果数量和 snippet 上限，返回一基线号与 Unicode 列号；大小写折叠改变字节长度时仍映射回原文列号。
- 空选择、过多事实、超大文本、空白文本、无效来源、NUL、空查询和超长查询返回 typed error。

## 工程边界

本记录只覆盖内存领域逻辑。没有调用 `keelshell-app`，没有增加 UI 审核按钮，没有接入 `StateStore`，没有向 `keelshell-ai` provider 发送任何上下文，也没有从任务审计正文自动导入知识。后续 UI 必须在保存或发送前重新展示产物并要求用户明确确认；当前 `MarkdownArtifact::disposition()` 是领域层提供的保存闸门。

## 执行命令

```text
cargo fmt --all -- --check
cargo test -p keelshell-core --locked ai_knowledge
cargo clippy -p keelshell-core --all-targets --all-features --locked -- -D warnings
cargo test -p keelshell-core --doc --locked
```

## 实际结果

- `cargo fmt --all -- --check`：通过。
- `cargo test -p keelshell-core --locked ai_knowledge`：7 项通过。
- `cargo clippy -p keelshell-core --all-targets --all-features --locked -- -D warnings`：通过，无警告。
- `cargo test -p keelshell-core --locked`：481 项通过。
- `cargo test -p keelshell-core --doc --locked`：6 项通过。
- 最终变更审查：仅修改 `keelshell-core` 新领域模块、其 `lib.rs` 导出和本 ADR/测试记录；`keelshell-app` 未修改。
- 复核修正：`Cancelled` 时间线分支显式保持 `(status, disposition, entry)` 三元组；unknown／not started／cancelled 和敏感摘要都不保留标题；Unicode 搜索通过折叠偏移映射回原文列号；补充 `api key` 标记、空白文本和 snippet 上限检查；领域产物移除通用 `Deserialize`，不依赖外部数据绕过构造不变量。生成 Markdown 现在对固定模板和 runbook 标题执行 typed 敏感标记检查，不依赖可能被 debug 构建触发的 `debug_assert!`。

跨平台原生 UI、文件持久化、真实模型发送和安装发布不在本记录范围内。
