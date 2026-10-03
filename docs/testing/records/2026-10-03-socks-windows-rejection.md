# SOCKS5 拒绝回复的 Windows 关闭竞态 — 2026-10-03

## 首次失败证据与原因

首次跨平台 Quality 中，macOS/Linux 通过；Windows 的 SSH 回环测试 28 项通过、`socks_rejects_auth_commands_addresses_and_malformed_requests` 失败，报 Winsock `10054 / ConnectionReset`。原日志保留在 ignored `work/library-socks-quality-windows-failed.log`。

源码存在确定的未读尾部关闭缺陷：`[05,00,00]` 和 `[04,01,00]` 在读取前两个字节后即可判定拒绝，原代码写出 `05 FF` 后立即释放 socket，仍有一个请求字节没有消费。真实客户端发送的完整 BIND/UDP 请求，以及非法域名后面的端口，也会在提前拒绝时留下未读数据。该关闭方式与 Windows 的重置表现一致；首次 CI 日志未指明具体子用例，不能将本机修复测试代替 Windows 复跑证据。

[Microsoft Winsock shutdown 文档](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-shutdown) 建议发送侧 shutdown 后继续接收至 EOF，再释放 socket，以保留优雅关闭的数据交付语义。直接忽略客户端读回复时的 ConnectionReset 会掩盖协议回复丢失，未采用这种处理。

## 修复与边界

所有显式拒绝（认证方法、请求格式、命令、地址、端口、SSH 远端拒绝）现在统一执行：

1. 写入完整 SOCKS5 拒绝响应。
2. 只 shutdown **发送方向**，将响应后的 FIN 发给客户端，继续保留接收方向。
3. 使用固定 512 字节缓冲，最多消费 4096 字节尾部，最多等待 250 ms；客户端关闭发送方向时提前结束。

协商的整个过程仍受原始握手总 deadline 和监听器取消信号限制，没有重新延长握手期限。SSH channel-open 错误后的拒绝路径也增加取消竞争，避免 stop 等待新增的 drain。成功响应仍进入原双向数据转发，不执行拒绝关闭逻辑。

4096 字节 / 250 ms 是资源上限，不是对无限垃圾尾流的交付承诺；超过上限、原握手到期或主动停止时，连接仍会释放。此修复不更改认证支持、远端 DNS、监听地址或已建立隧道的权限与生命周期。

## 回归验证

- 原异常 greeting 用例保留；客户端收到回复前不 shutdown 发送方向，并延迟 20 ms 读取，以覆盖服务端提前关闭的竞态。严格断言完整 `05 FF` 后正常 EOF。
- BIND、UDP 同时覆盖原短请求头和完整 IPv4 请求；非法域名补齐端口；畸形请求增加缓冲尾部。严格断言完整 10 字节响应和预期 REP，随后要求 EOF，不能用 reset 代替通过。
- 远端拒绝请求附带少量提前发送的数据，验证相同关闭路径；本地协议拒绝后仍断言没有发出 direct-tcpip，且共享 SSH 仍能执行测试命令。
- 两个有界内存流测试分别证明恰好读取 4096 字节、不消费其后的 3 字节，以及客户端保持发送方向开启时仍在规定期限内结束。
- 真实回环测试覆盖拒绝 drain 期间 stop、listener 释放、没有创建 SSH channel 和共享 SSH 保留。

本机执行：

- `cargo test -p keelshell-session socks --locked`：15 项通过（2 单元 + 13 回环）。
- `cargo test -p keelshell-session --locked`：61 项通过（27 单元 + 4 产品边界 + 30 SSH 回环）。
- `cargo clippy -p keelshell-session --all-targets --locked -- -D warnings`：通过。
- 两个修改 Rust 文件的格式检查：通过。

日志分别保存为 ignored `work/socks-windows-rejection-tests.log`、`work/socks-windows-rejection-all-tests.log`、`work/socks-windows-rejection-clippy.log`。本机结果只证明 macOS 回归；修复提交关联的 [Quality 流水线](https://github.com/cyruss648/keelshell/actions/workflows/ci.yml) 分别记录 Windows、Linux 与 macOS runner 的执行结果。
