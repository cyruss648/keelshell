# 2026-11-04 — Unix 夹具分片认证测试时序

## 真实失败与复现

Windows 准备预算修复 `9d4517e653e8484436c6a10a1e81229dbccb1103`
的 Quality run `41828929466` 中，macOS job `126570758589` 在
`member_socket_fragmented_nonce_is_required_before_stop` 失败。
该 job 已越过 122 项 Python 检查、格式和 Clippy；AI 库为
108 passed / 1 failed / 1 ignored，失败返回
`TimedOut: owned fixture request deadline expired`，其余 Rust gate 未完成。

原提交 `c366871` 的 macOS/Linux job 均已成功，Windows 失败单独保留，
见[前序回读](2026-11-04-theme-delivery-ci.md)。不能因同一 Rust 测试曾通过，
就把这次失败改记为通过，也不能据此认定 Windows Python 修复改变了 Unix 协议。

旧测试向真实 socket 写入 `Q` 与前 8 字节 nonce，再通过另一线程处理请求；
发送方使用 10ms `recv_timeout` 观察尚无结果，然后补发后 8 字节。
虽然调用给出一秒父期限，夹具函数会将请求期限收紧至 100ms。
这要求发送方在接收方开始计时后及时重新获得调度。

本机在原测试补发前插入 150ms 调度延迟，实际复现相同请求超时，
退出 101；之后原文件逐字节恢复。这个反例证实测试依赖线程调度时序，
不证明该次 runner 的具体调度或负载原因已经查明。

## 修改范围与不变量

只修改 `#[cfg(all(test, unix))]` 下的两份夹具/测试文件：
`unix_cleanup_tests.rs` 与 `unix_cleanup_tests/socket_tests.rs`。
生产清理、信号次数、原进程身份、原清理期限和应用实现未改动。

原 socket 入口仍设置非阻塞并按 `min(100ms 请求期限, 父期限)` 执行，
协议正文抽到同一个私有 `Read + Write` helper；有界读写循环和错误分支保持。
分片用例使用真实 `UnixStream::pair()`，适配器只在实际读取到 `WouldBlock`
且已消耗 `Q + 8 字节` 后，检查对端尚无身份回复，随后才发送剩余 8 字节。
接收方继续按同一个 100ms 期限读取；任何提前回复必须通过完整 17 字节和
已观察分片间隙的断言。

该用例不再使用线程间定时等待来安排分片。真实 socket I/O、分片边界、
完整 nonce 授权、完整回复内容，以及其它用例的 EOF、错误 nonce、到期、
背压和关闭对端断言保留。它不再把两个线程能否在 100ms 内交替运行当作认证条件。

## 验证

- 原测试加受控 150ms 发送延迟：预期超时失败，退出 101，来源已恢复。
- 首次格式检查指出一处断言换行，退出 1；运行格式化后修正。
- 修改后的 Unix 清理/真实 socket 专项：23 passed / 1 ignored，退出 0。
- 两项隔离 Cargo 突变均在测试执行期按预期退出 101：仅消费/比较前 8 字节 nonce
  被完整性断言拒绝；不发送后半片则在原 100ms 期限超时。精确突变源码、日志和摘要
  均保留，临时副本已删除，主 checkout 源码前后相同。
- 新的非作者 Banach 源码/证据/文档复核未确认 P0/P1/P2；协议正文、原请求截止时间、
  真实分片 I/O、两个突变及文档边界均已回读。没有重复执行 Cargo 或修改源码。
- 修后完整 `python3 scripts/check.py` 实际退出 0，耗时约 481 秒；
  543 个已跟踪非 Markdown 输入前后摘要相同。普通 Rust
  2,007 passed / 23 ignored（含 app 819 passed / 2 ignored），
  rustdoc 10 passed（其中 1 项仅编译），Python 122 项 / 1 skipped；
  格式、严格 workspace/all-targets Clippy、依赖策略及额外 readiness、
  observer I/O、2 MiB 控制器均通过。运行期间更新的 Markdown 不纳入输入不变声明。

证据位于忽略的 `work/theme-integration-20261104/ci/unix-socket/`；
原 CI 完整日志为同级 `fix-macos.log`。新提交与精确平台 CI 另行读回，
前序本机或 CI 成功不代替这次新候选的验收。

前序 `9d4517e` 的 Linux job 已成功，macOS 失败如上，Windows job
`126570758307` 仍在运行；本次 Unix 修复尚待提交及精确平台 CI。
