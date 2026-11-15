# 测试索引

当前状态以[开发交接](../HANDOFF.md)为入口；
[验证策略](STRATEGY.md)区分源码、受控工程、平台 CI、原生与发布安装。
本索引列最近阶段记录，实时交付与执行状态从交接入口接续；
完整历史保留在[记录目录](records/)，早期PENDING不覆盖后续验收。

多作者或源码副本复用编译缓存前，先遵循[共享target归属](SHARED_TARGET.md)，
串行移交Cargo槽并核对实际编译的测试清单。

| 日期（Asia/Shanghai） | 记录与范围 |
| --- | --- |
| 2026-11-15 | [客户端归档传输集中验证](records/2026-11-15-reviewed-archive-transfer.md)：ZIP/tar.gz 完整成员审核、源重验、新目标发布、取消与隔离；两项 P2 先红后绿并经独立复核关闭，P0–P3 为零，独立预构建 `9/9`。最终普通 Rust `2601/25 ignored`、rustdoc `14`、Python `131/1 skipped`，真实本机 OpenSSH `13/13` 和正常 GUI/MCP 构建通过；Windows 解包后端、原生矩阵及其余高级传输范围开放。 |
| 2026-11-15 | [恢复记录批量整理集中验证](records/2026-11-15-transfer-recovery-batch-integration.md)：有序逐条审核、显式批量丢弃、快照/claim/旧回调及三主题双语紧凑布局；worker 启动失败回队 P2 已修复，两项 GPUI 先红后绿，传输队列 `29/29`。最终完整门禁普通 `2533/23 ignored`、rustdoc `14`、Python `131/1 skipped` 通过；新独立复核批准冻结修复，P0–P3 均为零。修复前真实本机 OpenSSH `11/11` 与 macOS 可见窗口短验保留原归属；Podman 与在线依赖检查网络受阻，历史 rustdoc 重复分类已校正。 |
| 2026-11-14 | [连接状态灯审查修复](records/2026-11-14-connection-indicator-review-fix.md)：`9363485` 修复首轮独立 P2；完整门禁 ordinary `2442/23 ignored`、app `1134/2 ignored`、doctest `14` 通过，新冻结源码复核 `APPROVE`（P0–P3 全 0），独立预构建回归 `36` 项通过，519 个提交输入与冻结摘要一致；原生仍 `HOLD`。 |
| 2026-11-14 | [依赖官方复查与升级](records/2026-11-14-dependency-refresh.md)：`98f8f9d` 八个直接包升级，官方 `33/103/5` 版本检查与 Rust 1.100.0 完整门禁通过；ordinary `2440/23 ignored`、doctest `14`、app 清单 `1134`；首轮 `E0514` 原件保留，独立复核发现 UI 状态灯 P2，依赖输入无待修发现。 |
| 2026-11-14 | [响应式工作台紧凑修复](records/2026-11-14-responsive-workbench-follow-up.md)：`a33e4c3b154867e4ef542cd9ef6a1193aee31c19` 收口紧凑 AI/磁盘布局、stale Edit 与 session-tab reveal 修复；Rust 1.100.0 完整门禁通过，文档提交 `251a8ee` 的独立复核返回 `APPROVE`（P0/P1/P2/P3 均为 0）；原生桌面和真实联调边界保持开放。 |
| 2026-11-14 | [响应式工作台 macOS 精确包与启动短验](records/2026-11-14-responsive-workbench-native-smoke.md)：当前 `e4e9eb5` 的 arm64 包结构检查、真实进程启动和 57 项打包测试通过；窗口级 AX、跨平台桌面和真实联调边界保持开放。 |
| 2026-11-08 | [工作区窗口切换器](records/2026-11-08-workspace-window-switcher.md)：窗口摘要、真实原生窗口激活、稳定序号，以及 modal/Dialog、focus trap、Escape 和焦点恢复；作者完整工程门禁与独立复核通过，原生多窗口仍待收口。 |
| 2026-11-06 | [文本与路由诊断组合](records/2026-11-06-editor-traceroute-integration.md)：543输入/21源码路径，2355 ordinary/23 ignored、14 rustdoc、Python131/1 skip通过；combined-r3/r4限定批准源码和新包。 |
| 2026-11-06 | [文本与路径探测原生](records/2026-11-06-editor-traceroute-native.md)：38组macOS AX/JPEG，BOM/CRLF取消与保存、SSH UDP双栈一跳获native-r1限定批准；inline动作P2与合并验收开放，完整native HOLD。 |
| 2026-11-06 | [文本格式](records/2026-11-06-editor-format.md)：r3隔离538输入与25新增通过，差分审关闭三项P2；父最终组合、新包及限定原生续记分别列明。 |
| 2026-11-06 | [监控能力与知识分类](records/2026-11-06-monitor-knowledge-categories.md)：已交付fa7c059；fresh组合门禁/包通过，首轮HOLD保持；修后同包28对限定A/B原件获native-r2独立批准，完整矩阵另验。 |
| 2026-11-06 | [资源趋势与知识批量管理](records/2026-11-06-resource-knowledge-integration.md)：已交付850b77b；530输入完整门禁、独立源码/文档复核；[限定原生](records/2026-11-06-resource-knowledge-native.md)保留实际A/B、原capture拒绝及父只读导出归属。 |
| 2026-11-06 | [诊断与画布整合](records/2026-11-06-reviewed-diagnostics-integration.md)：5b7772c源码交付与精确三平台CI、CLI probe及画布限定原生分别验收。 |
| 2026-11-05 | [单窗启动恢复](records/2026-11-05-workspace-startup-restore.md)：源码、完整门禁及新包离线重启的限定读回，完整原生矩阵另行验收。 |
| 2026-11-05 | [CI 容量修订](records/2026-11-05-ci-storage-capacity.md)：新 profile 门禁通过，506 项输入一致，Nash 源码复核通过；原提交 Windows 重跑成功与 Linux/macOS 继承结果分开记录，Nash 组合复核通过、文档 P3 关闭，新提交绑定及精确 CI 仍 PENDING。 |
| 2026-11-05 | [本地 Ask scratch 诊断](records/2026-11-05-local-ask-scratch-diagnostics.md)：已提交诊断的本机门禁与非作者复核；旧 Linux 根因仍 UNKNOWN。 |

各切片按自己的冻结输入、开发包和回执验收；工程、原生、CI及安装结论不相互代签。
