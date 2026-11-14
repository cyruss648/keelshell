# 重启后传输恢复日志最终复核记录 — 2026-11-15

日期：2026-11-15（Asia/Shanghai）。本记录对应源码提交
`e2b00af3cb8bcae67d54cbf69cf04e8baab2b7cd`
（`fix(files): refresh recovery records after mutations`）。公司内网不可用，
本片只做本地 commit，不执行 push。

## 结论

本片已经完成作者完整工程门禁和新的独立、严格只读代码与功能复核，状态为：

`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`

作者完整 `scripts/check.py` 在独立 screen 会话中实际退出 `0`。新的 reviewer
固定在 exact SHA `e2b00af3cb8bcae67d54cbf69cf04e8baab2b7cd`，结论为
`APPROVE`，P0/P1/P2/P3 为 `0/0/0/0`。原始 reviewer 回执已归档为
[最终独立复核原文](2026-11-15-transfer-recovery-review-final-raw.md)。

本片修复了上一轮 reviewer 指出的恢复列表一致性竞态：成功的 Discard 和持久
恢复计划进度更新都会推进 mutation generation，并从持久 store 启动新的完整
reload；旧 load callback 同时校验 session、load generation 和 mutation
generation，因此不会覆盖新的恢复列表。新回归覆盖“旧快照 → durable
mutation → 旧 callback → 新异步快照”。

## 修复范围

- `FilesPanel::apply_persisted_recovery_progress` 在内存记录更新成功后推进
  mutation generation，并调用生产 `load_recovery_records` 读取持久权威列表；
 失败时不推进 generation、不伪造刷新成功。
- `FilesPanel::apply_discard_result` 在持久删除成功后推进 mutation generation、
  移除当前记录并调用生产 `load_recovery_records`，把其它窗口或 worker 在同一
  交错期间完成的新增、删除和状态变化重新同步到面板。
- 两个 GPUI 回归使用真实 window/context：Discard 场景验证旧快照中的并发完成
  记录不会回流；计划进度场景验证旧进度不会覆盖新进度，且新快照只保留仍存在
  的 durable 记录。
- claim 仍绑定 record ID、journal 和当前 session；旧 session、记录消失、
  失败、取消、挂起、确认失败和 stale review 均保持 fail closed 或释放 claim。

## 作者完整工程门禁

执行命令：

```text
CARGO_TARGET_DIR=work/transfer-recovery-refresh-20261115/full-target \
python3 scripts/check.py
```

最终权威日志和退出回执：

- 日志：[full-gate-screen.log](../../../work/transfer-recovery-refresh-20261115/logs/full-gate-screen.log)
- exit receipt：[full-gate-screen.exit](../../../work/transfer-recovery-refresh-20261115/logs/full-gate-screen.exit)
- 独立 target：`work/transfer-recovery-refresh-20261115/full-target`

实际结果：

| 范围 | 通过 | 失败 | 忽略 |
| --- | ---: | ---: | ---: |
| Python scripts | 131 | 0 | 1 |
| keelshell-app | 1174 | 0 | 2 |
| keelshell-core | 644 | 0 | 0 |
| keelshell-ai | 194 | 0 | 5 |
| keelshell-session | 448 | 0 | 16 |
| keelshell-mcp | 77 | 0 | 0 |
| 普通 Rust 合计 | 2537 | 0 | 23 |
| workspace doctest 合计 | 14 | 0 | 0 |

依赖版本策略、`cargo fmt --all --check`、workspace 严格 Clippy、
workspace tests、readiness controller、observer controller 和 small-stack
controller 均通过。应用链接阶段仍出现已知
`__eh_frame section too large` warning，但没有改变测试、Clippy 或完整门禁
退出码。

focused 交错回归使用：

```text
CARGO_TARGET_DIR=work/transfer-recovery-refresh-20261115/target \
cargo test -p keelshell-app --bin keelshell-app stale_recovery_load \
  --locked -- --nocapture --test-threads=1
```

结果为 `3 passed / 0 failed`。随后完整 `parallel` 测试集为
`20 passed / 0 failed`；新增两个交错测试已经包含在完整 workspace app
清单的 `1174` 项通过结果中。

一次前台长时 wrapper 在完整测试已完成后返回了无法由日志解释的 `1`，没有
对应测试失败或编译错误；为避免把 wrapper 生命周期误判为项目失败，随后在
独立 screen 会话中重复执行同一 `scripts/check.py`，取得上述权威 exit `0`。
前次日志保留在同一 work 目录，最终状态以 screen exit receipt 为准。

## 独立严格只读复核

reviewer 使用独立 detached worktree、独立 Cargo target 和独立日志，未修改
源工作树、未格式化写回、未 commit、未 push、未 merge，也没有使用共享 target。
复核 exact SHA：

`e2b00af3cb8bcae67d54cbf69cf04e8baab2b7cd`

关键生产代码证据：

- `crates/keelshell-app/src/files.rs:1079-1109`：后台 load 捕获 session、
  load generation 和 mutation generation。
- `crates/keelshell-app/src/files.rs:1120-1142`：旧 callback 在替换
  `recovery_records` 前同时校验三项身份。
- `crates/keelshell-app/src/files.rs:1144-1175`：计划进度 mutation 成功后
  推进 generation 并启动新的 reload。
- `crates/keelshell-app/src/files/parallel.rs:858-897`：Discard 成功后推进
  generation、更新内存列表并启动新的 reload。
- `crates/keelshell-app/src/files/parallel_tests.rs:799-888`：Discard 交错回归。
- `crates/keelshell-app/src/files/parallel_tests.rs:891-1012`：计划进度交错
  回归。
- `crates/keelshell-core/src/transfer_recovery.rs:251-439`：OS claim lock、
  journal/record 归属检查和 Drop 释放。

独立 reviewer 实际验证：

| 验证 | 结果 |
| --- | --- |
| app `parallel` focused | `19 passed / 0 failed / 0 ignored` |
| 两个新 stale recovery load 回归 | `2 passed / 0 failed` |
| core `transfer_recovery` unit | `8 passed / 0 failed` |
| core `transfer_recovery` integration | `3 passed / 0 failed` |
| 独立 workspace strict Clippy | exit `0` |
| 独立 fmt | exit `0` |
| `git diff --check 5fdd783..e2b00af` | exit `0` |
| merge recommendation | `APPROVE` |

reviewer 没有重复作者完整 `scripts/check.py`；作者 full gate 的证据见上一节。
reviewer 没有把 headless GPUI、loopback 或本地完整门禁扩展为原生三平台、真实
客户 SSH/SFTP、Podman、外部 AI、IME、辅助技术、签名、公证、安装升级或发行
验收。

## 未验证边界

以下范围仍需独立证据，不能由本片通过结果代签：

- macOS、Windows、Linux 原生 GPUI 窗口和完整主题、语言、尺寸矩阵；
- 真实客户 SSH/SFTP、跨进程恢复和重启后的真实远端内容；
- Podman 长流程联调；
- IME、VoiceOver、NVDA 和完整辅助技术验收；
- 签名、公证、安装、升级、回退和发布验收；
- 外部 AI provider；
- 公司内网恢复后的在线依赖最新版本检查。
