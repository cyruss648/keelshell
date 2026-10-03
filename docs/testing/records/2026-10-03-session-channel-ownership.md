# SSH 通道所有权专项验证 — 2026-10-03

## 范围与实现

统一管理 shell、exec、SFTP 和目录扫描专用 SFTP 的 session 通道打开过程。独立后台任务在协议请求发出前取得所有权；调用方取消后，继续接收原打开请求的迟到结果并关闭已确认的通道。传递结果的 oneshot 内保存完整所有权守卫，避免“结果已经发送、调用方尚未接收”期间取消造成通道遗失。显式关闭期间也保留守卫，关闭任务在协议队列背压时被取消仍有后台清理者。

打开请求使用会话配置的超时作为原始截止时间。取消不会重新无限延长这个截止时间；若远端始终不确认，后台任务通过有界断开流程停止共享 SSH 的底层 TCP。已知通道的 CLOSE 入队预算为 2 秒，失败后进入最多 2 秒的共享会话断开流程，并直接关闭同一 TCP 的底层传输句柄。断开逻辑不会因为 russh handle 已结束就跳过实际 TCP 关闭。正常迟到确认及关闭只影响对应通道；异常超时可能结束该连接上的其它会话。

SFTP 使用一个拥有真实 SSH 通道的 relay 任务，读写仍经过 russh 提供的 reader/writer，不复制或自行解析 SSH 帧。每个 SFTP relay 增加固定 64 KiB 容量的双工缓冲（每个方向各 64 KiB），以及 Tokio 双向复制缓冲；现有 SSH/SFTP 协议队列另计。高层 `SftpSession` 和目录扫描使用的 raw SFTP 包装器都持有独立取消控制。显式关闭或最终所有者释放能直接停止 relay，不必等待已经堵塞的 SFTP 写队列消费关闭标记。成功初始化后，该所有者持续覆盖整个 SFTP 生命周期；普通空闲连接没有新增绝对寿命限制。

CLOSE 入队成功只说明协议队列已接收关闭请求，不等于远端确认。SFTP 关闭也不表示撤销已发送的远端文件写入。本轮未修改公开的 `SftpSession` API，未新增依赖。

## 新增回归测试

以下 8 项使用本机真实 SSH 回环服务和隔离测试数据。测试通过服务端观察到的通道关闭与后续 SSH 操作，验证实际协议路径。

| 测试 | 验证内容 |
| --- | --- |
| `cancelling_each_session_open_closes_late_confirmation_and_preserves_ssh` | shell、exec、SFTP 与目录扫描分别在等待打开确认期间取消；迟到确认后服务端收到 Close，后续 exec/目录扫描继续成功。 |
| `unconfirmed_cancelled_open_releases_shared_transport_within_budget` | 服务端永久保留未确认请求；客户端在有界时间内终止共享 SSH，服务端观察到连接结束，后续打开失败。 |
| `sftp_initialization_cancellation_closes_stream_before_late_version` | 已确认通道的 SFTP 初始化被取消，在迟到版本响应前关闭对应通道，其它 SSH 请求仍成功。 |
| `owned_sftp_stream_moves_large_payloads_and_closes_without_ssh_disconnect` | 连续写入并读回 4 个 768 KiB 文件，共 3 MiB；流经多轮缓冲后内容一致，关闭 SFTP 后 SSH 仍可执行。 |
| `high_level_close_or_drop_cancels_a_zero_window_sftp_writer` | 服务端初始化后停止消费写入并耗尽窗口，4 MiB 写流水线产生真实背压；显式关闭和最终 Drop 两条路径均关闭对应通道，保留 SSH。显式关闭分支在任何 Drop/任务 abort 之前等待服务端 Close，独立证明 close 生效。 |
| `cancelling_explicit_close_while_protocol_sender_is_full_keeps_cleanup_owner` | 填满真实 russh 协议队列，使显式关闭在入队处等待；取消该关闭 future 后解除背压，服务端仍收到 Close，SSH 仍可使用。 |
| `dropping_completed_oneshot_handoff_closes_the_owned_channel` | 打开结果已经放入 oneshot 后丢弃接收方，完整守卫负责关闭通道。 |
| `failed_close_after_protocol_exit_still_shuts_down_the_shared_socket` | 协议 handle 已退出而 TCP 复制句柄仍存在；Close 入队失败仍触发实际传输关闭。 |

## 已执行

- `cargo test -p keelshell-session --test ssh_loopback --locked session_ownership:: -- --test-threads=2`：5 项通过。日志 `work/owned-session-open-tests-3.log`。
- `cargo test -p keelshell-session --locked -- --test-threads=2`：80 项通过，包含 33 项库测试、4 项仅远程连接测试和 43 项 SSH 回环测试；文档测试 0 项。日志 `work/owned-session-full-tests.log`。
- `cargo clippy -p keelshell-session --all-targets --locked -- -D warnings`：通过。日志 `work/owned-session-clippy.log`。
- 加强显式关闭测试的观察顺序后，单独重跑 `cargo test -p keelshell-session --test ssh_loopback --locked high_level_close_or_drop_cancels_a_zero_window_sftp_writer -- --test-threads=1`：1 项通过。日志 `work/owned-session-explicit-close-final.log`。此后生产实现未变。
- 修改的 Rust 文件已格式化；专项 `git diff --check` 通过。
- 独立审阅指出并复核关闭了三处生命周期缺口：显式关闭 await 期间守卫丢失、上层 SFTP 关闭等待背压写队列、协议 handle 已退出时遗漏物理 TCP 关闭。对应回归均纳入上述测试。

## 保留的失败与验证边界

早期编译和测试日志保留在忽略的 `work/owned-session-*.log` 中。初版大文件回归尝试写入单个 3 MiB 文件，触及既有回环文件系统每文件 1 MiB 的容量限制；后续改为 4 个 768 KiB 文件，保持累计 3 MiB 的连续传输验证，未放宽产品行为或删除失败证据。

本记录证明本机 macOS 上的真实 SSH 回环与受控背压场景，不代表真实远程主机、长时间网络故障、Windows/Linux 原生运行或三平台发布流水线已验收。正常取消保留 SSH 与异常清理允许断开共享连接的边界应在用户界面中保持一致；整仓门禁及原生桌面验证由集成记录补充。
