# ADR 0159：本地转发绑定原 SSH 生命周期

日期：2026-11-17。状态：已采纳，限定实现／本机工程验收通过；跨平台原生另验。

## 问题

`forward_local` 的本地 TCP listener 原先只等待新客户端和转发任务结束。
应用面板虽会轮询 SSH 状态并关闭 listener，库调用者仍可在 SSH 已关闭时
创建一个无法传输的端点；保留的 owner 也可使旧 listener 在断线后继续占用端口。
重新认证或创建另一条 SSH 连接不能为旧监听器重新授权。

## 决定

- 监听器只属于创建时的原 SSH 连接。`Closing`、`Closed` 和不可用的底层
  handle 都拒绝新监听，返回已有的 `SessionError::Closed`。
- 在实际 bind 后、返回 owner 前重新核对连接。监听任务直接观察原连接状态，
  在原连接退出可用状态后停止接纳、释放 listener，并取消及回收所属 streams。
- 不自动重连、不迁移 owner。新连接需要一次显式 `forward_local` 调用；
  旧 owner 的迟到关闭不能关闭新连接上后来创建的 listener。
- 保留现有 API 和流量计数，使用原连接的 watch 状态与有界健康检查。
  阻塞或网络 I/O 不进入 GPUI 线程，不增加依赖。

## 验证与范围

库级回归与独立 OpenSSH 互通分别验证。新增 opt-in `forwarding_openssh`
真实检查本地转发断线、显式停止、旧／新 owner 隔离、SOCKS 和远程转发停止；
每个用例只使用自有 loopback listener、生成密钥和严格 host pin。

`scripts/openssh_interop.py --forwarding` 单独选择该套件。只有此模式开启
TCP forwarding，目标及远程监听均限制为 `127.0.0.1:*`，保留
`GatewayPorts no`；其他套件仍禁用 TCP forwarding。CI 在 macOS／Linux
运行此模式，Windows 继续执行库级测试，Windows 真实 OpenSSH 转发另验。

实际命令、失败、计数、独立复核和原生边界从
[本批记录](../testing/records/2026-11-17-tunnel-transport-lifetime.md)接续。
