# ADR 0093：NET-01 命名隧道规则与审核闸门

**状态：** Accepted for the persisted rule domain slice
**日期：** 2026-10-23

## Context

NET-01 的运行时转发统计已经完成，但监听规则仍然只存在于当前进程。用户需要为已保存 SSH 连接定义可复用的 local、remote 或 dynamic forwarding 意图，同时避免加载配置时隐式启动监听或把凭据写入 profile 元数据。

## Decision

- `TunnelRule` 是 `AppState` 中的非敏感、版本化记录，包含稳定 ID、名称、连接 ID、转发类型、绑定端点、目标端点、启用意图和规则版本。
- local/remote 规则必须有非空目标主机和非零目标端口；dynamic 规则不保存目标，只允许 loopback 绑定地址。
- 规则必须绑定现有 active connection，规则 ID 唯一，最多保存 256 条；任何命令语法、凭据、密码、token、私钥内容和 runtime handle 都不进入规则。
- 缺失的 `tunnel_rules` 字段从旧 `state.json` 迁移为空列表；加载、校验和保存规则都不会绑定端口、打开 SSH、启动 listener 或自动恢复运行时隧道。
- 删除或移入回收站的连接如果仍被命名规则引用会被 typed error 原子拒绝；批量 Trash/Purge 也使用相同的引用闸门，避免留下不可保存的悬空规则。
- `AppState::plan_tunnel_rule_action` 生成无副作用的精确计划，计划绑定创建时的 state revision、完整非敏感 connection snapshot 和规则 snapshot；`TunnelRuleReviewToken` 还必须与当前 `AppState` 一起确认。连接路由、认证、规则内容或 state revision 变化都会使旧计划失效。
- Tunnels 面板只为当前已保存连接展示匹配规则；选择规则后先生成端点预览，用户点击“审核并启动”才会确认计划并交给现有 forwarding executor。确认后的运行行保留 rule ID，但 listener owner、relay task 和流量统计只存在于当前 live session，不写入 `AppState`。
- executor worker 同时观察 local listener task、remote owner 和 dynamic owner 的终止状态；local listener 因 accept 错误自行结束时也必须收敛到 UI `Finished`，不能只等待共享 SSH session 关闭。
- session 到 saved connection 的绑定由 Workspace 持有，split 和 reconnect 会复制或迁移该绑定；无 saved connection 的临时会话不能执行 named rule。面板收到新的 StateStore snapshot 或连接绑定变化时清除旧选择和 review plan，避免旧 revision 重放。

## Consequences

规则可以安全地进入版本化 StateStore，并能在 UI 中展示为明确的待审核启动/停止意图。存在引用规则时连接删除会失败并保留原 state；旧配置继续可读，规则保存不会改变已有连接和运行时会话。

当前仍需实现规则编辑、删除和持久化管理 UI；stop action 仍沿用当前运行行的 listener owner 停止语义，尚未提供按 rule ID 的独立 stop review。运行状态持久化、重启恢复、重连后旧 owner 失效的完整规则级验收、未知远程结果处理和跨平台原生 UI 仍未完成。规则与本地 fixture 验证不代表真实客户 SSH、三平台原生桌面或发布安装验收。

## Verification note — 2026-10-31

The reviewed named-rule executor was merged as `b218a4a`, followed by the listener lifecycle fix as `3c00ff3`. Independent review of `8268b0a` returned `MERGE` with P0/P1/P2 all zero. The post-merge macOS gate passed core (505), AI (176 passed, 5 ignored), app (766 passed, 2 ignored), workspace checks and the repository validation script. The evidence remains bounded to controlled local fixtures and does not close native macOS, Windows or Linux acceptance.
