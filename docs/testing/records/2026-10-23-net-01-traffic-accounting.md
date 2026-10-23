# NET-01 运行时转发流量统计记录 — 2026-10-23

## 范围

本记录覆盖一个独立 worktree 中的 NET-01 工程切片：local TCP、remote TCP 和 loopback SOCKS5 forwarding 的真实 payload 累计发送/接收字节，以及 app 隧道列表对活动/已停止 row 的展示。

统计方向固定为 listener 绑定侧视角：发送是写向隧道另一侧的字节，接收是从另一侧写回绑定侧的字节。remote forwarding 的测试使用 3 字节请求和 11 字节响应，验证方向不会因本机目标 socket 的参数顺序而反转。SOCKS5 greeting、方法选择、CONNECT 请求和代理响应不计入 payload；计数只在异步 writer 成功接受字节后增加。统计通过 `Arc<AtomicU64>` 共享，使用饱和加法，不引入 UI 线程阻塞。

## 实现验证

通过的命令：

```text
cargo fmt --all
cargo check -p keelshell-session -p keelshell-app --locked
cargo test -p keelshell-session --locked forwarding::tests -- --test-threads=4
cargo test -p keelshell-session --test ssh_loopback --locked local_forward_transports_bytes_and_closes_listener -- --test-threads=1
cargo test -p keelshell-session --test ssh_loopback --locked remote_forward_transports_bytes_and_is_cancelled -- --test-threads=1
cargo test -p keelshell-session --test ssh_loopback --locked socks_ipv4_streams_bidirectional_binary_bytes -- --test-threads=1
cargo test -p keelshell-app --locked tunnels::integration_tests::dynamic_panel_starts_real_proxy_copies_address_switches_language_and_stops -- --test-threads=1
```

结果：全部通过。计数器并发测试覆盖 8 个线程的累计加法和 `u64` 饱和边界；local TCP 覆盖 4 字节往返，remote TCP 覆盖 3 字节请求和 11 字节响应并得到 3 sent / 11 received；SOCKS5 真实二进制回环得到 128 KiB client payload 发送以及目标主动首包加回显的接收计数；GPUI 隧道行在真实 SOCKS `ui-proxy` 8 字节往返后显示 8 sent / 8 received，并在停止完成后再次确认该快照保持不变。

现有的撤权、关闭、未知结果和最多连接限制测试未被删除或放宽；新增计数只观察复制边界。`cargo check` 保留已有传递依赖 `block 0.1.6` 的 future-incompatibility 提示，不属于本切片引入。

## 未完成边界

- 统计仅存于当前进程；不持久化 profile/vault，不保存密码，不自动重启或恢复隧道。
- 尚未实现命名隧道规则、审核后启停、重连后的旧运行绑定失效策略；这些属于 NET-01 后续切片。
- 本机 GPUI/TCP 回环不是 macOS、Windows 或 Linux 原生桌面验收；原生窗口布局、IME、VoiceOver、系统主题、安装/发布和真实客户 SSH 主机仍未验收。
- 计数是两个独立原子快照，不是跨方向事务快照，也不代表 TCP/IP 或 SSH 加密层开销。
