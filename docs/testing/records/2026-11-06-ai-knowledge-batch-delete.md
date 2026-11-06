# 2026-11-06 — 本地知识库审核式批量删除

## 范围

该记录保留 `feature/knowledge-batch-delete-20261106` 来源切片的实现与专项证据；其源码已由 `main` 的后续知识分类组合吸收。本切片在既有本地
知识库的单条编辑/删除基础上，加入独立的管理选择、最多 32 条的批量删除审核、
原子内存候选和一次 StateStore 保存。Ask 附件、编辑器草稿、SSH 会话和远端
内容不属于批量删除授权。

## 实现

- `keelshell-core` 新增 `AppState::MAX_KNOWLEDGE_DELETE_ENTRIES` 和
  `remove_knowledge_entries`。它在删除前逐条核对完整 `SavedKnowledgeEntry`，
  拒绝空集合、超限、重复身份、缺失/变更原件和无效候选；失败保持状态与
  snapshot revision 不变，成功保留 survivors 的完整顺序。
- `keelshell-app` 将管理选择与 Ask context、编辑器 `selected` 分离。每次选择
  变化生成新 revision，审核捕获完整原件和审核 UUID；搜索隐藏已选条目时保留
  选择并显示隐藏计数。
- 批量审核逐条展示 UUID、正文、来源和原结果，正文滚动而确认/取消固定。
  Workspace 通过统一后台保存队列完成一次持久化；候选失败、保存冲突、耐久性
  未知或 I/O 失败均要求显式重新加载，不自动重放。

## 定向验证

以下命令均在本工作树、`--locked` 和本地共享 Cargo target 上执行，未启动远端
服务、SSH 会话或模型请求：

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all` | exit 0 |
| `git diff --check` | exit 0 |
| `cargo test -p keelshell-core --locked knowledge_batch_delete -- --nocapture` | exit 0；8 passed，578 filtered |
| `cargo test -p keelshell-app --locked knowledge_batch -- --nocapture --test-threads=4` | exit 0；17 passed，978 filtered |
| `cargo test -p keelshell-core --locked knowledge_library -- --test-threads=4` | exit 0；8 passed，578 filtered |
| `cargo test -p keelshell-app --locked knowledge_library -- --test-threads=4` | exit 0；31 passed，964 filtered |
| `cargo clippy -p keelshell-core -p keelshell-app --locked --all-targets --all-features -- -D warnings` | exit 0；无 Clippy 问题 |
| `python3 scripts/check.py`（第一轮） | exit 0；Python 131 run / 1 skipped，workspace app 993 passed / 2 ignored，core 181 passed，14 rustdoc；依赖策略、格式、workspace Clippy 和附加控制器通过 |
| `python3 scripts/check.py`（第二轮，补充 snapshot revision 修复后） | exit 0；Python 131 run / 1 skipped，workspace app 993 passed / 2 ignored，core 181 passed，14 rustdoc；workspace tests、doc tests、readiness controller、依赖策略、格式和 workspace Clippy 全部通过 |

测试覆盖了核心事务、GPUI 管理选择、旧绘制动作失效、ABA 选择、Ask 迟到
请求撤销、重载边界、键盘导航、三主题×明暗×双语、取消零写入、一次批量保存、
竞争配置冲突、重启读回和固定操作区。定向测试成功不等于完整工程门禁。

## 独立复核

新子代理对本切片进行了严格只读静态代码和调用链复核，没有编辑、提交、清理
或运行完整测试。复核覆盖 core 原子批量删除、完整原件/身份校验、管理选择
revision 与 ABA 旧绘制事件、Ask/编辑器状态隔离、Workspace 单次 `persist`
和冲突/未知耐久性后的显式 reload，以及紧凑/宽窗口三主题×明暗×中英文固定
审核 footer。结论如下：

- P0：0；P1：0；P2：0；P3：0
- `NO_BLOCKER`
- 合入建议：`APPROVE`

复核给出的关键源码证据为 `crates/keelshell-core/src/knowledge_library.rs:76-121`、
`crates/keelshell-app/src/assistant/knowledge_library/batch.rs:32-166`、
`crates/keelshell-app/src/assistant/knowledge_library.rs:389-439`、
`crates/keelshell-app/src/workspace/knowledge_library.rs:20-68` 和
`crates/keelshell-app/src/assistant/knowledge_library/batch.rs:297-346`。

当前未进行发行构建、安装、签名、远端同步、跨平台桌面或安全擦除验收；
配置备份保留历史副本的边界仍然有效。来源切片的完整工程门禁和独立复核通过后，
已通过后续 `main` 功能组合进入当前集成树；当前组合的提交、推送和原生验收
以交接文档及本次组合记录为准。

## English

This slice adds an independent management selection, a reviewed deletion of up to 32
saved knowledge entries, an atomic in-memory candidate and one StateStore save. Ask
attachments, editor drafts, SSH sessions and remote content have separate authority.
The core validates every complete original before changing memory; the UI binds the
selection revision and review UUID; Workspace performs the blocking save off the UI
thread and requires an explicit reload after any failed or uncertain outcome.

The focused core run passed 8 tests and the focused application run passed 17 tests;
the full knowledge-library application run passed 31 tests. Two complete project
gates passed with Python 131 run / 1 skipped, app 993 passed / 2 ignored, core 181
passed and 14 rustdoc checks, including the readiness controller. A new sub-agent
performed a strict read-only source and behavior review and reported P0/P1/P2/P3 = 0,
`NO_BLOCKER`, and `APPROVE`. The source slice was ready for integration at that stage;
it has since been absorbed by the later `main` knowledge-category combination.

Release builds, installation, signing, remote synchronization, native desktop
acceptance on other platforms and secure erasure remain outside this record.
