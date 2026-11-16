# Windows 远端只读监控 — 2026-11-17

状态：本批源码整合、本机完整门禁和两位独立非作者限定复核通过；
工程 `5d2df2fc5ac8566fe11182b7f4337076296dfa54` 已普通推送、三方一致。
精确 Windows API workflow及三平台Quality均已completed/success。基线
`4209e272260aff6374e6216423ce8e0157bea2f4`。
目标及数据合同见 [ADR 0156](../../adr/0156-windows-read-only-monitor.md)，
用户说明见 [Windows 监控](../../product/WINDOWS_MONITORING.md)。
源码、受控 SSH、实际 Windows API、真实 Windows OpenSSH、原生桌面及发行分开。

## 实现

会话层增加连接级单飞身份探针、固定 Windows PowerShell 采集、严格 JSON 协议及
按项缓存。CPU 百分比与累计计数分型，缺失数据用 Option 表达。界面提供来源、
当前点／断段、工作集与只读提示，保留三主题 token、双语和缓存分页。
Linux 动作必须由当前认证 Linux 身份及 Linux 样本共同授权。

三个作者分别负责采集解析、GPUI 和 Windows 工装，已收取报告并实际关闭；
父负责公共类型、连接权威、集成及最终门禁。两位新的非作者已完成独立复核，
均限定 APPROVE、P0–P3 全 0。
唯一 Cargo target 始终属于本 checkout，未切换或 clean。

## 已观察的开发结果

最终原件见 [540 成员封存](2026-11-17-windows-read-only-monitor-evidence/README.md)。
当前正常包及必要接续工装在 `work/windows-monitor-delivery-20261117/`；
原集成／reviewer／oracle／stage 副本已在精确 Git 核对后回收。

| 运行 | 实际结果与处理 |
| --- | --- |
| 首次 session 编译 | exit 101，旧 Darwin 单测直接访问 CPU ticks；适配新计数类型后第二次编译 exit 0。 |
| `session-batch-r1` | exit 101，197.250s、已 reap。library 391 passed／12 failed／4 ignored；另 remote-only 4 passed／1 failed，其余已执行套件无失败。Windows 路由 6 passed／1 ignored。 |
| 旧监控失败修复 | 旧 SSH 夹具补独立平台探针，保留完整采集／不信号／缓存断言。reset 后迟到结果加强为 Cancelled；新增双向平台帧不一致拒绝例。 |
| `session-monitor-regression-r1` | exit 0；只签该次监控过滤范围，附带 remote-only 过滤后 0 项不算其验收。 |
| `remote-only-regression-r1` | exit 0，5 passed；测试文件显式 test-only cfg，不放宽架构 guard。 |
| 首次 app 编译 | exit 101，新测试缺少 SocketInfo 导入；补齐后第二次编译 exit 0。 |
| `app-inventory-r1` | exit 0，编译库存 1400 名，monitor 101 名，其中 Windows 23 名。 |
| `app-monitor-regression-r1` | exit 101，41.662s、已 reap，98 passed／3 failed。两项测试时点问题、一项 Windows 资源权限失败未负缓存的真实行为缺口。 |
| `windows-routing-regression-r1` | exit 0，7 passed／1 ignored，权限负缓存新增例通过；不含之后的通道 admission 修正。 |
| `app-monitor-regression-r2` | exit 0，101 passed／0 failed／0 ignored，60.210s 含编译；23 个 Windows 名均实际执行。 |
| `channel-admission-regression-r1` | exit 0，20.670s、已 reap；session library 404 passed／4 ignored，Windows route 9 passed／1 ignored。两个新增延迟 channel-open 测试覆盖两类探针／三类缓存采集与 pause/reset 的十场景，撤权后 exec 增量 0。内嵌 Python helper 30 项另列。 |
| `linux-fixture-regression-r1` | exit 0，28 passed／0 failed／0 skipped，固定探针逐字节对齐与追加命令拒绝例通过。 |
| `darwin-openssh-r1`／`r2` | 均 exit 0、各实际 15 passed；r2 覆盖通道授权修正后实现，5.455s，服务／临时密钥／自有进程清理确认。两轮不合并为 30 个独立用例。 |
| `linux-recovery-r1` | exit 1，8.972s；测试服务 ForceCommand 未放行新增固定身份探针，保存 Unknown 原观察和原失败；本轮所有自有资源清理确认。 |
| `linux-recovery-r2` | exit 0，19.020s，两个真实 OpenSSH 用例通过，包含 df／ps／ss 九类降级／缓存／修复／显式恢复，以及传输失败分类。输入前后不变、清理确认；参数 source SHA 是基线 HEAD，实际差分另由冻结与 binary hash 绑定。 |
| 新 Python 工装初版 | 作者专项 70 passed／0 failed／0 skipped、exit 0；原 r1 failure 与 r2、r3 结果均保留。该版对稳定资源漏报的独立校验不足，不能作为最终 oracle 验收。 |
| oracle 遗漏修正 | 作者完整新 suite 111 passed／0 failed／0 skipped、exit 0，外层 2.663933s；四项新遗漏断言对保存旧实现均以 `LabError not raised` 失败，0 passed／4 failed，exit 1。原观察／失败哈希未变。 |
| `full-gate-r1` | exit 1、24.629160s、已 reap；Python 405 run／2 skipped、fmt 通过，严格 Clippy 停在两处 session 测试解码 helper 的 `chunks_exact_to_as_chunks` 告警。workspace tests 尚未执行。 |
| `full-gate-r2` | exit 1、32.500738s、已 reap；Python 405 run／2 skipped、fmt 通过，Clippy 继续发现 app 同类 helper。三处改用 `as_chunks::<2>().0.iter()`，不增加 allow、不改断言；完整原失败保留。 |
| `normal-release-build-r1` | exit 0、154.466855s、已 reap；正常 app／MCP。构建开始至最终 r3 冻结仅三处 test-only helper 和工装 README 变化，生产源码、嵌入 ps1、依赖和资产不变，不因纯测试调整重建正常程序。 |
| `package-r1`／`package-inspection-r1` | 均 exit 0、已 reap；正常 arm64 包结构及 minos 15.0 通过。未启动、未安装、未签名／公证，不代签原生。 |

Windows 权限失败缺口修复后，已确认 Windows 路由的非传输资源失败等待显式重检；
既有 POSIX 瞬时资源失败继续按原合同重试。新增 session 用例检查零重复执行、
原检查年龄、clone 共享、认证报告失效及显式恢复，专项已通过。
旧 Linux held 夹具将请求计数放在实际挂起前，未知环境提示断言放在暂停覆写状态前。
原失败原件不改写成通过。

非作者另发现通道打开等待后的发送授权缺口与 Windows 独立对照遗漏缺口。
前者修复为 crate-private guarded exec，在 channel-open 获准后、exec 发送前核对
generation；所有只读操作使用同一入口，已发送的 SIGTERM 保留原回执。
后者已补独立资源枚举和遗漏反例，最终非作者复核已批准。
oracle 前后独立枚举 IP-enabled 接口及已就绪固定卷，不接收生产 mounts；
稳定身份必须全部被生产观察覆盖，全部漏报、部分漏报、等行数替换均拒绝。
接口／卷增删或身份变化、计数器重置、观察不完整为 HOLD。
容量 skip 只豁免容量，合法完整空数组的 parser 合同保持。
Linux 测试服务只新增完全固定的真实 uname 探针及独立计数，未放宽任意 shell 白名单。
两轮真实 Linux 工装的派生镜像／容器／私有 network 和临时密钥均已清理；
父级新建 base image 也已按精确 ID／owner／零 consumer 检查后移除，并读回不存在。

app 编译出现 Apple linker compact-unwind 容量 warning，原输出保留；
编译成功不等于严格 Clippy、完整测试或原生窗口成功。
父级专用 prebuilt 的独立功能复核会明确记录独立编译 0，
不得把源代码摘要或父级结果替代该复核的实际执行。

## 依赖与真实平台

默认环境 Rust/Cargo 为 1.100.0；项目不 pin toolchain、不设 rust-version。
清单仍为 x.y；zip 锁文件从 9.6.0 更新至实际查询到的 9.6.1。
三轮官方完整依赖查询未全通过，最后一轮有界 49 请求仍缺六个包；
部分已解析官方 metadata 与请求记录保留，不能拼接成单次完整新鲜度通过。

本机为 macOS，没有直接执行 Windows API／cmd／PowerShell；
后续精确 hosted runner 的实际通过见下节，不改写本机执行范围。
Windows API 与 SSH 前置检查分开；SSH job 仅手动触发，当前明确 HOLD，
不产生真实 Windows SSH 成功声明。
本次以 `cua.getState` 重新观察仍为锁屏且自动解锁失败，本批 GUI 操作／截图 0。
后续精确 CI、正常包与交付会分别补记。

## 独立复核与资源回收

Bernoulli 对 r3 的 44 个输入限定源码 APPROVE，P0／P1／P2／P3 均 0；
独立 Cargo、实际测试、远端操作均 0。源审发现的 SSH admission P2 已在该范围关闭。
Gauss 对 app／受控 SSH／oracle 最终限定 APPROVE，P0–P3 均 0：

| 非作者实际执行 | passed / failed / ignored 或 skipped |
| --- | --- |
| r3 prebuilt app monitor | 101 / 0 / 0；23 Windows 与 78 既有名，独立总库存 1400，101 逐名相等。 |
| r3 prebuilt Windows routing | 9 / 0 / 1；库存 10 逐名相等，两项 delayed-admission 回归均 ok。 |
| 最终 oracle Python | 111 / 0 / 0，最终两份脚本 hash 与实际执行版本相同。 |
| reviewer 自备控制 | 11 个预期全部成立：5 个遗漏拒绝、3 个合法通过、3 个不确定库存 HOLD。 |

Gauss 独立重算最终 758 输入、aggregate 完全匹配，mismatch 0。
prebuilt 成绩绑定三处 helper 调整前的 Rust 二进制；最终三处等价 delta
由其静态检查，独立编译仍 0，父级完整 gate 负责重编与执行。
不将旧 prebuilt 的动态成绩移签为最终 helper 的独立动态验证。

三位初始作者、oracle 修正作者、两位非作者及上一 CI 封存 agent 均已实际关闭。
六份自有专用测试副本按原绑定 SHA 核对后回收，共 886,506,992 字节；
所有路径读回 absent，共享 target 目录身份与正常包 manifest 不变。
真实 Linux／Darwin 自有服务、密钥、容器／网络／镜像的原清理回执保持。
未新建用户 chat、worktree 或分支，其他任务资源保留。

## 最终冻结与完整门禁

r3 为 758 个工程输入，aggregate
`6759f672ae1efb83f6a400145109ee9bdc75ed192e2b7647813a252a1eded210`。
父在前两轮命令实际 reap 后修正 helper、fmt 并重新冻结，再执行
`RUST_MIN_STACK=2097152 python3 -B scripts/check.py`。期间不修改工程输入，
文档不在工程冻结中。最终 `full-gate-r3` exit 0、813.820404s、已 reap；
原子进程 active 0、pipes drained、无 cleanup error。依赖 x.y 策略、Python、fmt、
严格 workspace/all-target Clippy、完整 workspace 和三个 controller 均通过。

93 普通 Rust 套件实际为 **3407 passed／0 failed／59 ignored**；
GPUI app **1398／0／2**、session library **404／0／4**、Windows routing **9／0／1**，
rustdoc 14／0／0。全部声明数、唯一逐名结果和摘要相等。
Python 主 discovery **405 run／403 passed／2 skipped**，内嵌 helper 30 项另计。
精确 gate app／session／routing 二进制各自独立 --list 为 **1400／408／10**，
与实际执行逐名相等；23 个 Windows app、44 个 Windows parser 和两个延迟 admission
回归均 ok。二进制摘要清单前后不变，758 工程输入前后逐项相等。

正常包提交前 manifest 的 commit 为 null，来源是上述生产差分。
app SHA-256 `127526280ffc1409abe9f6519c0f8d5818c4fedf66a22468737bd8e0217e35bf`，
MCP `bb415f3ae3ae7ca62bdb78abe752b50f5a3a796dc65ec99490f54ee28cb3f693`。
后续精确提交来源包需重建 manifest，并核对全部五个应用文件逐字节相同。

最终 540 成员／758 输入的工作区只读重算 r2 实际 exit0、0.249717s、已 reap，
不重放测试。首轮封存误将 `package-`／`prebuilt-` 文件名当目录排除，
verifier 实际因缺 `package-r1.json` exit1；原候选、manifest 和诊断完整保留。
仅改目录过滤后生成新 stage 与最终封存，不覆盖原件或复用失败候选。

## 远端交付、精确来源包与后续清理

工程已普通提交／推送 `5d2df2fc5ac8566fe11182b7f4337076296dfa54`；
push `0/reaped/5.253968s`。本地、tracking、远端分支三方读回一致，工作区当时干净。
540 成员／758 输入从该提交真实 Git blob 重新校验，
`0/reaped/2.856026s`、mismatch 0，不重放产品测试。

精确正常包为 `work/windows-monitor-delivery-20261117/package-r2/KeelShell.app`。
manifest 的 source commit 为上述工程，全部五个应用文件与提交前包逐字节相等；
arm64／minos15.0 结构检查 `0/reaped/0.383260s`。
包内 app／MCP 摘要与上节相同，没有启动或覆盖安装。

核对精确 Git 封存及替代正常包后，再回收 1,968 文件／106,004,384 字节及七个
自有目录，包含原集成、源码／功能复核、oracle、两份 stage 和上一 CI 活动副本。
9个原辅助工装／报告已纳入后续69成员交付封存。
当前包六文件摘要和共享 target 目录身份前后相同，没有 clean target、清理他人资源，
或创建用户 chat／worktree／分支。本批此前六份专用二进制另计，不重复计入这 1,968 项。

## 精确 Windows runner 实际采集

workflow **43972365007** 已 completed/success；direct job **133667526984** 成功，
isolated-ssh job **133667529467** skipped。来源均为上述精确工程。
平台为 Windows Server2025／10.0.26100／AMD64，Python3.15.2、
Windows PowerShell5.1.26100.33648；不能扩大为其他 Windows 版本或 arm64。

artifact **14279316541**：46,825 字节 ZIP，SHA-256
`b2afa96f9ada2ae38d8e1d21b8b05587db512da3b43f0ba4ae071aa0a2914af9`，
与 GitHub digest 完全相同。34 原件完整保留，父级只读重算观察到：

- 五类实际 `cmd.exe → powershell.exe` 命令与本批生产源码装配逐字节相等；
  资源两种命令 7954 ASCII 字节，序列化完整 cmd 为 7993 字符。
- 五次 PowerShell 原对象均 live 时保留句柄、自然终止／exit0、原始 census 完整；
  每组 active0、pipes drained、admission closed、pending objects0，无 cleanup error。
  自有 fixture 也已 exit0、reap、active0。
- 四个自有 IPv4／IPv6 TCP 监听／UDP 端点全部匹配，两个 connected-only TCP 被排除；
  工作集差值0；独立枚举对照覆盖3稳定接口、2固定卷。
- 容量 skip 仍校验网络与核心资源；CIM CPU 与独立累计计数采样窗口不同，
  差值仅作信息，不宣称数值完全相等。

该 run 的实际生产采集、独立 Win32 对照及认证路由 capture replay 步骤均成功。
完整 job 原日志及逐名 replay 结果随三平台终态一并封存；不将 runner API、
原对象终态或 replay 当作真实 Windows OpenSSH、原生桌面或发行安装验收。
该job的Python实际 **111 run＝108通过＋3跳过、0失败**；3项是non-Windows负边界。
指定capture replay为1通过／0失败／0忽略、9 filtered out，不能泛称111项全部执行。

## 精确三平台 CI 终态与额外原件复核

**Quality43972364934** 的三个job均completed/success，全部绑定工程5d2df2f。
Windows／macOS／Ubuntu普通Rust为 **3394/0/57、3407/0/59、3400/0/60**；
app为1387/0/2、1398/0/2、1399/0/2，session lib为445/0/3、404/0/4、401/0/5。
各93普通suite及6个rustdoc subrun逐名／声明／摘要一致，rustdoc各14/0/0，
Windows routing各9/0/1。主Python各405 run，passed为368／403／403、
skipped37／2／2；包装57 run和内嵌helper分别计数。
Windows三个预检成功，macOS／Ubuntu标准OpenSSH及回执保存成功。

CI观察器只做221次有界GET，17次不同状态观察后自然终态，
`0/reaped/2550.580516s`；没有手动重跑、取消或远端变更。
最终原job日志、原API响应、34个Windows成员及原ZIP等
[91成员封存](2026-11-17-windows-read-only-monitor-evidence/ci-completion/README.md)
逐项校验通过，758工程输入绑定固定Git blob，`0/reaped/2.423535s`，
没有重放产品测试。原部分派生摘要不改写，最终结果另行完整派生。

新非作者Hypatia仅审指定API原件和capture replay：41输入／357,443字节、
11相关Git源码，102项离线复算断言通过，限定APPROVE、P0–P3全0。
首轮复算CRLF匹配错误及一次路径读取错误均保留，原报告／receipt字节不改。
独立Windows API执行、测试重跑、Cargo、target、远端、GUI、Git写均0；
未签Quality或真实SSH。复核者已实际关闭，见
[交付原件](2026-11-17-windows-read-only-monitor-evidence/delivery/README.md)。
再次CUA观察仍锁屏、自动解锁失败，GUI操作／截图0，当前正常包未启动。

本次后续提交仅更新文档和证据，758工程输入保持原冻结，
不重复未改变Rust全门禁。完整产品、真实Windows OpenSSH、原生桌面与六目标发行保持开放。
