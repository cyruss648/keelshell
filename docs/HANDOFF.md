# KeelShell 开发交接 — 2026-10-07

公开仓库为 [cyruss648/keelshell](https://github.com/cyruss648/keelshell)。目录镜像、文件审核呈现、远程协议诊断及 test-only 失败观察已在主线 `9865f1f121170c9eed81c19f4ca50f522c001f45` 提交推送，完整门禁及下述限定原生验证通过；该提交的 [Quality 37525054745](https://github.com/cyruss648/keelshell/actions/runs/37525054745) 已结束：macOS 成功，Windows 测试导入严格 Clippy 失败，Linux 定时任务与配置同步组合批准后的 18 秒状态等待失败。Windows 窄修复经新非作者复核并已导入；根整合门禁及新提交级 CI 待完成，见[修复记录](testing/records/2026-10-07-windows-diagnostic-import.md)。Linux 根因仍未确定。父检查点 `0cca8ac` 的 Windows／macOS 成功、Linux 目录同步字节断言失败继续保留，不追溯新观察为旧失败原因。

独立诊断分支 `feature/directory-sync-ci-diagnostics` 已在 `0f1a57756a78771dc0d4e2ca14703d55925841bf` 提交推送，远端逐 ref 一致、工作树干净；四路径 test-only 增量保持生产行为和原 18 秒／45 秒断言，六个限定测试（3 个面板、2 个 recorder、1 个原审批）与严格 Clippy 通过，完整封包已由根全文读回。[Quality 37523395932](https://github.com/cyruss648/keelshell/actions/runs/37523395932) 的三平台 job 已全部成功；该增量尚未合入主树，成功不替代主线或新修复提交 CI，也不证明历史失败原因。其父 `dd32392` 的 [Quality](https://github.com/cyruss648/keelshell/actions/runs/37506388205) 已结束：macOS／Windows成功，Linux保存资料审批等待超时；原目录同步字节用例该次通过，原失败原因仍未确定。见[CI定位](testing/records/2026-10-07-saved-profile-sync-ci-diagnosis.md)，历史证据继续保存在[测试记录](testing/records/)和Git历史。

当前组合的原生 macOS 应用已在自有 Linux SSH/SFTP 完成上传和下载递归镜像，各 12 项；取消、上传后的独立实际读回通过，下载后的根字节核对和新非作者只读复核通过。首轮 GUI 的原定期限超时、后端管理进程未知 wait 与恢复观察／清理分别保留；新的隔离 GUI 和所属资源已结束。中英文纵向审核可用，横向长行、最小窗口、辅助技术及其它平台仍开放。见[新原生记录](testing/records/2026-10-07-recursive-mirror-native.md)。

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

目录镜像包括显式方向、审核完整路径/类型/大小/内容校验、目标独有常规文件和已空目录删除、冲突拒绝及未知写隔离。独立审查实际复现最后一次远端LSTAT等待期间撤权仍删除本地目标的P1；发现者三行窄修复经另一位非作者32相关用例与严格Clippy复核通过，原旧正文反例101保留。两个完整封存包及包外终结材料已全文读回，27路径增量按明确preimage与三方合并整合，保留任务审计。非空独有子树镜像及新的限定非作者补充随后已导入，最终717输入组合完整工程检查实际通过；新的 macOS 产物与限定双向镜像原生通过，完整桌面和精确提交 CI 仍开放，见[当前组合](testing/records/2026-10-07-recursive-mirror-main-combination.md)。冲突差异应用与可编程shell补全继续推进，远程DNS/TLS/HTTP诊断已在同一组合中检查。

远程协议诊断的精确 `7f50d60` 作者候选已通过限定非作者代码/行为复核：25个不同Rust测试、30次成功Rust执行及3个额外真实Python HTTP边界场景；包括自有SSH/TLS/HTTP/DNS、受控GPUI与实际重连后的路线/指纹/关页迟到回复拒绝。29路径作者增量与两项同路径test-only补充已按690份当前输入完成隔离三方准备，并导入主工作副本；完整703份导入结果与隔离postimage逐字节相等，保留任务审计、目录镜像撤权、Linux CI诊断与V12记录。后续717输入最终组合门禁已实际通过，CI与新原生仍开放，见[主树整合记录](testing/records/2026-10-07-remote-protocol-main-integration.md)与[当前组合](testing/records/2026-10-07-recursive-mirror-main-combination.md)。诊断仅由捕获的已认证SSH向远端POSIX/Python 3.8+发起明确审核的DNS、验证TLS和HTTP(S) HEAD，不安装工具或发送模型上下文；远端Windows、UDP及更广协议继续开放。见[功能指南](product/REMOTE_PROTOCOL_DIAGNOSTICS.md)与[协议记录](testing/records/2026-10-06-remote-protocol-diagnostics.md)。

参数依赖任务、136份Linux磁盘采样与双4MiB并行传输已有各自产物绑定的有限macOS实际SSH证据；其它传输模式、最小窗口、辅助技术、Windows/Linux桌面仍开放。见[参数监控](testing/records/2026-10-06-target-parameters-disk-native.md)、[传输](testing/records/2026-10-06-parallel-transfers-native-v5.md)及[同步定时](testing/records/2026-10-06-sync-schedule-main-integration.md)。API三协议人工发送、参数读回、长Messages滚动已有有限macOS证据；本地CLI Ask的订阅登录、任意cwd/环境、可见步骤流与受限Agent工作流继续推进。

文件审核的两轴滚动、原文逐行Label和固定动作已通过新的非作者限定复核并导入主工作副本。取消后的自有文件／空目录、真实wheel偏移、完整长路径与异步新提案重置均有受控GPUI/TCP证据；后续717输入最终组合门禁已通过；限定中英文镜像已有实际像素／AX 与纵向证据，横向长行、最小窗口与 VoiceOver 仍开放。原纵向native未完成读回不认定为生产缺陷或通过，见[审核整合](testing/records/2026-10-07-file-review-main-integration.md)。递归非空子树已通过新的非作者两阶段限定复核，25路径作者与6路径test-only补充已精确导入717输入主副本；完整工程检查实际0，1,587普通／8doc／6Python、格式／x.y／严格Clippy及两种完整控制器通过，20ignored未执行。限定 macOS 双向镜像原生通过，完整原生和精确提交 CI 仍开放，见[组合记录](testing/records/2026-10-07-recursive-mirror-main-combination.md)。

## 外部MCP、发布和下一步

当前717输入组合工程检查之后，仅更新文档和双语README，全部非文档输入相同；新的719输入macOS标准双程序开发包已实际构建，57项打包、Info.plist和Mach-O动态库检查通过，六个所属进程组结束且私有临时目录移除。该包构建时尚未运行；后续相同程序已完成上述有限原生镜像场景，没有安装或发布，完整桌面／外部客户端业务及Windows/Linux范围仍开放，见[当前组合](testing/records/2026-10-07-recursive-mirror-main-combination.md)。README与MCP调用链已由另一位非作者只读核对，91个本地相对链接、工具与实际桌面分发能力、双语和证据边界一致；MCP持续由KeelShell向外部智能体提供服务。

最新 V12 使用当前 689 份工程输入的 macOS 双程序开发包和真实 Codex 0.160.0，第三次尝试取得 20 次配对工具调用及独立语义复核：准确选区、SFTP 读取、范围／路线拒绝、命令批准／拒绝、文件批准及完整内容读回。第二份文件的原生界面已显示拒绝，但根未及时交付测试确认标记，控制器 180 秒超时、CLI 实际退出 1；后续拒绝状态／文件读取和桌面撤权未执行，完整业务仍未通过。三次失败及新非作者复核、已记录自有资源清理与内部 recorder OS wait 未知边界均保留，见[V12 记录](testing/records/2026-10-07-codex-mcp-v12-native.md)。本轮未改生产源码或控制器规则。MCP 仍由 KeelShell 对外提供服务，应用内 AI 调用本地智能体是独立入口。

对外MCP八工具、受认证桌面IPC和stdio伴随程序已有实现。此前实际Claude Code七工具授权读取、范围拒绝、桌面批准/拒绝及撤权在限定自有服务范围通过；第八工具自有客户端文件提案与长正文六组合已验证。Codex完整业务仍未通过。新7f50尝试在300秒捕获配置界面期限结束，未进入CLI/模型/业务；此前V8首个会话读取后因记录器额外元数据停止。新的控制器纯协议/清理复核与二进制绑定属于准备证据，不能替代原生业务成功。见[当前尝试](testing/records/2026-10-06-codex-mcp-current-attempts.md)和[接入指南](product/EXTERNAL_MCP.md)。

白底图标、GitHub链接、内置变更日志、检查更新、SHA-256校验下载、人工确认安装与回滚helper和六目标标签发布流水线已有实现。完整Release产物、签名/公证、Windows/Linux桌面及安装更新仍开放；本阶段没有发布标签或覆盖安装。

任务审计0cca的新三平台CI已全文读回：Windows／macOS成功，Linux唯一目录同步失败，原同ID资料审批测试本次通过；见[同步诊断](testing/records/2026-10-06-directory-sync-ci-diagnostics.md)。镜像686输入历史主树门禁保留；后续两个非作者组合回归、test-only失败观察、远程协议、文件呈现和递归镜像已在717输入完成完整工程检查。新 macOS 产物及限定双向镜像原生已完成，完整原生流程与精确提交 CI 仍需完成，不把工程检查代作桌面验收。继续Linux失败定位、完整远程功能、外部Codex业务、UI/语言主题/辅助技术、三平台桌面及六目标发布安装矩阵。所有新源码/产物需要自己的证据。已结束树在根消费证据及组合消费者退出后可恢复归档，活跃树与未消费失败材料保留。

递归子树镜像独立候选已实现全部目标独有节点审核展开、子先父后删除、固定审核/完成记录、剩余子树名空间与内容复核、观察到变化废除旧确认。当前6核心、8新TCP（含17自有listener场景）及4 GPUI测试实际通过，38项相关组合通过；后台栈溢出、源出现和本地整树消失后旧审批复活均已实际复现并集中修复，子项准入后返回的非Closed镜像拒绝都会撤销旧owner审核，保持已知读失败、Closed/只读drop与未知写隔离边界。最终694输入语义修复epoch完整门禁实际0，1558普通/8doc/6Python、格式/x.y/严格Clippy通过，前后输入相等且进程组完整回收；仅结果文档随后更新。此694作者候选的历史时点见[递归记录](testing/records/2026-10-07-recursive-directory-mirror.md)。后续新的非作者限定复核、当前主副本717输入组合导入及完整门禁已完成，新原生和精确CI仍开放；不继承root689的原生或CI验收。
