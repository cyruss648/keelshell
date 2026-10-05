> 本文件归档自本轮整合前的开发记录，仅修正移入history目录后的相对链接。以下“当前/最新/未提交”等文字属于原历史时点；现行状态见[交接](../HANDOFF.md)和[路线图](../ROADMAP.md)。原始字节保留在Git检查点 `e247f2f` 及本轮本地收据中。

# 开发路线

功能源码已提交并推送 `cfac02eab96d1791660bfafeb9bda9552c7f4cfe`：本地Ask真实进度、两项已独立复核的test-only补充及配套文档。根实际核对remote main、0/0、推送时干净及260工程提交字节与完整门禁相等。本机第二门禁1171普通+8doc+6脚本/严格检查/两控制器通过，MAC15新构建、57打包及标准包检查通过。精确[Quality37367209582](https://github.com/cyruss648/keelshell/actions/runs/37367209582) attempt1整体failure：Linux完整通过；Windows1152普通+8doc通过，但额外2MiB控制器45秒总期限超时，具体pending场景UNKNOWN；macOS因托管arm64容量未获运行器、未执行。原失败保持，见[本次CI](../testing/records/2026-10-06-local-ask-progress-ci.md)。

新版真实macOS GPUI/Codex0.160.0→自有SSE完成中文浅色的等待、取消、新请求及旧尾释放隔离、最终回复与人工送入命令栏/清空；建议实际为[REDACTED]，未执行。GUI/SSH明确wait/reap、HTTP0、private删除，根三个已记录出生fresh absent及双端口errno61；不声称完整供应商后代普查。900×580逻辑窗口、全部8事实行、其余七个计划组合与其他平台原生保持开放，见[部分原生记录](../testing/records/2026-10-06-local-ask-progress-native.md)。MCP仅向外部智能体提供KeelShell能力，内置Ask独立。已合入的五个受管工作树可恢复归档并核验checkout/target消失，根保留全部失败及复核材料；没有新Release或安装更新。下方为历史时点，不能覆盖此状态。

最终Ask与两项test-only补充已在主副本整合，新非作者限定复核及根第二完整门禁通过1171普通+8doc+6脚本/严格检查/两控制器，260输入前后相等；源码未提交/推送，新MAC15双程序构建、57打包回归及标准包结构检查通过；新版原生窗口与新CI另行验收。第一次MCP超时和已推送bd9预算CI失败的原因分别未知，原记录不改，见[整合记录](../testing/records/2026-10-06-local-ask-main-integration.md)。以下旧整合“失败/待复核”属历史时点，由本段覆盖。

历史bd9提交；其[Quality37356943845](https://github.com/cyruss648/keelshell/actions/runs/37356943845)整体failure：macOS自托管CLI控制器在预算清理后断言失败，普通已完成80通过/0失败/2ignored，具体分支与原因未知；Windows1139普通+8doc、Linux1158普通+8doc完整通过，目录新回归实际ok。136/137封包与258提交输入经根读回，见[新CI记录](../testing/records/2026-10-06-catalog-probe-lifecycle-ci.md)。自有进程生命周期诊断在独立树进行，原本机通过与原CI失败分别保留。

Ask候选已通过作者与限定独立复审并导入主工作副本，但根完整门禁314.920秒exit1：MCP无效环境子进程2秒output超时，普通已完成930通过/1失败/2ignored，doc及额外2MiB未到达。格式/x.y/6脚本/严格Clippy通过，260工程输入前后相等。Ask未提交或推送；独立启动诊断、新源码CI及新版原生仍待完成，见[整合记录](../testing/records/2026-10-06-local-ask-main-integration.md)。

MCP只向外部智能体提供KeelShell能力，内置Ask独立。两项test-only作者候选已冻结并经根读回：预算诊断最终1158普通+8doc、MCP子进程守卫1164普通+8doc完整门禁通过；新的非作者正在分别复核，主树尚未导入，不替代原失败原因。Codex观察器G1原Unknown反例及A准入失败cleanup反例均保留；B的62离线测试和新非作者62复核通过，仅关闭离线缺口，不给native READY。G2仅设计未实现，原F缺退出回执原因未知。顶部覆盖下方历史时点，不把辅助脚本或源码检查当作完整外部客户端/跨平台原生验收。

模型目录夹具的两份测试修正已精确导入主线：header/body共享3秒总期限、接受连接恢复阻塞模式、异常退出取消/shutdown/join及真实GET1/noPOST断言；原5秒界面/观测限制保持。作者完整门禁431.334秒通过1158普通+8doc+6脚本/严格检查/两控制器，新的非作者24项及私有异常清理反例/严格检查通过；28/29和67/68封包已根全读回，主线258工程输入与最终门禁相等。生产/依赖/锁/工具链不变；新Windows CI尚待，原8d failure保留。见[修正记录](../testing/records/2026-10-06-catalog-probe-owned-lifecycle.md)。

最新主线 `8d074b014a9ef863944eadca02cbf190125443c7` 的 [Quality37351046635](https://github.com/cyruss648/keelshell/actions/runs/37351046635) 已结束整体 failure：macOS/Linux 各1155普通+8doc完整通过；Windows app428通过/1失败，当前已完成539普通/1失败/2ignored，doc和额外2MiB控制器未到达。失败是已有模型目录隐私测试夹具读取HTTP请求头返回10035，再因观测通道断开失败；本次没有测量请求头字节或到达时序，不认定产品泄露。SystemRoot双路径对照仍实际通过。原日志、149/150独立封包已保留；根正在另一个工作区修正测试I/O，不改变生产代码，原e821成功不关闭这次失败。见[本次CI](../testing/records/2026-10-06-windows-catalog-probe-ci.md)。

主线最新为已推送8d074b0，远端/0/0/推送时干净及258提交输入相等已核验；[新Quality37351046635](https://github.com/cyruss648/keelshell/actions/runs/37351046635)尚未结束，e821的三平台已通过保持原范围。已合入诊断工作区和分支清理完成，原证明保留；Ask候选继续独立复审。Codex目录记录器的新G仅离线准备，旧F缺终态保持失败，真实八工具/授权业务不先验收。

2026-10-06 当前源码：e821的[Quality37346734818](https://github.com/cyruss648/keelshell/actions/runs/37346734818)三平台全部success，Windows实际清空/显式SystemRoot对照完成，根已读回实际Windows原日志和全部job API；旧c7/3f26失败保持。主树已精确快进e821并恢复最新文档，258工程输入相等，173/174完整独立CI封包已根读回，根整合门禁370.567841秒通过1155普通+8doc+6脚本、严格检查与两控制器/258输入相等、尚未push main，不以本次源码CI验收桌面或安装更新。

独立本地Ask过程显示候选作者1162普通+8doc+6脚本/严格检查/两控制器/57打包通过，16源码与260输入冻结，77/78材料根完整保存核验；新的非作者正式复审开始，尚未生产整合/新版原生。MCP仅向外部智能体提供服务，当前工具与客户端验收范围见[指南](../product/EXTERNAL_MCP.md)。当前工程状态以[交接顶部](../HANDOFF.md)为准，下方为原产品与验证历史。

2026-10-05 当前状态：API 请求选项 F 经非作者差量复核通过，48 份冻结源码已精确整合；E 固定认证头大小写 P2 及此前失败保留。主树 F 完整门禁1146普通+8doc+6脚本/严格检查及57打包、新MAC15构建检查与标准双程序打包通过；新原生API临时/环境引用、重启及手动Ask已验证限定事实，中英文三主题长文件人工拒绝与实际SFTP读回通过。首提案到期、原Discover零请求预期未满足/内容未知origin与外层PTY失败保留，最小窗口未验证；新矩阵冻结独立限定复核及根143/144逐字节读回通过，无新已证P1/P2。见[整合记录](../testing/records/2026-10-05-ai-request-options-main-integration.md)、[API原生记录](../testing/records/2026-10-05-ai-request-options-native.md)与[文件六组合](../testing/records/2026-10-05-mcp-file-review-native-matrix.md)。前一df8源码三平台CI通过，新F提交CI另核验；Windows/Linux原生、供应商第八工具、Agent及发行/安装更新仍开放。MCP仅向外部智能体提供KeelShell能力。下方历史阶段记录不覆盖本段当前状态。

2026-10-05 最新更新：对外MCP方向保持；文件提案a909的Linux/Windows CI成功、macOS重新授权测试失败，整体failure保留；新的同步修正已通过全新非作者复核与主树完整门禁，新提交CI另核验。API D虽通过独立完整门禁，新的HTTP请求头名称秘密P1阻止整合，E修复进行中。

精确首次文件提案CI及跳过边界见[CI记录](../testing/records/2026-10-05-mcp-file-proposals-ci.md)。新P1的9个自有HTTP请求和真实GPUI来源反例已保存，不用D原门禁关闭新缺口。下方“D尚未独立审查”“新提交CI开放”均为历史，由本段与[交接顶部](../HANDOFF.md)覆盖；八工具macOS批准/拒绝/并发变化原生切片保留原范围，最小原生矩阵仍未通过。

新的测试同步已通过作者1083普通+8doc+6脚本及全新非作者19MCP/105TCP/严格工程复核，原限时与关键断言保持。根导入两份测试源码，主树439.533秒完整门禁1083普通+8doc+6脚本、严格检查与默认/2MiB控制器通过；原a909 CI失败保持，新提交CI另核验。见[同步记录](../testing/records/2026-10-05-mcp-grant-readiness.md)。API E追加Apply/持久化前秘密metadata保护，作者最终1121普通+8doc+6脚本通过，构建/冻结与新独立复审尚未完成。

最新结果以[交接顶部](../HANDOFF.md)为准：C133proof失败材料完整保存；D1109普通+8doc+6脚本作者检查通过，未合主树。MCP B145文件由根保存，主树1077普通+8doc+6脚本与严格检查通过；新源码绑定macOS自有八工具客户端实际批准/拒绝/已观察并发内容保护通过，首次AX操作失败保持。审查PASS与远端阻塞余留限制分别记录；供应商第八工具、最小原生矩阵、Windows/Linux原生和新提交CI仍开放。见[文件提案记录](../testing/records/2026-10-05-mcp-file-proposals.md)。下方均为历史范围。

历史限定状态：API B34文件及原五路径脱敏反例已通过作者门禁，根保存35proof，新非作者实际Models→Test入网反例证实第二P1，B禁止整合、C38文件/49proof根核验保存，作者1098普通/8doc/6脚本及严格门禁与原两反例精确通过，全新非作者已开始独立复核；文件提案A被32次实际失败重新授权计数泄漏P2否定，新B请求所有权修复完整门禁1077普通/8doc/6脚本及57打包通过，26源/53proof根核验保存、全新非作者独立复审已开始；原单次反例/独立fixed32通过，120份原复核材料保持。根F实际目录探针虽出现七名称/限定schema映射，仍因缺owned_exit失败；0业务/0GUI。文档78的Quality37266941663为macOS/Linux成功、Windows app374通过/1失败，SSH输出队列测试候选保持6秒截止并改为真实数据/Full条件，3专项和根完整工程门禁1060普通/8doc/6脚本通过，新非作者原3/私有5真实TCP-SSH与严格整仓复审PASS，64proof根核验保存、精确代码已导入；主树完整门禁1060普通/8doc/6脚本及严格检查通过，新精确CI待核验。首次外层编译加测试wrapper超时保持。上述候选均未产品提交，授权Codex、新第八工具与跨平台原生仍未关闭。见[交接](../HANDOFF.md)。

API请求头/代理A作者31文件已冻结，但新非作者五条GPUI prepare反例证实非活动代理Basic派生值进入其它配置已审核payload（0send/0CLI/job），P1阻止整合，新B仅修该秘密边界。文件修改提案作者26文件已冻结，1075普通/8doc/6脚本、严格工程门禁、57打包和host arm64开发编译通过，仍待新独立审查与根整合/原生。A的78证明、文件提案38证明和全部冻结源码由根核验保存；两项未提交/推送。该段覆盖下方作者候选进行中状态，不能把旧七工具材料扩为第八工具验收。

新的默认拒绝服务入口完成真实initialize/tools-list，首40352字节POST出现七项KeelShell函数及五项只观察的客户端定义；1POST/0SSE/0业务，没有GUI/SSH/grant。完整探针仍PROBE_ABORTED：六项原schema约束被转换、记录器最终owned_exit缺失。E根据版本源码准备限定转换，新非作者schema检查通过但进程扫描存在复用PID误认领P1，E预审FAIL/未实际执行；新F只修出生身份关系。A/P2、B/features、C/假IPC认证失败和D实际结果保留，不追溯认定误杀。MCP仍仅向外部智能体提供能力，API请求头/代理作者31文件已冻结并进入新独立复核；受审文件修改提案仍完成作者门禁，两者未整合。见[方向与目录记录](../testing/records/2026-10-05-codex-catalog-direction.md)。

两个test文件已修复确定性后台数据完整/前台Running积压的测试生命周期竞态；旧CI具体触发仍候选。真实WRITE屏障保持176场景并验证准确阶段/控件、Paused部分稳定及Continue完整字节，原5s/12s与生产代码不变。作者和根门禁1060普通+8doc+6脚本、严格Clippy/fmt/x.y及默认/2MiB控制器通过，作者57打包另记；新非作者44文件/208session+1doc/4私有Handler与严格检查复审PASS，无剩余限定P1/P2，79作者/160复核证明根核验复制。目录计数/subsystem只观测清理；精确233提交的新CI独立确认全success，macOS/Linux各1060普通/8doc/6脚本/57打包及9OpenSSH，Windows1040普通/8doc、脚本5通过1跳过、打包53通过4跳过且OpenSSH步骤跳过；三平台实际文件/EOF目标ok，不提供逐场景CIJSON或新原生/发行证据。160新CI材料根核验复制，文件工作树/分支/cache已清理，见[同步记录](../testing/records/2026-10-05-files-scene-readiness.md)和[三平台验证](../testing/records/2026-10-05-files-scene-ci.md)。

精确 `a19d0c1` 的 [Quality37253864420](https://github.com/cyruss648/keelshell/actions/runs/37253864420) 已结束 failure：macOS/Windows成功，Linux文件布局GPUI等待12秒超时，app375通过/1失败、MCP/OpenSSH未执行。macOS/Windows新EOF回归通过，macOS9项系统OpenSSH通过；Windows脚本5通过/1跳过、打包53通过/4跳过。失败证据独立核对保留，候选在新工作区诊断，原因尚未由该次CI证明；见[三平台记录](../testing/records/2026-10-05-integration-ci.md)。此前三个功能worktree可恢复归档、旧分支与两个审查cache已实际清理，241份证明保持。

MCP 仍仅由 KeelShell 向外部智能体提供服务。新的 Codex 0.160.0 授权只读尝试没有通过：首轮执行前脚本误解析功能表，0 POST/未 MCP exec；新范围只协商七项目录，首笔模型请求缺少 KeelShell 工具且含额外工具，安全门停止，实际 1 POST/0 业务 RPC。额外工具来源未知，原字节及两次失败保留；自有客户端与原生 GUI/SSH 已清理，根已明确撤销全部授权。新的非作者只读复核确认失败证据一致、无剩余限定范围P1/P2，15份证明由根核验复制；失败仍未关闭，详见[Codex 尝试记录](../testing/records/2026-10-05-codex-authorized-mcp-failure.md)。该限定失败覆盖下方历史“Codex MCP 未执行”，不替代原 Claude 授权通过。

此前精确 `40d092c` 的 [Quality37244888512](https://github.com/cyruss648/keelshell/actions/runs/37244888512) 已结束 failure（macOS/Windows success、Linux MCP EOF 退出断言失败），Linux OpenSSH 未执行；旧 e166 三平台通过保持为旧范围。完整失败日志保留；新的 EOF 分类修复、Ask 预算和弹窗输入修复均通过各自独立复核及根整合门禁 1053 普通/8文档/6脚本，限定 macOS 原生设置/当前 AX 隔离/焦点与 SSH 回显通过，见[整合记录](../testing/records/2026-10-05-ai-modal-mcp-integration.md)。最新CI范围见上方，不用本机检查关闭原 Linux 失败。

2026-10-05 最新对外 MCP 进展：实际安装的 Claude Code 2.1.285 已通过源码 `e16689b` 的标准 macOS 双程序开发包完成七项工具 schema 协商、13 次真实工具调用与 15 次模型请求。授权片段、目录及 UTF-8 文件读取、越界/未授权监控拒绝与失效路线拒绝均经过同一客户端的真实结果回传；受控桌面 UI 明确批准首条提案后返回成功，拒绝另一条后返回拒绝。撤销全部授权后客户端报告连接断开，未记录第 14 次 tools/call RPC，因此不声称新的服务端授权拒绝。独立复核已通过该限定范围，无剩余 P1/P2，见[授权客户端记录](../testing/records/2026-10-05-claude-authorized-mcp.md)。Codex 的新受限文本前置已通过；新的实际 MCP 尝试在目录协商后被模型请求检查拒绝，0 业务 RPC，读取闭环未通过。源码基线 `e16689b` 的[Quality37240943183](https://github.com/cyruss648/keelshell/actions/runs/37240943183)三平台成功；这不关闭其他平台原生、文件修改提案、最终六目标 Release 或已安装更新。

2026-10-05 工作区整合：连接库批量组织/永久清理、手工依赖工作流与文件响应布局已合入并通过独立整合审查；依赖工作流支持1–128个任务、32个已认证SSH目标及逐任务回执，文件区保留至少64px真实条目空间和固定审核操作。旧macOS开发包完成受控标签与双任务退出0/7，但实际小窗口发现终端约37px；新单行补全布局及32完整Files/16完整候选GPUI场景已修复该渲染预算，根门禁1031普通+8文档+6脚本、两种完整CLI控制器和57打包通过。fresh紧凑独立复审和新macOS包900×580八组合/显式补全闭环通过，生产提交40824f0已推送main并核对精确SHA，新Quality37232614315已结束：Linux/Windows成功，macOS的SSH测试在认证阶段超时，整次失败，旧包及GPUI场景不作为新原生通过，见[整合记录](../testing/records/2026-10-05-workspace-workflows-integration.md)。

SSH夹具增量：原408 CI失败仍保留；最终两文件测试修复已通过作者95项/12专项及根整仓工程门禁、9项系统OpenSSH和GUI/MCP构建。新独立复审无剩余P1/P2，95项及6项私有补充探针通过；修复f09651b已推送，Quality37235821726已结束failure（macOS/Linux成功、Windows后置TCP观察超时），下一观察修正单独核验，不沿用存在TCP探针副作用的旧候选通过结果，也不改变产品连接限时或将旧UI包改作新提交原生验收。见[夹具记录](../testing/records/2026-10-05-ssh-timeout-fixture-stability.md)。

Windows的后置观察已改为同截止内精确端点复绑，作者95/12/工程门禁、根整仓及fresh独立95/7探针复审通过，无剩余P1/P2；修正896073a已推送main并核对源码/远端SHA，Quality37238796819三平台实际success：macOS/Linux各1031普通+8文档、Windows1011普通+8文档、两个目标case及默认/显式2 MiB控制器均通过，macOS/Linux各9项OpenSSH及清理回执通过。159份新CI证据经根核验，本测试修正的源码CI已关闭；桌面/供应商MCP/Release保持独立，见[观察记录](../testing/records/2026-10-05-windows-forward-observer.md)。

此前 MCP 源码基线：MCP stdio 错误响应的接收取消窗口已用真实 SDK 与 ID4 进程超时证实；连接拥有的 I/O task 和有界 queue 已修复。最终根门禁989普通+8doc+6脚本、默认/2 MiB完整控制器、格式/严格Clippy/x.y通过；transport与新增首条短提案滚轮回归均经fresh独立审查，无P1/P2。生产修复34cff1b与新增回归958903c的三平台Quality及macOS/Linux OpenSSH均成功，新增回归三平台实际通过，见[Quality37220102376](https://github.com/cyruss648/keelshell/actions/runs/37220102376)。标准macOS双程序开发包完成隔离SSH/SFTP读取、人工批准/拒绝、撤权与重启原生闭环；供应商MCP、Windows/Linux原生、六目标Release与实际安装更新未关闭，见[记录](../testing/records/2026-10-04-mcp-response-cancellation.md)。

更新：2026-10-05。目标是完整的远程 SSH 管理与运维工作流，并增加参考 DBX 的 AI 配置与辅助能力。最新进展、代码接续与未验证模块见 [交接记录](../HANDOFF.md)。

新增正式范围见[界面设计与智能体计划](../product/DESIGN_AND_AGENT_PLAN.md)：默认跟随系统的明暗主题、统一专业视觉体系、本地Claude Code/Codex接入，以及**仅向外部智能体提供能力的MCP服务端**；不开发通用第三方MCP客户端。主题基础已接通并完成领域/GPUI及macOS部分原生验证；本地CLI命名配置/能力检查/凭据引用/审核式Ask已接通并完成macOS安装版CLI→回环服务原生问答/取消。MCP已接通受认证桌面桥接和标准macOS双程序开发包受控读取/桌面审阅/撤权闭环，实际Claude授权客户端流程已完成，新的独立复核已通过该限定范围，无剩余 P1/P2。其余视觉矩阵、Agent/订阅登录、Codex MCP、文件修改提案和其他平台原生继续实施。

| 阶段 | 范围 | 状态 |
|---|---|---|
| M0 | 研究、命名、工程、版本策略、Git 与公开仓库 | 初始基线完成；公开 GitHub 已创建并推送，标签发布流程已配置；仓库内 `CHANGELOG.md` 可由 `scripts/changelog.py` 生成并由标签流水线校验 |
| M1 | 远程专用界面、中文默认、多语言、紧凑工作区 | 远程工作区、双语与分屏目标通过回归；空工作区提供不落盘的一次性 SSH 快速连接和显式“保存为连接”入口；Mac SSH/SFTP 与 AI 受控流程通过；后续代码的测试证据按功能记录 |
| M2 | SSH身份、SFTP、文件管理与传输 | 连接目录树、标签、回收恢复、最近成功记录及显式选区的批量移动/标签/收藏/回收/恢复/永久清理已接通，完整候选一次保存并保留当前SSH；领域、24项GPUI及独立小窗口探针通过，整合门禁通过，受控两目标标签原生读回另记；紧凑修复后新macOS八组合与补全原生验证通过，其余平台/文件写入原生边界另记；文件区保留至少64px浏览空间与真实首行，工具/编辑/比较/传输卡片可滚动，审核按钮固定；独立21项专项和176个生产主题GPUI场景通过，尚未作为原生验收；基础协议、文件 UI、FIFO 传输队列、分块进度与取消已接通；SSH 加密凭据保存、每次解锁、解除关联与整库认证已接入；凭据轮换/清理与有界递归目录传输已接入；普通传输与续传支持确认后暂停/继续，文件及目录可显式校验内容后续传，重连/重启后可创建新计划；本机及 macOS/Ubuntu CI 的 4 项 OpenSSH 互操作测试通过；Windows CI 发现的目录下载路径问题已修复，7f95c46 三平台 Quality 通过且保留原失败与回归；已建立会话可在原标签手动/可选有界自动重连，旧草稿/输出保留且操作不重放；新增安全的 OpenSSH 配置剪贴板导入、精确 Host/跳板解析与跳过项审阅报告；一次性键盘交互/MFA 提示已接入认证弹窗，答案只在本次路由内存中传递并在取消时失败关闭；失败传输现在可在同一活动 SSH 会话中显式发起新的只读续传校验，挂起或会话边界会使候选失效，仍不自动重放；完整自动恢复传输和并行队列仍待补；文件面板已接入审核式 POSIX 权限修改，符号链接和非 POSIX ACL/所有权保持拒绝或不变 |
| M3 | 监控、隧道、终端搜索、命令效率与批量操作 | 基础监控、监听端口诊断、显式远程 TCP 连接探测、TCP/SOCKS5 转发、终端滚动区搜索和按 SSH 会话隔离的命令历史已实现；探测仅由远程 Linux 主机执行固定 `nc -z`，结果可审核且不发送应用 payload；命令片段 CRUD、多行审核与本地历史/片段建议已接入；显式远端 PATH/字面路径补全已通过本机整仓、独立 OpenSSH、macOS 受控原生及三平台 Quality；显式变量片段与已连接会话的批量 exec 审核已接入；批量完成后会保存不含命令正文、输出、地址或凭据的有界摘要审计，并在片段/凭据模态关闭时可靠刷盘；批量命令现支持受限的逐目标元数据模板（{{name}}、{{host}}、{{port}}、{{user}}、{{endpoint}}），审核面板逐目标展示最终命令并在确认时绑定；手工依赖工作流UI、完整目标/命令/依赖/选项审核及逐任务有界输出已接通，支持1–128个任务、最多32个已认证目标与32个直接前置；复用核心审核/成功放行账本和捕获连接的真实SSH调度，只明确成功释放下游；独立29项工作流GPUI/TCP、严格Clippy/格式/版本策略复审通过，原冻结独立128任务/32目标和9项OpenSSH证据另行记录；根整合门禁通过，受控两任务退出0/7原生另记，紧凑修复后新包最小窗口通过，本轮未重复原生工作流执行，896提交三平台Quality通过；可编程补全、自定义参数映射和定时继续开发 |
| M4 | DBX式命名AI配置、发现/测试、上下文、Ask/诊断与审阅 | 命名配置、发现/测试/取消与精确审阅已接通并验收；AI 密钥显式加密保存/解锁及草稿 Apply 已接入；新增有界诊断计划，逐步绑定会话并回到命令审阅区；新增显式 Chat Completions/Responses/Anthropic Messages 请求协议、协议精确预览、x-api-key/版本头、分页模型发现和 loopback 回归；已接通输出Token上限与保守上下文窗口预算、精确协议字段审核及无效草稿/revision回归；自定义请求头/显式代理已精确整合F，独立差量、主树完整门禁及新macOS API限定原生验证通过，新提交CI另核验；非默认推理参数与 Agent 工作流仍待补 |
| M5 | SSH代理/跳板、同步、网络诊断、文件差异与高级运维 | 最多四个已保存跳板、逐跳认证/指纹/取消、路线绑定凭据已实现并完成受控原生验收；每跳 SOCKS5 / HTTP CONNECT 上游代理已接通，523 项普通测试、2 项文档测试、4 项独立 OpenSSH 互通及 macOS 受控原生验收通过，新增提交的跨平台 CI 另行记录；远程文件编辑器已支持有界 UTF-8 unified diff 预览，差异只在本地草稿与已读基线之间计算，仍需审核后保存；新增有界本地/SFTP 元数据快照、核心目录比较引擎和文件面板只读比较卡片，支持缺失字段标记为待复核并展示前 100 条结果；核心层现支持 64 MiB 上限的 SHA-256 内容摘要和带复制/显式删除策略的审核指纹计划，应用已接通内容摘要收集和显式确认后的双向目录合并，执行前复核整树及逐项内容，原子替换后读回并保留目标独有项；镜像删除、冲突合并、差异应用和高级网络诊断继续开发 |
| M6 | 独立审阅、跨平台原生矩阵、打包与完整验收 | 白底图标、原生六目标发布矩阵已配置；应用内项目/变更日志/发布检查、SHA-256 下载校验和显式安全安装已接入；helper 会按清单逐文件备份/替换、失败回滚并成功重启；实际构建见发布记录，Windows/Linux桌面、签名、公证和已签名安装目录原生验收仍待完成 |

传输暂停、续传及 OpenSSH 验证范围见[传输验收记录](../testing/records/2026-10-03-sftp-resume.md)。继续已暂停任务沿用原 SSH 会话；重连或重启后的续传需要重新选择源与目标、校验并确认，不自动恢复旧队列。

本地终端、RDP、串口不属于当前范围。历史本地 PTY 记录只保留工程溯源。

OpenSSH 配置导入已接入安全子集：精确 Host、HostName、Port、User、IdentityFile、ProxyJump 和显式 Include 内容；通配块、Match、ProxyCommand 及未知连接语义会被跳过并生成可审阅报告。外部 ProxyCommand 执行、完整 OpenSSH 条件求值和自动文件发现仍待设计。

已建立会话重连的范围与验证见[重连验收记录](../testing/records/2026-10-03-reconnection.md)。默认手动；自动遇认证/指纹提示暂停待用户继续，不恢复远端进程。远端 PATH 与字面路径补全已接入，当前验证见[补全验收记录](../testing/records/2026-10-03-remote-completion.md)；交互 shell 的可编程参数补全仍使用终端 Tab。变量片段与基础批量 exec 的当前证据见[参数与批量验收记录](../testing/records/2026-10-03-parameterized-snippets-batch-exec.md)：参数填写不执行、不保存本次值，展开命令默认不记历史；批量只针对已认证会话，经独立审核执行，不自动重连或重试。依赖图已有核心审核/状态账本、真实SSH执行适配器、手工编辑器和完整会话/选项审核，失败、未知或跳过阻止下游，独立分支按策略继续；任务与结果只保留在当前工作区，可隐藏/重开但不自动重试/重连/重放。新入口的独立29项工作流GPUI/TCP复审通过；旧后端证据见[适配器记录](../testing/records/2026-10-04-workflow-ssh-adapter.md)，新UI范围见[指南](../product/DEPENDENCY_WORKFLOWS.md)及[工作流记录](../testing/records/2026-10-05-dependency-workflow-ui.md)，合并后验收见[整合记录](../testing/records/2026-10-05-workspace-workflows-integration.md)。逐目标自定义参数映射、定时任务和任务级持久化日志仍待实现。


## 新增设计与智能体阶段

| 阶段 | 范围 | 当前状态与退出标准 |
| --- | --- | --- |
| D1 | UI-01/02/06：System/Light/Dark、语义token、设计资料库 | 主题基础已实现：System默认/旧配置迁移、窗口通知、语义palette、显式切换/后台保存；866普通+6文档、47打包和新独立审查通过；macOS两会话/草稿/明暗重启证实。新包900×580中英/明暗/AI八组合原生通过；真实OS变化、完整全屏矩阵与Windows/Linux原生未关闭，见[主题记录](../testing/records/2026-10-04-system-themes.md) |
| D2 | UI-03/04/05：丰富但克制的控件、专业工作区、模态/日志/滚动 | 部分实现并继续实施；主题token基础见D1。连接批量审核、文件操作换行/真实首行/有界工具滚动/固定确认和依赖流程入口换行已合入，分别通过独立GPUI复审；已转换tooltip的语言即时重绘回归通过，余下静态调用点由根整合补齐。旧包小窗口实际发现终端37px后，短窗口补全收为单行并保留输入/文件实体；32完整Files与16完整候选列表GPUI场景、根1031普通+8文档+6脚本/57打包通过。fresh紧凑独立复审和新macOS包八组合/真实补全/Files恢复通过，896提交三平台Quality通过；实际原生终端高度为人工读像素约89/121px，GPUI为精确测量，176文件场景不关闭全屏视觉矩阵；新增弹窗最终输入修复通过独立11原/8私有回归、76完成帧场景与根1053普通门禁，新macOS包已证实当前背景AX节点移除、草稿/偏好保留与原键盘触发焦点返回，旧原生AX对象激活、屏幕阅读器/IME与其他平台仍待验收，见[整合记录](../testing/records/2026-10-05-ai-modal-mcp-integration.md) |
| A1 | 共享API/LocalAgent配置、上下文与授权契约 | 已实现显式backend、旧配置API默认、路径/地址独立校验、准确stdin预览、版本/能力探针、取消/revision与vault v2精确backend/path绑定；详见[ADR0040](../adr/0040-named-local-agent-settings-and-reviewed-asks.md) |
| A2 | MCP-01至04：KeelShell对外MCP | 已接通默认关闭的stdio伴随程序、受认证桌面IPC、精确活动SSH句柄和八项固定工具。此前实际Claude Code七工具的授权读取、越权/路线拒绝与桌面批准/拒绝在限定范围通过独立复核；Codex完整业务调用仍未通过。第八项文件修改提案已完成新非作者工程复核和macOS自有八工具客户端的批准/拒绝/已观察并发修改拦截，当前F包中英文三主题长文件审阅及人工拒绝也已独立核验；供应商第八工具、最小原生窗口、其他平台原生和最终六目标Release/安装更新仍开放。见[文件提案记录](../testing/records/2026-10-05-mcp-file-proposals.md)、[六组合记录](../testing/records/2026-10-05-mcp-file-review-native-matrix.md)及[接入指南](../product/EXTERNAL_MCP.md) |
| A3 | AI-LOCAL-01至03、AI-AGENT-01：本地CLI Ask/Agent | Codex0.160.0/Claude2.1.285固定Ask后端与应用接通；独立空目录/受控环境、完整JSONL终态与owned进程清理；929普通+8文档、47打包、新独立审查与macOS实际安装版→自有SSE窗口问答/取消/重启密钥失效通过。首轮新源码Windows CI出现控制器主线程栈溢出；两处读流缓冲已堆分配，尺寸与显式2MiB完整控制器回归通过，修复be9590f2的Windows原生CI已通过两种完整控制器，914普通+8文档；同次Linux另有MCP错误响应超时，整次CI仍失败；SDK取消窗口已修复，后续34cff1b三平台Quality通过，三平台默认与2 MiB完整控制器均成功，原失败记录保留，见[取消修复记录](../testing/records/2026-10-04-mcp-response-cancellation.md)与[小栈记录](../testing/records/2026-10-04-local-agent-windows-stack.md)。仅显式API密钥，不复用订阅登录；本地Ask时限/回答/累计输出预算已实现并通过新独立复核、根整合门禁与新macOS原生设置保存/重开读回，无效草稿跨配置/偏好保留；新预算供应商问答与其他平台原生仍待验收，见[预算记录](../testing/records/2026-10-05-local-agent-limits.md)；自定义工作目录/环境引用、可见步骤流、Agent与Windows/Linux原生继续开发/验证，见[记录](../testing/records/2026-10-04-local-agent-ui.md) |

D1先于后续新界面，原有后端与任务目标继续保留。全部新增条目的细节、参考与验收见正式计划；“本地智能体”不是本地terminal管理，也不等于模型离线。

2026-10-05早期客户端范围保留：首次Claude2.1.285只执行未授权list_sessions→DISABLED→同客户端模型请求的结果循环，没有GUI/SSH；当时Codex文本前置失败且0 POST，原证据及独立复审更正均保留在[前置记录](../testing/records/2026-10-05-external-client-mcp-preflight.md)。后续新范围证实Codex尝试连接受限策略禁止的代理端口，仅为子进程加入回环NO_PROXY后直连自有模型服务并文本成功；不把新errno追溯到未采集errno的旧失败。Claude授权流程的新通过范围见本页顶部与A2，不整体关闭MCP-01至04。
