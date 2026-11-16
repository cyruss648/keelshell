# 归档执行截止时间：可控时钟集成测试

## 当前结论

基线为 `5c23396c3cfcc6dd021fb870a7007373c25475d1`。本片只调整 session
测试及其 dev feature，不修改生产归档、审核、派发或隔离逻辑。
精确候选通过 `1/0/0`，完整 `server_archive` target 通过 `40/0/0`。
故意重置读回预算的变异实际编译成功，并被精确测试拒绝 `0/1/0`；
生产源精确恢复后，重新编译绿例通过 `1/0/0`。

最终完整门禁实际 `exit 0 / reaped / 785.431s`，普通 Rust
`3220/0/48 ignored`、rustdoc `14/0`、Python `290 run / 289 passed / 1 skipped`。
fmt、严格 workspace Clippy、x.y 政策和三组 AI controller 均通过；
app `1339/0/2 ignored`，完整 1,341 名库存逐项相等。
718 项路径集合及全部字节零漂移，归档 40 名均实际通过。
新的非作者源码及文档复核均 APPROVE，P0–P3 全 0，实际独立测试和 UI 重放均 0。
原报告在签署时对完整门禁的 HOLD 条件，由同一冻结输入的成功回执另行满足；
不覆盖或改写原报告。完整产品和精确远端 CI 不由此签署完成。

## 问题与调整

旧源码 `9d0f886acda4e48058004a28f85ef1fa3c510d4c` 的 Quality
`43801985537 / attempt 1` 在 macOS job `133084142077` 中，
该 target 为 `39 passed / 1 failed / 0 ignored`。唯一失败为
`server_archive_dispatch_and_readback_share_one_fixed_execution_deadline`，
只有 `Elapsed(())`，不能确定具体 await 或生产根因。
旧原件见[上一批证据](2026-11-16-archive-diagnostics-and-native-focus-evidence/README.md)。

旧测试在一秒执行预算中，先消耗真实 650ms，再要求真实 SSH、Python helper
及 SFTP 到达读回 barrier；剩余约 350ms 依赖 runner 调度速度。
本片把这个可静态确认的测试条件移除，不能据此认定它就是旧 CI 的失败原因。

- 会话建立、来源检查与计划审核继续使用真实时钟。
- 执行阶段使用 current-thread runtime 和暂停的 Tokio 时钟。实际抵达派发
  barrier 时断言执行时间为零，推进 650ms 后释放派发。
- 真实 helper 和传输仍必须到达读回 barrier；此时断言时钟仍为 650ms、
  派发恰好一次、执行尚未完成。再推进 351ms 越过原一秒截止。
- 结果仍须 `MutationUncertain`，保留目标及 staging 两条 quarantine、
  独立 SSH 存活和显式关闭。没有 skip、自动重试或新生产预算。
- `wait_frozen` 用 `std::time::Instant` 分别约束两次 barrier 的五秒和结果
  的两秒墙钟等待，并保持 runtime 可运行，防止 idle auto-advance 消耗受控截止。
  固定阶段和数字计数诊断不记录路径、审核内容或凭据。

`<1400ms` 断言改为生产使用的 Tokio 时钟，检查执行预算，而非机器性能。
额外 1ms 用于跨过 timer tick；原生产一秒预算不变。Tokio `test-util`
仅在 session dev-dependency 中启用，继承 workspace 的 `1.54` 要求；
Cargo.lock 未改变。

## 实际红绿

| 原件编号 | 结果 | 说明 |
| --- | --- | --- |
| `deadline-candidate-r1` | 1 passed / 0 failed / 0 ignored | 首次编译候选；用例 0.13s |
| `archive-target-r1` | 40 passed / 0 failed / 0 ignored | 完整 integration target；用例合计 5.60s |
| `deadline-reset-red-r1` | 0 passed / 1 failed / 0 ignored | 实际编译后的错误实现被拒绝，Cargo 101 |
| `deadline-restored-green-r1` | 1 passed / 0 failed / 0 ignored | 生产文件精确恢复，重新编译，Cargo 0 |

红例同时移除外层原截止约束，并在实际 SSH 执行后错误重建读回 authority，
模拟真正的分阶段续期；仅重置内层而被外层拦住，不足以证明测试有效。
红例到达 `dispatch_entries=1/read_entries=1/pack=1`，然后在固定
`phase=execution deadline` 的两秒墙钟上限失败。不是编译失败或准备阶段失败。

变异前/恢复后生产文件 SHA-256 均为
`d31d4eecccb5566097dd1f49e86a803b38fb43763f0cc6334449e8cfc1c2d8ba`；
脚本保留原件和补丁，`finally` 在确认没有并发写入后恢复原字节。
临时错误实现不提交。

## 冻结与验证边界

`inputs-freeze-r1.json` 包含 718 项完整非文档工程输入，集合 SHA-256：

```text
593459030133909451156982dbb949e53d872cccf6cfa30a18593b2411e66e21
```

唯一 target 仍属于当前父 checkout，没有切换源码归属；按
[共享 target 合同](../SHARED_TARGET.md)保留有效第三方缓存及正常程序。
最终门禁 `full-gate-r1` 的实际退出码、回收与原始日志摘要均已核对；
采用 `final-gate-summary-r3.json` 的派生汇总。
首次派生脚本误取 core 包的同名 `server_archive` target；第二版发现 MCP
程序已不等于旧摘要。两次派生失败的脚本/诊断单列保存，没有重跑成功门禁。
最终按包精确绑定 session 的 40 项，并如实记录正常产物观察，不放宽源冻结或测试断言。

GUI 仍为上批字节
`af2a88364650b90b5dfab4ca8a4655a8430febc4971f575c7a7fd0fde19eb9a3`；
门禁后 MCP 实测为
`3e9270c84f3aad14f331b06cb3f15e77f2d2a40602dcd61e90dd39b5b939ed4b`，
不能再将 target 中该文件当作上一批 MCP 原件。未另做正常构建、打包、安装或
新原生操作，历史原生回执仍绑定它当时使用的旧字节。

本次实时官方依赖检查覆盖 113 项声明/使用、37 个 registry 包；
23 个包的最新版本已核实，14 个包因元数据请求失败仍 UNKNOWN，检查整体退出 1。
`gpui-kit 0.9.1` 与 `tokio 1.54.1` 已核实；不声称所有包均已确认最新。
所有显式直接要求仍为 x.y。此片没有升级版本或添加工具链 pin。

新非作者已关闭，原报告、输入及文档快照、完整摘要清单全部保留。
原件与提交绑定入口见[本批证据](2026-11-16-archive-deadline-evidence/README.md)。
精确新提交的 macOS/Linux/Windows CI 分开记录。
上一批诊断源码 `76b8253acd9bfa62180563f482e851064bc1ea34` 的
Quality `43806549649 / attempt 1` 当前仍在运行，Windows 八项旧缺审失败
根因尚未由新日志确认。此片不替代 Windows 修复、原生 GUI、IME/辅助技术、
严格 Zmodem peer、完整产品或发行验收。
