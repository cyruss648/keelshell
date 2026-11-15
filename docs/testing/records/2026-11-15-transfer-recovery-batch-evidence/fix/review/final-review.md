**KeelShell 恢复队列 worker 启动失败修复：独立严格只读复核**

评审日期：2026-11-15，Asia/Shanghai。结论：**APPROVE**。本次三文件冻结修复 delta 未发现需报告的问题，**P0=0 / P1=0 / P2=0 / P3=0**。此前独立报告中的 worker 启动失败回队 P2，**在本次冻结候选上关闭**。

本结论针对未提交的冻结字节；HEAD `5cdbc9617164d6cfbd5e7a9a022539934221ee75` 是比较基线，**不含本次修复**。最终正式提交仍由父线程在作者完整 gate 与本次 review 都通过后完成，并把正式 SHA 与以下冻结字节绑定。本报告不代签并行运行中的作者完整 gate。

**固定输入与操作边界**

| 项目 | 实际核对值 |
| --- | --- |
| 唯一源码 / 全部 Cargo cwd | `/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell` |
| 基线 HEAD，开始与结束相同 | `5cdbc9617164d6cfbd5e7a9a022539934221ee75` |
| 独占输出目录 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/fix/review` |
| 唯一 Cargo target | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/review/target` |
| 冻结清单 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/fix/frozen-inputs.json` |
| 清单 SHA-256，开始与结束相同 | `f8d1acaa5dffdbf27fa0207dbceb03774cc154ffa84adde7f55b9befc98df724` |
| 本人重新核对冻结输入 | 开始与结束均为 **524/524 匹配，0 mismatch**；另核对清单中的 10 个脚本在评审 checkout 的对应字节，全部匹配 |
| 全部受跟踪文件 | **1139** 个；结束 `changed_tracked_bytes=[]` |
| 索引 SHA-256，开始与结束相同 | `8bd9743f5ed352eab059a19c423a548e2404aaf0d5f4237f7fb6fd4ce9f5246f` |
| 工具链 | `rustc 1.100.0 (0ef1bc386 2026-11-09)`；`cargo 1.100.0 (3acc79c0c 2026-11-05)`；`clippy 0.1.100 (0ef1bc3860 2026-11-09)`；host `aarch64-apple-darwin` |

冻结候选的三个文件 SHA-256：

| 相对源码路径 | SHA-256 |
| --- | --- |
| `crates/keelshell-app/src/files.rs` | `ae1f96e8da296332e755f5a01a3602b6736e9a7edba53a38241a79817da8e5d8` |
| `crates/keelshell-app/src/files/parallel.rs` | `f095398c37321a6be13af1808645844a66fa2826274680b44ff8a0a1807e7caf` |
| `crates/keelshell-app/src/files/parallel_tests.rs` | `12714f0fb2d33d93610e7ca2ca17993c6859580be81901c17fa79da5a9437bb8` |

本人已读用户提供的 AGENTS 指令、源码根 AGENTS.md、`/Users/hdh/.codex/RTK.md`、HANDOFF 顶部、`docs/testing/SHARED_TARGET.md`、`/Users/hdh/.agents/skills/rust-best-practices/SKILL.md` 及相关章节，并读取此前独立报告 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/testing/records/2026-11-15-transfer-recovery-batch-review-before-fix-raw.md`。仅评审相对上述 HEAD 的三文件修复；未重新泛审产品或重复作者完整门禁。

父线程的 `review/fix-target-invalidation.{log,json}` 已直接读回：在本评审源码 cwd、指定 target 下失效五个 workspace 包，exit 0，移除 4340 files / 2.5 GiB。本次没有再次 clean，没有触碰 author target，没有开子 agent 或新聊天。

没有修改源码、仓库文档或索引，没有格式化写回、提交、push、安装或资源清理。新增评审脚本、日志、回执、临时目录与报告均在指定输出目录，Cargo 产物仅在指定 target。测试夹具自身会结束并释放其隔离临时数据与受控连接，不涉及父线程资源清理。

**原 P2 的关闭依据**

1. 原错误链为：spawn 前先清掉 `recovery_review_active`，最后一条时再清掉 session；spawn 返回 Err 后调用的 helper 因 active 不匹配无法回队。当前 [parallel.rs:508](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel.rs:508) 在 active/session 仍有效时尝试启动 worker；[parallel.rs:608](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel.rs:608) 的 Err 分支先调用回队 helper，再清掉持有字段并 return。[parallel.rs:804](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel.rs:804) 的 helper 因而可以把当前 ID 放回队首并清掉 active，保留正确 session。
2. 消费 active/session 的原状态块现位于 [parallel.rs:617](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel.rs:617)，只在启动成功后执行；剩余队列非空时保留 session，最后一条成功启动时清掉 session。UI 状态转移发生在同一同步调用内；worker 结果的 UI 消费在其后才注册，没有将中间 active 状态暴露给另一 UI 回调。
3. [parallel.rs:296](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel.rs:296) 的 helper 在生产构建中直接调用既有 `spawn_owned_transport_worker`，保留 owner、名称、stop、闭包与 `io::Result`。忽略缩进后的生产 diff 没有改变 worker 闭包内部逻辑；恢复日志 start 与实际传输仍发生在 worker 内。启动拒绝会丢弃未执行闭包及其捕获的 claim；本次实际回归也重新取得 claim，验证释放。
4. [files.rs:271](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files.rs:271) 的注入字段和 [files.rs:1093](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files.rs:1093) 的初始化均由 `cfg(test)` 限定；[parallel.rs:304](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel.rs:304) 以 `mem::take` 消费面板局部的一次拒绝，不修改全局 transport registry。测试确认注入被消费，后续显式重试能成功。

**实际行为覆盖**

两个入口为 [parallel_tests.rs:488](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel_tests.rs:488) 的 `persisted_recovery_batch_worker_spawn_failure_restores_order_and_retries_explicitly`（两条），以及 [parallel_tests.rs:495](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel_tests.rs:495) 的 `persisted_recovery_batch_worker_spawn_failure_preserves_the_last_session`（一条）。两者共享实际 GPUI 控件点击、持久 store 与受控 SSH/SFTP 的行为 helper，均在本次 29 项运行中实际通过。

| 要求 | 断言与源码证据 |
| --- | --- |
| 一条和两条启动失败后回到原队首 | `parallel_tests.rs:384` 比较完整 queue 与原 IDs；两条要求 `[A,B]`，一条要求 `[A]`，并校验 session 等于当前 token |
| 失败后不残留已授权运行状态 | `parallel_tests.rs:395` 校验 review active、active recovery、claim、pending 清空，busy=false，注入已消费；failed job 恰为一条 |
| 无目标数据写入，持久记录仍可恢复，claim 实际释放 | `parallel_tests.rs:404` 读取真实 store 核对记录数量；逐条重新取得并释放 claim；`parallel_tests.rs:413` 读取目标，仍是原 `b"01"` |
| 重试经过新检查与新审核，再显式确认才执行 | `parallel_tests.rs:423` 点击“下一条审核”，等待新的 `ResumePersistedFile` pending、检查/claim/持久化任务完成；`parallel_tests.rs:428` 校验新 review ID 不同于失败前 ID，确认前目标仍为 `b"01"`；再次点击 Confirm 后读取 `b"0123456789"` 并校验 Completed |
| 第一条成功后后续条目保持待审核 | `parallel_tests.rs:440` 校验剩余 queue、session、无 active、无 claim requests、无 pending、无 operation、job 数准确；`parallel_tests.rs:467` 对每个等待项实际重新取得 claim 并读取未改变的目标；下一轮仍需显式 Review 和 Confirm |
| 最后一条显式完成后收口 | `parallel_tests.rs:448` 要求最终 session=None；`parallel_tests.rs:479` 要求真实 store 最终为空 |

“没有自动检查”的结论同时依据上述稳定点的队列/claim/pending/operation 断言及调用关系：`parallel.rs:694` 的终态路径仅 reload/prune；进入下一条的 `parallel.rs:913` 与 claim/计划路径 `parallel.rs:1013`、`parallel.rs:1049` 仍依赖显式审核入口。测试没有给远端每一种只读请求安装计数器；不把状态断言扩大成逐 RPC 追踪结论。

这里的“无写入”指失败启动没有执行目标文件续传，目标字节不变。此前只读计划会正常持久化审核进度；本报告不声称整个恢复日志文件在准备、计划和重试过程中从未改变。

**本人独立执行的相关门禁**

每条命令的完整 stdout/stderr 保存为同名前缀 `.log`，其 `.json` 保存 argv、固定 cwd、显式环境覆盖、UTC 起止时间、耗时、退出码和日志 SHA-256。汇总见 [gate-summary.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/fix/review/gate-summary.json)；runner 为同目录 `run_independent_review.py`。所有使用此 target 的 Cargo 命令串行执行。

公共环境覆盖为：

```text
CARGO_NET_OFFLINE=true
CARGO_BUILD_JOBS=2
CARGO_TARGET_DIR=/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/review/target
TMPDIR=/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/fix/review/tmp/
TMP=/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/fix/review/tmp
TEMP=/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/fix/review/tmp
GIT_OPTIONAL_LOCKS=0
PYTHONDONTWRITEBYTECODE=1
```

| 命令 | 真实结果 | 耗时 | 日志与回执前缀 |
| --- | --- | --- | --- |
| `cargo test -p keelshell-app --locked --offline -- --list` | exit 0；重新编译后 **1186 个不同 test 名称**，其中 **29 个 parallel_tests、10 个 persisted_recovery_batch_*** | 59.544 s | `app-inventory` |
| `cargo test -p keelshell-app --locked --offline -- parallel_tests --test-threads=1` | exit 0；**29 passed / 0 failed / 0 ignored / 1157 filtered** | 16.776 s | `app-parallel-tests` |
| `cargo clippy -p keelshell-app --all-targets --locked --offline -- -D warnings` | exit 0，strict Clippy 通过 | 26.947 s | `app-clippy` |
| `cargo fmt --all -- --check` | exit 0，没有写回 | 2.143 s | `fmt-check` |
| `git diff --check HEAD` | exit 0，检查未提交候选相对基线的 diff | 0.019 s | `git-diff-check` |

合计 **5 条指定门禁命令，全部 exit 0**。实际执行项目测试仅计 **29 passed / 0 failed / 0 ignored**；清单中的 1186 项未全量执行，10 项批量用例和其中 2 项新回归是 29 项的子集，不重复累计。没有运行整个 workspace 测试、doctest、依赖新鲜度查询或完整 gate。

清单的 10 个批量用例均带完整前缀 `files::transfer_tests::parallel_tests::`，全部在清单中且在本次测试日志中逐项为 `ok`：

```text
persisted_recovery_batch_busy_claim_fails_closed_and_keeps_record_available
persisted_recovery_batch_close_rejects_late_claim_and_old_session_entry
persisted_recovery_batch_discard_confirms_once_and_retains_busy_record
persisted_recovery_batch_discard_retains_record_changed_after_confirmation
persisted_recovery_batch_discard_unknown_worker_result_reloads_actual_store
persisted_recovery_batch_enters_one_independent_review_and_cancel_clears_queue
persisted_recovery_batch_record_disappearance_clears_waiting_queue
persisted_recovery_batch_selection_counts_and_preserves_record_order
persisted_recovery_batch_worker_spawn_failure_preserves_the_last_session
persisted_recovery_batch_worker_spawn_failure_restores_order_and_retries_explicitly
```

app 测试链接出现既有 `__eh_frame section too large` linker warning；清单、测试和单独执行的 strict Clippy 均保持上述真实 exit 0。

**重新编译归属与结束完整性**

`app-inventory.log` 实际记录从固定评审 checkout 编译 app/core/AI/MCP/session 五包，并从唯一 target 执行 app 测试二进制。`inventory-validation.json` 记录 1186 个唯一名字、29 个专项和 10 个批量回归的逐名核对。没有把作者预编译二进制作为本次独立重新编译。

本次测试二进制为：

```text
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/review/target/debug/build/keelshell-app/ae49e21ee6a97bff/out/keelshell_app-ae49e21ee6a97bff
SHA-256: d920a77fb6b68d741214ca2e0d89c499b36829dda230ef44695bc2d8342dc39d
bytes: 258978464
```

[compiled-provenance-r2.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/fix/review/compiled-provenance-r2.json) 核对 app 测试 dep-info 中 280 个规范化 Rust 输入路径，全部与开始快照相符，包含三个冻结文件，没有其它 checkout 的绝对 workspace Rust 源码路径。

归属辅助核验首次直接用 dep-info 原始相对路径查表，未规范化两个 `../../keelshell-session/tests/fixtures/…` 路径，因此比较键未命中并触发 `AssertionError`。这次评审辅助脚本错误的 `compiled-provenance.json` 已保留，未覆盖；r2 仅规范化这些路径后核对相同字节，辅助命令 exit 0，回执为 `compiled-provenance-r2-check.{log,json}`。它不是源码失败或额外功能测试，不改变上表五项门禁的独立结果。

[source-start.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/fix/review/source-start.json) 的时间为 `2026-11-15T01:46:17.834608+00:00`；[source-end.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/fix/review/source-end.json) 的时间为 `2026-11-15T01:48:43.494772+00:00`。结束核验显示：

- 冻结清单摘要相同，524 个 Rust/Cargo 输入全部匹配，三个候选文件摘要相同。
- 1139 个受跟踪文件逐字节相同，`changed_tracked_bytes=[]`。
- HEAD、索引字节和完整 porcelain 状态不变：`head_unchanged=true`、`index_unchanged=true`、`status_unchanged=true`。
- porcelain 始终只有父线程准备的三个 ` M` 文件；它们仍为未提交候选，没有把该脏状态误报为仓库干净。

**作者失败与成功证据的保留**

以下是直接读回的作者原件，路径均在主开发树 `work/recovery-batch-integration-20261115/fix/`，不计入本人独立测试。`author-evidence-readback.json` 保存原路径与摘要；六份日志的实际 SHA-256 均与作者 JSON 回执一致。

| 原件前缀 | 原始事实 |
| --- | --- |
| `worker-start-before-fix` | exit 101；**0 passed / 2 failed**，两者均在“恢复原队首”断言失败，证明原 P2 |
| `worker-start-after-fix` | exit 101；**0 passed / 2 failed**，测试编写中的 panel status 文案断言失败 |
| `worker-start-after-fix-r2` | exit 101；测试编译失败，`E0616`，访问私有 `QueuedTransfer.result`；没有实际执行测试 |
| `worker-start-after-fix-r3` | exit 101；测试编译失败，`E0308` 类型不匹配；没有实际执行测试 |
| `worker-start-after-fix-r4` | exit 0；**2 passed / 0 failed / 0 ignored** |
| `app-parallel-after-fix` | exit 0；**29 passed / 0 failed / 0 ignored / 1157 filtered** |

保留中间失败的真实退出事实；没有将它们写成通过。作者 full gate 在另一 target 并行运行，本次未读取或操作其 target，也未将尚未由父线程确认的结果纳入 APPROVE 的依据。

报告交付时，父线程补充作者整仓 gate 的 app 已实际为 **1184 passed / 0 failed / 2 ignored**，余下传输/controller 阶段仍在运行。此项为父线程提供的进度，本人未独立重跑或读回其完整 gate；按父线程指示直接交付限定 delta 结论，不等待作者 gate 或正式 commit。

**验收限制与交付结论**

本次验证是 headless GPUI 控件/状态、隔离持久存储和受控 SSH/SFTP 测试；worker 启动错误来自面板局部 test-only 拒绝。没有真的耗尽 OS 线程或触发原生线程资源创建失败，也没有验收系统 registry 资源耗尽行为。真实 OS 线程资源耗尽、原生窗口人工交互、macOS/Windows/Linux 平台验收、真实服务/客户机器、容器、发行安装与整产品验收均不在本次范围。

本次没有扩测、新增功能探针或运行完整产品门禁。源码检查与独立 29 项通过足以关闭本次指定 P2，未发现新的 P0/P1/P2/P3。**独立修复 review：APPROVE；正式提交及其 SHA/冻结字节绑定由父线程在作者 gate 同时通过后完成。** 无后续评审命令或资源清理等待执行。
