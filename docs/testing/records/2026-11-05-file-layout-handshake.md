# 文件布局测试的真实 WRITE 与暂停交接前提

日期：2026-11-05，Asia/Shanghai。
基线：`dcf7438a13fff737997ee5e8061044c94d4ba53a`，
分支：`feature/input-and-layout-readiness-20261105`。

## 结论与当前验证层级

测试源码修订和首次四组专项已通过。2026-11-05 13:26:31–13:27:58 UTC，
父执行会话54030退出0并回收；517项工程输入前后一致。
布局3项、暂停交接辅助1项、WRITE gate 3项及终端输入2项分别通过，
入口均保持四测试线程。新非作者限定审查批准源码与专项证据，开放P0/P1/P2/P3为0；
严格Clippy和完整工程门禁已在最终520输入通过，详见下方组合结果；
本轮专项不扩大既有平台或原生验收。

最终组合的`full-gate-r1`停在三项test-only Clippy，exit1/reaped；
两处断言写法和一处同类型转换经最小清理后，`full-gate-r2`于
14:01:24–14:12:28 UTC实际通过，exec88486已wait/reap、exit0。
2,221ordinary/23ignored、app963/2ignored、14rustdoc、Python131run/1skip，
格式、严格Clippy、依赖策略与附加控制器通过；520工程输入前后一致。
旧失败原件保留，测试预算、四线程入口及生产传输语义保持。

本次修订针对两个源码可证的测试前提缺口：

1. 同一个未确认 WRITE 下串行渲染四套语言/明暗组合，共享五秒 transfer idle
   预算；十秒 WRITE gate 未 expired 不足以证明五秒 idle 尚未耗尽。
2. UI 显示 Pausing 只证明写入了 UI→worker 的 watch；
   worker→session 的 `TransferHandle::pause()` 尚无先于 WRITE release 的保证。

旧 Linux Quality `42009970970 / attempt 1` 的确切根因仍为 **UNKNOWN**。
旧日志没有失败场景的阶段耗时、实际终态和第二次交接时刻；
本修订不补写这些缺失事实，也不把上述缺口冒称为旧失败根因。

## 依据与写集

已阅读当前 `docs/HANDOFF.md` 和原组合工作树的：

- `work/workspace-integration-20261105/file-layout-ci-diagnosis/DIAGNOSIS.md`
- 同目录 `validation.json`
- 同目录 `execution-r1/EXECUTION.md` 及 `test-readback.json`

这些证据保存在组合切片的受管工作副本中，属于本地未发布材料；
从该切片的交付回执定位，不依赖开发机目录。
按用户范围采用 Rust testing 指引中的隔离夹具、有界等待、明确失败诊断和
有意义的正负控；没有以 sleep、重试、串行化测试入口或降低断言掩盖竞争。

作者写集严格限定为：

| 文件 | 变更 |
| --- | --- |
| `crates/keelshell-app/src/files/workspace_layout_tests.rs` | 外移外观维度、持有 WRITE 的测量/释放前提、真实暂停交接等待、回归与负控、逐生命周期清理 |
| `crates/keelshell-app/src/files/test_server.rs` | 加载测试专用暂停交接辅助模块 |
| `crates/keelshell-app/src/files/pause_handoff_fixture.rs` | 按原 watch channel 隔离的转交闸门、转交后观察和 guard 清理测试 |
| `crates/keelshell-app/src/files/worker.rs` | 仅在两处真实 `transfer.pause()` 前后添加 `#[cfg(test)]` hook；不改变非测试分支的判断、操作或 select 顺序 |
| `crates/keelshell-session/tests/fixtures/sftp.rs` | 第一条匹配 hold 的单调时间戳与已有夹具回归中的代际检查 |
| 本记录 | 设计、实际验证层级与父任务交接要求 |

`workspace/pane_view/view.rs`、HANDOFF、ROADMAP、README、依赖和
Cargo.lock 均不在作者写集内。同树父任务的 terminal 输入测试和文档改动由父任务拥有。

## WRITE 前提与布局覆盖

主回归仍为
`real_file_rows_and_controls_survive_transfer_comparison_editor_and_review_states`。
原来的 2 尺寸 × 2 assistant 生命周期内，`measure_state` 会遍历 2 语言 × 2 明暗；
现在由 `LayoutScene::matrix` 在最外层枚举全部 16 个外观，每个外观重新建立
真实 SSH/SFTP 夹具、审核上传、hold、暂停、恢复和清理。
`measure_state` 每次只渲染传入的一套外观。

11 个状态及真实行、区域预算、可见性、滚动可达性、审核、编辑草稿和字节断言保持，
设计覆盖仍为 **16 × 11 = 176 个唯一组合**。父任务已从首次运行的
`FILES_LAYOUT_JSON` 重算：总178行包含负控的两行；按状态、尺寸、
assistant、语言和明暗去重为176组，11个状态各16组。
16条真实传输回执分别对应16个独立外观生命周期。

`RunningLayoutHandshake` 持有真实 `TransferWriteHold`：

- 测量必须发生在 `entered == 1` 后，且 gate 未 expired。
- `entered_at` 在 handler 内、mutation/ACK 前首次写入，区别于 UI 观察时刻。
- 每个 hold 只接受一次 running 测量；第二次即返回
  `RepeatedRunningMeasurement`，不执行后三次渲染。
- 测量后和 release 前都检查从实际 handler entry 起尚未到五秒；
  不再仅凭十秒 fallback 的 `!expired()` 放行。单套渲染若仍过慢，明确失败并由 guard 清理。

handler entry 不是最后一次已确认 I/O 的精确时刻，因此上述墙钟检查本身不能替代
真实协议结果。随后仍必须收到真正的 Paused、核实 staged 精确源前缀及 UI 字节数，
保持 staged bytes 不再变化，恢复后完整字节相等，再检查 subsystem/目录 handle 为零。
每个场景关闭真实测试窗口并核对 fixture admission 的 strong count 为零。

## 第二次交接与可证先后关系

`PauseHandoff` 用 `watch::Receiver::same_channel` 匹配所选 job 的原始 pause channel。
注册表只保留 Weak，不持有 FilesPanel、运行时或 fixture group。
正常矩阵的 observer 不阻挡 worker；只有负控会关闭其测试闸门。

两处 hook 分别覆盖 worker 初始读取 pause 和后续 `pause.changed()` 分支。
转交后的时间戳仅在真实 `TransferHandle::pause()` 返回之后写入；
测试读取 OnceLock 的已发布值后，才允许 consuming WRITE release。
完整关系为：

`真实 WRITE entered → 单套 running 测量 → UI Pausing → worker 调用 session pause
→ 观察到 forwarded → release WRITE → 真实 WRITE ACK/checkpoint → UI Paused`。

`forwarded` 不是 Paused ACK。release 后保留原 `state.phase == Paused` 谓词和
十二秒 GPUI test-clock 等待；没有接受 Completed、Uncertain 等替代终态，也没有直接改 phase。
暂停转交的等待只使用从同一 WRITE entry 起剩余的五秒预算，release 再做实际墙钟检查；
不会给每次等待重置一个新的五秒 idle 窗口。

| 原预算/并发 | 修订后 |
| --- | --- |
| fixture `SshOptions.timeout` / transfer idle 5 秒 | 保持 |
| handler WRITE fallback 10 秒 | 保持 |
| real WRITE admission 8 秒 | 保持 |
| Paused / idle 等待 12 秒 GPUI test clock | 保持 |
| 资源清理等待 5 秒 | 保持 |
| fixture Tokio worker threads=2 | 保持 |
| 后续测试入口 `--test-threads=4` | 必须保持 |

test-clock 与单调墙钟分别记录，不把 GitHub 日志回放时间或 GPUI 的十二秒当作阶段墙钟。
测试专用负控闸门最多等待五秒，超时会标记 expired，不能算作同步成功。
Drop 显式放开自有闸门；原 WRITE guard drop / cancel 回归保留。
所有新增 worker hook 在非测试构建中均由 `#[cfg(test)]` 移除。

## 确定性负控与诊断

新增
`running_layout_rejects_repeated_appearance_and_release_before_session_pause`：

1. 审核真实上传，等首块已确认、下一条真实 WRITE entered，测量第一套 running 外观。
2. 同 hold 尝试另外三套外观，逐一要求 `RepeatedRunningMeasurement`，阻止旧串行渲染顺序。
3. 挡住 worker 的转交前闸门，实际点击 Pause，等 worker 到达闸门。
   此时要求 UI=Pausing、forwarded=false、gate 未 expired。
4. 尝试 release，要求 `PauseNotForwarded`；hold 仍 owned，WRITE entered/总请求数不增加。
   因而确定性证实 UI Pausing 不能独自满足 release 前提，无需靠调度碰巧重现。
5. 显式允许 worker 转交，观察真实 pause 返回后才 release；
   随后仍要求真实 Paused、精确 staged prefix/字节数、稳定暂停、恢复精确字节和资源清理。

新增辅助测试
`handoff_is_channel_scoped_and_guard_drop_unblocks_only_its_worker`，
用显式 poll 确认两个不同 channel 都处于 Pending；
丢弃一个 owner 只释放该 channel，另一个仍 Pending，直到自身被显式释放。
它验证辅助闸门隔离与清理，不冒称为真实 session 暂停测试。
session fixture 原有测试还检查未匹配 WRITE 没有 entry timestamp、
真实 entry 有界且新 generation 不继承旧时刻。

诊断保留 panel/queue phase、字节、busy、selected job、状态文本、
session/subsystem/目录 handle 快照和 Paused 等待变化。
新增 scene 的 language/theme、真实 handler entry 墙钟、每 hold 的单套测量数、
worker reached / forwarded / expired，以及 release 的先后证据；
关键 running 耗时仍在后续等待前输出。
负控用独立 `FILES_LAYOUT_HANDSHAKE_NEGATIVE_JSON` 记录拒绝旧顺序及最终清理。
若后续断言失败，保留当次原始日志，不盲重跑。

## 执行状态与父任务交接

| 项目 | 当前状态 |
| --- | --- |
| 五个作者 Rust 文件的独立 rustfmt | 已执行，exit 0 |
| Cargo 编译 / 四组专项 | 9项通过、0失败；各入口四线程，执行会话54030退出0并回收 |
| Clippy、依赖策略、完整工程门禁 | 最终520输入full-gate-r2通过，exec88486已退出0并回收，结果见本记录开头 |
| 176 条唯一布局、16 条 transfer 回执 | 已从实际日志重算，见下方回执 |
| 新非作者 review | APPROVE_SCOPED_SOURCE_AND_TARGETED_EVIDENCE，开放P0/P1/P2/P3=0 |
| GUI / Podman / 外部网络 | 本测试切片未运行；父新包的离线恢复原生见[恢复记录](2026-11-05-workspace-startup-restore.md) |
| commit / push | 以Git及本轮交付回执核对，不从测试通过推导 |
| 旧 Linux CI 根因 | UNKNOWN，保持 |

格式化使用 `rustfmt --edition 2024 --emit stdout --config skip_children=true`，
逐文件由 stdin 格式化并只写回作者的五个文件，不递归格式化其它模块。
这是作者最初的定向格式检查；父后续full-gate-r2的全仓格式及严格工程检查另已通过。

首次专项复用现有共享target，`CARGO_BUILD_JOBS=2`，总预算900秒，
实际入口为`cargo test -p keelshell-app --locked <filter> --
--nocapture --test-threads=4`。每条均退出0并回收，517项输入保持。
精确过滤项和原始输出保存在本轮证据目录：

- `files::transfer_tests::workspace_layout_tests::`
- `files::test_server::pause_handoff_fixture::tests::`
- 父任务的 `terminal::text_input_tests::` 两项
- 受 fixture 时间戳变动影响的 `transfer_write_gate_tests`

实际新增 app 测试为两个（真实负控、辅助闸门隔离/Drop），原有两个布局测试保留。
主矩阵的成功须同时核实 176 个唯一布局、16 次真实 pause/resume/清理回执；
exact 单测的四线程选项不等于全 app 四测试并发。
本轮日志为`work/input-and-layout-readiness-20261105/targeted-r1-{1,2,3,4}.log`，
结果为`targeted-r1-result.json`，父重算为`targeted-r1-parent-validation.json`。
16次真实WRITE entry到release为0.211683–0.324300秒；每hold只测一次running，
均在session pause转交后release，暂停为65,536字节，恢复完成为786,432字节，
全部subsystem和目录handle归零。
负控拒绝三次重复外观及提前release，再真实暂停65,536字节、
恢复393,216字节并清理。独立报告为同目录
`independent-review/source-r1.md/json`，只批准当时冻结源码及九项专项，
不签后续文档或完整门禁。父最终组合完整门禁已通过，新非作者
`final-combined-r1`另行批准520输入的组合源码与工程证据；
任何本地成功均不能关闭旧CI UNKNOWN或替代三平台原生桌面验收。
