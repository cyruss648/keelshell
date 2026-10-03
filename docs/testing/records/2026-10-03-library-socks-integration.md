# 连接组织与动态代理集成验收 — 2026-10-03

本记录覆盖连接目录/标签/最近使用/回收恢复的应用接入，以及动态 SOCKS5 的最终集成。领域设计见 [ADR 0007](../../adr/0007-connection-library-organization.md)，协议与清理细节见 [SOCKS 专项记录](2026-10-03-dynamic-socks.md)。本轮没有新增依赖。

## 最终本地门禁

| 检查 | 结果 |
| --- | --- |
| `python3 scripts/check.py` | 直接 registry 依赖 `x.y` 策略、全仓格式、全 target Clippy `-D warnings` 及测试通过 |
| Rust 单元/集成测试 | 284 项通过：AI 47、App 86、Core 93、Session 58 |
| Rust 文档测试 | 2 项通过 |
| `python3 -m unittest discover -s packaging -p 'test_*.py' -v` | 47 项通过 |
| macOS debug 构建和 `.app` 打包 | 成功；实际启动后完成下述交互 |
| 独立只读复审 | 连接组织/焦点和 SOCKS 清理分别复审，已确认问题均修复；复审不替代动态验收 |

最终门禁日志为 ignored `work/connection-socks-final-gate.log`、`work/connection-socks-final-build.log`、`work/connection-socks-packaging-tests.log`。全仓门禁使用隔离临时目录，测试并发为 4。传递依赖 `block 0.1.6` 的 future-incompatibility 提示仍存在。

最终验收包位于 ignored `work/packages/library-socks-final/KeelShell.app`；主程序 SHA-256：

```text
b79b743691468b5a67873e4e98a808cc1a4f19a05d7938ef094cbd13b712a7f7
```

## 连接组织和焦点回归

新增 11 项工作区 GPUI 测试；工作区相关共 25 项通过。操作真实按钮、输入控件与保存通道，覆盖：

- 创建嵌套目录，连接移动到子目录，移入回收站后恢复原身份，父目录改名后路径联动。
- 目录循环、非空删除被拒绝，当前树和编辑草稿保留；目录同时改名/移动在最终树一次校验。
- 仅含空目录的 JSON 导入仍保存，不以新增连接数为零忽略目录变化。
- 标签、目录选择和语言切换保留草稿；导入/导出不携带凭据引用、最近历史或回收站。
- 连接成功事件等待当前保存完成，再合并最近记录，不覆盖收藏等同时修改的状态。入队和真正保存时均核对连接目的地快照。
- 已有最近记录在改名时保留，修改主机等连接目的地时清除；旧目的地迟到成功不能替新目标生成记录。
- 目标选择器输入不会发往底层 SSH；关闭连接表单/管理器后焦点回到可见控件。
- 使用真实回环 SSH 延迟密码认证，在连接过程中打开目录编辑器并输入；认证完成后继续输入仍留在编辑器，远端收到 0 字节；关闭编辑器后输入 `ok`，远端才收到对应 2 字节。

异步 SSH 连接改用既有 `runtime_bridge::spawn`：Tokio 运行网络 future，有界 mailbox 和 GPUI timer 接回完成结果。此前测试在 GPUI 确定性调度任务中执行 `runtime.block_on`，测试自身等待放行信号时阻塞连接任务，随后超时重试。修复后保留原异步交互断言，而非删除或放宽超时测试。

## macOS 原生流程

本机 Apple Silicon，使用独立 `KEELSHELL_DATA_DIR` 与仓库的回环 SSH/SFTP fixture；没有读取用户真实服务器、密码、密钥或连接库。测试服务仅回显终端字节，不执行系统命令。

1. 新建中文根目录及子目录；新建带标签的密码认证 SSH 配置，默认继承当前目录。
2. 移动连接，移入回收站，确认回收站只提供恢复；恢复后原连接重新出现。
3. 与 fixture 输出逐字匹配服务器 SHA256 指纹后信任，输入测试密码完成 SSH 登录；首次未知指纹流程未生成最近成功记录，实际认证成功后出现 1 条。
4. 启动动态 SOCKS5，实际 URI 出现在行内；切换英文后监听保持。停止后终端回显 `post-proxy-stop`，共享 SSH 仍可用。
5. 目录改名，磁盘 JSON 核对层级、连接成员映射、标签、空回收站和最近记录；重启最终构建后仍存在。
6. 最终构建重新登录，启动回环 SOCKS5；对原生进程实际监听端口发送 `05 01 00`，收到 `05 00`。停止后连接端口失败，终端继续回显 `final-socks-stop-echo`。
7. 最终底部状态显示“正在监听 1 条”，停止后显示“全部 1 条隧道已停止”；收藏列表表头不再截断。保存最终截图后退出测试 App，以 SIGINT 正常退出 fixture，临时远端目录自动清理。

首次视觉验收发现隧道行已 Listening，但底部提示仍停留在 Creating；已修复为按启动、监听、停止中、失败行汇总。既有隧道 GPUI 测试另覆盖两个真实监听器逐条停止及实际端口冲突，避免一条停止后误报全部结束。

最终安全截图位于 ignored `work/library-socks-native/final-library.png`、`final-socks-listening.png`、`final-socks-stopped.png`。旧截图及失败日志保留于同级目录与 `work/connection-library-ui-tests*.log`、`work/connection-library-focus-debug.log`，不使用删除失败证据的方式制造通过记录。

## 证明边界

- 原生 SOCKS 验收证明界面、监听、协商和停止；完整 IPv4/IPv6/域名及 128 KiB 双向代理数据通过专项真实 SSH 协议测试证明。原生 GUI fixture 没有启用 direct-tcpip 目标转发。
- 域名测试只证明域名交给 SSH 端；没有使用公共 DNS 或真实生产目标。无 SOCKS UDP/BIND/用户认证、隧道规则持久化或系统代理自动配置。
- 最近连接表示 SSH 认证成功，不承诺随后 PTY、SFTP 或任意远端命令可用。
- 本轮未进行 Windows/Linux 桌面交互、真实 OpenSSH 互通、签名、公证或安装。GitHub runner 的编译和测试结果须按关联提交查看 Quality；不能用较早版本的发布矩阵替代本轮产物验收。
- 目录批量编排、永久清空回收站 UI、凭据轮换/清理、跳板链、已建立会话恢复、递归/断点传输和高级 AI 仍在路线图中；本轮不表示完整产品目标已完成。

## 首次跨平台门禁发现的拒绝响应问题

提交 `baca282` 的 [Quality 运行](https://github.com/cyruss648/keelshell/actions/runs/37088353624) 中，macOS 与 Ubuntu 全部通过；Windows 的打包及其它 Rust 测试通过，但 `socks_rejects_auth_commands_addresses_and_malformed_requests` 返回 `ConnectionReset`（10054），29 项 SSH 回环中 1 项失败。失败日志保留在 ignored `work/library-socks-quality-windows-failed.log`。

拒绝握手或请求后，原实现写入拒绝字节便关闭 socket，可能遗留未消费的请求尾部。Windows 在这种关闭情况下可以产生 TCP reset，使客户端无法读取已写入的拒绝响应。完整但不支持的 BIND/UDP 请求也存在该路径；这不是仅靠容忍测试错误就能解决的问题。修复与后续门禁证据继续记录于下文。

拒绝响应关闭顺序及严格回归已修复，详见 [Windows 专项记录](2026-10-03-socks-windows-rejection.md)。修复后本机全仓门禁再次执行，日志为 ignored `work/library-socks-rejection-final-gate.log`；本轮新增 2 项有界清理单元测试及 1 项真实取消回归，使普通测试总数增至 287 项，另有 2 项文档测试。前述原生截图和二进制 SHA256 属于该关闭顺序修复之前的构建，后续增量由协议回归和关联修复提交的三平台 Quality 验证，不冒充重新进行过全部桌面交互。
