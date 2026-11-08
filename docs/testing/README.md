# 测试索引

当前状态以[开发交接](../HANDOFF.md)为入口；
[验证策略](STRATEGY.md)区分源码、受控工程、平台 CI、原生与发布安装。
本索引列最近阶段记录，实时交付与执行状态从交接入口接续；
完整历史保留在[记录目录](records/)，早期PENDING不覆盖后续验收。

多作者或源码副本复用编译缓存前，先遵循[共享target归属](SHARED_TARGET.md)，
串行移交Cargo槽并核对实际编译的测试清单。

| 日期（Asia/Shanghai） | 记录与范围 |
| --- | --- |
| 2026-11-08 | [工作区窗口切换器](records/2026-11-08-workspace-window-switcher.md)：窗口摘要、真实原生窗口激活、稳定序号，以及 modal/Dialog、focus trap、Escape 和焦点恢复；作者完整工程门禁通过，独立复核与原生多窗口仍待收口。 |
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
