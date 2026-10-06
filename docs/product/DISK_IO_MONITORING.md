# 磁盘 I/O 监控 / Disk I/O monitoring

KeelShell 在已认证的远程 Linux 会话资源面板提供按设备查看的磁盘 I/O。它与文件系统容量分开：容量来自 `df`，吞吐与请求计数来自 Linux `/proc/diskstats`。无需安装远端程序。实现已通过新非作者评审与主工作区整合，主线完整组合门禁通过1394普通/8doc/6Python；磁盘监控原生桌面与真实远程 Linux 连续采样验收仍待完成；证据见[本轮记录](../testing/records/2026-10-06-disk-io-monitoring.md)。

## 使用

1. 打开已经认证的 Linux SSH 会话，资源面板按已有五秒节奏读取；可以暂停并手动刷新。
2. 在“磁盘 I/O”区选择一行 `设备名 · major:minor`。选择本身不产生 SSH 请求。
3. 首次采样或新设备等待第二次有效采样；之后显示当前设备读取/写入吞吐、完成请求每秒次数、平均耗时、当前未完成 I/O，以及远端区间和活动时间增量。
4. 小窗口先滚动资源面板，再在设备列表内部滚动选择。超过显示长度的名字使用省略，选中后的完整身份可横向滚动。中文/英文及 System/Light/Dark 都使用同一流程。

设备、分区和逻辑设备分别展示。不会把父盘与分区相加，也不推断哪些名字是物理硬盘。设备消失保留当前选择，显示不可用；重新选择或后续有效采样可继续查看。缺少权限、内核不支持或数据超限不会变成零吞吐，也不阻止仍有效的 CPU、内存、网络和容量显示。整个命令失败时保持原有失败与旧样本年龄提示。

## 指标解释与限制

扇区统一按 512 字节换算，读写次数可能已经被内核合并。均值来自区间累计请求时间与完成请求数，未完成任何请求时显示“—”。当前未完成 I/O 是采样时刻的数量。活动时间只显示原计数增量，不能当作精确利用率或饱和度；并发 I/O 会影响时间统计。依据 [Linux I/O statistics](https://docs.kernel.org/admin-guide/iostats.html) 和 [block stat](https://docs.kernel.org/block/stat.html)。

采集按顺序读取各文件，并非原子快照。差值必须保持同一 boot_id、btime、major/minor/name 和布局，且远端 uptime 严格向前。新设备、消失、启动变化、计数回退、时间异常或数值溢出各显示不可用原因；有效无活动区间显示真实零。名字/设备号不是硬件序列号，未采到的同身份重新初始化可能无法区分。这里不提供物理设备总量、精确延迟分位数、历史趋势持久化、告警、远端修改或自动恢复连接。

完整命令输出上限仍为 2 MiB，可选磁盘段最多 256 KiB/256 行，名字最多 128 ASCII 字节，只接受官方 11/15/17 字段格式。监控采样只留在当前工作区。MCP 继续由 KeelShell 向外提供已有缓存字段，本次没有新增磁盘工具、schema 或授权。

## English

The authenticated Linux resource panel now offers individual disk/partition/logical-device observations, separate from `df` filesystem capacity. Sampling uses the fixed read-only collector on the existing SSH connection; selecting a device is local and sends no command. Pause, manual refresh and the existing five-second schedule remain available.

Select `name · major:minor`, then wait for two valid samples. The panel shows interval read/write byte rates, completed requests per second, mean request time when requests completed, and the current in-flight gauge. It labels remote elapsed time and raw activity milliseconds without claiming exact utilization. Parent disks, partitions and stacked devices are never summed. Long names remain inspectable after selection; the device list and resource column scroll at the minimum window size.

Boot identity, remote uptime, exact row identity, layouts and cumulative counters are validated. Missing, unsupported, reset, disappeared, backward-time and overflow observations remain explicitly unavailable; a valid idle interval can show zero. A row name/device number is not a physical serial or generation ID. Optional disk errors preserve otherwise valid resources, while full transport/output failures retain the existing stale-sample/error indication. The bounded parser accepts documented 11/15/17 counter layouts only. This feature adds no remote installation, connection automation, durable histories, alerts, MCP schema/tools or grants.

The author tests include owned encrypted TCP/SSH fixtures, genuine kernel sampling in an isolated offline Linux container, and GPUI minimum-window interactions. These are distinct evidence scopes, not customer SSH or cross-platform native desktop acceptance. Independent review and working-copy integration passed, followed by the 1,394 ordinary/eight doc/six Python combined-main gate. Native disk-monitor desktop and real remote Linux continuous-sampling acceptance remain open.


后续主副本完整组合门禁已通过，新的 macOS 标准包取得正常执行与真实 Linux SSH 采样的有限原生证据，见[参数与磁盘记录](../testing/records/2026-10-06-target-parameters-disk-native.md)。该后续证据保留原失败，不扩大为其它平台、完整窗口/辅助技术或所有运行分支验收。
