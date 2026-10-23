# NET-01 命名隧道规则领域测试记录 — 2026-10-23

## 范围

本记录覆盖 `keelshell-core` 中命名 forwarding rule 的持久化结构、旧状态迁移、连接绑定和显式审核计划。没有启动 SSH、监听 socket、远程转发或真实客户主机。

## 已验证行为

- local、remote、dynamic 三种规则的目标字段约束和 dynamic loopback 绑定约束通过。
- 规则名称、主机、端口、连接 ID、规则版本和最大数量经过有界校验。
- 规则只能绑定 active connection；缺失连接和重复规则 ID 被拒绝。
- `TunnelRule` 的未知字段被拒绝，测试确认伪造 password 字段不会进入反序列化路径。
- 旧 `state.json` 缺少 `tunnel_rules` 时加载为空列表，加载不重写原始字节。
- 保存后通过新 `StateStore` 重新打开，规则字段和 `enabled` 意图完整读回。
- start/stop 计划携带独立 review token；错误 token 不能生成确认收据。
- 领域操作只改内存快照或产生无副作用计划，没有 listener、SSH channel 或后台 task。

## 命令与结果

```text
cargo test -p keelshell-core --locked --offline tunnel_rules -- --nocapture
cargo test -p keelshell-core --test tunnel_rules --locked --offline -- --nocapture
```

结果：core 单元测试 4 项通过；StateStore 集成测试 4 项通过。

## 未完成边界

本记录不证明规则编辑 UI、审核后真实启动/停止、运行状态持久化、重启恢复、重连绑定失效、跨平台原生窗口或发布安装。后续 executor 必须继续绑定当前 connection/route、复核规则 revision，并将未知远程取消结果保持为 UNKNOWN。
