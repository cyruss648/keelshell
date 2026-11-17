# 监控读取取消 — 2026-11-17

状态：实现、本机最终完整工程门禁与非作者限定复核通过，已普通提交／推送并读回；
完整产品、三平台原生与发行仍开放。
基线 `4c8654f219779f48afd485bec8ff1b284905fbf0`，
设计见 [ADR 0161](../../adr/0161-monitor-read-cancellation.md)。

## 范围

父级负责 CapabilityCache 通知、exec／探针 admission 的取消与整合。
Franklin 仅实现受控 SSH 集成测试，父级独占 Cargo target。
Gibbs 并行收集前批 f910e001 的 CI 原件，Aristotle 独立核对空文件 peer 的状态机；
二者不修改产品源码，不将前批 CI 代签当前输入。

唯一 target 继续归属本 checkout：
`work/recovery-batch-integration-20261115/target`，
`CARGO_BUILD_JOBS=2`、`MACOSX_DEPLOYMENT_TARGET=15.0`。
未切换源码归属、不并行 Cargo、不 clean。

## 实际结果

- 工具链实测：Rust／Cargo `1.100.0`。不 pin toolchain、不设置 rust-version。
- 官方直接依赖检查 r1：`1/reaped/193.626455s`；37 包／113 声明，
  72 声明通过、41 项 `registry_check_failed`。未新增或更改依赖；
  当时全部依赖最新 UNKNOWN，保留原 JSON 与自然退出回执。
- 后续有界复查 r2：`0/reaped/9.855847s`，完整官方快照37包／113声明通过；
  仅确认该快照下的直接registry声明与lock，不扩成间接依赖、Actions或未来版本。
- Notify 底层专项：`0/reaped/5.002822s`，2通过、0失败、0忽略；
  覆盖首次轮询前撤权、多等待者及新 generation 的隔离。
- 旧实现红控制：准确还原基线三件生产文件后，当前集成19项实际
  6通过／12失败／1忽略，Cargo自然101/reaped、12.17s测试时间；
  外层`0/reaped/16.238688s`，finally逐字节恢复实现。
  12项失败均为等待取消超时，不是编译失败；原失败与回执保留。
- 修订后同套集成：`0/reaped/2.786202s`，18通过／0失败／1忽略。
  新增9项，强化3项原取消用例；核对资源／进程／端口的已发送读取、
  POSIX／Windows身份探针、leader仍持有探针锁时的waiter取消、
  迟到open后精确CLOSE、旧缓存／reset语义和新旧generation隔离。
  其他monitor及普通exec在旧通道回收前已打开，回收后继续原响应，
  普通exec完整stdout／stderr和实际非零status23分别核对。
  忽略项仍需实际Windows API capture，不以受控帧补签。
- 完整能力缓存专项：`0/reaped/2.034611s`，10通过／0失败／0忽略；
  包含上述2个新用例，不与它们重复相加成独立测试数。
- 真实Darwin OpenSSH监控回归：`0/reaped/5.813216s`，15通过／0失败；
  自有loopback sshd、固定指纹，回执77个身份观察、已跟踪进程停止、
  无未知祖先、临时密钥目录删除。此为采集回归，不是实际Windows取消
  或完整后代进程普查证明；累计ancestry／kernel birth观察边界保留。
- 格式整理`0/reaped/2.352304s`。当前777工程输入冻结r1，
  aggregate `22512505b86df8101b990974617593e869cdce55b7145b814b2bbab6ecfb4659`。
  r1与后续修订分别保留，不用旧冻结代签新输入。

## 门禁失败与修订

首轮完整门禁r1为`1/reaped/19.303036s`：Python主526 run／3 skip、格式通过，
严格Clippy因新增unit三处expect_used停止，Rust全量尚未运行。
三处改为具有相同timeout／future／说明及失败条件的assert，未改生产逻辑。
format-r2为`0/reaped/2.154196s`，strict session全targets Clippy
`0/reaped/9.784211s`。r2仍777输入，aggregate
`5add62bc9b71b90964b71ebdeb9927192a7bc2c29a03cb24dae0ac390a730df2`。

新非作者Turing the 2nd对r2五文件源码／功能限定APPROVE，P0–P3全0，已关闭。
独立64项完整性检查通过；自有r1预编译副本实际执行12项＝能力缓存10＋平台解析2，
0失败／0忽略、两个进程exit0/reaped。集成只独立核对库存、源码及父级原件，
未联网重跑；r2独立runtime为0，不能把r1副本当作新编译。
当前代spurious通知、多线程精确交错和CLOSE失败路径仅源码复核，未新增故障注入。
原一次r1→r2改动时的48 PASS／1 FAIL绑定检查保留，最终r2独立重算777项匹配。
63成员[审查原件](2026-11-17-monitor-read-cancellation-evidence/review/manifest.json)已封存；
两份67,079,424字节binary副本按原摘要核对后回收，唯一target未操作。

完整门禁r2为`1/reaped/516.102914s`：strict workspace Clippy已通过，
workspace在session lib的`traceroute_offline_process_contract_enforces_limits_and_actual_wait`
停止，421通过／1失败／4忽略。该旧工装内14Python测试的一项读取缺失PID文件失败；
当时只确认总status为timeout，未证明最终tool已启动，不从缺PID后验确定实际超时阶段。
此失败与原日志保留，不重试抹去。

仅修该生命周期测试：固定版本和数字IPv4解析前置输入，保留最终真实sleep30子进程、
生产supervisor／owned runner、原7.5秒期限及kill/wait，所有原断言保留，
额外核对reason／implementation／address。其他版本准入用例不变；
没有改生产诊断代码、延长期限或解禁真实工具。
直接Python14项通过，`0/reaped/9.685936s`。
r3与r2仅该Python文件不同，其余776项相等；777输入r3 aggregate
`6d495b849c63e00dfc7ba8b867f3fa8b5ba3481de9e4b66669dee77f40fa22d5`。
新的非作者Heisenberg the 2nd对该独立修订限定APPROVE、P0–P3全0，已关闭。
自有四件源码副本独立运行原14Python一次，14通过／0失败／0跳过，
driver自然exit0/reaped、11.559321s，最终11项完整性检查通过。
最终工具PID实际存活后消失，未独立捕获其exit；不把driver回收代签grandchild终态。
18成员[工装修订原件](2026-11-17-monitor-read-cancellation-evidence/fixture-review/README.md)
已封存，旧失败及具体超时阶段UNKNOWN保持。

## 最终完整门禁与库存

最终r3运行`python3 -B scripts/check.py`：
`0/reaped/709.145348s`，2026-11-17 UTC 07:17:28开始、07:29:17完成。
普通Rust3466通过／0失败／68忽略，另15rustdoc通过／0失败／0忽略、共6组；
Python主526 run／3 skipped／0失败、14.992s。包装／内嵌helper不重复加到主计数。
格式、严格workspace Clippy、依赖政策及readiness-error、
observer-io-error、controller-small-stack三个额外controller均通过。

96个普通Rust二进制的逐名库存与本次日志完全对应，前后文件摘要不变。
其中session lib为422通过／0失败／4忽略、426名；
Windows monitor集成为18通过／0失败／1忽略、19名。
两个harness=false controller没有libtest库存，依据实际执行单独核对。
库存工装只剥离三处可识别的内嵌Python unittest前缀，保留原日志及结果token，
没有重跑产品测试或改写结果。777工程输入最终前后相等，aggregate仍为上述r3。

本批没有新的正常应用包、GUI操作、Windows实际执行或发行；
原f910／5d2df2f正常包和唯一Cargo target保持。
Turing与Heisenberg的独立执行范围以原报告为准，父级完整门禁不当作独立重跑。

## 独立 peer 诊断

Aristotle 对固定 SexyZ 源码及维护者 WIP 协议静态核对，已定位空文件
在发送数据／ZEOF前被严格小于比较拒绝；父级另逐行读回该条件和协议上界。
三件源码与原准备基线摘要相等，协议同件摘要相等；没有执行 peer／新传输，
没有修改第三方代码或发送上游消息。8／10历史成绩及兼容OPEN保持。
回执仅封存自有分析、URL／摘要、HTTP、命令和选择读取的既有原件，
不归档下载的第三方实现或协议全文。

完整工程原件、独立复核和失败保存在
[本批封存](2026-11-17-monitor-read-cancellation-evidence/README.md)。
工程`6ae91d2e90668e7d9c04a2a7eea7c5de1bf6d3d4`已普通提交／推送，
2026-11-17 UTC07:38:37读回local／tracking／remote完全相等，工作区干净。
六归档540成员与777新输入／777旧f910输入精确Git绑定通过，
`0/reaped/0.325499s`。本批六位自有agent均已关闭。

按完整文件集和每件摘要回收六个自有展开目录、549文件／20,925,270逻辑字节；
其中540件有归档，另9件第三方正文／长摘录按已封存来源与命令摘要核对后回收，
没有将排除对象冒充归档成员。唯一target目录身份及两份正常包完整文件集不变。
67,079,424字节reviewer二进制副本早先已单独核对回收，不重复加到549文件。

GitHub连接器未提供本次所需的工作流读取工具，使用已有`gh api`只读GET。
2026-11-17 UTC07:38:44精确新源码CI：
Quality44040248864、Windows API44040248806、Windows OpenSSH44040248746
均in_progress；不预填完成或成功。前批CI只签f910。
推送、绑定、自然退出与清理回执见
[交付封存](2026-11-17-monitor-read-cancellation-evidence/delivery/README.md)；
已回收work目录不再作为维护入口。不把局部检查扩成原生验收。

## 精确 CI 终态与封存 — 2026-11-17

仅补签工程 `6ae91d2e90668e7d9c04a2a7eea7c5de1bf6d3d4` 的既有 CI 原件：
Quality `44040248864`、Windows API `44040248806`、Windows OpenSSH
`44040248746` 均 attempt1、completed/success。独立复核于 UTC08:08:28
首次 GET 取得新 Quality 终态，原 UTC07:38:44／07:56:50 pending 仍保持原记录。
三平台 job 的原 run／单 job／列表 metadata、checkout 最终 SHA、ZIP、
逐名库存和四个官方 artifact digest 已由本次封存父级另行核对。

| Quality 平台 | 普通 Rust passed／failed／ignored | rustdoc | Python 主 run／skip |
| --- | --- | --- | --- |
| Windows | 3453／0／66 | 15／0／0 | 526／53 |
| macOS | 3466／0／68 | 15／0／0 | 526／3 |
| Ubuntu | 3459／0／69 | 15／0／0 | 526／3 |

每平台 96 普通套件和 6 rustdoc 组逐名／声明／结果相等；Python 主 quiet
discovery 仅核对总数。preflight／packaging／内嵌 Python／controller 单列。
macOS／Linux 实际 OpenSSH 各 18、forwarding 各 6 通过。
Windows API lab111 run／3 skip、capture replay1通过；
SSH lab96 run／1 skip、真实 SSH Rust1通过。
独立 Parfit 限定 `APPROVE_CI_EVIDENCE_ONLY`，3318 离线检查通过、0 失败，
P0–P3 全0，两作者已关闭；没有独立产品重跑。

两个来源完整安全文件集 195＋254＝449 成员、13,052,356 逻辑字节全部封存，
包括原两份 SHA256SUMS、失败 helper／回执及 pending 快照，排除0。
7 个原始 ZIP／172 成员与4个官方 artifact digest 相等。
封存包 2,900,131 字节，SHA-256
`0a2d2b7908ca447718542503582d35589c9f2aaeb89dd8926f9652a32ce52e2a`。
父级来源复核8004项及归档读回复核8408项通过、0差异，后者自然
`0/reaped/0.254373s`；旧工程777输入与精确Git对象绑定相等。
可离线工具、逐件manifest、原件和具体边界见
[精确终态封存](2026-11-17-monitor-read-cancellation-evidence/ci-completion/README.md)。

SSH 原 seal HOLD／terminal_proven=false 保持；非master自然 exit0×14、
255×5、1×1，master cleanup125 非自然终态、owner observed_exit=null 保持。
API isolated SSH skipped、direct API ssh=UNRUN、两份 native_gui=UNRUN 保持。
此为旧监控取消工程的 CI 证据，不代签当前 MCP 滚动改动、当前原生矩阵、
完整产品或发行。本子任务未运行 Cargo／产品／CI、未读取或修改 target 产物、
未 Git 写入／推送或清理来源；父级在准确本地提交后运行
`ci-completion/verify.py --commit FULL_40_HEX_LOCAL_COMMIT` 再处理来源回收。
