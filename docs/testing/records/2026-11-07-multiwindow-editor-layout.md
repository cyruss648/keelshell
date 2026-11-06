# 独立工作窗口与编辑器动作可达性 — 2026-11-07

状态：源码候选已导入当前集成分支；定向验证与完整工程门禁通过；独立代码复核与真实桌面验收待完成。
时间使用 Asia/Shanghai。源码测试不能替代 macOS、Windows 或 Linux 原生窗口验收。

## 本轮范围

本轮接入两项下一片能力：

- 独立工作窗口：真正创建新的 GPUI Kit 原生窗口；新窗口拥有独立的
  `Workspace`、终端、SSH、草稿、AI 审核和远程面板。共享 `StateStore` 通过
  `AppWindows` 和 `ConfigurationGate` 串行化配置写入，并保留冲突、耐久性未知和
  显式 reload 语义。
- 编辑器动作可达性：内联编辑器和三方合并手工输入使用有界输入高度；正文和
  完整版本继续独立滚动，底部审核动作保持在可见动作区，不随正文滚动丢失。

窗口关闭先建立精确快照并要求用户确认，确认后按窗口 owner 撤销输入、暂停面板、
关闭 SSH 并有界 join worker。未确认的清理结果保留 owner，不自动重试。

## 实际验证

在切换隔离源码后，按 `docs/testing/SHARED_TARGET.md` 结束当前 KeelShell Cargo
进程并执行项目包定向清理，随后重新构建测试二进制。测试清单确认当前 app 二进制
包含 1,075 个测试，以及本轮新增用例。

| 检查 | 结果 |
| --- | --- |
| `cargo clean --locked --offline -p keelshell-app -p keelshell-core -p keelshell-session -p keelshell-ai -p keelshell-mcp` | 退出 0，移除 47,072 个文件 / 9.2 GiB 项目产物 |
| `cargo test -p keelshell-app --locked --offline -- --list` | 退出 0；1,075 tests / 0 benchmarks |
| `app_windows::tests::` | 4 passed |
| `workspace::tests::windows::` | 12 passed |
| `workspace::tests::routes::windows::` | 1 passed；真实 loopback SSH 双窗口清理与存活验证 |
| `terminal::workers::tests::` | 3 passed；窗口级 worker 回收、失败回执和 deadline 保留 |
| `files::transfer_tests::review_viewport_tests::` | 11 passed；900×580/1440×900、双语、键盘滚动和动作区 |
| `parallel_transfers::` | 11 passed；两个 macOS 临时目录根路径回归使用 canonicalized temporary root |
| `cargo test -p keelshell-app --locked -- --test-threads=4` | 1,075 passed / 2 ignored |
| `cargo test --workspace --locked -- --test-threads=4` | 2,397 passed / 23 ignored；11 rustdoc；0 failed |
| `python3 scripts/check.py` | 退出 0；Python 131 run / 1 skipped、workspace strict Clippy、依赖策略和 readiness/observer/small-stack controller 均通过 |
| `cargo fmt --all -- --check` | 退出 0 |
| `cargo clippy -p keelshell-app --all-targets --locked --offline -- -D warnings` | 退出 0，无问题 |
| `git diff --check` | 退出 0 |

## 未完成边界

本轮完整工程门禁已经通过；尚未完成发行包构建、精确提交 CI 或真实原生桌面
流程。尤其需要重新观察：

- macOS 多窗口创建、关闭、Quit/Install 聚合审核和窗口几何；
- macOS inline editor 的 `merge-adopt`、`merge-save`、late-save 与连续无换行保存；
- Windows/Linux 原生窗口、焦点、IME、辅助技术和多显示器行为。

本轮已启动两次新的非作者只读代码与功能复核，但均未在有界等待内返回可审计报告；
它们已停止且没有修改工作树。父级只读差分检查修正了新增的 worker stderr 输出，
但不替代非作者复核。固定源码提交和后续交付仍需独立复核报告、发行包构建及真实
原生验收。

## 2026-11-07 继续推进

当前提交 `ac397c87f3fc49e1e1f940ecd99fab20566c7a23` 已推送到
`feature/main-visual-integration-20261106`，并实际读回本地、tracking、远端三者
一致；`main` 未改变。

继续回归结果：

| 检查 | 结果 |
| --- | --- |
| `cargo test -p keelshell-app --locked --offline -- app_windows::tests:: --test-threads=4` | 5 passed |
| `cargo test -p keelshell-app --locked --offline -- workspace::tests::windows:: --test-threads=4` | 14 passed |
| `cargo test -p keelshell-app --locked --offline -- files::transfer_tests::review_viewport_tests:: --test-threads=4` | 11 passed |
| `cargo run -p keelshell-app --locked --offline` with isolated `KEELSHELL_DATA_DIR` | compiled and launched; manually stopped after startup |
| temporary state cleanup | verified absent |
| first `python3 scripts/check.py` after docs commit | exit 1; `local_agent_process --controller-small-stack` ended with `EmptyReply` |
| exact `cargo test -p keelshell-ai --test local_agent_process --locked -- --controller-small-stack` rerun | passed; all scenario groups/assertions passed |
| second `python3 scripts/check.py` | exit 0; Python 131 / 1 skipped, workspace 2,397 ordinary / 23 ignored, 11 rustdoc, strict Clippy and all AI controllers passed |
| independent non-author review | unavailable; no report produced |
| native macOS multi-window/editor interaction | NOT_RUN; CUA repeatedly reported locked desktop |

三次本轮非作者复核尝试都没有修改源码或项目资源：一次在有界等待内无报告，
一次被供应商 API-Key 限制拒绝，一次在工具调用循环中未进入审查正文。上述事实不
改变“独立复核待完成”的交付边界。
