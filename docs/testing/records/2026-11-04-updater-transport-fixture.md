# 2026-11-04 — 更新器真实传输测试夹具

## 原始平台结果

基线 `7b43af20e62c228972b461a412e063aac6ad9304` 的 Quality
`41834475286`：macOS job `126589736196`、Linux job `126589736295`
成功；Windows job `126589735715` 失败。macOS/Linux 的 OpenSSH 互操作
步骤也通过，但它们不是三平台原生桌面验收。

Windows 的唯一失败为
`automatic_download_verifies_real_archive_and_ready_continues_periodic_checks`，
GPUI Kit 报告 `UI condition timed out after 3s`；app 汇总
809 passed / 1 failed / 2 ignored。初始 Ready 等待为 5 秒，因此失败发生在
后续阶段；旧日志不足以区分请求观察、重新检查完成或暂存清理。
三份完整 job 日志和 API 结果保存于
`work/theme-integration-20261104/ci/unix-socket/`。

## 两个已复现的测试问题

1. `HttpFixture` 已接受一条请求后，若回复头或正文写入失败，就结束整个服务线程。
   被取消的 held response 可能触发此路径，后续请求无法获得它自己的脚本回复。
   新 TCP 回归先排队第二个请求，再放行第一条 32 MiB 回复；读取第一条的头和
   1 KiB 正文，使用 `peek` 确认还有未读字节，关闭首客户端。
   第二条必须收到精确的头、含二进制字节的正文和 EOF，断言前收尾并 join fixture。
   在旧服务器上首次及额外 10 次运行均因第二条回复 `ConnectionReset` 实际失败。
2. GPUI Kit 0.9 的 `wait_for` 累计测试时钟的 10ms 间隔；当前 GPUI scheduler
   默认禁止 parking，并可立即快进到下个 timer。真实 Tokio/TCP 工作仍按实际时间执行。
   新时钟控制在旧挂载方式下等待 100ms timer，实测只经过 155.834µs，退出 101。
   这确认了时间域不匹配，不能证明 Windows 那次失败的唯一触发原因。

## 修复范围与期限

只改 `updater/tests/automatic.rs`、`automatic/socket_tests.rs`，
新增测试 helper `automatic/transport_wait.rs`。生产更新器、网络 worker、
安装器、依赖、Cargo.lock 和 workflow 均不改。

- 已消费的响应写入失败后继续下一条脚本；不放松请求路径或授权头断言。
  部分请求头错误仍按原路径结束；停止标记仍在下一轮立即检查。
  原 8 秒 accept 生命周期、2 秒请求/响应期限、5 秒 held-release 上限保持。
- 这组真实传输测试挂载时启用 scheduler parking，使 timer 不再无条件快进。
  等待 helper 使用一个 `std::time::Instant` 截止点，在渲染/谓词前后检查，
  不用轮询次数累计时间，也不在重试时续期；到期后不接受迟到成功。
  原 3 秒和 5 秒数值保持，但语义明确改为真实经过时间，
  不声称旧虚拟时钟和新墙钟的实际等待时长相同。操作系统调度和单次渲染可能让
  返回略晚于截止点，不能让截止后的结果通过。
- 每个等待输出固定阶段名和实际经过时间，便于后续区分失败阶段。
  周期检查除恢复原暂存包外，还必须观察最后一次 `/latest` 请求及成功校验状态；
  网络失败后恢复 Ready 不能伪装成成功的周期检查。
- 新控制验证 timer 不抢跑、持续未完成在同一期限到期、已到期等待不接受 ready 谓词。
  原哈希、包内容、持有身份、取消、显式丢弃和不安装断言保留。

## 本机验证

- 旧 TCP 反例：首次及 10 次重复均按预期退出 101；旧有 socket 控制 3 项通过。
- 旧时钟反例：按预期退出 101，实际时间如上。
- 首次候选编译因 helper 缺少 `AppContext` trait import 失败，补齐 import；
  原日志保留，不将编译失败写成行为测试失败。
- 修后更新器专项：54 passed / 0 failed / 0 ignored，四线程，测试正文约 0.58 秒。
  首次链接的 `__eh_frame` 尺寸 warning 保留，测试实际执行完成。
- 完整 `python3 scripts/check.py` 实际退出 0，约 480 秒；544 个非 Markdown 输入
  （已跟踪文件及新增 helper）在运行前后摘要相同，运行期间更新的 Markdown 不在此声明内。
  普通 Rust 2,011 passed / 23 ignored，其中 app 823 passed / 2 ignored；
  rustdoc 10 passed（1 项仅编译），Python 122 项 / 1 skipped。
  格式、严格 workspace/all-targets Clippy、x.y 依赖策略及额外 readiness、
  observer I/O 和 2 MiB 控制器均通过。
- 新的非作者 Bacon 已复核三个测试文件、17 个原等待数值及顺序、原始反例、
  完整门禁和文档，P0/P1/P2 为 0。其独立摘要将 544 个输入绑定到 gate 前后和
  最终源码；未重复执行 Cargo，不扩大原生验收范围。

修复已推送为 `9ab663ea1aaafdcff69a3203d38910bea681f84a`，远端 main 读回一致。
Bacon 已额外核对精确提交的 544 个非 Markdown blob 和最终文档状态句，
没有 P0/P1/P2；工作树干净。Quality `41843572205` 的 Windows
`126621167159` 在 Python 原生 Job 准备测试失败，未进入 Cargo，不能判断
本次更新器修复的 Windows 结果。该独立失败及诊断增量见
[原生夹具记录](2026-11-04-native-barrier-diagnostics.md)。
macOS `126621167756`、Linux `126621167873` 最终均成功（含 OpenSSH 互操作步骤）；
三份完整 job 日志和摘要已保存在 `updater-cancel/ci-*`，整个 run 因 Windows 失败而未通过。

后续诊断提交 `7782979` 只修改 Python 测试与文档，更新器和其它 Rust 输入保持。
其 Quality `41848787327` 的 macOS 已成功；Linux 原更新器失败项实际通过，
但整体 app suite 在独立 MCP 夹具准备处失败。Windows `126639192119` 已成功；
其完整日志实际记录原失败用例、取消响应后的下一脚本回复及时间域控制通过，
app 814 passed/2 ignored。三平台完整日志位于 `native-barrier/ci-*`；
整体 run 因独立 Linux 失败而未通过。
原未具名的 3 秒超时原因仍为 UNKNOWN。

证据目录：`work/theme-integration-20261104/ci/updater-cancel/`。
`author/` 保存非父任务作者 Peirce 的 TCP 反例及旧代码摘要；
`clock-baseline*` 保存旧时钟源码和实际失败；
`first-compile-failure*`、`focused-updater*` 保存候选执行结果。

这是测试基础设施修复，原 macOS 主题原生包的生产输入未变；本轮不会重新把
普通测试作为桌面或更新安装验收。三主题、布局迁移及完整产品边界仍以
[主题记录](2026-11-04-native-themes.md)和[路线图](../../ROADMAP.md)为准。
