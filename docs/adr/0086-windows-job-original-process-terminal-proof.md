# ADR 0086：Windows 验证控制器按原进程对象确认终态

- 日期：2026-10-08
- 状态：新非作者限定复核与根完整门禁通过；最终根复核及功能分支精确 Windows 验证分别验收

## 问题与范围

实际 Windows 验证出现父进程已结束、其后代的原生进程句柄仍未结束，但控制器返回成功的情况。夹具在确认开始前持有原父子句柄并核对同属控制器 Job；因此这次结果不能用重新打开已复用 PID 解释。基线 `ecab604` 的 [Quality 37726023963](https://github.com/cyruss648/keelshell/actions/runs/37726023963) 也在同一原后代终态断言失败，Rust 门禁尚未进入。

本决策只修改 Python 验证与诊断控制器，不修改应用的 Rust Windows 会话或 AI 清理实现。它也不解释此前应用的 `0xc0000409`；五项 CDB 前置控制和原 706 项应用测试仍需要实际运行。

## 决策

成功终态同时要求直接子进程已实际回收、管道已排空并完成线程收尾、Job 活动计数为零，以及全部已绑定的原进程对象已结束。只查看活动计数不能替代进程对象观察。[Microsoft TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess)说明外部终止是异步操作；[WaitForSingleObject](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject)提供原对象的结束与超时结果。

在终止前完整枚举 Job 及嵌套成员并保留经过成员验证的进程句柄；集合还包含直接 CreateProcess 的原句柄和已经确认加入 Job 的被调试进程句柄。按句柄去重，未验证的额外句柄仅关闭，不能据此等待或终止其它对象。原句柄持续保留到验证结束，避免用清理后的数字 PID 重新推断对象身份。

读取并保留现有 ExtendedLimits，然后设置活动进程准入上限为 1、再次完整绑定成员，以累计 TotalProcesses 变化检测准备及终态观察期间的新关联。任何不完整枚举、打开失败、成员验证失败、关联数变化、对象查询失败或原期限到达，都保留失败与缺失终态。错误 87 也不等价于一个原对象已经结束。

原对象等待前后各读取一次 Job Accounting，两次均使用同一个绝对期限。等待后再次比较累计 TotalProcesses，任何变化都拒绝成功；活动状态取等待前、等待后及未结束原对象数量的最大值，防止后一个零覆盖前一个非零，或忽略等待期间的新关联。这是保守的两次观察，不宣称原子内核快照。v2 的 API 模型反例在最后一次等待中将累计数从 7 变为 8，旧候选错误返回成功；v3 相同输入拒绝成功，新的非作者源码／API 模型复核已通过；根最终证据复核与实际 Windows 验证分别验收。

累计关联数包含因超出限制而失败的关联，见 [Microsoft Job accounting](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_accounting_information)。因此失败的迟到关联也可能保守地使本轮验证失败。这是明确的不确定结果，不能改为成功。已有多个成员时降低活动上限的实际 setter 行为必须由新增 Windows 屏障测试验证；[Microsoft Job limits](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information)不单独证明该时序。

## 失败与预算

所有准备、终止、原对象等待、直接回收和管道收尾共用原绝对期限。失败清理仍尽力终止所属 Job、等待已验证的原对象并保留真实错误；预算耗尽或 API 失败不能生成成功终态。原非零 DWORD、SIGINT、超时、elapsed 断言、四线程和 CDB 条件保持，不增加清理宽限、不删除旧用例。

## 验证

保留原 42 项控制，新增打开失败、原对象等待、嵌套成员和迟到创建检查。API 模型只能证明代码分支，本机 POSIX 树只能证明对应 POSIX 流程；Windows 屏障、五项 CDB 与原应用套件需分别在实际 Windows 执行。具体准备、失败及后续结果记在[Windows 诊断记录](../testing/records/2026-10-08-windows-native-crash-diagnostics.md)。完整产品、其它平台桌面、发布签名与安装更新不由本切片验收。
