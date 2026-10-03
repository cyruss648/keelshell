# 动态 SOCKS5 TCP 转发 — 2026-10-03

## 实现范围

- `SshSession::forward_dynamic` / `forward_dynamic_with_options` 创建仅绑定回环 IP 的 SOCKS5 监听器。默认绑定地址由 UI 提供为 `127.0.0.1`，默认端口 `0`；启动成功后显示实际分配的 `socks5h://` 地址。非回环地址和需要本地 DNS 的监听名称被拒绝。
- 支持 SOCKS5 无认证协商与 TCP CONNECT；IPv4、IPv6、ASCII 域名（包括 punycode）经同一条已认证 SSH 连接的 `direct-tcpip` 传递。域名不在代理客户端本地解析。不会收集代理用户名、密码或生成额外 SSH 凭据。
- 无可接受认证方法回复 `05 FF`；BIND / UDP ASSOCIATE 回复 `REP=07`；未知地址类型或不可接受域名回复 `REP=08`；格式错误 / 零端口为一般失败。SSH 管理策略拒绝映射 `REP=02`，无法细分原因的远端连接失败映射 `REP=01`。
- SSH 不提供远端 socket 的实际绑定地址，成功回复的 BND 端点为 `0.0.0.0:0`，不冒充本机或目标地址。这里是 [RFC 1928](https://www.rfc-editor.org/rfc/rfc1928) 中的无认证 CONNECT 子集，不宣称实现全部认证、BIND 或 UDP 机制。
- 默认每个监听器最多 64 个客户端，包含未完成的握手；可配置 1–256。超额 TCP 连接立即关闭，不创建额外握手任务。握手总期限默认 10 秒，允许配置非零且最多 60 秒，并受 SSH 操作 timeout 的更短期限约束。域名 / 方法列表最多 255 字节，流转发使用有界复制缓冲；已建立流没有 idle timeout。

## 生命周期与异常边界

- 停止先释放 listener，再取消握手与本地数据流，并等待该监听器拥有的任务收尾。正常停止、Drop 和 SSH 断开均有回环测试；正常停止不会关闭共享 SSH。
- 对已确认的 channel，代码保留 `Channel` 所有权，有界等待 `channel.close()` 进入 SSH 会话管理的协议队列；没有借助 `ChannelStream` 的 detached Drop 任务。该 API 只确认入队，**不表示对端关闭确认**。实际 loopback 测试另行验证目标 TCP peer 收到 EOF。
- russh 0.63.3 的本地 close 会移除 encrypted channel 项，随后接收到的对应 Close 不再可靠地出现在该 channel receiver 中。因此不能在发送 close 后将 `channel.wait()` 当作可用的关闭确认 API。
- 对尚未确认的 direct-tcpip 请求，取消不会直接丢弃 open future。代码保留它直到原握手期限，迟到的成功结果立即显式 close。未被轮询的 open future 不会因进入清理分支而发起新请求。
- 如果 open 确认超时或已知 channel 的 close 入队停滞，清理先请求共享 SSH 正常断开，最多等 2 秒；仍未关闭时通过 `std::net::TcpStream::try_clone` 保留的同 socket 控制句柄执行 `shutdown(Both)`。控制句柄只在系统确认 shutdown 成功或返回 NotConnected 时标记已中止。其它 I/O 错误不伪造已关闭状态，UI 明确显示断开未确认。正常停止不走这条异常断开路径。
- 同 socket 控制句柄也覆盖连接 / 认证 future 被取消的清理；连接正常释放时沿既有生命周期所有者保留至断开处理结束。无需额外依赖，未创建另一个网络连接。
- 最慢的取消可能先等待剩余握手期限，再等待 close 入队及异常断开各最多 2 秒。UI 在结果返回前保持“正在停止”；`DynamicForward::close` 返回真实清理错误，Drop 仅请求已拥有任务收尾。应用的 transport worker registry 会等待显式 close。

## 界面覆盖

端口转发面板新增“动态 SOCKS5 / Dynamic SOCKS5”。动态模式隐藏固定目标输入，切换时将焦点移到监听地址；隐藏字段中的旧输入不影响代理启动。面板说明仅回环、无代理认证、TCP CONNECT、远端 DNS、UDP/BIND 不支持；每行显示实际代理 URI，支持复制与停止。语言切换保留输入、模式和运行中的监听器。底部按实际启动、监听、停止中和失败行汇总；停止一条隧道不会将其它活跃行误报为全部停止。通道清理异常明确说明是否已断开共享 SSH，不能将失败显示为正常停止。

本切片仅维护当前会话的运行时隧道，不新增隧道配置持久化、HTTP 代理、外部监听、跳板链或 SOCKS 用户认证。

## 验证结果

- `cargo test -p keelshell-session --locked`：58 项通过（25 单元、4 产品边界、29 SSH 回环）。其中新增 12 项 SOCKS 回环测试与 2 项 socket 清理单元测试。
- SOCKS IPv4、IPv6、`remote-only.invalid` 三条路径分别校验目标主动发送的 12 字节数据，以及客户端到目标再回传的 128 KiB 二进制载荷。测试服务器断言 direct-tcpip 收到原域名；该 `.invalid` 名称只在测试 SSH 服务器内部映射，证明未在客户端解析。
- 协议测试覆盖无可用认证、BIND/UDP、未知地址类型、错误版本/保留位、零长度/非法域名、零端口、远端策略拒绝和实际目标连接失败。
- 资源测试覆盖部分请求握手到期、1 个连接容量时超额连接在握手期限前关闭、容量恢复、部分请求取消不产生远端 channel、listener 重新绑定、已建立连接停止、Drop、客户端和服务端主动 SSH 断开。
- 迟到 open 的正常停止验证目标 EOF 且原 SSH `exec` 仍可用；open 超时验证清理错误、监听器释放以及共享 SSH 不能继续执行命令。
- 可控停滞测试将 graceful cleanup future 固定为永不完成，经过 30 ms 的测试期限后在真实 TCP 对端验证 EOF；另一测试证明正常完成的 cleanup 不会提前关闭原 socket，并验证控制句柄释放时关闭连接。这证明 socket fallback，不等同于模拟了所有真实 SSH 服务的背压情况。
- `cargo test -p keelshell-app tunnels:: --locked`：3 项通过。真实 GPUI 点击动态模式 / 启动 / 复制 / 停止，中英文切换与焦点检查；复制得到的实际代理端点经真实 SSH 连接完成 `ui-proxy` 字节回显。同一流程还验证双监听逐条停止及实际端口占用失败。纯 GPUI 测试不等同于桌面视觉验收；最终 macOS 验收见 [集成记录](2026-10-03-library-socks-integration.md)。
- `cargo clippy -p keelshell-session -p keelshell-app --all-targets --locked -- -D warnings`：通过。已有传递依赖 `block 0.1.6` 的 future-incompatibility 提示仍存在，不属于本切片引入。

最终日志位于 ignored `work/socks-session-owned-cleanup-2.log`、`work/socks-app-owned-cleanup.log` 和 `work/socks-owned-cleanup-clippy.log`。早期应用编译失败日志 `work/socks-app-tests*.log`、错误等待不可观察 close ACK 导致正常停止误超时的 `work/socks-session-owned-cleanup.log` 均保留。首次断线测试曾只取消服务器外层 task、并未关闭 russh 的实际协议连接；夹具改为发出真实 SSH disconnect 后，断线测试通过。

本机为 macOS，IPv6 loopback 在本次实测中通过。Windows/Linux 原生桌面、系统代理设置、真实远程 OpenSSH 服务器和第三方代理客户端未在本记录中验收；仓库整体门禁及原生 UI 由集成任务另行记录。
