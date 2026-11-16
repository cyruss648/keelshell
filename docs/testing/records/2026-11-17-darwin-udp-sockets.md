# 远端 macOS UDP 端口 — 2026-11-17

状态：专项、真实 SSH、两位独立复核、集中完整门禁与正常包结构通过；工程已推送，
精确CI的macOS通过、Ubuntu／Windows栈溢出失败，后续修复另见
[栈修复记录](2026-11-17-monitor-render-stack.md)；
完整产品仍未完成。
基线 `e1f7359a975726dc6cf110fb12edcbb8a582cd95`；
活动目录 `work/darwin-udp-20261117/`。
设计见 [ADR 0154](../../adr/0154-darwin-udp-sockets.md)。

## 实际工具实验

`owned-socket-probe-r1` 为 `0/reaped/0.286212s`。脚本只对自身 PID 的
TCP／UDP socket 运行 lsof，四种选择顺序原件保存在 log；
观察包含 IPv4／IPv6 的绑定、wildcard、指定对端及未绑定对象。
组合查询包含一个自有 TCP listener 和九个 UDP 文件，排除自有 TCP 客户端
及 accepted 端点。UDP 无 `T`，未绑定输出为 `n*:*`；socket 均通过 RAII 关闭。
实验 JSON 的完整输出保留在 `.log`；运行器的同名 `.json` 是退出回执，
不是实验结果，不能混读。

## 依赖与工程归属

本批未修改 manifest、lock 或工具链约束。第一次官方依赖查询 r1 因 registry
请求失败而退出 1，耗时 115.747313s；113 声明中部分查询失败，不能签为最新。
原始报告与日志保留，后续有界重查结果见下文。

唯一 Cargo 槽继续属于同一 checkout：
`work/recovery-batch-integration-20261115/target`，
`CARGO_BUILD_JOBS=2`、`MACOSX_DEPLOYMENT_TARGET=15.0`。
没有切换归属或执行 clean。后续须核对当前实际编译库存，不能沿用旧计数。

## 原生边界

本轮可用性观察报告桌面锁屏，自动解锁失败；GUI 操作／截图／启动／安装均 0。
这不阻塞解析器、受控 GPUI 或真实本机 OpenSSH 验证，也不能写成原生验收通过。
三平台桌面、辅助技术和发行仍开放。

## 专项、真实 SSH 与修订

实际 Cargo 编译来自当前 checkout，app、session 和真实 interop 的库存分别为
`1377`、`358` 和 `15`；15 个新 session 名、3 个新 app 名和 6 个新真实
SSH 名全部存在。初次专项 r1 为 session `36/0/0`、app `11/1/0`：
分页测试未先滚到按钮，Kit 因元素不可见拒绝点击。仅修测试为有界真实滚动后
点击，r2 专项为 session `36/0/0`、app `12/0/0`，新 18 个默认测试逐名通过。
原失败日志、旧库存和二进制摘要保持。

真实 `darwin-ssh-r1` 为 `15/0/0`，外层 `0/reaped/4.931470s`。
包括 6 个新增 UDP 用例及原 9 项资源／TCP 互操作；明确核对自有 PID、
双栈绑定／wildcard、精确指定对端、未绑定正常过滤和 TCP 客户端排除。
`darwin-ssh-r1/result.json` 证明自有进程停止、祖先关系无未确认项、临时目录
与密钥已移除。该成绩属于 r2 Rust 输入，不代签完整平台或桌面。

r2 冻结 729 输入，aggregate
`ad6a7b3d3bf471c833301373b3d89b768e48258b4afda15ae6f561059da2ab09`。
第一次集中 gate 为 `1/reaped/12.072717s`，停在 Python
`293 run / 1 failure / 2 skipped`；Rust 门禁尚未开始。
失败来自联调工装的精确命令镜像仍为旧帧／TCP-only。
只同步 `scripts/monitor_recovery/fixture.py` 的帧 2 与 `-iUDP` 两行，
保留完全相等断言、Linux 命令和全部生产 Rust。

最终 r3 仍为 729 输入，aggregate
`e94405c2858d9796d9256dfd44eb277914c18fedd14e258e8025c747528e9a0e`；
其余 728 输入逐项不变。第二轮集中 gate 的实际终态见下节。

## 最终集中门禁

r3 上的 `full-gate-r2` 为 `0/reaped/754.599981s`，工程输入前后逐项相等。
普通 Rust `3325 passed / 0 failed / 58 ignored`，app为其中的
`1375/0/2`、session lib为`354/0/4`，另有14 rustdoc。
Python主discovery为293 run/2 skipped；归档helper另列30。
fmt、严格全workspace/all-target Clippy、x.y策略及三个专用controller组均成功。

从本次完整gate实际路径读回app1377和session358库存，分别与逐名执行相等；
18个新默认名全部ok。该gate app SHA为
`0ea0f976ccde1ecbe79dcd50969ef3a81d8281685086ccd6abbbc54fc7ca8eb4`，
session SHA为`adefbbd4e316e47f9c836f40fb792ef8136caeb25b00c20e83bc32d64d846142`。
专项／独立副本和workspace gate是不同编译产物，分别保存原路径、SHA与库存，
不以其中一份库存代签另一份。真实Darwin的15项在默认gate中仍为ignored，
实际15/0/0单独由真实SSH原件证明，不重复加入普通Rust通过总数。

## 独立非作者复核

Hooke 的最终 r3 静态复核为 APPROVE，P0/P1/P2/P3 全 0；729 输入前后 bytes、
SHA 和路径集合相等，并审阅两次追加修订。共 29 次命令块，27 次退出0、
2 次校验器退出1，诊断及修订原件保留；独立编译、测试和 prebuilt 执行均0。

Nash 的最终 r3 功能复核为 APPROVE，P0/P1/P2/P3 全0。
使用父级本轮编译并按完整 SHA 复制的专用测试程序，独立执行 app monitor
`33/0/0`、session Darwin sockets `36/0/0`，合计 `69/0/0`。
两条库存、两条执行均实际 wait/exit0，18 个新默认名全部存在并执行 ok。
729 输入及两份副本的前后身份、bytes、SHA 相等；r2→r3 只有工装镜像两行，
没有为该非 Rust 变动重跑已有测试。真实 SSH15 项及清理仅独立核对父原件，
没有重复启动服务或使用凭据。

功能复核的19个记录子命令全部 wait/exit0；一次初始 reviewer helper 失败另存，
不混为源码问题。独立 Cargo／编译、共享 target 读写均0。
两份批准不签署后来完成的父 gate、正常包或原生桌面，也不移用上一 a1 CI。
作者 Einstein／Epicurus、reviewer Hooke／Nash 四位均已关闭；
专用测试副本按摘要核对后回收2文件／316,832,336字节，主 target 产物保留。

依赖 r2 为 `1/reaped/196.846271s`，37 包／113 声明中 uuid、zeroize、
win32job、windows-sys、winresource 的官方请求仍失败，其余查询按报告通过。
r3 仅对官方 GET 加有限重试，外层预算用尽：
`124/reaped/535.086980s`，原始业务退出未知、输出 0 字节；
已确认自有进程终止与 pipes drained，没有把超时写成通过。
manifest／lock 均未改变，在线最新版本复查保留 UNKNOWN，确定性 x.y 策略另验。

## 正常包与原件封存

正常GUI／MCP release构建为`0/reaped/152.549983s`，
构建后729输入与r3相等。提交前正常包和本机结构检查均exit0，两个Mach-O
均为arm64/minos15.0；没有启动、安装、签名或公证。
app SHA为`c18d78d41c229205ccbcae4bbd0ca27620158fbd8559ac7d6006ecd09c30826d`，
MCP为`bb415f3ae3ae7ca62bdb78abe752b50f5a3a796dc65ec99490f54ee28cb3f693`。

[199成员封存](2026-11-17-darwin-udp-sockets-evidence/README.md)保存全部原失败、
最终门禁、两份独立复核与真实SSH、正常包摘要；重算
`0/reaped/0.182444s`、729输入零mismatch，不重放测试。
最初193成员封存因回执误入目录排除条件失败，原archive／manifest／脚本及失败
均在diagnostics保留；只修封存过滤规则后重新封存，不修改生产或重跑功能测试。

## 提交、来源正常包与清理

工程 `16703293012d798f885e1590f02c348c47e81a10` 已普通commit／push，
分别为`0/reaped/0.124313s`、`0/reaped/4.165023s`。
`2026-11-16T20:05:04.804446Z` 读回本地／tracking／远端三方一致，工作区干净。
主199成员与729工程输入从该提交的Git blob核对为
`0/reaped/2.004278s`、零mismatch，不重放测试。

以已有r3 release产物生成提交来源的
`work/darwin-udp-20261117/package-r1/KeelShell.app`，打包和arm64/minos15.0
结构检查分别为`0/reaped/0.311835s`、`0/reaped/0.173312s`。
五个应用文件与提交前包逐字节相等，GUI／MCP SHA保持上文值，
只有package-manifest的`build.commit`从null变为精确工程SHA。
没有重新构建、启动、安装、签名或公证。

主199成员与上一a1 CI的27成员从工程Git blob逐项核对后，回收235文件、
70,913,865字节，包含旧活动原件副本、已替代的提交前正常包、下载手册和自有
Python缓存。共享target目录device／inode及当前正常包六文件SHA前后相等。
原失败和独立复核从docs封存读取，不再依赖旧活动路径；
`work/editor-backup-ci-20261117/`已回收。

新Quality `43945988599` 绑定本工程；初始排队，随后Windows／Ubuntu完整gate
运行中、macOS排队。该快照不是终态，上一a1的绿色CI不代签UDP输入。
交付／CI工装在`work/darwin-udp-delivery-20261117/`；watch结束后再记录终态。
[25成员交付补充](2026-11-17-darwin-udp-sockets-evidence/delivery/README.md)
保存上述原件，重算`0/reaped/0.106333s`、零mismatch，测试重放0。

## 后续精确 CI 终态

Quality43945988599已completed/failure，watch为`1/reaped/1959.911713s`，
三平台原件已收齐，没有取消或重跑。
macOS完整gate及OpenSSH成功，普通Rust3325/0/58 ignored、app1375/0/2、
14rustdoc；Ubuntu／Windows完整app因嵌套监控栈溢出而中止，没有完整app摘要。
已观察／未报告数及两个准确失败名见
[37成员终态](2026-11-17-darwin-udp-sockets-evidence/ci-completion/README.md)，
重算`0/reaped/1.964279s`、729精确Git输入零mismatch。
该失败不会被后续渲染重构或绿色运行改写，修复后731输入另行验收。
