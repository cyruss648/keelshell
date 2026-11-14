# ADR 0127：命名隧道运行状态持久化与显式恢复

日期：2026-11-14。产品实现基线是 exact HEAD
`fd4cc894aa876f23cde49698b693779e81027489`，主体实现提交为
`093f725037d5c037cfdd7b75a66f7448bc0732a`，最终实现提交为
`7e6ecb3ea8c347f7f633ba0f205d936cd58d72ec`。作者工程门禁完成后，独立严格只读
复核和最终文档提交会在本文末节绑定；原生桌面和真实远端边界单独保持开放。

## 背景

命名隧道规则已经能够保存本地、远程和动态转发意图，并由用户审核后启动。进程
崩溃、强制退出或清理结果不明确时，用户需要知道上次可能运行过哪些命名隧道，
同时不能把旧 SSH 连接、监听器或 worker 当作可以安全恢复的运行时对象。

把完整运行时对象写入 StateStore 会把 socket、session、凭据和执行上下文带入
持久化边界；只写一条没有连接和规则约束的“自动重连”记录又会在规则或路由变化
后产生错误启动。因此本片需要把“过去的观察”与“现在的显式启动授权”分开。

## 决策

### 1. 持久化不可执行的非敏感观察记录

新增 `TunnelRuntimeSnapshot`，保存：

- 快照 ID、当前格式版本、启动时的完整非敏感 `TunnelRule`；
- connection ID、按顺序排列的 profile IDs、规范化路由身份；
- 每个跳板的认证类别；
- 认证目标的域隔离 SHA-256 指纹；
- worker 实际报告的监听端点、首次监听时间和候选状态。

快照不保存密码、私钥内容、私钥路径原值、vault credential ID 原值、SSH session、
socket、worker handle、代理凭据、命令内容或流量原始数据。路径和 credential ID
只作为内存中的指纹输入，持久化字段不能反推原值。

未知非零快照版本可以作为不透明记录继续保存，确保降级时不静默丢失候选；当前
构建的恢复规划拒绝不支持版本。快照加载和 `AppState::validate` 只验证数据，不
连接、不启动、不复用旧运行时。

### 2. 绑定当前规则、连接路由和认证目标

恢复规划必须同时通过：

- 快照版本和字段边界检查；
- 规则 ID 与规则完整内容检查；
- connection ID 存在性检查；
- profile ID 顺序和规范化 endpoint/route identity 检查；
- 认证类别和认证目标指纹检查；
- `CleanupUnconfirmed` 状态检查；
- 当前 StateStore 状态有效性检查。

任何不匹配都返回 typed `TunnelRecoveryError`，候选保留，界面只提供重新加载、
重新选择、重新审核或显式 Discard 的路径。

### 3. 用 state-bound review token 隔离观察和启动

`plan_tunnel_runtime_recovery` 是无副作用的当前状态审核，生成绑定候选 ID 和
`SnapshotRevision` 的 `TunnelRecoveryReviewToken`。确认时再次验证 token、
revision、候选完整内容和当前状态。成功确认返回的对象没有执行能力；面板只把它
转换成当前规则和当前连接绑定的全新 worker 启动请求。

确认不会传递旧 listener、SSH session、socket、worker handle、代理凭据、命令或
流量。候选也不会因为确认而隐式删除：若旧 worker 仍然存在，用户必须通过当前
清理路径解决旧候选；启动失败不会被解释成旧运行已停止。

### 4. 把 StateStore 写入放在 Workspace 后台执行

隧道面板只产生 `TunnelRuntimeEvent::Upsert` 或 `Remove`。事件带有预期
`AppState`、操作 ID和候选 ID；Workspace 在后台 executor 中重新检查面板 owner、
当前 revision 和完整 state equality，再调用现有 StateStore 保存路径。

保存结果未知、冲突、耐久性错误或面板审核过期时，Workspace 使隧道面板进入
fail-closed 状态，并保留候选。成功清理确认才发出 Remove 的成功回执。GPUI UI
线程不执行阻塞状态文件 I/O。

### 5. 用两个候选状态表示崩溃和未知清理

- `PossibleRunning`：worker 曾报告监听端点，但当前进程没有证明它已经停止；
- `CleanupUnconfirmed`：清理路径没有得到确认，所有恢复动作都被阻止。

正常显式停止且监听器和所属连接流释放已确认时，候选被删除。进程崩溃、重启、
未知结果和未确认清理保留候选。用户可以只读 Inspect，也可以显式 Discard；
Discard 删除观察记录，不执行网络动作。

### 6. 复用现有工作台主题和审核布局

恢复卡片和审核区只消费现有主题 registry、Palette 与组件 token，使用 Frost、
Graphite、Lavender 三套主题的语义 surface/border/warning 颜色。业务代码不按
theme ID 分支。卡片和审核正文遵循现有 bilingual 文本、fixed footer 和 bounded
scroll 约束，状态文案明确“可能仍在运行/需要重新审核”，不画在线指标。

## 后果

用户可以在重启后找到过去的命名隧道观察记录，并在当前连接和规则完全匹配时，
通过一个可审计的显式审核流程启动全新的 worker。配置变化、路由变化、认证目标
变化、保存不确定和清理不确定都会停止恢复链路，减少静默重连或错误连接的风险。

StateStore 中会多出最多 256 条有界的非敏感候选。未知候选可能长期保留，直到
确认清理或用户显式 Discard；这是刻意保守的状态，而不是自动清理策略。

实现增加了 core 序列化/校验和恢复规划 API，也增加了 app 的运行状态事件、面板
恢复卡片及后台保存回执处理。session 传输层继续负责实际 worker 与清理；本片不
扩展传输协议，也不声称能探测另一个进程中的旧 listener。

## 拒绝的方案

- **自动重连**：会把历史观察记录误当成当前授权，且无法安全重建密码输入、host
  key、跳板认证和当前连接。
- **持久化 session/socket/worker handle**：对象不可跨进程恢复，并会将敏感或
  进程本地状态写入磁盘。
- **只比较 connection ID**：不能发现规则、跳板路线、端点或认证目标变化。
- **保存明文私钥路径和 vault credential ID**：超出本片非敏感持久化边界；改用
  域隔离指纹做变化检测。
- **保存失败时立即删除候选**：未知结果可能代表磁盘已经写入，删除会丢失审计和
  后续处理入口。

## 验证与边界

源码和测试绑定最终实现提交
`7e6ecb3ea8c347f7f633ba0f205d936cd58d72ec`、作者独立 Cargo target、
独立复核 Cargo target 和原始门禁日志。独立严格只读复核报告位于
`/Users/hdh/.codex/worktrees/6711/keelshell/work/review-final-7e6ecb3/review-report.md`，
项目内归档摘要见
[独立复核记录](../testing/records/2026-11-14-named-tunnel-runtime-recovery-review.md)；
其 P0/P1/P2/P3 计数均为 `0`，merge recommendation 为 `APPROVE`。独立复核实际
验证了 core `8/8`、app 命名规则 `19/19`、隧道面板 `22/22`、测试清单
`1146` 项、fmt、core/app 严格 Clippy 和 diff check；作者完整
`scripts/check.py` 的通过证据仍绑定作者日志，而不是被独立复核重复声称。

对应的完整测试记录见
[2026-11-14 测试记录](../testing/records/2026-11-14-named-tunnel-runtime-recovery.md)。

本 ADR 不把 GPUI headless 回归当成 macOS/Windows/Linux 原生桌面验收，也不把
loopback/fixture 连接当成真实客户 SSH 或 Podman 联调。IME、辅助技术、窗口级
主题/滚动、真实远端、签名、公证、安装升级和发布仍需独立证据。
