# 2026-11-15 有界 Zmodem 最终握手等待

状态：`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_APPROVED / DIRECT_PTY_ACCEPTANCE_OPEN`。
基线：`b66ec7e564e97c3cc8261b447288377ac1ffccc2`，仍只本地提交、不 push。
本批延续[阶段集中验证](../STRATEGY.md#阶段集中验证)，不代表完整产品或 FILE-05/M4 完成。

## 原因调查

上批严格直接 PTY 的本机 `6/7`、Debian `5/7` 与 `4/7` 保持原件，
不由后来的绿色运行或新结果类型覆盖。

本轮在原生产输入上重新构建独立上游 lrzsz 0.13.1，没有宿主安装或应用捆绑。
原 archive SHA-256 为
`112f91059fbd118e016d800c1a5c5a1702e965d8b9bfbda6015e44e20bfb93c9`，
下载内容与先前独立来源检查一致。原始 peer 的真实 SSH raw PTY
`native-original-r1` 为 `7/7 / exit 0`；带只读 tcflush 观察的
`native-observed-r1` 也为 `7/7 / exit 0`。这说明缺 OO 有时序差异，
不能以这两次通过关闭先前严格互操作缺口。

`tail-probe-r2` 在本地 raw PTY 下使用同一未修改的 peer，接收一个空文件；
返回 ZFIN 后，故意让 master 等 peer 退出再读取。四次都观察到：
最终 `tcflush(TCIOFLUSH)` 前的输出队列为 `2` 字节，syscall 返回 `0`，
peer 退出 `0`，随后 master 读不到 OO。诊断 interposer 仅观察原调用，
没有改 selector、返回值或 peer 二进制。上游源码的收尾顺序与此一致。
此结果证明受控模式中的队列清空风险，不推断每个旧 SSH 失败的唯一根因，
也不是应用传输或桌面验收。

首个 `tail-probe-r1` 因诊断脚本错误地等待未转义的 NUL 而失败，已实际退出并
回收 peer；修正为空文件 wire 中实际的受转义元数据与 CRC32 EOF 后使用新编号，
原失败不覆盖。最早 wrapper 因尚未创建 logs 目录没有启动 peer，亦不算运行通过。

只读诊断 agent 检查 SSH reader、russh queue 与夹具生命周期，未找到已证实的
因果缺陷，P0–P3 均为 0；只写诊断报告，没有运行 Cargo 或测试。agent 已关闭。
父继续负责同一主树的唯一有效 Cargo target
`work/recovery-batch-integration-20261115/target`，未切换源码归属。

## 实现与验收

设计见 [ADR 0137](../../adr/0137-bounded-zmodem-farewell.md)。
core 作者独立编写协议收尾和有界终端尾部保存；父实现 session、bridge、GPUI
及实际 peer 交互夹具，写集互不重叠，集中编译、完整验证与新非作者复核。

严格 `Finished` 继续要求实际 OO。新的 `EndedWithoutFarewell` 只表示等待已结束，
明确提示握手未确认，不代替逐文件 FileSaved。精确 lease 的所有回复写入完成后
开始三秒等待；新增回复撤销旧 flush 确认。只有等待界限已到或有序 clean exit，
且没有当前未审核/未完成/未知发布时才可进入此结果。

尾部普通输出保留；原未知结果、quarantine 和正在工作的 publication hold
不因后续警告而清除。输入继续由精确 UUID handoff 释放，不重放旧输入。
新增独立 peer 场景检查回到同一 SSH shell、完整内容、返回标记和随后的新输入，
将严格 OO 与缺 OO 结果分别计数。原七项严格断言保持。

本批命令、冻结、逐名库存、实际测试计数、最终新非作者审查与清理在取得真实
终态后补齐。原临时目录为 `work/pty-farewell-20261115`；收口时无损保存文本证据，
清理临时 peer、observer 动态库与重复测试副本。
本轮全量在线依赖复查在 60.009s 的外层期限实际被终止并 reap，未生成完整报告，
不能签全部依赖最新；本批不新增依赖。原命令与空输出摘要保留。

## 首轮复核与修复中的边界

r1 冻结 `570 + 12` 输入，SHA-256
`cf4cb0e53b80b09c44da99e3fe358cd7091622314625f881fd2769df783beba4`。
首轮实际通过 core `83/83`、session integration `22/22`、app Zmodem `22/22`，
真实本机 peer `8/8`（原七项 strict 与一项新 interactive）。
这些是 r1 输入的作者结果，不能代签后续修复输入。

两名非作者首轮均 HOLD，合计三个 P2，原件保留：

- `CORE-R1-01`：UTF-8 的 `0xaa` 可能被当作重复 FIN 的 parity pad 并裁掉。
  新六项真实 Rust 红回归 `core-utf8-red-r1` 实际 `0 passed / 6 failed / exit 101`，
  准确复现 `e4b8aa2420` 变为 `e4b82420`。修复只在 farewell 候选起点有限回看
  一个 UTF-8 序列，合法 parity protocol pad 继续受专门回归保护。
- `P2-001`：原 bridge 把 russh queue admission 当成实际 transport flush。
  修正为同一 FIFO 中数据后的真实认证连接往返，output token 绑定 lease/revision；
  下层背压、断连与迟到回执需要独立回归，不把手工调用 completion 的旧测试当证据。
- `P2-002`：已观察非零/信号/未知 shell completion 后，grace tick 仍可能抢先
  产生警告。需要在实际 pump 中抑制该 tick、按序 drain；保留排队的实际 OO 和尾部。

完整 `full-gate-r1` 实际退出 `1 / 18.095s`，停在 Clippy 的 `extend(drain(..))`。
代码已经改用 `append`，后续成功门禁另编号。
Linux `linux-peer-r1` 实际退出 `1 / 56.381s`：独立 reference 通过，但父八项测试
全部因启动环境缺 `KEELSHELL_ZMODEM_RZ/SZ` 而 `NotPresent`，尚未进入应用协议；
不是八项协议拒绝，也不撤销这次夹具失败。容器、自有进程和临时密钥已清理。
修正启动环境后必须另编号运行，原失败不覆盖。

第二轮在线依赖检查将每请求超时降为三秒、并发八项，仍在外层 `55.013s` 超时，
实际 `exit -15 / reaped`，没有完整报告。当前 Rust/Cargo 实测均为 `1.100.0`；
项目没有 toolchain pin 或 `rust-version`，本批不新增依赖，也不宣称已验证全部最新。

## 第二轮冻结与新增复核

r2 冻结 `571 + 12` 输入，SHA-256
`8efdd6ebf0253323d169abd4e87a05bef98011e17c3dfa63beed52dac5eecd1a`。
父通过 core `94/94`、session `23/23`、真实底层 barrier `4/4`、app Zmodem `26/26`；
后者包括实际 production pump 的四项 due-grace 回归。其原红运行
`app-exit-order-red-r1` 实际 `0/4 / exit 101`，都由 timer 抢先 warning。
修复后的 `app-r2` 有三项新测试预期错误：失败 shell 的未发送 cancellation 仍应
保持 hold，只有实际 transport cleanup 才释放；修正这项新测试模型后
`app-r3` 为 `26/26`。旧 handoff 和未知保护断言没有放宽。
底层测试真实阻塞 TCP AsyncWrite 超过三秒，确认排队成功时不产生 warning；
实际 roundtrip 后才允许结果，并覆盖 reply 断连、reply 超时和先行重协商。

当前输入的真实 macOS peer `native-current-r2` 为 `8/8 / exit 0`，正常 GUI/MCP
arm64 Mach-O 构建通过。Linux `linux-peer-r2` 为 `7/8 / exit 1`，
原严格 Unicode CRC32 用例的 peer exit 0 断言先通过，随后缺严格 Finished；
保持该失败，未改断言。独立 reference 通过，interactive 实测
`strict_OO=1 / missing_optional_OO=0 / same_shell_continued=true`，
owned 进程、容器与临时密钥清理均确认。此结果不补签全部直接 PTY 兼容。

新 core 非作者独立执行父预构建 `94/94`，关闭原 UTF-8 P2，但新增
`CORE-R2-01 / P2`：已保留的普通尾部在 peer cancellation 或输入失败后丢失。
父停止完整 `full-gate-r2`，实际 `exit -15 / 218.112s / reaped`；
其中 Python、fmt、严格 Clippy 已通过，但没有完整工程通过结论。
原 r2 HOLD 和父已声明修复后出现的一项源码 drift 记录保持。

父新增八项 core 与两项 session 回归；`core-tail-error-red-r1` 实际
`0 passed / 7 failed / exit 101`，覆盖 peer cancel、输入错误和 retry 失败。
另有拒绝超大输入时只保留此前已接收尾部的边界测试。
修复让尾部所有权跨 Cancelled/Failed 状态存续，清理只返回一次普通字节；
CRC 验证成功的协议头及五 CAN 留在协议侧，终态仍是取消或失败。
新 r3 冻结 `571 + 12`，SHA-256
`e83c251520832cbb3a15f0c52c9ae23365729e0c4f192cf0edf2c537c0ba6434`。
相对 r2 只有 core codec/receiver/test 和 session test 四项变化；
最终运行、复核、门禁与清理按新编号绑定，不能沿用 r2 的成功作最终签署。

Windows 安全文件后端、Linux 本地 Rust 文件后端、旧 Python runtime、真实
三平台桌面、IME/辅助技术、安装、签名与发行验收保持各自开放边界。

## 第三、四轮发现与实际修复

r3 作者通过 core `102/102`、session `25/25`、底层真实 transport barrier
`4/4`，并以新的正常 GUI/MCP 构建和本机原版 peer `8/8` 绑定生产输入。
Linux `linux-peer-r3` 实际 `6/8`：严格 CRC16 和 Unicode CRC32 缺少
`Finished`，保持失败。interactive 场景实际
`strict_OO=0 / missing_optional_OO=1 / same_shell_continued=true`，
证明本次有界缺握手结果确实允许在同一个真实 SSH shell 中继续显式新输入。
这不替代两项严格握手或 Linux 原生桌面验收。

新的 session/UI 非作者发现 `P2-003`：pump 结束后，`protocol.fail()`
恢复的普通尾部仍停在私有 output queue，worker 返回时丢失。四项真实
production worker 红回归 `app-cleanup-red-r1` 实际 `0/4 / exit 101`。
修复后 shell close 和普通尾部投递共享原有两秒 cleanup 预算，不被取消标志
打断；临时 Full 按序重试，持续 Full 或 receiver 断开显式失败，并保留 primary
错误。完成这段清理才释放 transport/input fence。四项回归覆盖实际写入确认
超时、worker cancel、暂时背压和永久背压。

完整 `full-gate-r3` 实际 `exit 1 / 7.410s`，暴露 Linux fixture runner 的
进程清单读取碰到绝对 deadline 时，`ownership.InteropFailure` 直接逃出，
没有形成原有 timeout receipt。独立实现 agent 仅修改
`run_linux_file_interop.py` 与其测试：只在 command deadline 已到且明确是
inventory 预算错误时归入 timeout；deadline 前或身份/权限等其他错误仍失败。
capture 成功后也复查同一个 deadline。保留 `finally stop_owned`、birth
identity、七秒清理预算和失败回执；原真实子孙 timeout 仍为 `0.8s`。
新增七项 named test、17 个受控场景，作者新回归 `7/7`、真实子孙 `1/1`、
全 Python `148 run / 147 passed / 1 skipped / exit 0`，专项已含于总数中。
原预期红测与后续三次绿色 subprocess 回执均保留。

r4 冻结 SHA-256
`9133ce6141be9498034641d25fe064b60c0904fb4ced9f8d73f52a76e2e4c58c`，
仍为 `571 + 12`。r3→r4 仅 bridge 生产/测试和上述两个 Python 脚本变化，
`231` 个 core/session/root 输入逐项相同，连续性证明保留。
正常 GUI/MCP `native-build-r4` 实际 `exit 0 / 3.884s`，均为 arm64 Mach-O。

session/UI reviewer 在 r4 独立 full bridge `14/15 / exit 101` 发现
`P2-004`：既有永久 UI 背压测试仍以 `worker?` 要求成功，与修复后的显式
cleanup timeout 契约不一致。父停止 `full-gate-r4`，实际
`exit -15 / 235.951s / reaped`，没有签成完整门禁通过。
r5 只修正该测试为精确 `SSH_SHELL_OUTPUT_CLEANUP_TIMEOUT` 断言，并加强为
精确 128 个 `x` 字节检查；原六秒外层预算、真实 Full receiver、typed
`ShellEnd::Cancelled` 和 `session.close()` 保留，生产代码没有变化。

最终 Linux `linux-peer-r4` 实际 `exit 1 / 42.769s`，在公开包 bridge 的
apt 安装阶段连接失败，未进入应用协议。四轮 Linux fixture 的 owned
进程、容器和临时密钥清理回执均保留，不重复运行以替换已有失败。

## 最终冻结与非作者复核

r5 冻结 `571` 个工程输入及 `12` 个门禁脚本，SHA-256：
`d8d27989768ae1827697458ee52d366d407ca8543a9eebd41baf6d8f283a2b2e`。
r4→r5 唯一变化为上述既有 app 测试断言。所有生产输入及全部 Python 脚本
未变；core/session/root 的 `231` 个输入从 r3 到 r5 连续相同。
父实际 `app-bridge-r6` `15/15 / exit 0 / 19.308s`，完整 app 库存 `1238`，
逐项绑定最终冻结。新增默认 Rust `59`、Python `7` 和 opt-in peer `1`，
各用例名保留在 `new-cases-r4.json`；r5 没有新增名称。

三名新非作者最终均为限定 APPROVE，范围内开放问题 P0/P1/P2/P3 全为 `0`：

- core 最终连续性补绑关闭 `CORE-R1-01`、`CORE-R2-01`，沿用独立
  `102/102`，没有把 r5 的零次执行改写成重新运行。
- session/transport/bridge/UI 关闭 `P2-001` 至 `P2-004`，独立 r5 full
  bridge `15/15 / exit 0 / 22.990s`；沿用同源 app Zmodem `30/30`、
  session `25/25`、barrier `4/4`。两个 app 组有 12 项重叠，合计 33 个
  不同用例，不能算成 45 项。reviewer 使用核对摘要与完整库存的父预构建
  稳定副本，没有独立重编译。
- Python 非作者在最终文件独立 `17/17 / exit 0 / 1.393398s`，并在基线
  实现交叉红测 `7` 项，预期 `exit 1`、2 failures/5 errors；共两次测试
  subprocess、24 次 named-test 执行，最终候选唯一用例为 17，r5 未重复运行。

原 notice、HOLD、失败、源码 drift 及各轮限定批准全部保留，最终批准不改写
报告签署时完整 gate 尚待父确认的事实。父负责取得同一 r5 冻结的完整工程门禁
成功终态并补齐输入绑定。三名 reviewer 与实现 agent 已关闭；没有为本批新建
用户 chat 或新的实现 worktree，其他任务资源保留。

## 最终集中工程门禁

最终 `full-gate-r5` 已实际 wait/reap，`exit 0 / 646.751s`，没有外层超时。
完整命令为 `python3 scripts/check.py`，仍独占原主树的唯一有效 target；
没有切换 checkout 或复用另一个源码副本的工程产物。

| 范围 | 实际结果 |
| --- | --- |
| Python discover | 148 run，147 passed，1 skipped，0 failures/errors |
| 普通 Rust harness | 2935 passed，0 failed，35 ignored |
| workspace rustdoc | 14 passed，0 failed |
| 格式、严格 workspace all-targets Clippy | 通过 |
| 全部直接 registry requirement `x.y` 策略 | 通过 |
| readiness、observer、small-stack controller | 三组实际执行通过，单列而不重复累加 harness 数量 |

新增默认 Rust `59` 项均按名字出现在最终 gate 的 passing results 中；Python
新增七个方法也纳入完整 discovery。普通总数已经包含这些新增，不叠加专项、
复核、旧冻结运行或 opt-in peer。app 完整库存 `1238`，本轮实际
`1236 passed / 2 ignored`。逐 suite 原始 summary、59 项逐名绑定、七个 Python
方法及 gate/log 摘要保存在 `final-validation-r5.json`。

门禁后重新核验 `571 + 12` 输入，摘要及完整工程路径集合均一致，零 drift。
父以该真实成功终态满足原 reviewer 的工程条件；三份原始批准和此前 HOLD
没有重写，另存 `review-conditions-satisfied-r5.json`。
正常 GUI/MCP r4 构建与 r5 的生产输入相同，arm64 Mach-O 摘要另存
`native-build-artifacts-r4-r5-binding.json`，不补签桌面操作。

本机独立原版 peer `native-current-r3` `8/8` 的 core/session/root 输入与
r5 连续一致；Linux r3 `6/8`、缺握手 interactive 成功及 r4 包安装失败保持
各自原件。严格互操作继续 `HOLD/PARTIAL`。Windows 安全后端、Linux 本地
Rust 文件后端、旧 Python runtime、真实三平台桌面、IME/辅助技术与发行保持
开放；本批工程与源码批准不结束 FILE-05/M4 或完整产品总目标。

最终持久证据、无损导出清单和资源回收见
[本批证据入口](2026-11-15-bounded-zmodem-farewell-evidence/README.md)。
本批仍只本地提交，不 push。文档和提交绑定不改变最终工程输入，不重复执行
已通过且输入相同的完整 Rust 门禁。

`archive-manifest.json` 的 `708` 份原件已逐字或确定性 gzip 保存并复核。
清理前再次核对全部持久文件、解压原文和待删除文件摘要；随后删除本批
`869` 个工作文件、回收 `626,097,502` 字节。加上先前十个过期测试副本的
`921,528,032` 字节，合计回收 `1,547,625,534` 字节。原始失败、notice、
限定批准、原库存及二进制摘要均保留。三个 reviewer 已实际关闭，四轮
owned 容器再次确认不存在；自有 SSH/peer 进程及临时密钥有原始清理回执。
未新建用户 chat 或实现 worktree，不删除其他任务资源；唯一有效 Cargo
target 的目录身份在清理前后相同。入口及最终清理回执在持久证据目录内。

## 本地提交与原件 Git 保存

本地源码提交为 `590cdfb654c41d615f01bc135375ffc02d525461`，父提交为本批基线。
[提交绑定](2026-11-15-bounded-zmodem-farewell-evidence/commit-input-binding.json)
从精确提交 Git blob 和当前文件两侧各核对 `1291` 项：
最终 `571 + 12` 工程输入及 `708` 份原件，零 mismatch。
本地 commit 命令退出 `0`，未执行 push，也未查询远端 SHA。

提交前的首次 staged diff check 在三份原始 stdout 的 EOF 空行报错，使用局部
binary 属性保存原件，没有裁剪空白。随后 index 检查发现根目录 `*.log`
忽略规则使原始 `.log` 尚未纳入；为本批证据目录增加局部 `!*.log` 规则后，
全部 `708` 份原件逐项纳入且摘要匹配，staged diff check 通过。两次检查失败与
修正保留为 `post-export-index-check-r1/r2.json`，不改写原始运行。
后续提交只补文档、证据属性和提交回执，最终工程输入不变。
