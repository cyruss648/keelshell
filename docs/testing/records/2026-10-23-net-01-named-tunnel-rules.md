# NET-01 命名隧道规则领域测试记录 — 2026-10-23

## 范围

本记录覆盖 `keelshell-core` 中命名 forwarding rule 的持久化结构、旧状态迁移、连接绑定和显式审核计划。没有启动 SSH、监听 socket、远程转发或真实客户主机。

## 已验证行为

- local、remote 共用的目标字段校验分支和 dynamic loopback 绑定约束通过；专项测试直接构造 local 与 dynamic，remote 分支由同一校验实现覆盖，仍缺独立 remote fixture。
- 规则名称、主机、端口、连接 ID、规则版本和最大数量经过有界校验。
- 规则只能绑定 active connection；缺失连接和重复规则 ID 被拒绝。
- `TunnelRule` 的未知字段被拒绝，测试确认伪造 password 字段不会进入反序列化路径。
- 旧 `state.json` 缺少 `tunnel_rules` 时加载为空列表，加载不重写原始字节。
- 保存后通过新 `StateStore` 重新打开，规则字段和 `enabled` 意图完整读回。
- start/stop 计划携带独立 review token，并绑定 state revision、规则 snapshot 和 connection snapshot；错误 token、连接更新、规则修改、规则删除或再次保存 state 后的旧计划都不能生成确认收据。
- soft delete、hard remove、purge 和批量 Trash/Purge 的生产路径都 fail closed；专项测试直接覆盖 soft delete、hard remove、purge 和包含一个被引用连接的多选批量 Trash，原 state 保持可验证。有效 state 不允许 deleted connection 被规则引用，因此批量 Purge 的引用分支是防御性检查。
- 领域操作只改内存快照或产生无副作用计划，没有 listener、SSH channel 或后台 task。

## 命令与结果

```text
cargo fmt --all -- --check
cargo clippy -p keelshell-core --all-targets --all-features --locked --offline -- -D warnings
cargo test -p keelshell-core --locked --offline tunnel_rules -- --nocapture
cargo test -p keelshell-core --test tunnel_rules --locked --offline -- --nocapture
cargo test -p keelshell-core --locked --offline
```

结果：格式检查和严格 Clippy 通过；core 全量测试 494 项通过；`tunnel_rules` 集成测试 10 项通过。测试记录中的删除引用、批量 Trash、规则变更、规则删除和 state revision 失效场景均通过。

## 未完成边界

本记录不证明规则编辑 UI、审核后真实启动/停止、运行状态持久化、重启恢复、重连绑定失效、跨平台原生窗口或发布安装。后续 executor 必须继续绑定当前 connection/route、复核规则 revision，并将未知远程取消结果保持为 UNKNOWN。
