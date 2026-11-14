# 命名隧道运行状态持久化与显式恢复独立复核 — 2026-11-14

复核类型：严格只读代码与功能复核。复核 agent 没有修改源码、测试、Cargo 或
项目文档，没有提交、push、merge 或清理目标源码。

## 冻结输入

- 产品基线：`fd4cc894aa876f23cde49698b693779e81027489`
- 复核实现 SHA：`7e6ecb3ea8c347f7f633ba0f205d936cd58d72ec`
- 复核 worktree：`/Users/hdh/.codex/worktrees/6711/keelshell`（证据采集后已清理）
- 独立 Cargo target：
  `/Users/hdh/.codex/worktrees/6711/keelshell/work/review-final-7e6ecb3/target`（证据采集后已清理）
- 原始复核报告：
  [项目内归档原文](2026-11-14-named-tunnel-runtime-recovery-review-raw.md)
- 工具链：`rustc 1.100.0`、`cargo 1.100.0`

## 结论

独立复核结论为 `APPROVE`，建议在作者绑定最终文档提交后合并该实现 SHA。
P0/P1/P2/P3 均为 `0`。复核确认：

- 快照边界只保存非敏感规则、连接路由/认证目标指纹、监听端点、时间和候选
  状态；不保存密码、私钥、私钥路径、vault ID、代理密码、session、socket、
  worker、命令或流量；
- 规则、连接、路由、跳板、认证类别、认证目标、版本、StateStore revision、
  cleanup 状态和 review token 均在显式恢复前重新检查；
- 恢复结果没有执行能力，不会自动连接、自动启动、复用旧 listener/session/
  worker/traffic 或重放命令；
- StateStore 事件在 Workspace 现有后台持久化路径执行，GPUI 面板只产生事件；
- Inspect、Review start、Confirm and start fresh、Discard 流程和双语主题 token
  卡片均可达。

## 独立验证

- core `tunnel_runtime`：`8 passed / 0 failed / 0 ignored`；
- app `tunnel_rule_tests`：`19 passed / 0 failed / 1127 filtered`；
- app `tunnels`：`22 passed / 0 failed / 1124 filtered`；
- app 测试清单：`1146 tests listed`；
- `cargo fmt --all -- --check`：exit `0`；
- core 严格 Clippy：exit `0`；
- app 严格 Clippy：exit `0`；
- `git diff --check fd4cc894...7e6ecb3`：exit `0`。

app 测试链接阶段出现已知 macOS warning：
`__eh_frame section too large`，没有改变测试结果或命令 exit code。作者完整
`scripts/check.py` 没有由该复核重复执行，Python、workspace ordinary Rust、
app 全量和 doctest 计数继续以作者测试记录及其日志为证据。

## 未验证边界

该复核没有把 macOS/Windows/Linux 原生窗口、真实客户 SSH、跨进程 listener 探测、
Podman、断电耐久性、CI、签名、公证、安装升级、发布制品或业务验收写成通过。
本片保留“可能仍在运行”观察候选并要求人工复核，不能自动发现另一个进程中的旧
listener。
