# ADR 0155 — 监控视图的渲染栈边界

日期：2026-11-17。状态：已实现、宿主限定门禁与独立复核通过；精确修复后CI和原生另验。

## 问题

源码`1670329`的Quality43945988599在Ubuntu与Windows均于监控面板嵌入工作区时
发生真实栈溢出，分别中止于独立磁盘主题回归和最小窗口磁盘回归。
macOS同源码完整门禁成功，不能据此推断其他平台已修复。
原失败、完整job日志与未报告测试保留在
[精确CI终态](../testing/records/2026-11-17-darwin-udp-sockets-evidence/ci-completion/README.md)。

原`MonitorPanel::render`同时构建所有分区。GPUI的按值样式builder在debug产物中
保留较大的临时值；本机arm64反汇编观察到该函数栈预留为1,576,336字节。
这项局部指令观察不是完整调用树峰值，也不直接测量Windows或Linux。

## 决定

主`render`只装配分区。端口区移入`monitor_sockets.rs`，
控制、资源、网络、容量、进程和终止确认区移入`monitor_view.rs`。
私有helper以`AnyElement`返回已经构建的分区，确认区保留`Option<AnyElement>`。
各builder使用`#[inline(never)]`，明确保留局部临时值的生命周期边界。
它服务于栈边界，不是吞吐性能优化。

原端口231行、其他六块681行正文逐字保留。父子位置、内容顺序、ID、AX、
disabled条件、审核门槛和listener保持原合同。新的构建调用顺序只装配缓存数据；
builder不得采集、发命令、修改样本或启动worker。用户回调继续走原有独立审核流程。

本片不改变产品线程栈配置、依赖、测试断言或授权；三主题继续通过语义token绘制。
后续增加分区时采用同样的私有builder边界，避免重新堆回主`render`。

## 验证与限度

最终731输入冻结后，显式2 MiB测试栈上的完整app1375/0/2通过，
1377个实际编译库存名与运行结果相等；fmt、严格全workspace/all-target Clippy、
依赖x.y策略通过。两位新非作者分别限定批准静态源码和父级专用prebuilt；
后者独立78/0/0，独立编译0。

最终本机arm64 debug主render栈预留为70,992字节。
分区自身及其他调用仍消耗栈；该数值不能当作全应用峰值或跨平台安全余量。
Linux／Windows须由精确修复后CI验证，真实桌面、辅助技术和发行另验。
初次仅抽端口的r1、主动中止的限定gate及所有诊断均保留，不能移签为最终通过。
实际运行、冻结和原件见
[本批记录](../testing/records/2026-11-17-monitor-render-stack.md)。
