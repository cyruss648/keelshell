# KeelShell 持久化传输恢复批量整理：独立只读评审

评审日期：2026-11-15，Asia/Shanghai。评审者为本轮非作者 agent，未创建子 agent 或新的用户聊天。

**建议：REQUEST_CHANGES。发现计数：P0 0 / P1 0 / P2 1 / P3 0。**

独立重新编译后的项目专项为 **38 passed / 0 failed / 0 ignored**；app 清单为 **1184** 项，全部 8 个新增 `persisted_recovery_batch_*` 用例均在清单中且实际通过。app/core 严格 Clippy、fmt 检查和 first-parent diff 检查通过。发现的问题位于 worker 启动失败后的队列恢复顺序，现有通过用例没有覆盖这个错误分支。另做的源码抽取状态诊断为 2 passed / 1 failed，失败原件保留；它不是完整 app 测试或原生验收。

## 固定输入与只读边界

| 项目 | 核对值 |
| --- | --- |
| 源码 checkout | `/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell` |
| exact HEAD | `5cdbc9617164d6cfbd5e7a9a022539934221ee75` |
| first parent / 比较基线 | `858e7f158fec80a7e784f3d66159bb12733bc6cd` |
| 实现父提交 | `c2040bbe03ea6f545ee4a9688d1994bd18cbcc7a` |
| 唯一输出目录 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/review` |
| 唯一 Cargo target | 上述输出目录的 `target` |
| 编译器 | `rustc 1.100.0 (0ef1bc386 2026-11-09)` |
| Cargo | `cargo 1.100.0 (3acc79c0c 2026-11-05)` |
| Clippy | `clippy 0.1.100 (0ef1bc3860 2026-11-09)` |

HEAD 与两个父提交均通过 Git 直接读回。first-parent diff 涉及 15 个文件，1869 行增加、34 行删除；生产代码改动集中在 app 的 files 状态、队列、审核入口及 worker 分派，core/session 的持久化和传输接口未改变。

本报告的发现范围严格限定为 `858e7f158fec80a7e784f3d66159bb12733bc6cd..5cdbc9617164d6cfbd5e7a9a022539934221ee75` 的增量。读取基线 core/session 和既有审核逻辑只用于理解本批调用关系；不对基线或全产品追加风险发现。文末列出的其它平台和原生边界属于未验范围，不计为本批缺陷。

已阅读 AGENTS.md、`/Users/hdh/.codex/RTK.md`、HANDOFF 顶部、SHARED_TARGET、ADR 0131、产品 `PERSISTED_TRANSFER_RECOVERY_BATCH_REVIEW.md`、本批集中验证与作者专项记录，并应用 `/Users/hdh/.agents/skills/rust-best-practices/SKILL.md` 及其相关章节。评审关注所有权、后台阻塞 I/O、错误路径、快照一致性、状态失效和有意义的行为断言。

没有修改源码或仓库文档，没有运行会写回的格式化，没有提交、push、merge、清理 target 或删除共享资源。Cargo 的工作目录始终为固定评审 checkout，没有在主开发树运行 Cargo。所有新增编译产物、诊断源码、临时目录、日志和本报告只位于指定独占输出目录。

父线程提供的 `preparation.json` 记录五个 workspace 包已在评审 checkout 内失效，退出 0；`target-invalidation.log` 保留该事实。本评审没有再次执行 clean，也没有借用其它 target。

## 唯一待修项

**[P2] worker 启动失败时，已清空的 active 标记使当前恢复记录无法回到待审核队首。**

主要位置：

- [parallel.rs:477](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel.rs:477)：在调用 `spawn_owned_transport_worker` 之前，将 `recovery_review_active` 清为 `None`；当前条目是队列最后一条时，还将 `recovery_review_session` 清为 `None`。
- [parallel.rs:602](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel.rs:602)：worker 启动返回错误后，调用 `requeue_failed_recovery_review_item(recovery_id)`。
- [parallel.rs:790](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel.rs:790)：该 helper 只有在 `recovery_review_active == Some(id)` 时才会把 ID 放回队首，因此上述错误路径无法满足条件。

触发步骤：

1. 将恢复记录 A、B 建立为有序待审核队列。
2. 对 A 取得 claim，完成独立 Inspect、Review，并明确 Confirm。claim 回调已在 [parallel.rs:1035](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/files/parallel.rs:1035) 将 A 从队首取出，设为 active。
3. `run_queued_transfer` 在启动 worker 前执行 477 行的状态转换，此时待审核队列为 `[B]`，active 为 `None`。
4. `spawn_owned_transport_worker` 返回 `Err`，例如系统拒绝创建线程；[terminal_workers.rs:70](/Users/hdh/.codex/worktrees/transfer-recovery-final-review-v3-20261115/keelshell/crates/keelshell-app/src/terminal_workers.rs:70) 的线程创建错误会向上传递。
5. 错误处理尝试回队，但 helper 的 active 检查失败；实际队列仍为 `[B]`，而不是 `[A, B]`。

影响：尚未成功交给 worker 的 A 从待审核队列中漏掉，下一次“逐条进入审核”直接进入 B；只有一条记录时，审核队列会直接消失。恢复日志仍保留 A，界面也记录 failed job，因此这不是持久化记录丢失或未经确认执行的问题，但失败后的回队恢复行为失效，用户必须额外找到 A 并重新组织审核。

队列容量不足分支在 active 清空前调用同一个 helper，因此可以恢复队首；它与 worker 启动失败分支的行为不一致。

该项属于本批增量：477 行的提前清空状态块、602 行的错误回队调用和 790 行的 helper 均由本次相对 first parent 的改动新增；结论不依赖把基线原有风险列为缺陷。

建议将当前项的消费状态转换放到 worker 成功创建之后，或保留足够的队列来源信息，在启动失败时先恢复队首再清除 active/session。恢复的只是 ID 和待审核顺序；下次进入仍须重新 claim、检查和确认。应补一个受控 worker 启动错误回归，断言 A/B 顺序、最后一条队列的 session、claim 释放及不产生实际传输。

本项依据是固定源码中的确定性状态顺序。为核实该顺序，从原文件逐字抽取 477–482 行的状态转换和 790–799 行的 helper，在独占输出目录生成最小状态诊断：

- `capacity_failure_before_transition_restores_original_order`：通过，队列恢复为 `[1, 2]`。
- `accepted_start_preserves_waiting_second_record`：通过，剩余队列为 `[2]`。
- `failed_worker_start_should_restore_original_head`：失败，实际 `[2]`，期望 `[1, 2]`。

诊断只保留相关状态字段，UUID 以 `u128` 的等值语义代替；未运行 GPUI、store I/O 或实际 OS worker 创建，也未故意耗尽系统线程。不得将它描述为完整 app 故障注入或原生失败复现。原始诊断源码、抽取位置和源文件 SHA-256 见 `worker_start_state_probe.rs` 与 `worker-start-probe-provenance.json`，编译/运行回执见 `worker-start-probe-compile.*`、`worker-start-probe-result.*`。

未发现其它达到 P0、P1、P2 或 P3 报告标准的问题。

## 功能复核证据

| 风险 | 代码与行为复核结果 | 证据范围 |
| --- | --- | --- |
| 选择顺序、数量、有界集合 | 从当前 `recovery_records` 顺序过滤选中 ID，不使用点击顺序或 BTreeSet 顺序；数量不一致时撤销；Selection 限制为 32 | `parallel.rs:809`、`parallel.rs:865`、`selection.rs:61`；新增 selection GPUI 用例通过 |
| queue/session/revision 失效 | 队列绑定 session；revoke 清空选择、队列和 active，并推进 revision；claim callback 同时校验 session/revision/队首；计划回调也核对 revision | `parallel.rs:758`、`parallel.rs:995`；`files.rs:1683`；关闭、旧入口、记录消失用例通过 |
| 每条 claim 和 store 快照 | 每次仅 claim 队首，后台重新 load 并比较完整 `TransferRecoveryRecord`；未通过时不进入计划 | `parallel.rs:987` 至 `parallel.rs:1056`；Busy 用例通过；core claim 用例通过 |
| 当前连接、主机、Inspect/Review/Confirm | 保存连接不匹配在计划前拒绝；无绑定记录先产生主机确认；计划完成产生新的 pending review；确认按钮绑定当前 review identity；最终传输还校验 ID/claim/路径/连接 | `parallel.rs:976`、`parallel.rs:1046`、`parallel.rs:303`；`review.rs:89`；无绑定主机与丢失 claim 的回归通过 |
| 取消、导航、关闭与迟到回调 | 撤销只处理待审核权限；正在检查时由 worker 持有的 claim 随结果放弃释放，旧计划不能重建审核；已确认传输由原 worker 所有权继续记录真实终态 | `parallel.rs:766`、`local_browser.rs:96`、`files.rs:1228`、`files.rs:1683`；关闭期间迟到 claim、取消、挂起相关回归通过 |
| 第一条成功后的剩余队列 | 入队只移除本条 active；有剩余时保留 session；终态处理重新 load，未调用下一条检查；下一条只由显式入口处理 | `parallel.rs:477`、`parallel.rs:680`、`parallel.rs:899`；静态路径检查和抽取状态诊断，未新增完整“两条持久恢复均成功”的 GPUI 端到端测试 |
| worker / queue 启动失败 | queue 初始化失败保留失败记录的既有行为用例通过；worker 创建失败回队有上述 P2 | `parallel.rs:497`、`parallel.rs:531`、`parallel.rs:590`；27 项中包含 queue 初始化失败回归；OS 线程创建错误未动态注入 |
| 批量 discard 的第二次确认 | 第一次只保存完整 candidates 和审核文案；只有当前 review 的 Confirm 消费该 operation，再进入后台逐条处理 | `parallel.rs:1123`、`files.rs:2067`、`review.rs:95`；确认前 store 不变的用例通过 |
| 完整快照变化拒绝删除 | claim 内 load 后用派生的完整相等比较核对 candidate，再 `remove_with_claim`；记录 ID 相同仍不能绕过其它字段变化 | `parallel.rs:1215`；`keelshell-core/src/transfer_recovery.rs:64`；真实 store 同 ID 替换回归通过 |
| Busy 中间项和部分失败 | 每条 claim 在当前 map 迭代结束时释放，错误独立记录，后项仍可处理；失败项重新选择 | `parallel.rs:1216`、`parallel.rs:1292`；三条中间 Busy、其余两条删除的回归通过 |
| 整体未知结果 | 所有结果都推进 mutation generation 并 load 实际 store；未知结果标为未确认删除，重新保留实际仍存在的选择，不自动重试 | `parallel.rs:1257`；先删除一条再注入整体未知结果的 GPUI 完成路径回归通过；不等同于真实 worker 崩溃注入 |
| 阻塞 I/O 与层次 | 新增 claim/load/remove 由 `spawn_blocking` 执行；UI 状态留在 app，既有 core claim/原子写与 session 审核计划保持分工 | `parallel.rs:997`、`parallel.rs:1214`；无新增依赖、跨 crate API 或业务主题分支 |
| 主题、双语、紧凑布局与 AX | 使用 Palette 语义颜色及组件主题；新增按钮和复选框有双语 accessibility label，数量有 Label/aria；操作条换行、详情滚动，复用固定审核操作区 | `parallel.rs:1489`、`parallel.rs:1667`、`review.rs:95`；560×440、Frost/Graphite/Lavender、中文/英文的现有 GPUI 断言通过；未执行原生读屏验收 |

本表中的“静态路径检查”与“实际用例通过”分别表述；不能将没有动态覆盖的组合路径推导为原生功能验收。

## 独立实际执行的命令

下列命令均由本评审在固定 checkout 执行。公共环境覆盖：

```text
CARGO_TARGET_DIR=/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/review/target
CARGO_BUILD_JOBS=4
CARGO_NET_OFFLINE=true
TMPDIR=/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/review/tmp/
GIT_OPTIONAL_LOCKS=0
PYTHONDONTWRITEBYTECODE=1
```

每条命令通过 `run_review_command.py` 保存完整 stdout/stderr 和 JSON 回执，回执包含 argv、cwd、环境覆盖、UTC 起止时间、耗时与退出码。没有同时运行两个使用本 target 的 Cargo 命令。

| 命令 | 实际结果 | 耗时 | 日志/回执前缀 |
| --- | --- | --- | --- |
| `cargo test -p keelshell-app --locked --offline -- --list` | exit 0；1184 个不同用例，27 个 parallel_tests，8 个新增批量用例；这是清单，不计作执行 1184 项 | 36.670 s | `app-inventory` |
| `cargo test -p keelshell-app --locked --offline -- parallel_tests --test-threads=1` | exit 0；27 passed / 0 failed / 0 ignored / 1157 filtered | 12.060 s | `app-parallel-tests` |
| `cargo test -p keelshell-core --lib --locked --offline -- transfer_recovery --test-threads=1` | exit 0；8 passed / 0 failed / 0 ignored / 214 filtered | 6.846 s | `core-recovery-unit` |
| `cargo test -p keelshell-core --test transfer_recovery --locked --offline` | exit 0；3 passed / 0 failed / 0 ignored / 0 filtered | 3.032 s | `core-recovery-integration` |
| `cargo clippy -p keelshell-app -p keelshell-core --all-targets --locked --offline -- -D warnings` | exit 0 | 13.932 s | `app-core-clippy` |
| `cargo fmt --all -- --check` | exit 0，未写回格式 | 1.765 s | `fmt-check` |
| `git diff --check 858e7f158fec80a7e784f3d66159bb12733bc6cd 5cdbc9617164d6cfbd5e7a9a022539934221ee75` | exit 0 | 0.013 s | `first-parent-diff-check` |

以上 **7 条工程验证命令**，实际执行不同项目用例合计 **38 passed / 0 failed / 0 ignored**。清单、重复涉及的用例名、过滤项和作者运行均未重复累计。未执行本轮 workspace rustdoc 或全仓 gate。

另外 **2 条诊断命令**：

```text
rustc --edition=2024 --test /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/review/worker_start_state_probe.rs -o /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/review/worker-start-state-probe
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/review/worker-start-state-probe --test-threads=1
```

诊断编译 exit 0，0.123 s；运行 exit **101**，0.399 s，**2 passed / 1 failed / 0 ignored**。该失败用于揭示上述 P2，未覆盖或改写失败原件，未合并到项目 38 项通过数。

app 独立测试链接实际出现已知 `__eh_frame section too large` linker warning；测试构建、清单和执行均退出 0。严格 Clippy 另行执行，退出 0。

### 全部八个新增用例

以下名称均带实际完整前缀 `files::transfer_tests::parallel_tests::`，全部在 `app-inventory.log` 中且在 `app-parallel-tests.log` 逐项为 `ok`：

1. `persisted_recovery_batch_selection_counts_and_preserves_record_order`
2. `persisted_recovery_batch_enters_one_independent_review_and_cancel_clears_queue`
3. `persisted_recovery_batch_close_rejects_late_claim_and_old_session_entry`
4. `persisted_recovery_batch_busy_claim_fails_closed_and_keeps_record_available`
5. `persisted_recovery_batch_record_disappearance_clears_waiting_queue`
6. `persisted_recovery_batch_discard_confirms_once_and_retains_busy_record`
7. `persisted_recovery_batch_discard_retains_record_changed_after_confirmation`
8. `persisted_recovery_batch_discard_unknown_worker_result_reloads_actual_store`

core 的 8 项覆盖 claim 排他性、claim 与 journal/ID 不匹配、malformed/schema 拒绝、进度与绑定校验、路径约束、无运行权限的序列化、upsert_and_claim 原子可见性以及多 store 对其它记录的保留。3 项集成测试覆盖重启可见而无续传权限、连接绑定与旧格式 round trip、非法 schema 拒绝。

## 编译归属与源码完整性

`app-inventory.log` 实际列出从固定评审路径编译五个 workspace 包，未把已有作者二进制的执行当作独立编译。当前 Cargo 产物的 dep-info 位于 `target/debug/build/<package>/<hash>/out/`，源码条目采用相对 workspace 路径；这些条目按实际命令 cwd 解析后均存在于固定 checkout。

最终 `compiled-provenance.json` 核对到 app/core/AI/MCP/session 分别 280/50/20/7/38 个不同本包 Rust 输入路径，未发现指向其它 checkout 的绝对 workspace 源码输入。初步扫描曾按传统 `debug/deps` 目录和绝对源码路径查找，未命中；没有据此接受归属，最终核对以实际目录、相对输入、编译路径和用例清单为准。

app 测试二进制：

```text
.../review/target/debug/build/keelshell-app/ae49e21ee6a97bff/out/keelshell_app-ae49e21ee6a97bff
SHA-256: cf917f8e61b3ffe89d153c9e686f76a828fddb44bff549e6655c33409fd7e3b2
字节数: 258869120
```

开始时对固定 checkout 的 1139 个受跟踪文件逐字节取 SHA-256。结束核对回执 `source-after.json`（2026-11-15T01:28:31.066372+00:00）显示：

- HEAD 仍为 `5cdbc9617164d6cfbd5e7a9a022539934221ee75`。
- 1139 个文件没有字节变化，`changed_tracked_bytes=[]`。
- `git status --porcelain=v1 --untracked-files=all` 输出为空。
- `git diff --exit-code HEAD` 退出 0。

**源码工作树仍干净。** 报告、诊断和日志只写入父线程指定的输出目录；本评审没有清理 target、worktree 或父线程资源。

## 作者证据与未验证边界

集中记录所述作者 gate：普通 Rust 2531 passed / 0 failed / 23 ignored，另有 rustdoc 14 passed；合计 2545，通过 Python 131 run / 1 skipped、fmt、workspace 严格 Clippy、依赖策略及三组 AI controller。这些是已提供的作者证据，本评审没有重复执行，不能加到本评审 38 项的计数中。

本机 OpenSSH 11/11、隔离 macOS 可见窗口启动短验同属集中作者证据。本评审没有重复启动原生应用、OpenSSH runner、容器或服务，没有安装或替换现有应用。

以下仍未验证或不能扩大结论：

- 真实 OS 线程创建失败的完整 GPUI 故障注入；上述 P2 由源码顺序与抽取状态诊断确认。
- 两条持久恢复均成功、等待第一条终态后再进入第二条的完整 GPUI 组合路径；当前只作静态路径检查，现有专项分别验证独立审核取消、普通传输成功、并行终态及状态诊断。
- 本批恢复控件的原生交互、原生三主题切换、VoiceOver/完整 AX 行为、键盘/IME 矩阵、Windows/Linux 原生运行。
- Podman 协议联调：集中记录中的 Docker Hub pull 超时保持未验；本评审没有重试。
- 官方依赖新鲜度：集中记录中的无响应/HTTPS 403 保持未验；本轮离线验证不证明最新版本。
- 真实客户 SSH/SFTP、外部 AI provider、签名、公证、发行、安装或升级。

父线程最后明确要求在现有范围收口，不扩测或重复整仓 gate。本评审遵从该指示，仅交付此原始报告和已有证据。最终文档归档、review target/worktree 及运行资源清理由父线程负责。

本轮评审现已收口，无等待中的额外测试、探针、构建或产品探索。此前完成的额外状态诊断保留原始结果；未完成的完整 GPUI 组合路径与 OS 故障注入已如实列为局限，不阻塞本报告交付。本次收口仅核对已有报告、Git 增量归属及工作树状态。

**最终建议维持 REQUEST_CHANGES：先修复这 1 个 P2，再对受影响的 worker 启动失败与队列顺序路径做针对性回归及独立确认。**
