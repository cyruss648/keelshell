# 命名隧道运行状态持久化与显式恢复测试记录 — 2026-11-14

产品实现基线：exact HEAD
`fd4cc894aa876f23cde49698b693779e81027489`。

源码与测试实现提交范围：
`093f725037d5c037cfdd7b75a66f7448bc0732a`（主体）到
`7e6ecb3ea8c347f7f633ba0f205d936cd58d72ec`（最终修复）。

作者 worktree：
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`

独立 Cargo target：
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/named-tunnel-runtime-20261114/target`
（证据采集后已清理）

完整门禁日志：
`work/named-tunnel-runtime-20261114/logs/full-gate-7e6ecb3-rerun.log`

## 结论

作者工程门禁已在上述 exact implementation SHA 和独立 target 上实际完成并通过：

- `python3 scripts/check.py`：exit `0`；
- Python：`131 run / 1 skipped`；
- workspace ordinary Rust：`2476 passed / 0 failed / 23 ignored`；
- app 全量：`1144 passed / 0 failed / 2 ignored`；
- doctest：14 项全部通过，其中包含 2 项 compile-fail；
- `cargo fmt --all -- --check`：通过；
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过；
- readiness、observer、small-stack 三组 AI controller：通过；
- 依赖版本策略检查：通过。

完整门禁在 app 测试链接阶段输出已知 macOS linker warning：
`__eh_frame section too large (max 16MB) to encode dwarf unwind offsets in compact unwind table`。
它没有改变门禁 exit `0`，也没有形成本片的编译失败。

独立严格只读复核已经完成，原始报告已归档为
[项目内原始复核报告](2026-11-14-named-tunnel-runtime-recovery-review-raw.md)。
项目内归档摘要见
[独立复核记录](2026-11-14-named-tunnel-runtime-recovery-review.md)。
复核固定 exact SHA
`7e6ecb3ea8c347f7f633ba0f205d936cd58d72ec`，使用独立 worktree 和独立 target，
结论为 `APPROVE`，P0/P1/P2/P3 均为 `0`。复核没有修改源码、测试、Cargo 或
项目文档，只在其 review artifact 目录保存日志和报告。

## 本片 focused 回归

所有命令均使用上方独立 `CARGO_TARGET_DIR`、`--locked` 和当前本机 Rust/Cargo
`1.100.0`：

| 命令 | 实际结果 |
| --- | --- |
| `CARGO_TARGET_DIR=work/named-tunnel-runtime-20261114/target cargo test -p keelshell-core --test tunnel_runtime --locked --offline -- --nocapture` | `8 passed / 0 failed; exit 0` |
| `CARGO_TARGET_DIR=work/named-tunnel-runtime-20261114/target cargo test -p keelshell-app --bin keelshell-app --tests --locked --offline tunnel_rule_tests -- --nocapture --test-threads=1` | `19 passed / 0 failed / 1127 filtered out; exit 0` |
| `CARGO_TARGET_DIR=work/named-tunnel-runtime-20261114/target cargo test -p keelshell-app --bin keelshell-app --tests --locked --offline tunnels -- --nocapture --test-threads=1` | `22 passed / 0 failed / 1124 filtered out; exit 0` |
| `CARGO_TARGET_DIR=work/named-tunnel-runtime-20261114/target cargo clippy -p keelshell-core --all-targets --locked --offline -- -D warnings` | 通过；完整门禁 exit 0 |
| `CARGO_TARGET_DIR=work/named-tunnel-runtime-20261114/target cargo clippy -p keelshell-app --bin keelshell-app --tests --all-features --locked --offline -- -D warnings` | 通过；完整门禁 exit 0 |
| `cargo fmt --all -- --check` | 通过 |
| `git diff --check` | 通过 |

focused 用例覆盖：

- 序列化后重新加载只保留规则、连接绑定、端点和时间等非敏感观察数据；
- 私钥路径和 vault credential ID 原值不写入快照；
- 规则、connection ID、路由、跳板、认证类别和认证目标变化 fail closed；
- 不支持的快照版本、StateStore revision 变化、review token 不匹配和保存冲突
  不能启动；
- 崩溃候选只读展示，必须 Inspect、Review start、Confirm and start fresh；
- 未确认清理保留候选并禁止恢复；
- 正常确认清理和用户明确 Discard 只移除选定候选；
- app 事件由 Workspace 接收，StateStore 保存位于后台 executor；
- 三套主题、双语文案和 fixed footer/scroll 的恢复卡片与审核控件保持可达。

## P0/P1/P2/P3 自查

作者实现自查：

| 级别 | 数量 | 说明 |
| --- | ---: | --- |
| P0 | 0 | 未发现会绕过显式确认、泄露凭据或自动重连的阻断问题 |
| P1 | 0 | 未发现会让恢复流程在规则/路由/保存不确定时继续启动的问题 |
| P2 | 0 | 定向测试和实现检查未发现当前切片内的中等级别缺陷 |
| P3 | 0 | 当前切片内未登记低等级实现问题 |

这四项是作者自查，不替代独立复核。独立复核必须重新给出自己的计数和证据。

## 独立严格只读复核

| 级别 | 数量 | 结论 |
| --- | ---: | --- |
| P0 | 0 | 未发现绕过显式确认、泄露敏感数据或复用旧运行时的问题 |
| P1 | 0 | 未发现规则/连接/路由/认证目标/保存不确定时继续启动的问题 |
| P2 | 0 | 未发现当前切片内会影响主要流程的中等级别缺陷 |
| P3 | 0 | 未发现需要登记的低等级实现缺陷 |

独立 target：
`/Users/hdh/.codex/worktrees/6711/keelshell/work/review-final-7e6ecb3/target`。
独立复核实际结果：

- core `tunnel_runtime`：`8 passed / 0 failed / 0 ignored`；
- app `tunnel_rule_tests`：`19 passed / 0 failed / 1127 filtered`；
- app `tunnels`：`22 passed / 0 failed / 1124 filtered`；
- app 测试清单：`1146 tests listed`；
- `cargo fmt --all -- --check`、core/app 严格 Clippy 和
  `git diff --check fd4cc894...7e6ecb3`：全部 exit `0`。

app 测试链接阶段出现已知 macOS warning：
`__eh_frame section too large`；它没有改变 exit code 或测试结果。
独立复核没有重复运行作者完整 `scripts/check.py`，因此完整 Python/Rust
计数仍以作者日志为证据。

上述作者/复核 target 和评审 worktree 均在证据采集、原始报告归档后清理；
作者专项日志仍保留在 `work/named-tunnel-runtime-20261114/logs/`。

## 未验证边界

本记录没有把以下内容写成通过：

- macOS 真实窗口级 GPUI 视觉、主题切换、完整辅助技术和 IME；
- Windows/Linux 原生窗口、输入法和辅助技术；
- 真实客户 SSH、真实跳板/认证服务和跨进程旧 listener 探测；
- Podman 容器联调、断电耐久性和跨机器 StateStore；
- 签名、公证、安装升级、发布制品和 CI runner；
- 自动发现另一个进程仍在运行的隧道；本片故意只保留可能运行候选并要求人工
  重新审核；
- 完整业务验收和长时流量稳定性。

`cargo test`、loopback/fixture 和 headless GPUI 结果只证明当前受控本地路径。

## 证据保留与后续

实现提交之后才能由独立复核确认是否需要修订源码。若复核发现 P0/P1/P2/P3
问题，修复必须产生新的本地实现提交，并以新的 exact SHA 重新执行相关 focused
测试、Clippy、fmt、完整门禁和文档绑定；不得在本记录中覆盖旧失败原件。

文档设计见[ADR 0127](../../adr/0127-named-tunnel-runtime-recovery.md)，产品边界
见[命名隧道规则管理](../../product/NAMED_TUNNEL_RULE_MANAGEMENT.md)。
