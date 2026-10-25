# AI-04 单条本地知识持久化测试记录 — 2026-10-25

## 范围

本记录覆盖 AI-04 第一条 StateStore 持久化切片：单条 completed/failed session capture 的显式保存、受控加载、stale snapshot 冲突、敏感内容阻断、幂等 receipt，以及 `context-selection`/`context-screen` 到 Workspace 捕获、预览和保存的 GPUI 集成链。

## 已验证行为

- completed 和 failed 知识条目能够保存到临时 StateStore，并在重新加载后保留 source/session identity、capture type、revision、时间和结果。
- StateStore 的 stale snapshot 会返回 `Conflict`，不会替换已经保存的状态；相同 receipt 重复提交是幂等的。
- unknown、敏感、包含 exclusion 的混合文档和非 session 来源不能构造或写入 `SavedKnowledgeEntry`；未知结果不自动重试。
- 受控 wire 加载拒绝敏感标题/摘要和未知字段；旧 AppState 缺少 `knowledge_entries` 时按空列表加载。
- 真实 Workspace GPUI 测试通过 `context-screen` 和 `context-selection` 进入 Assistant，完成/失败确认后由保存按钮发出事件，后台 StateStore 写入并重新加载确认结果。
- 保存中的按钮不可用，成功后显示已保存；保存结果未知的 UI 状态保留 UNKNOWN 并禁用自动重试。

## 命令与当前结果

本轮功能 worktree 已通过：

```text
cargo fmt --all -- --check
git diff --check
cargo test -p keelshell-core --test knowledge_persistence --locked --offline
cargo test -p keelshell-app --locked --offline knowledge -- --nocapture
cargo test -p keelshell-app --locked --offline workspace_capture_review_and_save_round_trips_confirmed_knowledge -- --nocapture
cargo test -p keelshell-app --locked --offline workspace_selection_capture_uses_the_focused_terminal_source -- --nocapture
cargo test -p keelshell-app --locked --offline explicit_terminal_context_builds_reviewable_knowledge_preview_and_keeps_source_binding -- --nocapture
cargo test -p keelshell-app --locked --offline unknown_knowledge_write_result_disables_automatic_replay -- --nocapture
```

结果为 core 持久化 6 项通过、AI-04 相关 app 6 项通过；测试使用临时目录和受控 GPUI/内存 SSH 夹具，不启动真实模型或客户主机。保留已有 `block 0.1.6` future-incompatibility 和 macOS linker 警告，它们不改变退出码。

本轮完整门禁的实际结果为：`cargo fmt --all -- --check`、`git diff --check`、三个 crate 的目标 Clippy 均通过；`cargo test -p keelshell-core --locked --offline` 为 505 项通过，`cargo test -p keelshell-ai --locked --offline` 为 176 项通过、5 项忽略。`cargo test -p keelshell-app --locked --offline` 首次为 758 项通过、1 项失败、2 项忽略，失败是 `files::transfer_tests::fixture_isolation_tests::removed_window_keeps_admission_until_real_worker_cleanup_finishes` 的有界 fixture setup 超时；立即单独重跑该测试为 1 项通过、760 项过滤。之后 `python3 scripts/check.py` 退出码为 0：Python 检查 67 项通过、1 项跳过，脚本内 workspace app 为 759 项通过、2 项忽略，core、session、MCP、集成测试和 doctest 均通过。

完整 workspace 门禁、独立 reviewer 和主线合入后的最终结果在本记录后续补记；在这些步骤完成前，不把该 worktree 结果写成发布或三平台原生验收。

## 未完成边界

条目编辑、删除、搜索/分类 UI、批量管理、模型发送、重启恢复旧 SSH session、macOS/Windows/Linux 原生桌面、发布安装和提交级 CI 仍未完成或未验证。
