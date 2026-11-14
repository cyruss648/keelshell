# 重启后传输恢复日志独立复核（修复前）— 2026-11-15

日期：2026-11-15（Asia/Shanghai）。

## 结论

独立、严格只读 reviewer 固定在文档提交基线
`83db250d175845d084926c0742d2ab20620edc2f`，复核了其祖先实现提交
`3a61562f2cc5031b6b6e5d4d69a452e283ea4a73`。本轮修复前结论为
`REQUEST_CHANGES`：P0 为 `0`、P1 为 `0`、P2 为 `2`、P3 为 `0`。

reviewer 使用独立 detached worktree 和独立 Cargo target。复核完成后，已关闭
reviewer 会话并清理 worktree、target 和临时资源；当前记录保留可审计的 SHA、
命令、计数和边界。后续修复与最终复核另见新的 final review 记录。

## 修复前发现

### P2-1：并发恢复列表刷新允许旧快照覆盖新状态

终态处理会为每个 queued transfer 启动异步恢复列表刷新；旧的
`store.load()` 结果回调只校验 `session_token`，随后直接替换
`recovery_records`。两个任务接近同时结束时，较早读取的旧快照可能晚于较新
刷新返回，重新显示已成功删除的记录、回退只读计划进度或清空较新的成功结果。
claim 仍阻止重新执行，因此影响是 UI/内存状态一致性，未证明会导致未经确认的
远端重放。修复方案是给每次 load 分配单调 generation，并且只接受当前
session 与当前 generation 的回调；同时增加交错刷新回归。

### P2-2：持久 store 与真实 queued worker 的业务端到端覆盖不足

原 focused 测试主要使用禁用 store 的默认 Harness，持久化测试使用预置恢复
记录和自定义 store，writer 的成功、失败和未知结果主要由 unit test 覆盖。
缺少 `FilesPanel + persistent TransferRecoveryStore + run_queued_transfer +
真实 loopback worker` 的成功、队列初始化失败、取消/未知结果和多个终态并发
刷新组合。修复方案是增加真实持久 store 队列集成回归，覆盖这些终态以及
并发刷新后的列表一致性。

## 代码与行为复核

- 首次持久化 `Unfinished` 时调用 `upsert_and_claim`，在记录公开前取得同一条
 记录的 OS claim；
- 持久恢复 Inspect 在后台 worker 中取得 claim；绑定连接直接进入只读计划，
 无绑定记录先完成当前 SSH 主机确认；
- 只读计划把 claim 传入后台 worker，重新计算已有字节后用同一 claim 回写，
 再将 claim 交回审核状态；
- 确认前置失败、worker 启动失败、计划失败、挂起、导航撤销、旧审核替换和
 审核取消均释放 claim；成功终态删除记录，失败、取消和未知结果保留记录；
- 跨窗口/跨进程使用 OS 文件锁、journal 锁和 claim 所有权校验，普通无 claim
 更新与删除不能绕过占用记录；
- StateStore 和文件系统阻塞操作保持在后台 worker 或 `spawn_blocking` 路径，
 恢复日志不保存 SSH session、SFTP handle、worker、review token、凭据、命令
 或秘密；
- `docs/HANDOFF.md`、`docs/ROADMAP.md`、ADR 0130 和测试记录的实现 SHA、
 作者门禁结果和未验证边界互相一致。

## 独立验证

独立 reviewer 使用 Rust/Cargo `1.100.0`、`--locked --offline` 和专用 target。
所有命令均 exit `0`：

| 命令范围 | 实际结果 |
| --- | --- |
| `keelshell-core` `transfer_recovery -- --list` | listed `8` |
| `keelshell-app` `persisted_recovery -- --list` | listed `8` |
| `keelshell-app` `recovery -- --list` | listed `37` |
| `keelshell-app` `parallel -- --list` | listed `14` |
| `keelshell-core` `transfer_recovery -- --nocapture` | `8 passed / 0 failed / 0 ignored` |
| `keelshell-app` `persisted_recovery -- --nocapture` | `8 passed / 0 failed / 0 ignored` |
| `keelshell-app` `recovery -- --nocapture` | `37 passed / 0 failed / 0 ignored` |
| `keelshell-app` `parallel -- --nocapture` | `14 passed / 0 failed / 0 ignored` |
| `cargo fmt --all -- --check` | exit `0` |
| `cargo clippy -p keelshell-core --all-targets --locked --offline -- -D warnings` | exit `0` |
| `cargo clippy -p keelshell-app --all-targets --locked --offline -- -D warnings` | exit `0` |

app 测试链接阶段出现已知 `__eh_frame section too large` linker warning，但没有
改变任何命令的退出码，也没有测试失败或忽略项。

## 未验证边界

本次独立复核没有代签以下验收：

- macOS、Windows、Linux 原生 GPUI 窗口和完整尺寸、主题、语言矩阵；
- 真实客户 SSH/SFTP、重启后的真实远端内容和独立进程长流程竞态；
- Podman 长流程联调；
- IME、VoiceOver、NVDA 和完整辅助技术矩阵；
- 签名、公证、安装、升级、回退和发布验收；
- 在线依赖最新版本检查；
- 作者完整 `scripts/check.py`，其结果仍以作者独立 target 的记录为准。

