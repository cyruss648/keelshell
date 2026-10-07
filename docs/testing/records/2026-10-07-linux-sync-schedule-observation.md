# Linux 定时参数与连接同步组合失败观察 — 2026-10-07

精确主线 `9865f1f121170c9eed81c19f4ca50f522c001f45` 的 Quality 37525054745 在 Linux 只有一个应用测试失败：`scheduled_parameter_authority_survives_real_sync_approval_without_redirect_or_persistence`。原始完整 job 正文 315,191 字节已稳定全文读回，SHA-256 为 `db1b3c1337a7ed4f58964d1ddab061a41f13b6ef49459a2acfd69923f67e1551`；应用结果为 576 passed、1 failed、2 ignored。

失败是 `scheduled.rs` 原有 `wait_real` 的 18 秒墙钟期限。该调用在明确批准后等待工作区 `state.connections` 出现所选同步资料；此前解密差异的另一个 18 秒循环没有失败，后续 40 秒定时完成等待未到达。30 秒首次定时触发提前量、两处 18 秒期限、40 秒完成期限、原始数据和全部断言保留。

旧日志没有同步面板终态，无法区分未准入、后台等待、服务返回错误、前台投递或其它状态变化。`0f1a577` 诊断分支后续三平台成功不证明本次失败的原因。没有据此调整等待期限、串行化测试、减小 Argon 参数、改变审批／保存／加密规则或断言。

v1 候选只修改两个现有测试文件。原 `wait_real` 入口委托到相同循环的 `wait_real_observed`；失败信息的回调只在原断言失败时求值，不是完成信号，不续期或执行操作。该组合测试的 inspect 和批准后等待分别标记阶段，追加现有面板 summary、工作区是否含所选资料、原认证会话绑定、定时运行／审核／slot 状态和两个自有 SSH peer 的请求数量。只读取内存，不检查存储、输出凭据、地址、参数值、命令或响应内容。

当前基线的面板 summary 不提供 worker 开始、服务返回和前台完成阶段；`busy` 不能据此证明原因。以后整合独立的 test-only 面板阶段观察时，同一 summary 可带有更多阶段事实，需要依照相应新证据解释，不能回溯补足原日志。

初稿时限定原测试、Clippy 和非作者复核尚待执行；后续完成情况见下文，精确提交级 Linux CI 另行回读。本记录不把本机运行、增加观察或后续 CI 成功称为原始原因已解决。

## 本机候选验证（2026-10-07）

本地受控运行使用单独的 `TMPDIR`、`CARGO_BUILD_JOBS=2` 和 900 秒外层上限；没有复用或终止其它工作树进程。精确测试命令按完整测试名称筛选，实际结果为 1 passed、0 failed、0 ignored，测试本身耗时 31.21 秒；runner 总耗时 176.03 秒（包含首次编译）。原 18 秒批准后工作区等待和原测试其它 30/40 秒边界均由测试自身执行，生产加密与断言未改。输入前后均为 721 个文件且字节映射相同，测试进程组已结束，临时目录已移除。完整收据在 `work/linux-sync-schedule-observation-20261007/limited-case-v1/RECEIPT.json`。

工作区 `cargo fmt --all --check` 实际退出 0；严格 `cargo clippy --workspace --all-targets --locked -- --no-deps -D warnings` 实际退出 0，耗时 56.84 秒，唯一输出是已有的 future-incompatibility 提示。两份收据分别为 `fmt-check-v1/RECEIPT.json` 和 `clippy-v1/RECEIPT.json`。这些是 macOS 本地候选证据，不替代 Linux CI；也不能从本地通过推断原 Linux 失败原因已经解决。

按项目标准命令再次运行 `cargo clippy --workspace --all-targets --locked -- -D warnings`，实际退出 0，耗时 46.29 秒；输入前后 721 个文件映射相同，进程组结束且临时目录移除。收据为 `work/linux-sync-schedule-observation-20261007/clippy-full-v1/RECEIPT.json`。此前带 `--no-deps` 的同样严格检查也实际退出 0；这里以完整项目命令作为候选门禁证据。

## P2 有限摘要修复（2026-10-07）

独立复核发现初版失败观察中的两个潜在无界复制：`batch_peer::Server::requests().len()` 会先复制完整命令正文向量，`WorkflowPanel::reviewed_for_test()` 会复制完整审核计划。初版材料与该 P2 发现保留在 `work/linux-sync-schedule-observation-20261007/candidate-v1/`，没有覆盖。

v2 仅增加 test-only 标量访问：SSH fixture server 的 `request_count()` 只读取锁内长度；工作流审核新增由审核 UUID、revision 和计划 fingerprint 组成的 `review_signature_for_test()`，不复制命令、目标或参数正文。失败观察改用这两个标量接口；生产执行、18/30/40 秒等待、原有断言和批准路径均未改变。观察器仍只在原断言失败时执行。v2 的四个源码文件为：`crates/keelshell-app/src/workflow_commands.rs`（`cfg(test)` 审核签名）、`crates/keelshell-app/src/workspace/tests/batch_peer.rs`（fixture 请求计数）、`crates/keelshell-app/src/workspace/tests/dependency_workflow/scheduled.rs`（失败专用等待器）和 `crates/keelshell-app/src/workspace/tests/dependency_workflow/scheduled/target_parameters/sync_combination.rs`（组合观察器）。

v2 本地验证实际通过：精确 scheduled case 为 1 passed、0 failed、0 ignored（测试耗时 31.45 秒）；`cargo fmt --all --check` 实际退出 0；`cargo clippy --workspace --all-targets --locked -- -D warnings` 实际退出 0（23.52 秒）。输入映射均为 721 项且前后相同，进程组结束，临时目录移除。对应收据为 `limited-case-v2/RECEIPT.json`、`fmt-check-v2/RECEIPT.json` 和 `clippy-full-v2/RECEIPT.json`。本机通过仍不等同于 Linux CI 原失败已解决。

## 主线整合与 CI

v2 已经非作者复核为 `NO_BLOCKER`，复核结果 SHA-256 `00e621ce836b7fec015f2b8d1a5241b18ebae1043a3cae413e2c35bc79c3cb61`，并整合到主线 `0d64f09`。主副本应用测试输出为 583 passed、0 failed、2 ignored，两项同步组合均通过；该次根调用未持久保存最终退出码收据，不能当作完整正式门禁收据。

父提交 `4c9b14c` 的 Quality 37585913129 已结束：macOS／Windows 成功，Linux 应用测试 575 passed、2 failed、2 ignored。两个失败均在同步批准后的工作区等待，已有诊断报告 busy 状态；仍不足以判定根因。完整 Linux job 日志 470,958 字节，SHA-256 `f39061431b473d644b47c11d33417fe70d8d294b976d40a59501229d316fdda2`，保留在 ignored work 中。新提交 CI 终态另行回读。

2026-10-07 后续平台门禁：0d64 Linux 因 AI 目录控制器 readiness 失败，未进入 app 同步测试；macOS job成功。根平台修复的新正式本机733输入质量门禁退出0，app583（2ignored）通过；该运行不是Linux根因证据。新提交CI需再检查，不能把本机正常结果或观察器本身当作生产修复，见[平台记录](2026-10-07-local-agent-directory-platform-fix.md)。
