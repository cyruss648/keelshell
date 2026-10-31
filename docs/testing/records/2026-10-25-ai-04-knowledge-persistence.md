# AI-04 单条本地知识持久化测试记录 — 2026-10-25

## 范围

本记录覆盖 AI-04 第一条 StateStore 持久化切片及其冲突恢复：单条 completed/failed session capture 的显式保存、受控加载、stale snapshot 冲突、敏感内容阻断、幂等 receipt，以及 `context-selection`/`context-screen` 到 Workspace 捕获、预览、确认和保存的 GPUI 集成链。

本轮继续实现了 Conflict/Unknown 后的显式 reload/reconcile。reload 在后台 executor 读取最新 StateStore，成功后替换 Workspace 配置快照、清除旧 knowledge preview/review ID、撤销旧保存 candidate，并要求用户重新生成预览和重新确认。AI Settings、Updates、Vault、Profile Sync、Configuration Recovery 等配置入口在 reload 期间被串行化；迟到的配置回调不能污染新快照。该动作不会自动保存、自动合并或自动重放旧请求。

## 已验证行为

- completed 和 failed 知识条目能够保存到临时 StateStore，并在重新加载后保留 source/session identity、capture type、revision、时间和结果。
- StateStore 的 stale snapshot 会返回 `Conflict`，不会替换已经保存的状态；相同 receipt 重复提交是幂等的。
- unknown、敏感、包含 exclusion 的混合文档和非 session 来源不能构造或写入 `SavedKnowledgeEntry`；未知结果不自动重试。
- 受控 wire 加载拒绝敏感标题/摘要和未知字段；旧 AppState 缺少 `knowledge_entries` 时按空列表加载。
- 真实 Workspace GPUI 测试通过 `context-screen` 和 `context-selection` 进入 Assistant，完成/失败确认后由保存按钮发出事件，后台 StateStore 写入并重新加载确认结果。
- 保存中的按钮不可用，成功后显示已保存；保存结果未知的 UI 状态保留 UNKNOWN 并禁用自动重试。
- StateStore Conflict 后，真实 Workspace GPUI 测试通过用户点击 `knowledge-reload` 读取竞争写入后的最新配置；旧 preview/review ID 被清除，保存按钮不再出现，必须重新生成预览，且外部状态和知识条目保持不变。
- reload 期间 AI Settings、Updates、Vault、Profile Sync 和 Configuration Recovery 的打开或 Apply 入口被拒绝或序列化；reload 失败会解除 busy 状态、保持知识保存阻断并允许再次读取。
- reload 成功后会重新绑定 active AI profile、清理临时 AI credentials、应用 locale/theme，并刷新 update service、library selection、reconnect、MCP、Agent、snippet source 和 command source revision。

## 命令与当前结果

功能 worktree `2e4dad0` 已通过：

```text
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test -p keelshell-core --test knowledge_persistence --locked --offline
cargo test -p keelshell-core --test storage --locked --offline
cargo test -p keelshell-app --locked --offline knowledge -- --nocapture
cargo test -p keelshell-app --locked --offline configuration_recovery -- --nocapture
cargo test -p keelshell-app --locked --offline profile_sync -- --nocapture
cargo test -p keelshell-app --locked --offline workspace_knowledge_reload_rebinds_authoritative_ui_configuration -- --nocapture
cargo test -p keelshell-app --locked --offline workspace_blocks_ai_settings_apply_while_state_reloads -- --nocapture
cargo test -p keelshell-app --locked --offline workspace_blocks_update_and_secure_state_panels_while_state_reloads -- --nocapture
cargo test -p keelshell-app --locked --offline workspace_reload_failure_keeps_knowledge_save_blocked -- --nocapture
cargo test -p keelshell-app --locked --offline workspace_knowledge_conflict_invalidates_review_without_replay -- --nocapture
cargo test -p keelshell-app --locked --offline -- --test-threads=4
```

功能 worktree 结果为：core knowledge persistence 6 项通过，core storage 18 项通过，AI/knowledge 过滤测试 10 项通过，Configuration Recovery 过滤测试 9 项通过，Profile Sync 过滤测试 13 项通过，reload 聚焦测试 7 项通过，busy guard 测试 2 项通过，app 全量 `765 passed, 2 ignored`。测试使用临时目录和受控 GPUI/内存 SSH 夹具，不启动真实模型或客户主机。

主线代码合入提交链为 `f9c6956`、`a38060e`、`d18a227`、`4c483b7`，当前代码验证以 `4c483b7` 为准：`cargo fmt`、`git diff --check`、workspace 严格 Clippy 均通过；core 全量为 `505 passed`，AI 全量为 `176 passed, 5 ignored`，app 全量为 `765 passed, 2 ignored`。本次 `python3 scripts/check.py` 退出码为 0：Python 检查 `67 passed, 1 skipped`，脚本内 workspace Rust 为 `1930 passed, 23 ignored`。保留已有 `block 0.1.6` future-incompatibility 和 macOS linker 警告，它们不改变退出码。最终文档提交和远端 readback 在本记录提交后执行，并在交接文档和交付回执中记录。

## 独立复核

最终独立 reviewer Hypatia 审查功能提交 `2e4dad0`，结论为 `MERGE`：P0=0，P1=0，P2=2。

剩余 P2 为：

1. 本记录需要在主线合入和最终门禁后补写准确 SHA、数量和远端读回。
2. 还缺少 Configuration Recovery 迟到 `Restored` 回调的完整时序集成测试。当前实现已有 panel identity、`state_reloading` 和入口 lease 保护；该项是验证覆盖建议，不改变当前 reload 的 fail-closed 行为。

## 未完成边界

应用内知识条目合并、编辑、删除、搜索/分类 UI、批量管理、知识内容的显式模型发送、重启恢复旧 SSH session、NET-01 真实 forwarding executor、listener 启停、运行状态持久化、Finder 原生拖放完整验收、macOS/Windows/Linux 原生桌面、发布签名、公证、安装升级回滚和提交级 CI 仍未完成或未验证。

本记录中的所有 GPUI 结果均使用临时目录和受控 fixture；它们不代表 macOS/Windows/Linux 原生桌面验收、真实客户 SSH、模型服务或发布安装验收。reload 只读取最新 StateStore、更新当前配置并清除旧审核，不提供条目合并或自动重放。
