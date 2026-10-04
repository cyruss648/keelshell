# ADR 0042：MCP 接收取消期间的消息所有权

- 日期：2026-10-04
- 状态：本地实现、门禁与独立复审通过；新源码 CI 待完成
- 关联：[ADR0037](0037-external-mcp-stdio-server.md)、[ADR0039](0039-authenticated-desktop-mcp-ssh-bridge.md)

## 已证明的问题

Linux Quality 37210745161 的 MCP 响应读取超过原 3 秒期限。根当前源码也能
重现，轻量 instrumentation 确认等待的是 ID 4、`method:9` 应产生的错误。
真实锁定 `rmcp 3.5.0` 的确定性探针进一步证明：默认 `receive` 消费错误帧
并清空 buffer 后等待 writer mutex；SDK service `select!` 的其他分支取消
该 future，会永久丢失尚未发出的 `Invalid Request`。

## 决策

项目内私有 `MessageTransport` 保留官方 `JsonRpcMessageCodec`、SDK server
和工具契约，不升级、修改或 vendor SDK。显式启用已有 `tokio-util` 的
`codec` feature，不新增依赖或改变 registry `x.y` 策略。

两个连接拥有的 I/O task 分别负责读取与写入。reader 沿用 `BoundedReader`、
持久行 buffer 和官方 decoder；合法消息进入容量 32 的 inbound queue，
错误形状进入容量 32 的 outbound queue，固定回复 `id:null / -32600`。
语法错误继续忽略，reader 不等待入队；满队列保守关闭，以持续观察 EOF。
协议仍要求 newline framing，不增加无换行尾帧的交付承诺。

唯一 writer 沿用 `BoundedWriter`，把 SDK 消息和传输错误串行编码、写入并
flush。SDK send 等待实际 flush 的一次性回执；排队、后端完成与部分写入
不归还 frame budget。SDK receive 只等待 `inbound.recv`，取消它不会取消
已排队或在途错误，也不会从头重写部分完成的响应。

保持含换行最多 128 KiB/帧、32 个未 flush 输入帧、10 秒协商、原后端预算
和 3 秒测试读取。reader 先分类 EOF/失败，再发布连接取消；内部 bound
token 为连接 token 的 child，防止启动阶段把 I/O 结束误分类为外部撤销。
外部 AEAD shutdown token 继续取消工具 future 与 SDK service。

`IoTasks` 由 `serve_stream_with_shutdown` 持有；startup/drop 路径取消并
abort 两个 task，显式收尾在同一个 2 秒 deadline 内等待 SDK 和 I/O tasks。
不为各部分重启 deadline，也不调用 underlying `AsyncWrite::shutdown`；
认证外层仍负责有界认证 EOF，避免重复关闭记录。

## 验证与边界

确定性回归覆盖 send 阻塞时取消 receive、错误部分写入后取消、随后有效
RPC、错误输出阻塞时 EOF、满队列和 RAII 析构。真实进程另用四个并发
客户端收齐混合回复。完整 gate、旧失败/新成功与原 CI 日志保留于
[测试记录](../testing/records/2026-10-04-mcp-response-cancellation.md)。

连接终止仍可能丢失或截断在途响应，不承诺已发生远端 I/O 的回滚。静态
诊断不包含请求正文、私有参数或底层错误。此证据不证明供应商 MCP 客户端、
实际 SSH 业务、安装更新、发布或 Windows/Linux 原生桌面；新源码 CI 单独核对。
