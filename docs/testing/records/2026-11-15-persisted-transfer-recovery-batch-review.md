# 2026-11-15 持久化传输恢复批量整理专项验证

状态：`LOCAL_FOCUSED_PASS / MAIN_INTEGRATION_AND_REVIEW_PENDING`。
本记录与同一提交内的源码对应；产品范围见
[批量整理说明](../../product/PERSISTED_TRANSFER_RECOVERY_BATCH_REVIEW.md)。

## 源码与范围

- 开发分支：`feature/persisted-recovery-batch-review-20261115`。
- 基线：`2fa3557adf7513d96987b1758c42790c66155681`。
- 主线程后续基线 `59c59a8` 的依赖升级不在本切片内；本片没有修改
  Cargo.toml、Cargo.lock 或新增跨 crate 依赖。
- 未修改 HANDOFF、ROADMAP、CAPABILITIES、testing/README 或 ADR 0131；
  集中集成、全量门禁、Podman 与最终独立复核由主线程完成。
- 环境为 macOS arm64，实际 active `rustc 1.100.0`、`cargo 1.100.0`。
  没有设置项目工具链 pin 或改动 MSRV。

## 专用 target 与实际结果

本 worktree 独占：

```text
CARGO_TARGET_DIR=work/transfer-recovery-batch-review-20261115/focused-target
```

没有借用主线程或其他 checkout 的 target。源码在当前 worktree 实际重新编译，
最终 app 测试清单明确列出全部 27 项 `parallel_tests`，包含本片新增的 8 项。

| 检查 | 实际结果 |
| --- | --- |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo check -p keelshell-app --locked` | exit 0 |
| `cargo clippy -p keelshell-app -p keelshell-core --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test -p keelshell-app --locked -- parallel_tests --list` | 27 tests，exit 0 |
| `cargo test -p keelshell-app --locked -- parallel_tests --test-threads=1` | 27 passed / 0 failed / 0 ignored，1157 filtered，exit 0 |
| `cargo test -p keelshell-core --lib --locked -- transfer_recovery --test-threads=1` | 8 passed / 0 failed / 0 ignored，214 filtered，exit 0 |
| `python3 scripts/check.py --policy-only` | passed，exit 0 |
| `git diff --check` | exit 0 |

实际通过的不同用例合计 35 项；测试清单、重复运行和旧轮次不额外累计。
本次没有执行全仓 `scripts/check.py`、workspace 测试或 workspace doctest。
基线的完整门禁不能代替当前切片的集中合入验收。

日志与退出回执保留在
`work/transfer-recovery-batch-review-20261115/`：

- `parallel-tests-final-r2.log` 与 `.exit`：最终 app 27/27，耗时 15.73 秒。
- `core-recovery-tests.log` 与 `.exit`：core 8/8。
- `strict-clippy-final.log`、`app-check-final.log` 与各自 `.exit`。
- `app-test-inventory-final.log` 与 `.exit`：当前源码的实际 27 项清单。
- 前轮失败和诊断日志保留；可重建的专用 target 在交付时清理。

## 新增 GPUI 覆盖

以下用例均使用已有 SSH/SFTP fixture、真实 GPUI window/context、隔离临时日志，
等待限定为 5 或 8 秒，没有真实客户主机：

- `persisted_recovery_batch_selection_counts_and_preserves_record_order`：
  多选与可访问的真实数量；点击顺序不同仍按当前列表建立队列；
  560×440 紧凑窗口中的三主题、中英文操作按钮可滚动到达；
  建立队列不产生 claim、只读计划或传输。
- `persisted_recovery_batch_enters_one_independent_review_and_cancel_clears_queue`：
  一次只进入队首的独立只读计划与审核，取消清空队列并释放 claim；
  丢失已审核 claim 后确认被拒绝，未创建传输任务。
- `persisted_recovery_batch_close_rejects_late_claim_and_old_session_entry`：
  同一轮同步事件启动 claim 后关闭详情，迟到 claim 不产生计划；
  旧绘制入口不能采用新 session，恢复记录保留。
- `persisted_recovery_batch_busy_claim_fails_closed_and_keeps_record_available`：
  外部持有同条 claim 时拒绝审核，队列清空，原记录可重新 claim。
- `persisted_recovery_batch_record_disappearance_clears_waiting_queue`：
  当前有效 load 发现待审核记录缺失后清空整个队列。
- `persisted_recovery_batch_discard_confirms_once_and_retains_busy_record`：
  展示确认前不删除；三条记录逐条 claim，Busy 中间项保留，其余两条删除，
  失败项重新选中并报告。
- `persisted_recovery_batch_discard_retains_record_changed_after_confirmation`：
  显示待确认候选后、点击确认前，真实 store 用同 ID 替换为新路径/进度/状态；
  后台 claim 内的完整快照比较拒绝删除新版本，并重新加载实际记录。
- `persisted_recovery_batch_discard_unknown_worker_result_reloads_actual_store`：
  故障注入模拟先删除一条后整体结果丢失；生产 GPUI 完成路径必须重新 load，
  不恢复已删除项、不删除剩余项，未知删除不冒充“全部保留”，且不自动重试。

同组保留的单条未绑定主机确认、连接绑定、claim 生命周期、旧 load/mutation
交错、持久化进度、原队列与隔离回归也全部通过。

## 失败历史与修正

首轮 focused app 为 19 passed / 5 failed：新增测试辅助点击仍假定旧
`file-tools-scroll` 容器。已让现有点击夹具使用实际的队列详情滚动表面，
没有修改业务逻辑以迁就测试。

第二轮为 23 passed / 1 failed：新只读计划 fixture 使用了 macOS 的 `/var`
符号链接路径，触发既有安全路径检查；改用 canonical 真实路径后通过。
一次诊断编译遇到 Operation 缺少 Debug，一次精确过滤执行 0 项，均不作为验收。

首轮严格 Clippy 发现 `manual_contains`、`type_complexity` 和
`if_same_then_else`；已修正，最终严格运行退出 0。
app 测试链接出现已知 `__eh_frame section too large` warning，未改变退出码。

## 未验证边界与交付

当前是作者专项源码/工程回执，不是全量集成或独立复核通过。
macOS 原生窗口、Windows/Linux 原生、真实客户 SSH/SFTP、Podman、IME、
完整辅助技术矩阵、签名公证、安装升级与发布均未在此切片验收。

只做本地源码提交，不 push。保留功能分支和 worktree 供主线程 cherry-pick；
没有启动应用、容器、服务或新用户聊天。临时只读审查进程使用 ephemeral 模式，
已按集中复核分工终止，不提供独立审批结论。
