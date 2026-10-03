# 只读 TCP 服务探测验收记录

日期：2026-10-03

对应设计：[ADR 0019](../../adr/0019-read-only-tcp-service-probes.md)

## 范围

监控面板从监听 TCP 行启动远程只读 `nc -z -w 2` 探测。桌面端不建立目标服务连接；探测由已认证 SSH 会话上的 Linux 主机执行。UDP、应用层协议检查和自动重试不在范围内。

## 验证结果

| 层级 | 验证 | 结果 |
|---|---|---|
| Session 解析 | `tcp_probe_parses_reachability_and_rejects_unsafe_endpoints` | 通过；成功/拒绝状态、IPv4/IPv6/通配地址和不安全端点均有断言 |
| Session SSH | `loopback_ssh_tcp_probe_is_bounded_and_does_not_send_payload` | 通过；真实 SSH channel 先重新读取监听行，再收到固定探测命令；夹具返回 marker，命令不包含 payload 写入；过期进程信息被 `SocketChanged` 拒绝 |
| GPUI | `tcp_socket_probe_is_explicit_and_keeps_the_result_reviewable` | 通过；真实监听行按钮触发探测，结果绑定当前面板并显示中文状态 |
| 静态策略 | `cargo fmt --all`、session/app `cargo check --all-targets` | 通过 |

## 证据边界

这些测试证明命令构造、边界校验、探测前监听行复核、SSH 通道交接和 GPUI 交互。它们不证明任意发行版都安装了兼容的 `nc`，也不证明探测结果代表应用协议健康；真实 Linux 主机和 Windows/Linux 桌面验收仍需在对应平台运行。探测超时和非零 `nc` 状态只产生结果，不会重试或执行任何修复动作。
