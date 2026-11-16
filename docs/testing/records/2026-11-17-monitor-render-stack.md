# 监控渲染栈边界 — 2026-11-17

状态：最终r2宿主限定门禁、两位独立复核、精确Git绑定和正常来源包结构通过，已普通推送。
精确工程`bbd4eacc…`的修复后三平台CI已成功并由本地原件派生封存；
完整产品、当前dirty Windows监控实现、原生及发行仍开放。
基线`262a642605ffdb52ef4d6bef075aef17cc910149`；
设计见[ADR0155](../../adr/0155-monitor-render-stack-boundaries.md)。

## 原始失败与收取

原工程`16703293012d798f885e1590f02c348c47e81a10`的
Quality43945988599已completed/failure，watch
`1/reaped/1959.911713s`，无活动watcher；没有取消或重跑。
macOS133575666472完整gate及标准OpenSSH成功，普通Rust3325/0/58 ignored、
app1375/0/2、14rustdoc、Python293/2 skipped。

Ubuntu133575666946与Windows133575666982在完整app中栈溢出：
前者SIGABRT，声明1378、已观察696 ok／2 ignored、680未报告；
后者STATUS_STACK_OVERFLOW，声明1366、已观察688 ok／2 ignored、676未报告。
两平台均没有完整app摘要；后续OpenSSH未执行。
[37成员终态](2026-11-17-darwin-udp-sockets-evidence/ci-completion/README.md)
保存原job日志、失败下载和partial派生；重算
`0/reaped/1.964279s`、729个精确Git输入零mismatch。
完整parser拒绝中止输出、Windows路径分隔符和tuple／JSON表示诊断均保留，
只修派生工具，不修改原测试、输出或失败事实。

## 实现、冻结与原件

本片只改`monitor.rs`，新增私有`monitor_sockets.rs`和`monitor_view.rs`。
所有分区从主渲染函数移入独立`AnyElement` builder，确认区保持optional。
没有变更测试、预算、依赖、采样或审核授权。
同一checkout连续使用唯一Cargo target，未切换归属或clean。

r1仅抽端口231行／10,905字节，730输入aggregate
`271b4419cd3e10fe825bbfa29d4a53882c64bd1f8890ad09b024c17f40679118`。
实际编译和2 MiB监控专项78/0/0通过；Mill限定静态批准。
父后续本机prologue观察显示主render仍约1.19 MB、端口helper约0.383 MB，
组合余量尚未证明，继续拆分其他分区。
限定gate-r1主动SIGINT，为`130/reaped/351.198864s`、observed_exit=null；
fmt、Clippy、策略已过，app未跑完，不能签为完整成功。

最终r2增加其他六块681行／29,809字节的机械提取；
731输入aggregate
`2ea283af87af7e0af2fa912ebc28f9360e0c5113cd40e1111502f8cdd6d8551e`。
实际编译`0/reaped/12.187367s`，没有沿用旧源码的binary摘要。
主render的本机arm64 debug局部栈预留由1,576,336降为70,992字节；
完整调用树峰值和其他平台未由该观察测得。
初次符号子串筛选混入adapter而发生同标签文件覆盖，原工装结果保留；
按精确符号和独立文件名生成的r3八份prologue才是最终观察依据。

## 最终限定门禁

`scoped-gate-r2`为`0/reaped/354.196674s`：
fmt、严格全workspace/all-target Clippy、x.y策略及完整app通过。
app在显式`RUST_MIN_STACK=2097152`、4测试线程下
`1375 passed / 0 failed / 2 ignored`，harness345.67s。
实际gate二进制的独立1377名库存与声明／逐名运行结果相等；
两个原平台失败名及UDP三名均ok。工程输入在gate后和release后逐项相等。

实际gate app SHA为
`805d56eaff8546ada4906971a660b2739d50a9fd8ba28afdb437d4f3d15f6190`。
本片门禁范围为应用及编译／策略检查；没有把原3325项workspace运行移签到新731输入。
依赖／lock未改；最新版本在线复查继续UNKNOWN，确定性x.y策略独立通过。

## 独立复核与回收

Mill最终r2静态APPROVE，P0–P3全0；19条记录命令，独立编译／测试／target读取0。
六块正文和主装配可完整重建；一项reviewer静态诊断的错误假设与修正保留。

Hubble最终APPROVE_SCOPED_R2，P0–P3全0。独立1377唯一库存与父实际库存逐名相等，
在2 MiB、单测试线程下独立monitor78/0/0，
外层72.141823s／harness72.13s，12个受控子命令均wait／exit0。
731输入和私有binary四次完整身份／bytes／SHA相等。
独立编译、Cargo、共享target读取均0；不签父后来门禁、正常包或真实桌面。
r1执行及交付阶段源码漂移／exit1原件保持，没有移签成r2。

两位reviewer与Windows监控规划agent Singer均已关闭，未创建用户chat、
worktree或容器。两份专用prebuilt按完整摘要核对后回收：
2文件／548,027,216字节，共享target未修改。
Windows规划只保存后续分工与未知边界，没有执行或交付该平台采集能力。

## 正常包与交付范围

正常GUI／MCP release构建`0/reaped/135.074719s`。
提交前打包和arm64／minos15.0结构检查分别为
`0/reaped/0.185526s`、`0/reaped/0.175547s`。
提交前manifest的build.commit=null，明确绑定r2工作差分。
GUI SHA为`f91b702502382d1839cb1a51012a071e5ab408249c6f1ffb35e5eaef38047e67`，
MCP为`bb415f3ae3ae7ca62bdb78abe752b50f5a3a796dc65ec99490f54ee28cb3f693`。
没有启动、安装、签名、公证或新的原生桌面操作。
提交后来源包和新精确CI须另外记录，旧CI失败保持。

## 封存重算

[192成员主封存](2026-11-17-monitor-render-stack-evidence/README.md)
保存上述原始日志、冻结、库存和复核报告；archive共1,610,285字节，
SHA-256为`60d2357e7dba125c8e72f45be04c2e19c239e9c0d2ff4ce9002ce5094a78bca1`。
只读验证器已核对192成员、731工程输入、1377库存及完整app结果、
78项独立结果、两份r2复核、局部帧观察和提交前包；零mismatch，测试重放0。
提交后的精确Git绑定与正常来源包在交付补记中另列。

## 提交交付

工程`bbd4eaccfa8bba2feb487ff43e0423567428bc82`已普通提交／推送，
`2026-11-16T20:59:29.916244Z`读回本地／tracking／远端三方一致、工作区干净。
主封存从该精确提交的731个真实Git输入核对通过，
`0/reaped/2.007765s`、零mismatch，测试重放0。

正常来源包位于`work/monitor-stack-delivery-20261117/package-r2/KeelShell.app`，
manifest绑定上述工程。只复用已冻结且完成release构建的二进制，没有重复构建；
五个应用文件与提交前包逐字节相等，manifest仅build.commit变化。
打包／arm64／minos15.0结构检查分别
`0/reaped/0.192433s`、`0/reaped/0.162028s`，没有启动或安装。

新Quality43955379234已运行；`2026-11-16T21:02Z`快照中，
Windows133608443190处于session backend预检，
macOS133608443592及Ubuntu133608443616处于完整Rust质量门禁，均尚无终态。
没有取消或重跑旧run，没有活动watcher；该快照不能签署跨平台修复成功。

按本次工程的192成员与37成员，以及上一交付25成员的精确Git原件核对，
回收266文件／74,562,243字节、4个已无用途的自有目录。
包括已被提交来源包替代的提交前包；当前正常包摘要和共享target目录身份前后不变。
首轮清理预检因一项Python bytecode未列入计划而exit1，无删除；
原脚本和失败回执保留，第二轮明确纳入该自有派生产物后`0/reaped/0.310310s`。
源码在交付后仍为同一731输入。原件见
[交付补充](2026-11-17-monitor-render-stack-evidence/delivery/README.md)。
43成员交付补充及731精确Git输入只读重算
`0/reaped/2.158798s`、零mismatch，测试重放0；该封存时CI尚在运行。

## 精确 CI 终态补充：本地原件纯派生

Quality `43955379234` 已 `completed/success`，绑定精确工程
`bbd4eaccfa8bba2feb487ff43e0423567428bc82`。
Windows `133608443190`、macOS `133608443592`、Ubuntu `133608443616`
三个 job 均 `completed/success`。既有状态快照的 `updatedAt` 为
`2026-11-16T21:31:17Z`（北京时间 `2026-11-17 05:31:17`）；
上段运行中快照保留其历史时点，本补充没有重新查询或下载。

| 平台 | 普通 Rust passed / failed / ignored | app passed / failed / ignored | rustdoc passed / failed / ignored | Python 主 discovery run / passed / skipped |
| --- | --- | --- | --- | --- |
| Windows | 3312 / 0 / 56 | 1364 / 0 / 2 | 14 / 0 / 0 | 293 / 259 / 34 |
| macOS | 3325 / 0 / 58 | 1375 / 0 / 2 | 14 / 0 / 0 | 293 / 291 / 2 |
| Ubuntu | 3318 / 0 / 59 | 1376 / 0 / 2 | 14 / 0 / 0 | 293 / 291 / 2 |

各平台 92 个普通 suite、6 个 rustdoc subrun 的声明／唯一逐名结果／摘要相等；
app 声明分别 1366／1377／1378。普通 Rust 已包含 app，不与 rustdoc／Python 相加。
包装 Python 独立为 57 run，Windows 53 passed／4 skipped，macOS／Ubuntu 各57 passed；
workspace 内嵌 Python helper 在 macOS／Ubuntu 各30 passed，Windows 未报告该 subrun。
CI 没有独立 app `--list` 或 Python 主 discovery 逐名 `-v` 库存，不补写该证据。

三平台 gate 与包装步骤成功；Windows filesystem／archive codec／session backend
三项预检成功，macOS／Ubuntu 标准 OpenSSH 和回执保存成功。
42个步骤共32 success／10 skipped，完整步骤与时点保存在派生JSON。
原Ubuntu失败名
`monitor::tests::independent_disk_monitor::independent_disk_samples_survive_production_theme_buttons_and_focused_input`
及原Windows失败名
`monitor::tests::disk_monitor_minimum_workspace_languages_themes_scroll_and_selection_are_stable`
均在三个app逐名ok，共六项核对。

原 `collect-terminal-r1` 和 `r2` 的 exit 1、日志与脚本保持：
r1为gh拒绝ANSI，三份输出0字节且stderr说明拒绝；
r2已保存三平台完整原日志／suites JSON，只在外层Windows app路径反斜杠选择处
`StopIteration`。本次新建纯派生helper，在选择app时用
`suite.replace('\\','/')`，原parser不改，三份保存JSON与重算结果完全相等。
原r2未保存成功聚合摘要或逐job独立下载exit／耗时回执，本补充不倒填；
新成功回执只属于本地派生与封存验证。

[37成员终态封存](2026-11-17-monitor-render-stack-evidence/ci-completion/README.md)
保存上述原件／失败及731输入freeze、SHA-256和精确Git绑定。
731个路径／bytes／SHA均从`bbd4eacc…`真实Git blob核对，aggregate仍为
`2ea283af87af7e0af2fa912ebc28f9360e0c5113cd40e1111502f8cdd6d8551e`。
freeze内`head=262a642…`只是当时基线元数据，不替代精确提交绑定。
archive共800,304字节，SHA-256为
`f5c06c21bbe58879d25e9e0066ee97b952d4d65f1cacfb401bf256328fdd4737`。
纯派生`0/reaped/2.126181s`，封存后只读重算
`0/reaped/2.147902s`，37成员／731输入零mismatch。
本次网络、CI重跑、测试重放、Cargo及target访问均0；
没有commit／push，不改HANDOFF／ROADMAP或现有Windows工程代码。
最终报告与封存后验证回执留在`work/monitor-stack-ci-20261117/`，待父收取关闭。
本补充只签精确历史工程，不移签当前HEAD `4209e272…`、
dirty Windows代码或原生／应用包／安装／发行；最新依赖在线UNKNOWN保持。
