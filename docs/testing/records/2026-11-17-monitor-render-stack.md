# 监控渲染栈边界 — 2026-11-17

状态：最终r2宿主限定门禁、两位独立复核和正常包结构通过，提交推送进行中。
完整产品、修复后跨平台CI、原生及发行仍开放。
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
