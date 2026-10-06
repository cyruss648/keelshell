# 2026-10-06 加密连接同步候选

候选从公开精确 `24a19b954b2634c3f54d9554e5ce9b04a0062e9b` 创建独立工作树，实现 [SYNC-02](../../product/PROFILE_SYNC.md)，没有修改根在途原生验收输入、上传真实配置、启动用户 SSH 或发布标签。

## 作者实现与行为证据

core 新增 21 项同步测试：5 项真实本机 pending 状态/继续确认/放弃/解除关联/认证重放内部用例，6 项两个隔离 StateStore 与同一共享密文目录的集成用例，以及 10 项独立构造认证 wire 和多记录冲突选择的集成用例。覆盖双端拉取、审核后发布、离线并发编辑、删除与离线编辑的选择、旧快照与旧记录版本、丢失墓碑、同版本不同密文、密码错误、篡改、未知字段、目录变更、逐项选择缺失/多余/替换、取消和审核后本机/共享源变更。

初始隔离客户端使用尚无同步关联的现有 AppState 文档；可选 `profile_sync` 在自定义 wire 读取中缺省为 `None`，在未配置时不序列化。原连接的本机私钥认证保持，另一客户端接受连接后使用 Agent 且没有凭据引用。共享记录结构不能表示认证方式、私钥路径、vault、主机信任、AI、历史或输出；认证后注入凭据引用字段被拒绝。同步密码没有写入本机元数据或共享文档。

5 项生产 GPUI 用例覆盖实际后台 handler 与密码提交后清空、未逐项选择时批准点击无效、中文/英文与明暗主题的小窗口固定确认区、41 条长内容跨 3 页的真实滚动/点击/选择保留、连接库实际入口与同步保存后 transport 实例及远端引用保持，后者观察到 0 次 `TerminalCommand::Write`。另一个工作区用例把审核期间到达的真实使用记录与有效批量审计加入队列，确认窗口关闭后两者均实际保存。

测试作者不能充当最终非作者审查者；这些 GPUI 受控 transport 与本机文件系统测试不代表原生桌面或真实网络挂载验收。

## 门禁与失败记录

| 范围 | 实际结果 | ignored 本地证据 |
| --- | --- | --- |
| 第一轮完整工程检查 | 6 Python、格式、依赖策略通过；测试 fixture 的 unwrap 违反严格 Clippy，exit 1；本轮没有运行完整 Rust 套件 | `work/sync-gate/full-failure-v1/`，578 输入前后相等 |
| 第二轮完整工程检查 | 严格 Clippy 通过；应用 491 通过、2 失败、2 忽略，exit 1；英文最小窗口全局主题按钮越界，AI 审核入口用例当时仍停留连接管理器而缺少 assistant 内容 | `work/sync-gate/full-failure-v2/`，578 输入前后相等 |
| 第三轮完整工程检查 | `python3 scripts/check.py` exit 0，547.30 秒；1,264 普通 Rust、8 doc、6 Python 通过，13 按既有约定忽略；格式、x.y 策略、workspace/all-targets/locked 严格 Clippy，以及默认和额外小栈 CLI 进程夹具均通过 | `full-check.log`、`full-check-receipt.json`、`full-inputs-before.json`、`full-inputs-after.json`，578 输入前后相等 |
| 最终双语文案增量与 macOS 构建 | 将审核卡片 Rust Debug 枚举改成中英文代理、重连和收藏说明后，格式、策略、严格 Clippy、整个应用 494 通过/2 忽略、app/MCP 构建、57 包装 Python 测试、标准 stage 与 macOS native 结构检查均 exit 0；211.80 秒 | `final-ui-check.log`、`final-ui-check-receipt.json`、`final-ui-inputs-before.json`、`final-ui-inputs-after.json`，578 输入前后相等 |
| late receipt 负向与修复验证 | 分别受控移除 recent 与 batch flush 的同步保护，完全限定生产 GPUI 用例每次真实执行 1 项并 exit 101，观察待保存队列从应有的 1 项变为 0；恢复原始源码后同一用例 exit 0，实际关闭后两类记录均保存；19.56 秒 | `lease-without-recent-guard.log/.patch`、`lease-without-batch-guard.log/.patch`、`lease-restored-positive.log`、`lease-counterexample-receipt.json`，578 输入前后相等 |

修复把同步入口放入可换行的连接库工具栏，并保留全局工具栏原布局；第三轮及最终应用套件都重新通过原最小窗口与 AI 审核目标用例。同步窗口持有本机持久化操作范围时，recent/batch flush 必须在取出队列之前返回，关闭后统一补写；只阻止一般 persist 会丢失已经取出的记录。负向 patch 属于作者受控反例，没有混入最终源码或标准包。

第三轮完整日志 SHA-256 为 `46e604003435cea124b2c32f8c23d10ecc133334a3c2a5c2021efb7623cc671c`；最终双语增量与 macOS 检查日志 SHA-256 为 `8958df2f7e784dc3f9b8ace001eea9de3f1161082741b9851e432dacfbf55692`。两轮冻结输入正文仅有 `profile_sync/view.rs` 的双语展示差异。本文及交接状态更新发生在这些检查之后，不能把更新后的文档当作之前冻结的输入正文。

初期 SHA-256 输出 trait、GPUI imports、test 宏递归、focus borrow、布局 lifetime、Kit 未提供的 disabled snapshot 属性、未滚动到选择控件和订阅事件尚未结算的失败日志均继续保留。既有依赖 `block 0.1.6` 的 Cargo future-incompatibility 提示没有被宣称已修复。

macOS 使用 `MACOSX_DEPLOYMENT_TARGET=15.0` 从最终源码构建 app 与 MCP，标准包同时包含二者，结构检查实际读取 Mach-O、系统链接与 Info.plist，app minos 为 15.0。包装 Python 中的其它平台结构夹具不提供目标平台原生证据。本轮没有启动 GUI、安装、签名或公证。

## 尚未关闭的验收

新的非作者[代码与功能复核](2026-10-06-profile-sync-independent-review.md)已真实重现并修复完整路线认证引用、本机文件夹标签映射及远端删除时间三项问题，独立完整1,276普通/8doc/6Python、严格检查、57包装及新macOS结构检查通过。根整合及完整组合门禁、新标准包 GUI 流程、真实两机器共享挂载和三平台桌面验收仍开放。没有云账户、最终一致文件复制器、自动后台轮询、供应商 CLI/模型或真实客户设备验收；没有上传用户配置、发布标签或覆盖用户应用。共享目录必须支持参与客户端间的协作锁及原子替换，测试只在隔离本机文件系统验证了此约定下的双端状态流程。
