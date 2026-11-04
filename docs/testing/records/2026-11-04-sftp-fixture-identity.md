# 2026-11-04 — SFTP 测试服务器身份隔离

## 已提交源码与平台回读 — 2026-11-05

修复已提交并推送为 `dc94e0e03782ba3d115e784c72823ee0f082b1b2`；
`2026-11-04T16:00:43Z` 读回本地与远端 main 一致。
Hypatia 的最终提交复核将 545 个非 Markdown 输入及九个声明变更文件
逐一绑定到已通过的修后门禁和文档，结论为 APPROVE，P0/P1/P2 均为 0。
提交复核位于
`work/theme-integration-20261104/ci/sftp-fixture-identity/independent-review/p2-closure/final-binding/committed-review.json`。

精确 Quality `41863354940` 第一次尝试的最新分项回读如下：

| 平台与 job | 已保存结果 |
| --- | --- |
| macOS `126691000390` | cancelled；未获得 runner，未执行任何步骤。GitHub annotation 明确记录多次未能分配托管 runner，并提示 macOS ARM 容量紧张；没有测试日志。 |
| Linux `126691001067` | success；app 826 passed / 2 ignored。三个新增身份/展开回归、原 MCP 失败项和原更新器失败项实际通过，OpenSSH 互操作通过。 |
| Windows `126691000868` | success；app 817 passed / 2 ignored。三个新增身份/展开回归、原 MCP 和更新器失败项实际通过；原生 Job 准备、身份绑定、回收及空清理错误回执通过。 |

第一次尝试整体为 **failure**，原因是 macOS 未启动；不得把两个成功平台
合称该次完整 CI 通过。两份完整日志与逐项测试回读保存在
`work/theme-integration-20261104/ci/sftp-fixture-identity/attempt-1-test-readback.json`。
Windows 原日志摘要为
`35c28b0e7e4c95cf3f5651a428a01107ba4ace182011b1ca28004036a6337e46`。

初期 `2026-11-04T16:17:54Z` 的 API 返回 run/jobs queued、runner 为空；
后续完整日志证明 Linux 当时已经执行，因此这只保留为当时 API 观测，
不能据此认定三个平台都未运行。Linux 原日志摘要为
`8bfe0345335ab31c02a392ce395941f0c88c502e5b067999818d4666de31edce`。
同期 GitHub 官方状态接口报告 Actions 服务异常；macOS 未获 runner 有独立 annotation
证据，但该服务事件与本次所有状态延迟的因果关系没有单独证明。
API、annotation 和服务快照保存在
`work/theme-integration-20261104/ci/sftp-fixture-identity/queue-observation/`。
`2026-11-04T16:38:18Z` 已成功请求仅重跑未启动的 macOS job；
同一提交、同一 run 的第二次尝试创建 macOS job `126711796757`。
该 job 于 `2026-11-04T16:53:22Z` 再次 **cancelled**，
runner ID 为 0、步骤为空，annotation 再次明确未能分配托管 runner 及 ARM 容量紧张。
第二次尝试整体也为 **failure**；macOS 远端 CI 为 **未执行**，不是代码测试失败，
也不能以本机 macOS 完整门禁替代该 CI 结果。
Windows/Linux 在新尝试中的成功条目分别为 `126711800213` / `126711919205`，
继承第一次执行：完整日志摘要相同，不宣称它们又执行了一次。
第二次尝试证据另存 `work/theme-integration-20261104/ci/sftp-fixture-identity/attempt-2/`，
第一次失败与注释保持。两次回读均已结束并回收观察进程；
托管 runner 可用后再补 macOS CI，本轮不重复触发相同的未启动任务。
没有修改测试断言、期限、runner 或 workflow。

## 原始平台失败

提交 `7782979351291abfc95966b286f824fca683deb6` 的 Quality `41848787327`，
Linux job `126639192564` 在
`workspace::tests::mcp::file_changes::mcp_file_changes_after_review_and_missing_atomic_extension_do_not_overwrite`
失败。失败位于 `Harness::new` 的 UTF-8 种子写入，返回 `MutationQuarantined`；
没有进入该测试正文的审核、外部变更和不覆盖断言。
app 汇总为 822 passed / 1 failed / 2 ignored。原更新器失败项实际通过，
但 Linux 未到达后续 OpenSSH 步骤。

完整原日志在 `work/theme-integration-20261104/ci/native-barrier/ci-linux.log`，
其 SHA-256 为 `bc4276b62353ef0a88031baa925d357f13c5e5c2a6d43928d1371f9674324ad8`；
失败定位和原日志绑定保存在 `ci/sftp-fixture-identity/baseline.json`。

## 源码观察与待验证假设

应用的 `files/test_server.rs` 为每个独立内存文件系统绑定临时回环端口，
但使用相同的 Ed25519 固定种子。生产远端隔离身份包含主机、端口和主机指纹，
且未知写入不会因重新连接自动清除。如果临时端口被复用，
新夹具可能与旧文件系统得到相同隔离身份。

这是源码中的隔离隐患，不能直接证明本次 CI 的准确触发原因。
原日志没有隔离 owner、端点身份或 registry 容量回执；进程级 32 组容量也是
另一可能来源。因此原失败的准确原因保持 **UNKNOWN**。

## 已复现机制与限定修复

真实回环 SSH/SFTP 反例使用同一个仍绑定的 `TcpListener`，在两个独立内存文件系统
之间显式交接，不依赖 OS 随机分配或重绑端口。旧连接继续访问旧文件系统；
监听器和所有客户端始终由测试持有，最后实际 join。原固定主机密钥下，
新文件系统成功创建 `/approved`，随后无关 UTF-8 种子写入被
`MutationQuarantined` 拒绝。最终回归测试与此失败中的测试源码逐字节相同。

只修改 `files/test_server.rs` 并新增 `files/test_server_identity_tests.rs`：

- 每个独立文件系统从两个新 UUID 生成一次测试 Ed25519 密钥；
  同一 `Server` 的后续连接保留原 config、密钥和固定指纹。
- 私有 listener 交接用于确定性地重用端点，旧 acceptor 停止接受新连接并 drain
  原客户端；新增交接、join 和子系统检查均有界。
- 测试先通过实际已发送的第二个 atomic WRITE 和取消产生隔离；
  同服务器新连接必须继续被拒绝，并保留相同隔离记录。
- 新文件系统在相同端点成功写入并完整读回；在确认清理前，旧 scope 仍被拒绝，
  原完整文件内容、WRITE 数量和隔离条目均保持。
- 最后只检查并显式确认本用例 UUID 目标/临时路径对应的两条隔离记录，
  防止耗用进程级 32 组容量。验证隔离已清除、两个服务器已 join、
  活跃 SFTP 子系统和打开的目录句柄为零，再丢弃监听器。
  独立 current-thread runtime 在失败展开时拥有协议任务，不启动子进程。

不清空全局 registry，不修改生产主机校验、隔离规则、既有期限或业务断言。
这证明了身份碰撞机制及其修复，仍不能确定历史 Linux 的准确触发者。

## 作者验证与后续门禁

- 原固定密钥反例实际退出 101：新文件系统种子写入被隔离；预期断言前已完成
  两条所属记录的确认、两个 server join、子系统和句柄清理。
- 修后专项 32 passed / 0 failed / 0 ignored，四线程：1 项新身份回归、
  22 项现有 MCP 文件变更测试（包括原 CI 失败项）、9 项并行传输测试。
- 严格 app/all-targets Clippy、workspace 格式和静态 x.y 依赖策略通过。
  首次 import 编译错误及 Clippy lint 失败均保留并已修正；
  macOS `__eh_frame` 链接尺寸 warning 也保留，实际测试退出 0。
- 除原 fixture 文件外，543 个既有非 Markdown 输入与作者开始时相同，
  另新增一个测试文件。没有新增依赖、生产代码或 workflow 变更。

作者 Poincare 的源码、命令、结果和 cleanup 证据保存在
`work/theme-integration-20261104/ci/sftp-fixture-identity/author/`。

首候选的父任务完整 `scripts/check.py` 实际退出 0，耗时 486.088 秒；
545 个非 Markdown 输入的摘要/大小/权限在运行前后保持一致。
普通 Rust 2,012 passed / 23 ignored，其中 app 824 passed / 2 ignored；
rustdoc 10 passed（1 项仅编译），Python 128 项 / 1 skipped。
格式、严格 workspace/all-targets Clippy、x.y 策略及额外 readiness、
observer I/O 和 small-stack 控制器均通过。实际 wait 0、进程已 reap、
所属进程组不存在。原日志、命令、计数和输入清单位于 `ci/sftp-fixture-identity/`。
运行期间更新的 Markdown 不属于 545 输入不变声明。

Hypatia 独立初审为 **HOLD，P0=0 / P1=0 / P2=1**。
P2-1：原目标种子写入发生在其清理 guard 创建前，
独立文件系统的 mkdir/种子写入没有自己的 guard。若这些准备请求产生未知结果，
runtime 退出不会自动释放进程级隔离记录，可能污染后续测试容量。
成功门禁和旧密钥反例不能覆盖这一失败路径。

另有证据措辞修正：`active_directory_handles()` 只计数目录句柄，
未直接计数普通文件句柄。旧原日志/作者 receipt 的 “file handles zero” 字样
原样保留，以本段的较窄范围解释；新诊断已使用准确措辞。

父任务已把原目标 guard 前移至首次写入之前，为独立夹具的 UUID 目录及其子路径
在 mkdir 前创建独立 guard，并补充实际已发送 WRITE 取消后展开的控制，
覆盖两种所有权和拒绝清理外来记录。

- 文件和目录两种控制各自发送真实第二个 in-place WRITE，观察其进入 hold 后取消，
  放行并确认该 handler 完成；此时实际隔离已产生。
  不相关路径的 guard 必须拒绝确认，并保持原隔离条目逐项相同。
  随后显式触发受控 panic，在捕获展开后验证原 guard 已清理自己的隔离。
- 临时禁用 guard 的 Drop，两项控制均在原“隔离仍残留”的断言失败；
  断言前分别通过显式、限定所有权的恢复完成清理，不污染后续用例。
- 在最终相同回归源码上恢复旧固定密钥，仍复现独立文件系统种子写入被拒绝；
  这次使用 UUID 目录，失败前确认 2 条原记录、0 条独立记录并 join 两个 server。
- 精确恢复候选字节后，3 项专项通过，严格 app Clippy 与 workspace 格式检查通过。
  `test_server.rs` 摘要仍为
  `11f5d4f07c157d448c44fa9eccee8001592af9c7bd2ba7512efd245781504d73`，
  修后测试摘要为
  `d157e103dafc39dfbaed63c3452f4926718d23d109eec0136f098caa38f95c5c`。

修后原始日志、源码快照和恢复摘要保存在 `ci/sftp-fixture-identity/p2-closure/`。
修后重新运行的完整 `scripts/check.py` 实际退出 0，耗时 478.094 秒；
545 个非 Markdown 输入前后完全相同，实际 wait 0、已 reap、所属进程组不存在。
普通 Rust 2,014 passed / 23 ignored，其中 app 826 passed / 2 ignored；
rustdoc 10 passed（1 项仅编译），Python 128 项 / 1 skipped。
格式、严格 workspace/all-targets Clippy、x.y 策略及三项附加控制器命令均通过。
新门禁原日志、前后输入清单、命令计数与退出回执位于 `p2-closure/`，
首候选门禁和初审 HOLD 原样保留，Markdown 仍不纳入源码输入不变声明。

Hypatia 最终复核为 **APPROVE，P0/P1/P2 均为 0**，P2-1 与句柄措辞问题均已关闭。
两个源码摘要、新完整门禁 545 个输入及 7 份状态文档已独立绑定；
初审 HOLD 和首候选证据保持。独立复核检查了控制日志、命令和最终源码，
没有重复运行 Cargo；临时突变及 finally 恢复过程属于父任务报告，
各次突变运行没有独立的源码快照/摘要，不将其写成 reviewer 亲自重跑的证据。
清理保证限于仍可安全检查的自有连接；不能检查时保留隔离和错误，
不承诺任意失败都能无条件回收进程级状态。
上方平台回读继续记录后续 Linux/Windows 执行；普通测试不构成原生桌面或真实客户 SSH 验收。
