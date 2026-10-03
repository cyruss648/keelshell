# SSH 跳板传输验收

日期：2026-10-03。范围：通过已认证 SSH 的 `direct-tcpip` 建立下一跳 SSH，独立身份核对、认证、取消和连接生命周期。没有新增依赖；依据锁定的 russh 0.63.3 crate 源码核对 `connect_stream`、`channel_open_direct_tcpip`、确认等待和 ChannelStream Drop 行为。

## 接口与所有权

`SshSession::connect_through(&SshSession, SshOptions)` 与 `connect_through_with_retry(&SshSession, SshOptions, RetryPolicy)` 保持原有 `SshOptions` 和直连接口兼容。下一跳主机名称原样交给跳板解析，不启动本地监听、不做本地 DNS 查询、不回退直连。每次尝试以同一绝对截止时间覆盖通道打开、目标 SSH banner、密钥交换及认证；无法表示的时钟范围返回类型化参数错误。

目标重新创建自己的 host-key handler 和转发路由表，沿用原认证实现。目标密钥未知或变化时，在认证之前返回错误。重试仅沿用既有瞬态错误分类；身份、凭据及明确通道拒绝不重试，关闭的跳板不会在此 API 内重连。

通道打开沿用独立 oneshot owner：调用方取消后，已发起的请求继续等待原截止时间内的确认，迟到确认随后关闭。已确认通道由有界 64 KiB duplex relay 持有，子会话的 TransportControl 只取消该 relay，未引用父会话的 TCP control。relay 持有父会话生命周期，但不反向持有子会话 control Arc，避免取消握手时无法触发最终 Drop。

正常目标关闭、握手拒绝及取消释放目标通道，父 SSH 可继续使用。若通道从不确认，或 CLOSE 无法进入协议队列，现有有界清理可能断开共享父 SSH；产品应为每条路线建立专属父链，不复用无关标签的 SSH。CLOSE 入队不是远端确认，channel control 的关闭状态是本地停止状态，也不是远端资源释放凭证。

## 协议回归

新增 11 项真实回环 SSH 回归，分别验证：

- 只由跳板识别的域名、精确目标地址、exec、PTY shell 回显与 256 KiB SFTP 双向字节。
- 目标使用独立 host-key pin，未知/变化时认证计数为零，错误密码及身份失败正常释放通道并保留父连接。
- 无效参数在打开通道之前拒绝；显式 direct-tcpip 拒绝不重试、不回退目标直连。
- 已观察到打开请求后取消，迟到确认关闭，父 SSH 与同父另一个目标仍能执行。
- 永不确认时在截止时间和清理预算内关闭专属父连接，同服务器上的独立 SSH 连接仍正常。
- 目标不回 banner 时取消，以及已收到客户端二进制 KEX 数据后的取消，均关闭该目标流，父 SSH 保留。
- 目标认证已开始但尚未返回时取消，仅关闭目标通道。
- 三跳链中释放外部父引用后末跳仍可工作，最终 clone 释放后整条无人使用的链退出。
- 通道延迟和目标握手共享截止时间，不能各自重新获得完整超时。
- 目标内部通道异常触发 `close_or_abort` 时，只停止目标 relay，父与同父 sibling 仍正常。
- 第一次目标握手遭遇 EOF 后，瞬态重试使用新的独立通道，旧通道关闭，最终仅一次认证成功。

`cargo test -p keelshell-session --all-targets --locked -- --test-threads=2` 通过 97 项：35 项库单元测试、4 项远程范围测试、54 项回环协议测试、4 项 example 测试。既有 SFTP 取消、CLOSE 背压、oneshot 交接、递归传输和 SOCKS5 回归同时通过。严格 Clippy 与定向 rustfmt 通过。

证据位于 ignored `work/jump-transport-{tests-3,session-tests-final,clippy-final}.log`。首轮新增测试误用了不存在的 SFTP 方法名，编译错误保存在 `work/jump-transport-tests-1.log`，修正为现有 `read`/`write` 后再运行，没有修改产品 API 或放宽断言。

## 受控原生验收夹具

现有 `loopback_fixture` 可选接收第二个参数作为固定目标回环端口：

```sh
# 第一个进程：目标，只回显终端输入，文件仅位于独立临时目录。
cargo run -p keelshell-session --example loopback_fixture -- 300

# 第二个进程：将 TARGET_PORT 替换为第一个进程打印的端口。
cargo run -p keelshell-session --example loopback_fixture -- 300 TARGET_PORT
```

在应用中，网关使用第二个进程打印的 `127.0.0.1` 端口；下一跳使用 `target.fixture.invalid:22`，并选网关为跳板。两端指纹分别使用各进程实际输出。夹具既有测试账号保持不变，不新增秘密输出。

网关只允许固定域名和端口，转发目标始终为启动时指定的 `127.0.0.1` 端口，其他请求拒绝；没有第二参数时仍禁止所有 direct-tcpip。端口 0、越界端口和额外参数拒绝。relay 进入原有 Fixture JoinSet，最多 64 个在途 jobs，进程截止或 Ctrl-C 仍关闭连接并清理临时文件树。新增 allowlist 回归覆盖默认禁用、固定目标、错误主机和错误端口。

本记录证明本机协议测试和夹具实现，不证明已完成原生多跳界面验收、真实服务器互通、生产认证策略或 Windows/Linux 原生测试；这些应由后续整合记录补充。
