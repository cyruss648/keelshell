# 2026-10-03 — SFTP 传输队列切片

## 范围

本次切片把远程文件传输从单次调用扩展为会话层 FIFO 队列。队列只操作已建立的 SSH/SFTP 会话，不创建本地 shell，也不包含 UI 状态。

每个请求通过 `TransferSpec` 描述上传或下载，并返回 `TransferHandle`。句柄提供：

- `Queued`、`Started`、`Progress` 和一个终态事件；
- 已传输字节数与已知的总大小；
- 在下一个数据块边界取消活动请求，或在开始前取消排队请求；
- 有界 Tokio channel，避免无限累积进度事件；
- 单 worker FIFO 顺序，保证同一会话上的结果顺序稳定。

上传沿用流式替换语义，取消或传输错误可能留下部分远端文件；下载使用 `create_new`，部分本地文件会保留。调用方必须在重试前检查结果，队列不会自动重放不确定的远端写入。

## 验证

在仓库根目录执行：

```text
cargo test -p keelshell-session
cargo clippy -p keelshell-session --all-targets -- -D warnings
```

结果：17 个回环 SSH/SFTP 集成测试、17 个会话单元测试和 4 个远程边界测试通过；Clippy 在所有目标上通过。

新增集成测试 `sftp_transfer_queue_reports_progress_and_cancels_pending_work` 覆盖：

1. 大文件上传的队列、总大小、分块进度和完成字节数；
2. FIFO 队列中排队请求在开始前取消，并收到唯一终态；
3. 远程文件下载的大小、进度、完成事件和本地内容校验；
4. 传输句柄和 SFTP subsystem 的显式关闭。

## 尚未覆盖

当前队列是单 worker，不提供并行度配置、断点续传、目录递归、暂停/恢复、速率限制或持久化任务历史。递归目录传输应在加入前明确符号链接、权限、时间戳和失败后重试策略；这些行为不能由当前单文件 API 推断。
