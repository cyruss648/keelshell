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
- `AppState::plan_tunnel_rule_action` 生成无副作用的精确计划，`TunnelRuleReviewToken` 绑定一次 start/stop 审核；只有匹配 token 的显式确认才能生成后续执行收据。
- 当前切片只建立持久化领域和审核闸门，尚未把收据交给 forwarding executor，也不把 `enabled` 解释成启动时自动执行。

## Consequences

规则可以安全地进入版本化 StateStore，并能在 UI 中展示为明确的待审核启动/停止意图。旧配置继续可读，规则保存不会改变已有连接和运行时会话。

后续 UI 仍需实现规则编辑、逐条审核、运行状态绑定、启动/停止结果、重启不自动监听、重连后旧 owner 失效和未知远程结果处理。规则领域本身不代表跨平台原生 UI 或真实客户 SSH 验收。
