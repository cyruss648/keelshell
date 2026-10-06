# KeelShell 开发交接 — 2026-10-06

公开仓库为 [cyruss648/keelshell](https://github.com/cyruss648/keelshell)。当前整合基线为 `7f50d60a59034dd145c40414c334e8bfc4e09f67`，本文件记录任务审计与测试诊断整合阶段；没有把新组合表述为已提交、三平台CI或原生验收完成。历史证据继续保存在[测试记录](testing/records/)和Git历史。

## 产品与工程约束

- Rust原生GPUI Kit，macOS、Windows、Linux；只管理远程SSH。默认简体中文并维护英文；System/Light/Dark主题与白底跨平台图标。完整功能、专业界面、最小窗口及辅助技术要求保持，不能用夹具或编译替代原生验收。
- **MCP由KeelShell向外部智能体提供服务。** 不做通用第三方MCP客户端。API与本地Claude Code/Codex CLI Ask是另一个入口。分享上下文需要用户明确选择，命令/文件变更需要在应用中审阅；模型不能自行批准、执行或扩大授权。见[对外MCP](product/EXTERNAL_MCP.md)与[AI计划](product/DESIGN_AND_AGENT_PLAN.md)。
- UI/core/session/AI分层，阻塞I/O不进UI线程；直接registry依赖使用x.y，保留应用Cargo.lock与精确工具链；公开API有rustdoc，行为测试隔离且有界。
- 凭据默认临时，明确保存使用OS存储或认证加密vault；master password、明文凭据、真实客户日志不进metadata、源码或文档。主机指纹变化关闭连接。
- 已研究Reef/模板的工程与生命周期配置和AI配置交互，见[研究目录](research/)。用户已授权公开GitHub、提交推送与标签触发多平台Release；没有授权遥测或覆盖现有应用。
- 新功能需非作者代码/功能复核、主树格式/Clippy/测试/依赖策略及相关原生检查；requirements在docs/product、决策在docs/adr、测试在docs/testing、余项在ROADMAP。及时可恢复归档已无消费者的worktree/分支/缓存，保留失败材料。

## 当前源码与证据

连接库、SSH认证/跳板/代理、加密vault、远程终端、SFTP与内容校验续传、文件审核编辑/比较/合并、主机监控及磁盘I/O、隧道、批量命令、逐目标参数、依赖任务、有限定时和审核式加密连接配置同步已有实现。详细覆盖以[路线图](ROADMAP.md)、[能力清单](product/CAPABILITIES.md)与各记录为准，不能概括为完整产品完成。

精确7f50基线的主树1478普通/8doc/6Python、格式/x.y/严格Clippy通过；652源输入前后相等，新macOS标准双程序开发包已构建和检查。其 [Quality 37460171202](https://github.com/cyruss648/keelshell/actions/runs/37460171202) attempt1 已结束，macOS/Windows成功、Linux失败。Linux唯一app失败发生在同ID保存资料审核后的18秒等待，后台终态未采集，原因未确定。新的三路径test-only诊断仅增加观察与明确失败反馈，保留18秒和45秒断言；新的非作者两次正常实际运行及合法Stale反例通过，原故意失败及CI日志保留。诊断已整合并纳入当前主树完整门禁，仍需要新提交级CI，见[诊断记录](testing/2026-10-06-saved-profile-sync-approval-diagnostics.md)。旧b26三平台通过仍只属于b26。

任务级只读结果审计新增100次运行、128任务每次、2048任务累计的固定结果/UUID/时间记录，包含有限调度每次触发，保存失败可重试。无命令、输出、地址、参数或摘要，重启不恢复/重放执行，连接同步不携带设备审计。原解析P2的36/60额外字段接收已实际复现；窄修复经另一个新非作者验证60+510拒绝、表示往返、存储与GPUI/TCP行为通过。根以当前基线增量整合，保留profile_sync与workflow_audits各自默认与校验；根两StateStore组合已实际通过；新非作者真实模态Close遗漏反例已复现，根两文件三行修复后同一case及Changed/真实存储冲突3项通过，回归已导入主线。新的非作者当前671输入组合限定PASS已全文读回；根1508普通Rust/8doc/6Python、格式/x.y/严格Clippy、默认/2MiB控制器全部实际通过，16ignored未执行，671输入前后相同。相同源码的新macOS双程序开发包构建、结构检查及57打包测试通过；随后仅更新结果文档。新的桌面验收与提交级CI仍开放。见[任务记录指南](product/WORKFLOW_HISTORY.md)、[ADR0065](adr/0065-bounded-read-only-workflow-history.md)及[整合记录](testing/records/2026-10-06-task-audit-main-integration.md)。

目录镜像作者候选包括显式方向、审核完整路径/类型/大小/内容校验、目标独有常规文件和已空目录删除、冲突拒绝及未知写隔离。新的独立复审已实际发现最后一次远端LSTAT等待期间撤权仍能删除本地目标的P1；三行窄修复和新的行为门禁进行中，尚未整合。非空独有子树、冲突差异应用和可编程shell补全等仍按全范围推进。

参数依赖任务、136份Linux磁盘采样与双4MiB并行传输已有各自产物绑定的有限macOS实际SSH证据；其它传输模式、最小窗口、辅助技术、Windows/Linux桌面仍开放。见[参数监控](testing/records/2026-10-06-target-parameters-disk-native.md)、[传输](testing/records/2026-10-06-parallel-transfers-native-v5.md)及[同步定时](testing/records/2026-10-06-sync-schedule-main-integration.md)。API三协议人工发送、参数读回、长Messages滚动已有有限macOS证据；本地CLI Ask的订阅登录、任意cwd/环境、可见步骤流与受限Agent工作流继续推进。

## 外部MCP、发布和下一步

对外MCP八工具、受认证桌面IPC和stdio伴随程序已有实现。此前实际Claude Code七工具授权读取、范围拒绝、桌面批准/拒绝及撤权在限定自有服务范围通过；第八工具自有客户端文件提案与长正文六组合已验证。Codex完整业务仍未通过。新7f50尝试在300秒捕获配置界面期限结束，未进入CLI/模型/业务；此前V8首个会话读取后因记录器额外元数据停止。新的控制器纯协议/清理复核与二进制绑定属于准备证据，不能替代原生业务成功。见[当前尝试](testing/records/2026-10-06-codex-mcp-current-attempts.md)和[接入指南](product/EXTERNAL_MCP.md)。

白底图标、GitHub链接、内置变更日志、检查更新、SHA-256校验下载、人工确认安装与回滚helper和六目标标签发布流水线已有实现。完整Release产物、签名/公证、Windows/Linux桌面及安装更新仍开放；本阶段没有发布标签或覆盖安装。

保存并推送已完成组合复核、主树门禁和macOS开发包检查的任务审计／诊断检查点，单独核验新提交CI；完成目录镜像P1的新非作者修复复核再整合；继续完整远程功能、外部Codex业务、UI/语言主题/辅助技术、三平台桌面及六目标发布安装矩阵。所有新源码/产物需要自己的证据。已结束树在根消费证据及组合消费者退出后可恢复归档，活跃树与未消费失败材料保留。
