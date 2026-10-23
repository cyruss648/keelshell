# ADR 0090：转发运行时流量统计

## 状态

已接受并实现 NET-01 的运行时统计切片。持久化规则和跨平台原生桌面验收仍开放。

## 背景

KeelShell 已支持本地 TCP、远程 TCP 和 loopback-only SOCKS5 CONNECT 转发。转发列表能够显示监听地址和停止结果，但用户无法判断某条活动或已停止隧道是否实际传输了数据。统计必须覆盖多个并发连接，且不能改变现有 listener、route 撤权、channel 清理、未知结果或最多连接限制。

## 决策

每个 forwarding owner 创建一份 `TrafficStats`。统计对象通过 `Arc<AtomicU64>` 在 listener task、每个连接 task 和 UI polling task 之间共享，并公开只读 `TrafficSnapshot`。原子计数使用饱和加法，避免极端累计量回绕为较小数值。

统计方向以 listener 的绑定侧为参照：

- `sent`：从 listener 绑定侧写向 SSH 另一侧的 payload 字节。
- `received`：从 SSH 另一侧写回 listener 绑定侧的 payload 字节。

对于 remote forwarding，listener 绑定侧是远端转发监听器，因此远端连接方写入本机目标的字节计为 `sent`，本机目标写回远端连接方的字节计为 `received`。

TCP forwarding 和 SOCKS relay 都使用有界异步复制函数。复制函数只在目标 writer 成功接收字节后增加计数，因此失败写入不会被虚报；EOF 仍执行原有半关闭。SOCKS greeting、CONNECT 请求和成功响应不属于隧道 payload，不计入统计。

`LocalForward`、`RemoteForward` 和 `DynamicForward` 公开 `traffic()` 快照句柄。app 的 `TunnelRow` 在监听事件确认后保存统计句柄，每 30ms 读取一次原子快照，并在列表行显示累计发送/接收字节。停止前先撤权，remote forwarding 还等待已建立 relay task 退出，停止后的 row 保留最终快照；关闭操作、撤权和异常清理仍由原 owner 负责。

## 取舍与边界

统计是当前进程内的运行时数据，不写 profile、vault 或其它持久化文件；应用重启、SSH 重连和旧 owner 释放后不会恢复或自动重建隧道。后续若增加保存规则，规则不得保存密码，且必须经过显式审核后启停；本 ADR 不定义这些规则。

`AtomicU64` 快照分别读取两个方向，活动流量下两个数字不构成同一时刻的事务快照。字节统计不等同于 SSH 加密包流量、TCP/IP 开销或压缩前后线速。它统计应用 payload 在转发复制边界的成功写入量。

## 验证

计数器有并发累加和饱和边界单测；session loopback 覆盖真实 local TCP、remote TCP 和 SOCKS5 二进制往返；app 的真实 GPUI 隧道测试检查 SOCKS payload 完成后行级快照和英文展示文本。测试命令与结果记录在 [NET-01 测试记录](../testing/records/2026-10-23-net-01-traffic-accounting.md)。

本切片没有完成 macOS Finder/桌面视觉、Windows/Linux 原生桌面、IME、VoiceOver、安装发布或真实客户 SSH 主机验收；这些边界不能由 Rust/GPUI 回环测试替代。
