# NET-01 命名隧道规则领域测试记录 — 2026-10-23

## 范围

本记录首先覆盖 `keelshell-core` 中命名 forwarding rule 的持久化结构、旧状态迁移、连接绑定和显式审核计划；2026-10-31 增量补充当前 GPUI Tunnels 面板到真实 forwarding executor 的审核启动链。所有联调仍使用内存 SSH server 和临时 StateStore，没有真实客户主机。

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

## 2026-10-31 executor 增量

- Workspace 为每个远程 tab 维护 saved connection ID；split tab 继承该绑定，reconnect replacement 迁移该绑定，ephemeral quick connect 没有规则执行权限。
- Tunnels 面板只显示当前连接匹配的规则；规则选择生成端点预览，禁用规则不能生成 start plan。
- “审核并启动”重新用当前面板快照确认 `TunnelRuleReviewToken`，确认失败会清除旧计划；确认成功后才调用已有 local、remote 或 dynamic forwarding worker，运行行保留 `rule_id`。
- StateStore snapshot 或绑定刷新会清除 selection/review plan，旧规则不会自动重放；运行时 owner 仍由当前 live session 管理，配置加载不会启动 listener。
- GPUI/SSH fixture 测试通过真实 dynamic SOCKS5 listener 读取实际 `socks5h://` 地址，并验证 named rule 运行行带有 rule ID；原有 free-form dynamic/local/remote 生命周期测试继续覆盖。

## 2026-10-31 listener 生命周期修复

- 独立复核发现 LocalForward listener task 因 accept 错误自行结束时，app 只检查 DynamicForward 的 owner 状态，导致 worker 可能一直等待而不发送 `Finished`。`LocalForward::is_closed` 现在反映 listener task 的终止状态，统一 `Forward::is_closed` 检查 local、remote 和 dynamic owner；新增 session 回归测试覆盖已结束的 listener task。
- 本轮修复后的格式检查、diff 检查、session forwarding focused tests（5 passed）、app tunnel focused tests（5 passed）、session/app 严格 Clippy 均通过。完整 workspace 门禁和最新独立 reviewer 仍待补记。

## 命令与结果

```text
cargo fmt --all -- --check
cargo clippy -p keelshell-core --all-targets --all-features --locked --offline -- -D warnings
cargo test -p keelshell-core --locked --offline tunnel_rules -- --nocapture
cargo test -p keelshell-core --test tunnel_rules --locked --offline -- --nocapture
cargo test -p keelshell-core --locked --offline
cargo test -p keelshell-app --locked --offline dynamic_panel_starts_real_proxy_copies_address_switches_language_and_stops -- --nocapture
cargo clippy -p keelshell-app --all-targets --all-features --locked --offline -- -D warnings
```

结果：领域测试记录中的 core persistence 场景和本轮 executor 的格式检查、严格 Clippy、GPUI/SSH 聚焦测试通过；修复后的 app tunnel 聚焦测试为 5 passed、763 filtered out，session forwarding 聚焦测试为 5 passed、421 filtered out。完整门禁和最新独立 reviewer 仍需在修复提交上重新执行。`tunnel_rules` 集成测试继续覆盖规则持久化、删除引用、批量 Trash、规则变更、规则删除和 state revision 失效。

## 未完成边界

本记录仍不证明规则编辑/删除 UI、按规则 stop review、运行状态持久化、重启恢复、规则级重连 owner 失效、远程未知结果处理、跨平台原生窗口或发布安装。当前已证明的启动链只覆盖本机 macOS 上的受控 GPUI 和内存 SSH fixture，不能替代真实客户 SSH 或 macOS/Windows/Linux 原生验收。
