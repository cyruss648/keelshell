# SSH 跳板路线领域与存储回归 — 2026-10-03

## 数据与接口

`Connection.jump_host: Option<Uuid>` 关联一个保存的 SSH 配置；缺省为直连，不增加任何明文秘密。`AppState::connection_route` 返回独立的 `ConnectionRoute` 元数据快照，顺序为最外层跳板至最终目标，最多四个跳板、五个 SSH 端点。单独连接的 identity 也包含一个端点，便于统一凭据绑定。

`RouteIdentity` 是带版本号的规范化端点数组，通过现有 serde JSON 序列化。端点由 host、port、username 组成；DNS 小写并去末尾点，IP 使用标准形式，用户名区分大小写。`RouteEndpoint::from_connection` 对应用层开放同一套校验及规范化规则。身份不含本地 UUID、名称、目录、标签或凭据引用。

每条活动路线只能经过活动配置。活动与回收站的完整图都拒绝 nil、自引用、循环、缺失、重复身份及过深路线。回收被活动配置引用的跳板会失败；先回收依赖者后可以回收跳板。恢复时须先恢复跳板。永久删除仍被活动或回收站配置引用的节点会失败，禁止自动清除跳板并降级为直连。

复制仅给当前配置生成新身份，保留跳板关联，清除复制品自己的凭据引用。`update_connection` 在候选状态完整校验后原子提交；路线身份改变或某节点认证方式/私钥路径改变时，清除该节点及受影响下游的凭据引用和 recent 记录，回收站的相关凭据引用也清除。加密条目本身不删除。仅改变名称、标签、收藏或等价 DNS 拼写保留身份和引用。

## 信任作用域

首跳继续使用原 `known_hosts` 的 `[host]:port` key。后续跳转的 pin 放入默认空的 `route_known_hosts`，key 是截至当前端点的 canonical `RouteIdentity` JSON。读取不会回退到全局 pin；不同跳板后的同一个私网地址不共享信任。修改上游端点或用户名后产生新作用域，必须重新核对下游。

保存校验版本、深度、规范端点、严格 JSON key 和 fingerprint；direct 与 routed pin 合计最多 10,000，整体存档仍最多 4 MiB。应用须在批准异步指纹和提交认证前重新核对路线快照，领域 pin API 本身不等同于用户批准。

## 导入与兼容

导出仍包括全部活动配置和目录，因活动路线完整性而自然包含跳板闭包；每个配置的 vault 引用、所有 pin、回收站与 recent 都不导出。

导入先验证输入完整图，不能用外来 UUID 猜测本地跳板；两阶段生成本地身份映射并改写 jump 引用，保留原显示顺序。去重按完整规范路线而非仅最终地址。引用复用要求唯一活动匹配与整条认证配置兼容；ID 冲突、回收站、不同认证或多个候选不能成为新节点的跳板，整次导入原子失败。完全相同的重复定义可去重；同 ID 的不同定义拒绝。重导入不覆盖本地已有配置或复活回收项。

旧 schema 1 状态缺少新增字段时仍可读取，直连配置序列化省略空 jump，空 routed pin map 也省略。新版本不改变 schema 编号或增加依赖。旧版本遇到实际包含新字段的文件会因严格未知字段校验而拒绝加载，不能用旧程序覆盖带路线的新状态。

## 验证

```sh
cargo test -p keelshell-core --test profiles --test connection_library --test host_keys --locked
cargo test -p keelshell-core --test routes --locked
cargo clippy -p keelshell-core --all-targets --locked -- -D warnings
cargo test -p keelshell-core --locked -- --test-threads=4
```

- 既有 profile、library、host-key 回归 39 项通过。
- 新路线回归 25 项通过：图边界与 owned 顺序快照、旧文档、元数据等价性、复制、回收/恢复/删除、精确下游失效、scope 隔离、pin 总量/格式、打乱顺序导入、身份重映射、同私网不同路径、别名去重、映射冲突与外来引用拒绝。
- 临时 StateStore 真文件验证路线和 pin 往返，以及另一个 store 提交后旧路线草稿不能覆盖磁盘。
- 完整 core 为 147 项单元/集成测试和 1 项文档测试；严格 Clippy 通过。
- 独立只读复审核对路线身份、导入映射/歧义、active/trash 删除约束、候选原子性、存储冲突与下游引用失效，无阻断发现；两处沿用旧 endpoint 表述的 rustdoc 已更新为完整路线。

最终完整回归使用独立 TMPDIR 和四个测试线程。日志保存在 ignored `work/jump-domain-existing-tests.log`、`work/jump-domain-tests.log`（最初 23 项）、`work/jump-domain-clippy-final.log`、`work/jump-domain-core-tests.log`（最终 25 项）。

## 证明边界

本记录证明领域、导入与本机存储规则。不会据此认定多跳 SSH 网络、逐跳认证/指纹交互、共享连接取消清理、凭据 v2 绑定或跨平台原生界面已通过；这些由传输与应用集成记录分别验证。路线信任不自动批准新指纹，配置文件仍应由用户保管。
