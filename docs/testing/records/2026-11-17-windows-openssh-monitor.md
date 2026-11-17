# Windows OpenSSH 真实监控验收 — 2026-11-17

状态：本机完整门禁、独立源审与修复已推送；
修复工程660e90d6的两次真实Windows OpenSSH、新直接API及三平台Quality已成功，
非作者原件复核限定批准。原首轮FAIL与未知子条件保持；原生桌面或发行分别验收。
基线工程 `bfbea7bcc84ff433d28a635e85daed9cca337fb0`。
设计见[ADR0157](../../adr/0157-windows-openssh-monitor-acceptance.md)。
上一批5d2df2f的真实Windows直接API及三平台质量门禁保持原范围，
见[上一批记录](2026-11-17-windows-read-only-monitor.md)。

## 本批工程分工

- 父负责隔离配置、默认shell观察、真实SSH编排、独立Win32对照与CI。
- Aquinas负责调试创建／原对象／Job终态及其Python专项。
- Galileo负责真实公开Rust SSH／monitor路由及结构化回执。
- 所有写集互不重叠，当前checkout共享，作者不运行Cargo或整仓门禁。
  唯一target归属与此前相同，未切换、不clean。

## 已实际观察

| 范围 | 结果与边界 |
| --- | --- |
| 父级配置／接纳初轮专项 | `ssh-parent-unit-r1`，exit0/reaped，26项通过，0失败、0跳过，外层0.289769s。包括一次实际宿主子进程的显式环境控制；不是Windows执行。此后集成变化需另验，不移签原范围。 |
| 父级配置／报告／清理专项 r2 | `ssh-parent-unit-r2`，exit0/reaped，39 run、0失败、0跳过，外层0.303391s。补齐客户端报告、清理后的接纳集合与PARTIAL状态控制；均为本机离线协议测试。 |
| Rust真实用例库存 | `windows-ssh-rust-inventory-r1`，exit0/reaped/2.685961s，实际编译后逐名读回 `actual_windows_openssh_monitor_matches_owned_fixture`，1 test。显式ignored，未在本机冒充Windows运行。 |
| Rust目标严格Clippy | `windows-ssh-rust-clippy-r1`，exit0/reaped/12.889656s，`-p keelshell-session --test windows_openssh_monitor -- -D warnings`；仅编译与静态检查。 |
| Rust非Windows负向控制 | `windows-ssh-rust-nonwindows-r1`，实际调用ignored用例，以`WindowsRequired`退出101/reaped/0.748447s，0通过／1失败。该失败符合平台边界预期，原失败保留，不记作Windows执行或普通门禁失败。 |
| 集成Windows相关Python专项 | `windows-ssh-python-integration-r1`，exit0/reaped/8.922646s；`test_windows_*.py`实际271 run＝268通过＋3跳过，0失败。包含既有Job／lab回归、调试接纳与真实宿主子进程控制，未代签Windows ABI。 |
| 调试owner作者最终专项 | r7实际49 run＝48通过＋1 native跳过，exit0/0.122686s；原r1超时exit124/60.005464s保留。作者Cargo、全仓门禁和Windows实际执行均0，已实际关闭。 |
| 最终完整门禁r1 | exit0/reaped/699.028665s，94普通Rust套件3407通过／0失败／60 ignored，14 rustdoc通过；app1398/0/2、session404/0/4。Python主discovery493 run＝490通过＋3跳过；格式、严格Clippy、依赖x.y策略及三组独立controller均通过。 |
| 独立编译库存 | app1400、session408、Windows routing10、真实Windows SSH用例1；分别逐名核对最终gate结果，完全相等，新真实用例在普通门禁中明确ignored。routing首轮误写不存在的target名退出101、未执行测试，原命令错误保留；改为实际`windows_monitor`后r2退出0。 |
| 官方Windows原文 | 7个learn.microsoft.com直连均URLError；原失败保留。GitHub官方MicrosoftDocs内容GET成功3份，1条错误路径404。原Markdown与文件blob／SHA回执用于设计核实，不是运行验收。 |
| 依赖新鲜度 | `dependency-freshness-r1` exit1/reaped/159.629281s。37个registry包、113声明／使用，19行通过、94行registry_check_failed；完整新鲜度仍UNKNOWN，不合并旧部分响应。未改Cargo依赖或锁文件。 |
| 首轮本机门禁时的原生边界 | 当时本批Windows尚未执行；推送后的精确CI在下文另记。CUA最新原生通道返回Mac锁屏且自动解锁失败，操作／截图0；旧直接API与replay不迁移为本批真实SSH成功。 |

首轮开发原件从[本机封存](2026-11-17-windows-openssh-monitor-evidence/README.md)读取；
两位作者、两位首轮reviewer、研究副本及stage已经按原摘要回收。
官方原文只保留固定Git blob、出处及摘要绑定，已回收路径不作为当前维护入口。

新非作者Chandrasekhar审查Rust／SSH编排，Confucius审查调试接纳／清理，
均已限定APPROVE，P0–P3全0并实际关闭；不与作者身份混用，
独立Cargo和Windows执行均0。Chandrasekhar实际两轮各39项Python通过、
2项编排故障控制、14项静态契约和12项原件检查通过，并独立复算765输入；
Confucius实际6次专项合计173 run＝170通过＋3跳过，0失败／错误，
其中仓库169项、独立续接控制4项。不同轮次不相加声称唯一用例总数。
父发现并已修复
debug owner关闭异常可能跳过管道收口与子进程回执保存的失败路径，
新增控制验证失败状态、双管道原件和后续stream关闭均保留；
两位非作者均窄复核关闭该项。

`_refresh_original` 的早退在离线early-signal注入下已独立续接留证；
官方调试终态语义未证明该顺序在正常受管Win32链中可达，故不计确认平台缺陷。
注入后自然seal保持HOLD，退出码不一致也不能成为terminal_proven；
将来若真实Windows出现同序，保留原句柄／事件并继续HOLD后修复，不放宽验收。

最终完整门禁r1在两位源码批准后执行并收取exit0终态。冻结765输入，
aggregate `71872297a9a7b067fd85723de4e5c9c10f908dcc6a7e63ab91eeed6cb902db3a`，
与候选、门禁后和库存后输入逐项相同。唯一target归属未切换、不clean。

本机原件已封存到[本批证据](2026-11-17-windows-openssh-monitor-evidence/README.md)，
按成员摘要、最终测试日志／库存、两位reviewer及完整Git输入独立重算。
该只读重算不重新运行产品测试，也不签Windows原生成功。

## 推送与精确工作流

工程 `41671be1a7fdfd96cec3a57f876519aeab2fa0d5` 已普通提交／推送，
`git-push-r1` exit0/reaped/4.358905s；本地／tracking／远端三方完全相等，
读回工作区干净。完整765工程输入及168原件的Git绑定重算
exit0/reaped/2.410740s，未重新运行产品测试。

本批真实OpenSSH工作流 `43990824640`、直接API `43990824533`、
Quality `43990824137` 的API读回均为同一精确head_sha。
直接API已success，34成员artifact14287416838／46,698字节ZIP与GitHub digest相等，
receipt为PASS；原手动前置SSH job仍skipped，不将其升级为真实SSH成功。
Quality已三平台completed/success，原完整日志与首轮SSH失败、API成功原件
一并保留在[修复封存](2026-11-17-windows-openssh-monitor-evidence/repair/README.md)；
它只签工程41671be，不代签后续四文件修复。

## 真实OpenSSH首轮失败与修复候选

`43990824640` 的原job133730641759已completed/failure。
原artifact14287312342和完整job日志已下载，GitHub ZIP digest相等。
Rust真实用例实际PASS：错误pin拒绝、正确认证、snapshot／processes／sockets各一次，
四个自有双栈端点、两项connected-only排除、工作集及独立资源对照完成。
原Windows身份为AMD64／10.0.26100／PowerShell5.1.26100.33648。
原接纳包含7个sshd、5个cmd、5个conhost和4个PowerShell，共21；
20个非master自然终态，四个PowerShell均真实exit0。

整体验收仍FAIL：`debug_admissions_job_census_incomplete`及后续
`direct_termination_failed`保留；admission_closed和terminal_proven均false，
根receipt清理失败，API保持PARTIAL。最终Job active0、21原对象均signaled、
0 pending对象／自有handle／debug连接、closed true不能替代缺失的完整census。
旧实现没有记录首次失败的before／PID列表／after，故确切失败子条件仍UNKNOWN。

候选修复只在原3秒／总deadline中重泵事件并重取census；
自然seal仍须完整累计／当前集合匹配、全部非master自然退出、
仅master存活和封接纳前后复核。异步终止中的不一致保持active>0，
封闭后累计total改变立即失败，不把未知对象或无终态转成PASS。
增加前16个不完整观察及最后观察，按独立快照保留计数和PID，不输出凭据。

实际cmd在`powershell.exe`与首参数之间有双空格；该输入旧validator也会拒绝。
候选只归一化argv0边界空格，固定选项和Base64仍逐字节精确，
参数内部额外空格、注入和多余后缀仍失败。原命令行本身不改写。
新增8项控制后受影响SSH专项96 run＝95通过＋1 native跳过。
候选765输入aggregate
`fb78496b8d0b22bd6fc38da46df994d16c144d1f040f83ff468a98c49ff633af`，
新非作者Copernicus已限定批准、P0–P3全0，独立36个仓库风险用例与13个
自备控制共49项通过、0跳过，实际Windows执行0，已实际关闭。
其批准只覆盖四文件增量；原两位非作者批准保持首轮基线范围。
Windows相关Python整合专项实际279 run＝276通过＋3跳过，
`0/reaped/8.971765s`。

关闭发现并冻结后的最终完整门禁r2已`0/reaped/731.529731s`：
94普通Rust套件3407通过／0失败／60 ignored，14 rustdoc通过，
app1398/0/2、session404/0/4；Python主501 run＝498通过＋3跳过，
格式、严格Clippy、x.y策略和三组controller通过。
门禁前后765输入逐项相等；相较工程41671be只有上述四个Python文件变化，
Rust／Cargo输入不变，原独立编译库存与新gate逐名重核，不重复运行未变的库存命令。
本机修复证据独立封存，首轮168成员archive不改写。
修复工程`660e90d6f7c7111853db7b73f4ca2344f633700d`已普通提交并推送，
`git-push-repair-r1`为`0/reaped/4.193711s`；local／tracking／remote三方相等，
读回工作区干净。50成员修复封存与765输入的精确Git绑定
`0/reaped/2.714595s`，本修复不升级首轮失败。

精确真实Windows SSH工作流`43997403565`及同源码独立workflow_dispatch
`43998970900`均completed/success，直接API`43997403484`也success。
首个SSH成功artifact14290995772为46成员／54,256字节，ZIP与GitHub digest相等；
原seal累计21、仅master active1，20非master自然终态、四个PowerShell自然exit0，
全部固定生产命令与精确Git来源相等。最终Job active0、原对象集合不变、
terminal_proven／closed为true，pending／handles／debug连接均0，临时私钥目录删除。
同源码复跑43998970900的artifact14291880367同为46成员，53,967字节ZIP；
两次均21原对象、20非master自然终态、四个PowerShell自然exit0，完整seal及终态清理。
两次不完整census观察数均为0，不以新成功推定原失败就是race。
新非作者Darwin限定APPROVE，P0–P3全0，4018离线检查通过，
`0/reaped/0.976s`，已实际关闭；原生新执行与产品测试重跑0。
该复核覆盖6 ZIP、126 artifact成员及36日志成员，不覆盖Quality。
首轮非作者Faraday也限定批准原工程41671be的CI原件，P0–P3全0并已关闭；
他所供的首版两个interop artifact只有metadata，ZIP内容未代签。

精确Quality43997403533现已三平台completed/success：

| runner | 普通Rust passed／failed／ignored | rustdoc通过 | Python主run／skipped |
| --- | --- | --- | --- |
| windows-2025 | 3394／0／58 | 14 | 501／38 |
| macos-26 | 3407／0／60 | 14 | 501／3 |
| ubuntu-24.04 | 3400／0／61 | 14 | 501／3 |

三平台各94普通套件，声明／逐名／结果摘要一致；rustdoc为14项测试、6个子运行，
不称为14套件。包装与内嵌helper单列，Windows前置专项不重复相加。
新Quality的macOS／Linux真实OpenSSH interop各18项通过并清理，
本次两个7成员ZIP均实际下载、核对GitHub digest，不迁移首版未核验边界。
Python stdout／stderr交错使首次父级统计解析停止，错误及修正保留；
采集器诊断行不改变同一门禁步骤内唯一Ran／OK配对。

新的[精确CI封存](2026-11-17-windows-openssh-monitor-evidence/ci-completion/README.md)
保存510成员／2,555,157字节，SHA-256
`106af79167ef5a942b45f9bd12a3a346bc7328635670fc2a3c0dbed3fdb5e9ce`。
包括两次修复SSH、新API、Quality原ZIP与元数据、两位CI原件reviewer、
推送读回、工装失败及8位自有agent关闭记录；原首轮168与修复50成员archive不改写。
离线复核同时绑定765工程输入；不重放产品测试，不代签桌面或发行。

首轮封存已绑定工程Git后回收6个自有关闭作者／reviewer／研究／stage目录，
302文件／3,173,304字节均先核对原摘要；正常包、唯一target、当前CI原件保留。
完整产品目标继续进行。
