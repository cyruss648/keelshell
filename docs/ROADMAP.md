# 开发路线

## 当前产品缺口与实施顺序 — 2026-10-08

最新 `437f896` 的[精确三平台 Quality](https://github.com/cyruss648/keelshell/actions/runs/37717370699)已全部结束且失败；首次[真实 Windows 诊断](https://github.com/cyruss648/keelshell/actions/runs/37717377052)只通过 SDK inventory，随后控制器回归失败，五项 CDB 控制与原706项应用测试未运行。Windows READY／原后代句柄身份夹具修正已通过本机48项及新非作者限定复核，准确新Windows仍待运行；Linux原8秒目录超时和macOS ProgressAsk取消后的连接身份继续OPEN；本机工程及限定原生结果保留其范围，不关闭这些失败，见[实际诊断与平台后续](testing/records/2026-10-08-windows-native-crash-diagnostics.md)。

本次配置恢复提交收录 v9 功能与数量归属修正。最终同 824 输入／16,151,246 字节完整检查实际通过 1,812 普通 Rust／10 rustdoc／6 Python及严格工程检查，新标准 macOS 包／57 打包通过；17 份修后原生观察完成有效／损坏恢复、取消、重启和完整字节读回，另四份中文浅色原图补齐当前 2／恢复后 1。两个新 owner 实际 wait=0及应用／所属资源收尾已消费；旧失败、中断、旧 P2 和 owner UNKNOWN 保留。新的非作者最终组合复核无 P1/P2，数量归属 P2 已关闭；收尾仅四份状态文档、其余 820 输入不变，精确提交 CI 另行验证；最小原生几何、辅助技术、其它平台及断电耐久性独立开放，见[整合记录](testing/records/2026-10-08-configuration-recovery-main-integration.md)。

本轮测试修复基于已推送检查点 `7c11954`（本地／远程 SFTP 浏览）。精确三平台 Quality 的 macOS 成功、Windows 与 Linux 失败；Windows 字面上级路径测试修正及 Linux 夹具准入诊断已通过本机完整门禁（1,776 普通 Rust／10 rustdoc／6 Python、严格 Clippy）和 57 项打包测试，新的非作者最终复核无 P1/P2 阻断；修复提交 a717400 的精确 CI 已结束，macOS／Linux 成功，Windows 应用测试进程异常退出，触发者仍未知；诊断 v2 三文件经独立限定审查导入，根 42 项 Python 回归通过，实际 Windows 原源码／四线程诊断待运行，见[诊断记录](testing/records/2026-10-08-windows-native-crash-diagnostics.md)。准确 Linux 历史原因仍未知，见[平台记录](testing/records/2026-10-08-file-browser-platform-ci.md)。以下 a940 为历史检查点。Windows 准备失败收尾的 test-only 候选已在根通过十项新控制、原默认／2MiB控制器、AI库和严格Clippy；它没有改变产品范围，也未确定历史Windows失败原因，见[准备期记录](testing/records/2026-10-08-readiness-cleanup-diagnostics.md)。

已推送基线 `219093c` 包含审核式 Agent、推理/采样参数、显式本地 CLI 工作目录、有界任务结果审计、加密连接配置同步、远程 DNS/TLS/HTTP 诊断及文件审核视口／生命周期v5；其工程与限定原生证据各自保留。精确219三平台CI已结束：macOS/Linux成功，Windows后代端口3秒准备期失败，Ask/清理终态未记录，原因UNKNOWN。当前新增自动更新策略和会话工具栏组合已完成最终800输入完整门禁、同源码标准macOS包、限定宽窗口更新设置／双SSH交互及新非作者最终源码／证据复核；提交CI与英文原生small P2另行追踪，见[组合记录](testing/records/2026-10-08-update-toolbar-main-combination.md)。以下十项依据当前源码与[原始远程要求](research/remote-ssh-requirements.md)列出，保留真实余项；下一步优先日常SFTP浏览／编辑和工作区布局。

| 顺序/级别 | 真实剩余缺口 | 最小实施范围与验收边界 |
| --- | --- | --- |
| 1 / P1 | 更新签名信任与安装验收（PLAT-01） | 项目链接、变更日志、手动检查、启动／周期检查、失败退避、持久化策略及可选校验下载已实现，安装保留明确确认。剩余签名清单／可信发布者与平台验签兼容契约、macOS签名／公证和Windows签名；以隔离安装目录完成真实下载、失败回滚和重启原生验证。六目标构建、受控服务测试和宽窗口设置保存不代替这些验收。 |
| 2 / P1 | 日常 SFTP 浏览与批量入口（FILE-01/02/03） | 已接通系统单目录选择、本地／远程双栏、隐藏项、排序和仅填入路径的选择动作；继续多选／拖放和完整批量目标预览。复用现有队列、内容校验和审核；路径或会话变化撤销旧计划，部分失败逐目标报告。验证真实本地文件与自有 SFTP 读回，未经审核不写入。 v5 新非作者专项及136项文件回归通过；根810输入完整门禁、新macOS标准包和宽窗口 picker／人工审核双向 SFTP 已完成。新组合源码／工程／宽窗口原生独立复核无 P1/P2，精确CI／最小原生窗口／其它平台、多选／拖放／完整批量目标继续开放。 |
| 3 / P1 | 日常远程文本编辑（FILE-04） | 现有有界 UTF-8 编辑、差异、冲突合并/patch 草稿和审核保存保留；先改进真实可用视口、查找/替换与语法高亮，再设计编码/换行保持和保存前可恢复备份。当前候选视口修复只关闭其限定问题；长行、键盘/IME、取消与恢复、完整字节读回和其它平台仍需分别验证。 |
| 4 / P1 | 专业 pane 工作区与布局恢复（TERM-02/UI-04） | 现有标签与成对分屏保留；设计类型化横/纵嵌套 pane、拖动比例、活动 pane 作用域、多窗口及布局持久化。恢复布局和连接意图时不重放命令/传输。用最小窗口、焦点切换、关闭/重连及重启的真实原生场景验收。 |
| 5 / P1 | 用户显式选择 CLI 自身登录身份及版本兼容（AI-LOCAL） | 现有 API 身份调用本地 Codex/Claude Code 已实现，默认继续 API；订阅/登录复用尚未实现。先证明请求前可禁用 hooks、MCP、skills、plugins 和工具，并处理组织策略强制启用；不能只凭 help/标志或事后事件拒绝。供应商 CLI 自己处理认证，KeelShell 不读/复制 token、不自动登录，未知版本拒绝。若当前版本无法证明准入边界则保持 OPEN，不提供占位开关；真实账号/模型和原生验收独立记录。 |
| 6 / P2 | 配置备份轮转、损坏恢复与升级回退（CON-04/SYNC-01） | 隔离候选已编写八槽单调metadata历史、显式预览／确认恢复、完整原件不自动删除、revision与资料字节绑定和失败回滚；独立vault／OS凭据不复制，既有schema缺省迁移保持，未来schema不推断。v1静态P2已修正为deferred-close失败后保留owner和typed原因；冻结v8实际通过23项core集成／3项失败控制／6项Workspace GPUI／2项typed UI控制、依赖策略／最终格式／全workspace all-targets严格Clippy，新非作者已复核同一输入和原始结果；主树及数量修正已整合；最终824输入1,812普通Rust／10doc／6Python严格门禁、新标准macOS包／57打包和17+4修后原生／完整字节核对通过，两个新owner实际wait0；新非作者最终组合复核无P1/P2、数量归属P2已关闭，提交CI待验证，最小OS几何、辅助技术、其它平台与断电耐久性开放；见[指南](product/CONFIGURATION_RECOVERY.md)与[候选记录](testing/records/2026-10-08-configuration-recovery.md)。现有加密跨设备同步不能代替本机恢复。 |
| 7 / P2 | 远端能力识别、资源趋势与网络诊断（MON/NET-02） | 已有 Linux 监控及 DNS/TLS/HTTP HEAD/TCP 诊断保留；补能力识别、短期趋势、ping/丢包/traceroute 的明确降级，再设计远端 macOS/Windows 适配。缺少命令/权限时展示不可用，不能用桌面平台兼容性声称远端 OS 支持。以自有远端固定有界命令与实际样本验收。 |
| 8 / P2 | 持久化隧道规则与真实流量统计（NET-01） | 已有 local/remote TCP 与 SOCKS5 转发保留；补命名规则、审核后启停和真实字节计数。保存规则不保存密码，不在重启时自动监听；重连使旧运行绑定失效。验证端口占用、半关闭、显式停止、断线释放与实际流量往返。 |
| 9 / P3 | 高级传输和用户中继（FILE-05/NET-03） | 保留归档批量传输、PTY Zmodem 和用户管理中继/测速目标。先做审核式归档与路径穿越拒绝，再单独完成 rz/sz 实机协议；中继必须有可重复的延迟/丢包 A/B 数据，无收益不称加速，跳板功能不能代替此项。 |
| 10 / P3 | AI 知识与复用（AI-04） | 基于用户选择的已完成事实，提供来源绑定的脱敏摘要、时间线、Markdown/runbook 保存与本地搜索。先预览再明确保存/发送，不上传真实日志或把任务结果审计当知识库；未知结果和秘密排除分别有验证。 |

上述开发与已有实现的验收并行推进：完整外部 Codex MCP 批准/拒绝/撤权、新 Agent 原生回合、语言/主题/最小窗口/两轴滚动/键盘与辅助技术、Windows/Linux 桌面、六目标发布安装更新都保持各自未完成边界。MCP 始终由 KeelShell 提供给外部智能体，应用内 API/CLI 推理是另一入口；不新增通用第三方 MCP 客户端。双语 README 的现有能力表述继续保留。

## 工程检查点与阶段记录

下文按原时间保留各轮源码/CI/原生证据、失败与候选状态；早期“未整合/待补”只描述当时范围，不覆盖上面的当前产品状态。2026-10-08更新／工具栏最终800输入门禁实际通过1,731普通／10doc／6Python、严格检查及默认／2MiB各626阶段；同输入标准macOS包与57打包实际0，新宽窗口设置保存／重开、双SSH及草稿保留已完成。未测的最小几何／辅助技术／实际安装／其它平台保持OPEN。文件审核v5此前限定工程／原生证据继续单独保留，不能套用旧主线或早期候选的结果。

2026-10-07 独立同步运行阶段观察与开发 KDF 优化候选：四项测试专用观察和原 18/45 秒保存资料同步用例、严格 Clippy/格式/x.y 策略实际通过；仅 Argon2 依赖开发优化在自有 Linux ARM64 单 CPU 容器获得约 2.31–2.36 倍操作速度，安全参数、锁定依赖及既有期限不变。项目优化配置的 12 项 app、65 项 core 与严格工程检查实际通过，741 输入前后相同，所属进程组/TMP 已回收；新非作者复核、主线组合与精确提交 CI 仍开放，历史 Linux 超时原因未知；详见[阶段观察](testing/records/2026-10-07-profile-sync-runtime-observation.md)、[优化记录](testing/records/2026-10-07-development-kdf-build-optimization.md)与[ADR0075](adr/0075-development-kdf-build-optimization.md)。

2026-10-07 当前增量：已将新的非作者限定复核通过的审核式 Agent epoch3 和 MCP原SSH生命周期撤权精确导入主工作副本，29路径、共享代码三方合并且无关基线保持。新的组合源码审查无阻断，根765输入完整门禁实际0/850.425秒，1677普通/10doc/6Python及严格检查、两种626控制器通过；同输入新macOS包与57打包实际0/36.350秒、资源回收。结果Markdown更新后非文档相同，新精确提交CI另记[整合记录](testing/records/2026-10-07-reviewed-ai-mcp-main-integration.md)。后续继续新组合原生Agent/外部客户端、同时活跃两owner撤权、UI最小窗口/键盘/辅助技术和Windows/Linux桌面。旧候选P1与原始失败保留，源码实现不等于完整产品完成。

b54文件合并及严格patch的两次有限macOS草稿/保存已完整远端读回并独立证据复核，见[原生记录](testing/records/2026-10-07-text-merge-patch-native.md)；短审核视口与offscreen AX输入仍需改进并验证键盘操作。精确b54 Quality为macOS/Windows成功、Linux首次目录Ask原8秒Timeout，原因未知且未进入app同步，见[平台CI](testing/records/2026-10-07-text-main-platform-ci.md)。test-only同步阶段recorder与debug KDF优化候选需新的非作者及主树整合，不以本机或LinuxARM64成本样本宣称历史CI根因修复。以下历史条目保留各自时间与范围。

2026-10-07 精确 `319cb2a` 三平台CI已结束，macOS/Linux成功、Windows两项应用工作目录夹具失败。新的test-only路径/JSON值修正保持生产限制与原期限，完整本机门禁1612普通/8doc/6Python及严格检查实际0，736输入相同、资源回收；非作者源码/两项GPUI/全证据复核无阻断，新提交Windows执行与原生仍待完成。旧Linux同步原因不追溯关闭。见[应用夹具记录](testing/records/2026-10-07-selected-directory-app-platform-followup.md)。同一734输入MCP新尝试停在首个120秒窗口确认，操作端记录未成功，0CLI/模型/业务；隔离资源已回收，单独截图探针不是业务成功。见[启动记录](testing/records/2026-10-07-external-mcp-current-main-startup.md)。对外MCP方向和完整产品目标保持。

17路径的三方冲突合并/审核patch已完成独立复核、作者门禁及根全量证据消费，按精确preimage/postimage导入主副本并保留其余基线输入；主树745输入完整门禁实际0/994.033秒，1641普通/10doc/6Python及新macOS开发包/57打包通过，新原生与精确新CI仍待完成，见[整合记录](testing/records/2026-10-07-reviewed-text-main-integration.md)。当时的Agent旧候选因关闭捕获页签后仍可后台写入的真实独立P1被阻止整合；修正epoch2冻结742输入，完整门禁实际0/506.581秒、新macOS开发包/结构检查/57打包通过，当时仍需新的独立撤权与raw终端结束复核。后续epoch3已整合，当前范围见本页顶部与[Agent指南](product/REVIEWED_AGENT.md)。候选检查不作为主树已交付或完整产品验收。

2026-10-07 后续 test-only 修复已通过本机正式门禁和新非作者源码/证据复核：Windows单元夹具路径修正、命名空间拒绝反例、按既有单次预算推导总目录控制器304/224秒。734输入前后相等，142普通AI/2doc、Unix目录20及默认/2MiB各626阶段实际0；4 ignored未执行。原3873 Windows/Linux两项失败继续保留；新提交级CI、未进入的Linux app同步根因、完整原生和产品目标仍开放。见[后续记录](testing/records/2026-10-07-directory-controller-platform-followup.md)。用户再次确认MCP只由KeelShell对外提供服务，应用内API/CLI推理独立，不新增第三方MCP客户端。

本轮平台修复在已推送 `0d64f09` 基础上通过正式本机门禁：1,612普通/8doc/6Python、格式/x.y/严格Clippy，57打包及macOS双程序构建实际0；三份733输入相同，新的非作者源码/证据复核无新增阻断。旧0d64 CI仍为macOS成功、Windows条件编译和Linux目录readiness失败；修复需要自己的新提交三平台CI，完整Windows/Linux桌面及Linux同步根因继续开放。详见[平台记录](testing/records/2026-10-07-local-agent-directory-platform-fix.md)。MCP始终由KeelShell向外部智能体提供服务，应用内API/CLI Ask独立。

2026-10-07 当前组合的新macOS标准双程序开发包已实际构建并通过57项打包、Info.plist和动态库检查；719输入前后相等，非文档生产输入保持完整工程检查的717范围。后续同一程序已完成限定双向镜像原生验证，没有安装或发布；完整桌面／外部客户端／Windows/Linux原生与精确提交CI仍开放，见[组合记录](testing/records/2026-10-07-recursive-mirror-main-combination.md)。

2026-10-07 文件审核呈现：新非作者限定复核通过，原文逐行Label、真实两轴滚动、固定动作与新审核重置已导入主副本；受控GPUI/TCP及取消后文件读回通过，后续717输入最终组合门禁实际0；新原生仍开放，见[整合记录](testing/records/2026-10-07-file-review-main-integration.md)。递归非空子树镜像已通过新的非作者两阶段限定复核并精确整合；1,587普通／8doc／6Python及严格工程检查通过，20ignored未执行，详见[组合记录](testing/records/2026-10-07-recursive-mirror-main-combination.md)。

2026-10-07 对外 MCP 更新：实际 Codex 新 V12 原生流程完成 20 项配对及语义复核，包含命令批准／拒绝、文件批准和完整 SFTP 内容读回。第二份文件在原生界面已拒绝，但测试确认标记迟到导致超时，后续两项读取及撤权未到达，完整业务保持未通过。三次真实失败、独立证据复核与限定自有资源清理见[V12 记录](testing/records/2026-10-07-codex-mcp-v12-native.md)。范围始终为 KeelShell 提供 MCP 给外部智能体，应用内 API／CLI Ask 独立。

完整远程SSH产品目标仍未完成。当前API推理/采样与审核可达性整合通过1211普通/8doc/6Python完整门禁和新非作者代码/GPUI复核；新macOS三协议限定原生证据见下文。此前整合普通AI命令脱敏误报修复与test-only控制器阶段观测，已通过新的非作者代码/行为复核、主树完整门禁1179普通+8doc+6脚本及严格检查。默认/2MiB控制器各626条阶段记录、38次原TCP调用和5080字节future通过；原Windows45秒失败根因仍未知，不由本机通过关闭。详见[脱敏记录](testing/records/2026-10-06-ai-redaction-boundaries.md)、[控制器记录](testing/records/2026-10-06-ask-controller-stages.md)。

新macOS开发包确认普通建议可读、明确捕获SSH上下文后人工送入精确命令并清空、8条事实最后一行可达；新非作者原生证据复核通过限定范围。首次无会话交付未确认，原失败保留；没有执行shell命令、900×580测量、完整语言/主题矩阵或其它平台原生。CI与发布边界见[交接](HANDOFF.md)、[CI终态记录](testing/records/2026-10-06-local-ask-progress-ci.md)。以前的进展保存在[历史路线](history/2026-10-06-roadmap-before-redaction.md)，不作为当前状态。

预算修正源码检查点5b6b7b4的Windows整体测试预算90秒修正已通过新非作者审查和精确源码三平台Quality；Windows小栈实际69.28秒完成全部626阶段/38TCP，原失败保持。这只关闭该项源码测试预算，完整跨平台桌面、发布与更新验收继续开放，见[记录](testing/records/2026-10-06-windows-small-stack-budget.md)。

MCP始终是KeelShell向外部智能体提供能力的服务端，应用内API/CLI Ask独立，不接入通用第三方MCP服务。API参数v2数值秘密准入P2已通过实际Apply/持久化/正文探针及新独立复审关闭；设置显式滚动条、长错误反馈与请求审核固定确认区已整合。最终主树1211普通/8doc/6Python完整门禁及新非作者代码/GPUI复核通过，新macOS中文/System包实际完成三协议人工发送与回复、参数读回和Messages真实拖动正文；最小原生窗口、最终语言/主题原生矩阵及其它平台仍开放。见[参数记录](testing/records/2026-10-06-ai-inference-options.md)、[滚动与确认记录](testing/records/2026-10-06-ai-request-review-scrollbar.md)。命令目标提示已整合并通过新非作者限定原生复核，精确1e07a28三平台Quality已success；当前API新提交CI另核验，静态AX和可读会话标签仍开放，见[目标记录](testing/records/2026-10-06-ai-command-review-target.md)。并行传输写入口与待CLOSE隔离P2已修复并通过独立复审，后续READ EOF空闲记账修复也已通过完整复审并整合；原macOS两文件上传零完成失败保留；新源码macOS受控双上传已确认同时Running、两条完整Completed、独立路径、暂停/继续、队列可达性及两份4MiB精确远端内容；其它原生范围继续开放，见[记录](testing/records/2026-10-06-parallel-transfers-native-v5.md)。外部Codex目录比较器与recorder finally两项P2已通过限定预检复审，实际固定400、0 SSE、0业务，完整授权业务仍未通过。各切片均须新非作者复核、主树门禁及各自原生验证，完整目标继续追踪。

| 阶段 | 范围 | 状态 |
|---|---|---|
| M0 | 研究、命名、工程、版本策略、Git 与公开仓库 | 初始基线完成；公开 GitHub 已创建并推送，标签发布流程已配置；仓库内 `CHANGELOG.md` 可由 `scripts/changelog.py` 生成并由标签流水线校验 |
| M1 | 远程专用界面、中文默认、多语言、紧凑工作区 | 远程工作区、双语与分屏目标通过回归；空工作区提供不落盘的一次性 SSH 快速连接和显式“保存为连接”入口；Mac SSH/SFTP 与 AI 受控流程通过；后续代码的测试证据按功能记录 |
| M2 | SSH身份、SFTP、文件管理与传输 | 连接目录树、标签、回收恢复、最近成功记录及显式选区的批量移动/标签/收藏/回收/恢复/永久清理已接通，完整候选一次保存并保留当前SSH；领域、24项GPUI及独立小窗口探针通过，整合门禁通过，受控两目标标签原生读回另记；紧凑修复后新macOS八组合与补全原生验证通过，其余平台/文件写入原生边界另记；文件区保留至少64px浏览空间与真实首行，工具/编辑/比较/传输卡片可滚动，审核按钮固定；独立21项专项和176个生产主题GPUI场景通过，尚未作为原生验收；基础协议、文件 UI、FIFO 传输队列、分块进度与取消已接通；SSH 加密凭据保存、每次解锁、解除关联与整库认证已接入；凭据轮换/清理与有界递归目录传输已接入；普通传输与续传支持确认后暂停/继续，文件及目录可显式校验内容后续传，重连/重启后可创建新计划；本机及 macOS/Ubuntu CI 的 4 项 OpenSSH 互操作测试通过；Windows CI 发现的目录下载路径问题已修复，7f95c46 三平台 Quality 通过且保留原失败与回归；已建立会话可在原标签手动/可选有界自动重连，旧草稿/输出保留且操作不重放；新增安全的 OpenSSH 配置剪贴板导入、精确 Host/跳板解析与跳过项审阅报告；一次性键盘交互/MFA 提示已接入认证弹窗，答案只在本次路由内存中传递并在取消时失败关闭；失败传输现在可在同一活动 SSH 会话中显式发起新的只读续传校验，挂起或会话边界会使候选失效，仍不自动重放；并行队列已整合并通过组合门禁，macOS受控双上传、队列可达性与暂停/继续通过，其它传输模式与平台仍开放；完整自动恢复传输仍待补；文件面板已接入审核式 POSIX 权限修改，符号链接和非 POSIX ACL/所有权保持拒绝或不变 |
| M3 | 监控、隧道、终端搜索、命令效率与批量操作 | 基础监控、监听端口诊断、显式远程 TCP 连接探测、TCP/SOCKS5 转发、终端滚动区搜索和按 SSH 会话隔离的命令历史已实现；探测仅由远程 Linux 主机执行固定 `nc -z`，结果可审核且不发送应用 payload；命令片段 CRUD、多行审核与本地历史/片段建议已接入；显式远端 PATH/字面路径补全已通过本机整仓、独立 OpenSSH、macOS 受控原生及三平台 Quality；显式变量片段与已连接会话的批量 exec 审核已接入；批量完成后会保存不含命令正文、输出、地址或凭据的有界摘要审计，并在片段/凭据模态关闭时可靠刷盘；批量命令现支持受限的逐目标元数据模板（{{name}}、{{host}}、{{port}}、{{user}}、{{endpoint}}），审核面板逐目标展示最终命令并在确认时绑定；手工依赖工作流UI、完整目标/命令/依赖/选项审核及逐任务有界输出已接通，支持1–128个任务、最多32个已认证目标与32个直接前置；复用核心审核/成功放行账本和捕获连接的真实SSH调度，只明确成功释放下游；独立29项工作流GPUI/TCP、严格Clippy/格式/版本策略复审通过，原冻结独立128任务/32目标和9项OpenSSH证据另行记录；根整合门禁通过，受控两任务退出0/7原生另记，紧凑修复后新包最小窗口通过，本轮未重复原生工作流执行，896提交三平台Quality通过；逐目标自定义参数与认证连接实例绑定已通过新的非作者复审并在根整合，最终独立1225普通/8doc/6Python；根27项专项、格式/x.y/严格Clippy通过，589完整输入前后相等；完整组合门禁已通过，macOS双目标与共享参数依赖任务的正常执行原生范围通过，更多场景/平台仍开放，见[原生记录](testing/records/2026-10-06-target-parameters-disk-native.md)及[参数记录](testing/records/2026-10-06-target-parameters.md)；逐设备磁盘I/O已通过新的非作者1257普通/8doc/6Python及GPUI/TCP/真实内核样本复审并窄合14路径，主线组合门禁已通过，macOS真实SSH/Linux VM采样、设备选择及暂停后手动刷新取得有限原生证据；其它平台/暂停无自动请求仍开放；有限定时已与参数/同步组合并通过新非作者限定审查，根整合完整检查已1478普通/8doc/6Python通过，可编程补全继续开发 |
| M4 | DBX式命名AI配置、发现/测试、上下文、Ask/诊断与审阅 | 命名配置、发现/测试/取消与精确审阅已接通并验收；AI 密钥显式加密保存/解锁及草稿 Apply 已接入；新增有界诊断计划，逐步绑定会话并回到命令审阅区；新增显式 Chat Completions/Responses/Anthropic Messages 请求协议、协议精确预览、x-api-key/版本头、分页模型发现和 loopback 回归；已接通输出Token上限与保守上下文窗口预算、精确协议字段审核及无效草稿/revision回归；自定义请求头/显式代理已精确整合F，独立差量、主树完整门禁及新macOS API限定原生验证通过，新提交CI另核验；非默认推理/采样v2、设置滚动条与固定确认区已整合，新非作者代码/GPUI和最终1211普通门禁通过；新macOS中文/System实际三协议人工发送/回复、读回与长预览拖动通过限定范围，原失败保留；最小原生窗口、最终语言/主题矩阵及其他平台仍开放；审核式 Agent epoch3 已整合，每轮请求与每项 SSH/SFTP 操作分别人工审阅，停止、会话结束及上下文/配置变化撤权；新的原生 Agent、供应商账户/模型与完整平台验收仍开放，见[指南](product/REVIEWED_AGENT.md) |
| M5 | SSH代理/跳板、同步、网络诊断、文件差异与高级运维 | 最多四个已保存跳板、逐跳认证/指纹/取消、路线绑定凭据已实现并完成受控原生验收；每跳 SOCKS5 / HTTP CONNECT 上游代理已接通，523 项普通测试、2 项文档测试、4 项独立 OpenSSH 互通及 macOS 受控原生验收通过，新增提交的跨平台 CI 另行记录；远程文件编辑器已支持有界 UTF-8 unified diff 预览，差异只在本地草稿与已读基线之间计算，仍需审核后保存；新增有界本地/SFTP 元数据快照、核心目录比较引擎和文件面板只读比较卡片，支持缺失字段标记为待复核并展示前 100 条结果；核心层现支持 64 MiB 上限的 SHA-256 内容摘要和带复制/显式删除策略的审核指纹计划，应用已接通内容摘要收集和显式确认后的双向目录合并，执行前复核整树及逐项内容，原子替换后读回并保留目标独有项；常规文件／已空目录镜像撤权修复及非空子树镜像已通过新的限定非作者复核并整合；完整逐项审核、子先父后删除、剩余命名空间／内容复核及717输入最终组合门禁通过，限定 macOS 双向镜像原生已通过，完整原生与精确CI开放；审核式文本冲突合并和严格单文件 patch 草稿已整合，两次有限 macOS 草稿/保存有完整 SFTP 读回，短视口/键盘/辅助技术和完整矩阵继续整改，见[文本整合](testing/records/2026-10-07-reviewed-text-main-integration.md)；远程DNS/TLS/HTTP(S)明确审核与有界监督候选已通过限定非作者复核，29路径与两项同路径test-only补充已按当前690输入完成隔离三方准备并导入主工作副本，703份导入结果逐字节相等；后续717输入最终组合门禁通过，新CI与原生仍开放，见[整合记录](testing/records/2026-10-07-remote-protocol-main-integration.md)及[当前组合](testing/records/2026-10-07-recursive-mirror-main-combination.md)；UDP、远端Windows和更广协议继续开放，见[协议记录](testing/records/2026-10-06-remote-protocol-diagnostics.md) |
| M6 | 独立审阅、跨平台原生矩阵、打包与完整验收 | 白底图标、原生六目标发布矩阵已配置；应用内项目/变更日志/发布检查、SHA-256 下载校验和显式安全安装已接入；helper 会按清单逐文件备份/替换、失败回滚并成功重启；实际构建见发布记录，Windows/Linux桌面、签名、公证和已签名安装目录原生验收仍待完成 |

传输暂停、续传及 OpenSSH 验证范围见[传输验收记录](testing/records/2026-10-03-sftp-resume.md)。继续已暂停任务沿用原 SSH 会话；重连或重启后的续传需要重新选择源与目标、校验并确认，不自动恢复旧队列。

本地终端、RDP、串口不属于当前范围。历史本地 PTY 记录只保留工程溯源。

OpenSSH 配置导入已接入安全子集：精确 Host、HostName、Port、User、IdentityFile、ProxyJump 和显式 Include 内容；通配块、Match、ProxyCommand 及未知连接语义会被跳过并生成可审阅报告。外部 ProxyCommand 执行、完整 OpenSSH 条件求值和自动文件发现仍待设计。

已建立会话重连的范围与验证见[重连验收记录](testing/records/2026-10-03-reconnection.md)。默认手动；自动遇认证/指纹提示暂停待用户继续，不恢复远端进程。远端 PATH 与字面路径补全已接入，当前验证见[补全验收记录](testing/records/2026-10-03-remote-completion.md)；交互 shell 的可编程参数补全仍使用终端 Tab。变量片段与基础批量 exec 的当前证据见[参数与批量验收记录](testing/records/2026-10-03-parameterized-snippets-batch-exec.md)：参数填写不执行、不保存本次值，展开命令默认不记历史；批量只针对已认证会话，经独立审核执行，不自动重连或重试。依赖图已有核心审核/状态账本、真实SSH执行适配器、手工编辑器和完整会话/选项审核，失败、未知或跳过阻止下游，独立分支按策略继续；可执行任务、命令与输出只保留在当前工作区，可隐藏/重开但不自动重试/重连/重放；另有已整合的有界只读结果审计，只保存固定结果、UUID和时间，不保存命令/输出/地址/参数，也不恢复执行。新入口的独立29项工作流GPUI/TCP复审通过；旧后端证据见[适配器记录](testing/records/2026-10-04-workflow-ssh-adapter.md)，新UI范围见[指南](product/DEPENDENCY_WORKFLOWS.md)及[工作流记录](testing/records/2026-10-05-dependency-workflow-ui.md)，合并后验收见[整合记录](testing/records/2026-10-05-workspace-workflows-integration.md)。逐目标自定义参数映射已整合并通过根专项及完整组合门禁，双目标正常执行与共享参数依赖任务已取得有限macOS原生证据，其它场景与平台仍开放。有限定时与连接库加密共享目录同步已在主线整合；以下保留当时作者候选及新非作者冻结证据：参数×有限序列作者组合已有专项与完整门禁证据。新的同步×参数×定时组合已通过作者完整检查和新非作者72项专项及同ID保存资料真实批准探针，根已整合作者与测试补充，主树完整检查已1478普通/8doc/6Python、格式/x.y/严格Clippy通过，652输入前后相等。组合原生与新提交CI仍待完成，见[主树整合记录](testing/records/2026-10-06-sync-schedule-main-integration.md)。任务级有界只读结果审计已通过作者门禁及两轮新的非作者复核，并已整合到同步与有限定时组合；只保存固定结果、UUID和时间，连接同步不携带本机审计。模态关闭自动保存遗漏已实际复现并由根修复，新非作者3项组合通过；任务审计0cca组合已通过1508普通/8doc/6Python完整门禁及新开发包检查，新三平台CI与原生矩阵仍待完成，见[任务记录](product/WORKFLOW_HISTORY.md)与[整合记录](testing/records/2026-10-06-task-audit-main-integration.md)。


## 新增设计与智能体阶段

| 阶段 | 范围 | 当前状态与退出标准 |
| --- | --- | --- |
| D1 | UI-01/02/06：System/Light/Dark、语义token、设计资料库 | 主题基础已实现：System默认/旧配置迁移、窗口通知、语义palette、显式切换/后台保存；866普通+6文档、47打包和新独立审查通过；macOS两会话/草稿/明暗重启证实。新包900×580中英/明暗/AI八组合原生通过；真实OS变化、完整全屏矩阵与Windows/Linux原生未关闭，见[主题记录](testing/records/2026-10-04-system-themes.md) |
| D2 | UI-03/04/05：丰富但克制的控件、专业工作区、模态/日志/滚动 | 部分实现并继续实施；主题token基础见D1。连接批量审核、文件操作换行/真实首行/有界工具滚动/固定确认和依赖流程入口换行已合入，分别通过独立GPUI复审；已转换tooltip的语言即时重绘回归通过，余下静态调用点由根整合补齐。旧包小窗口实际发现终端37px后，短窗口补全收为单行并保留输入/文件实体；32完整Files与16完整候选列表GPUI场景、根1031普通+8文档+6脚本/57打包通过。fresh紧凑独立复审和新macOS包八组合/真实补全/Files恢复通过，896提交三平台Quality通过；实际原生终端高度为人工读像素约89/121px，GPUI为精确测量，176文件场景不关闭全屏视觉矩阵；新增弹窗最终输入修复通过独立11原/8私有回归、76完成帧场景与根1053普通门禁，新macOS包已证实当前背景AX节点移除、草稿/偏好保留与原键盘触发焦点返回，旧原生AX对象激活、屏幕阅读器/IME与其他平台仍待验收，见[整合记录](testing/records/2026-10-05-ai-modal-mcp-integration.md) |
| A1 | 共享API/LocalAgent配置、上下文与授权契约 | 已实现显式backend、旧配置API默认、路径/地址独立校验、准确stdin预览、版本/能力探针、取消/revision与vault v2精确backend/path绑定；详见[ADR0040](adr/0040-named-local-agent-settings-and-reviewed-asks.md) |
| A2 | MCP-01至04：KeelShell对外MCP | 已接通默认关闭的stdio伴随程序、受认证桌面IPC、精确活动SSH句柄和八项固定工具。此前实际Claude Code七工具的授权读取、越权/路线拒绝与桌面批准/拒绝在限定范围通过独立复核；Codex完整业务调用仍未通过。第八项文件修改提案已完成新非作者工程复核和macOS自有八工具客户端的批准/拒绝/已观察并发修改拦截，当前F包中英文三主题长文件审阅及人工拒绝也已独立核验；供应商第八工具、最小原生窗口、其他平台原生和最终六目标Release/安装更新仍开放。见[文件提案记录](testing/records/2026-10-05-mcp-file-proposals.md)、[六组合记录](testing/records/2026-10-05-mcp-file-review-native-matrix.md)及[接入指南](product/EXTERNAL_MCP.md) |
| A3 | AI-LOCAL-01至03、AI-AGENT-01：本地CLI Ask/Agent | Codex0.160.0/0.160.1与Claude2.1.285固定Ask后端及应用已接通；独立空目录/受控环境、完整JSONL终态与owned进程清理；929普通+8文档、47打包、新独立审查与macOS实际安装版→自有SSE窗口问答/取消/重启密钥失效通过。首轮新源码Windows CI出现控制器主线程栈溢出；两处读流缓冲已堆分配，尺寸与显式2MiB完整控制器回归通过，修复be9590f2的Windows原生CI已通过两种完整控制器，914普通+8文档；同次Linux另有MCP错误响应超时，整次CI仍失败；SDK取消窗口已修复，后续34cff1b三平台Quality通过，三平台默认与2 MiB完整控制器均成功，原失败记录保留，见[取消修复记录](testing/records/2026-10-04-mcp-response-cancellation.md)与[小栈记录](testing/records/2026-10-04-local-agent-windows-stack.md)。仅显式API密钥，不复用订阅登录；本地Ask时限/回答/累计输出预算已实现并通过新独立复核、根整合门禁与新macOS原生设置保存/重开读回，无效草稿跨配置/偏好保留；新预算供应商问答与其他平台原生仍待验收，见[预算记录](testing/records/2026-10-05-local-agent-limits.md)；显式密钥环境引用已整合并通过源码CI，新桌面入口待验收；显式绝对工作目录、目录身份校验、固定bootstrap及审核入口已整合；有界步骤事实与KeelShell编排的审核式Agent已整合，供应商工具保持关闭。CLI订阅/登录身份复用尚未实现，仍需有效的请求前隔离证明；任意环境/config/hooks/tools转交不开放。新Agent、真实供应商和Windows/Linux原生继续验证，见[工作目录指南](product/LOCAL_AGENT_WORKING_DIRECTORY.md)、[Agent指南](product/REVIEWED_AGENT.md)和[原Ask记录](testing/records/2026-10-04-local-agent-ui.md) |

D1先于后续新界面，原有后端与任务目标继续保留。全部新增条目的细节、参考与验收见正式计划；“本地智能体”不是本地terminal管理，也不等于模型离线。

2026-10-05早期客户端范围保留：首次Claude2.1.285只执行未授权list_sessions→DISABLED→同客户端模型请求的结果循环，没有GUI/SSH；当时Codex文本前置失败且0 POST，原证据及独立复审更正均保留在[前置记录](testing/records/2026-10-05-external-client-mcp-preflight.md)。后续新范围证实Codex尝试连接受限策略禁止的代理端口，仅为子进程加入回环NO_PROXY后直连自有模型服务并文本成功；不把新errno追溯到未采集errno的旧失败。Claude授权流程的新通过范围见本页顶部与A2，不整体关闭MCP-01至04。


2026-10-06 本地配置增量：隔离候选复用 `AiSecretRef::Environment`，增加用户显式读取 API 密钥的环境引用编辑、接收方绑定、请求冻结与保留秘密保护。初版独立 argv 反例确认探针秘密准入 P2，未合入；v2 已补齐全目录/保留秘密的调度前检查，新的非作者1238普通/8doc/6Python完整门禁及双CLI零调度反例、合法有/无模型fixture通过；27 个候选路径已主副本整合，根22项专项及1281普通/8doc/6Python完整组合门禁通过；精确24a已提交本地CLI路径且三平台Quality完整读回通过，见[CI记录](testing/records/2026-10-06-local-agent-environment-ci.md)；新桌面验收仍待完成，见[修复记录](testing/records/2026-10-06-local-agent-probe-admission.md)。当时工作目录与受限 Agent 尚未整合；后续显式工作目录和审核式 Agent 已进入主线，见上方当前状态。订阅登录仍 OPEN，任意环境转交不开放；不使用 help 或 mock-child 关闭原生验收。

2026-10-06 传输修复已通过新的非作者完整复审并整合：1334普通/8doc/6Python、格式/x.y/严格Clippy及10项OpenSSH。原1秒预算/750 ms READ/350 ms CLOSE/22字节反例与0字节两路径均完成，真实未回复CLOSE仍超时，同会话其它owner活动不能续期未知写入；原正式exit101与原macOS零上传失败均保留。根合入27个不同路径并保留参数/本地智能体改动。随后615输入的完整组合1394普通/8doc/6Python、11项OpenSSH、57项打包与新标准Mac包检查通过；后续新准入macOS受控场景已验证同时Running、两条完整Completed、各自路径、暂停/继续、队列可达性、两份4MiB精确内容及独立清理复核，见[原生记录](testing/records/2026-10-06-parallel-transfers-native-v5.md)；其它原生范围开放，原失败保留。见[组合记录](testing/records/2026-10-06-remote-workspace-main-integration.md)。

历史工作区检查点 c158 与后继测试隔离提交 6b4 的 Quality 均为 macOS成功、Linux/Windows失败。四测试路径窄隔离修复已通过独立复核和1394普通/8doc/6Python本机完整门禁；6b4 的新失败为目录取消 helper 无终态和辅助 `SFTP read` 超时，新的真实 TCP 反例已保留，新的三个测试路径修复已完成独立复核和根1395普通/8doc/6Python完整检查，精确b26c4c8已推送，Quality37455024526三个job已actual completed/success，完整上游日志、attempt ZIP及两份OpenSSH收据已由新非作者和根读回，Linux/macOS各11项互通通过。生产资源隔离和1秒期限不放宽，原Windows超时具体原因未回溯确定。见[CI记录](testing/records/2026-10-06-workspace-ci-test-isolation.md)。

精确6b4的新macOS MCP尝试完成真实Codex八工具目录准入及新非作者复核，随后首个业务请求被验证记录器拒绝额外元数据，0个已授权业务RPC。两轮原生失败、直接实际wait、所属端口拒绝与文件观察保留；四个间接身份的旧探针仅为无类型null，完整间接/原进程组清理未证明；实际批准、拒绝、撤权和Codex完整业务仍开放，见[原生尝试记录](testing/records/2026-10-06-codex-mcp-native-partial.md)。

## 历史整合检查点 — 2026-10-06

精确 `7f50d60` 的 [Quality 37460171202](https://github.com/cyruss648/keelshell/actions/runs/37460171202) attempt 1 已实际结束：macOS、Windows成功，Linux失败。Linux在同ID保存资料审核测试中等待18秒未观察到预期元数据；原日志没有后台终态，因此具体原因仍未确认。测试诊断候选已通过新的非作者两次正常45秒运行及合法Stale反例；保留原期限和批准后的全部断言，不改变生产权限、同步或调度。整合后仍需新提交的CI，见[诊断记录](testing/2026-10-06-saved-profile-sync-approval-diagnostics.md)。

任务审计严格解析的原P2已实际复现并修复，新非作者验证全部60原控制及510额外字段控制拒绝、15结果/2触发往返、真实StateStore与受控GPUI/TCP行为。根两字段双存储测试通过；同步模态关闭遗漏已实际复现并由根修复，新非作者3项GPUI/TCP组合及新的双存储/wire golden通过，完整证据已读回。当前671输入的主树1508普通Rust/8doc/6Python、格式/x.y/严格Clippy及默认/2MiB控制器全部实际通过；新macOS双程序开发包构建、结构检查与57打包测试通过，新提交CI及原生范围仍开放，见[整合记录](testing/records/2026-10-06-task-audit-main-integration.md)。目录镜像独立审查发现删除前最后一次等待期间撤权仍删除本地目标的P1，三行修复经新非作者32项相关用例及严格Clippy复核通过；27路径增量已按preimage／三方合并整合。当前主树组合、新构建和原生仍开放，见[镜像整合](testing/records/2026-10-06-directory-mirror-main-integration.md)。任务审计0cca新[Quality 37481523251](https://github.com/cyruss648/keelshell/actions/runs/37481523251)已结束：Windows／macOS成功，Linux唯一目录同步最终字节断言失败，原因未知；完整三job与attempt ZIP已读回，保持12秒等待与字节断言的test-only观察已加入，见[同步诊断](testing/records/2026-10-06-directory-sync-ci-diagnostics.md)。镜像686输入主树1541普通／8doc／6Python与严格检查通过；新增两个独立组合回归后需重新验证最终源码。

外部MCP保持服务端方向。新 `7f50d60` macOS原生尝试在捕获临时配置的300秒界面确认期限结束，未启动外部CLI、模型准入或业务调用；该次不能计为目录或业务成功。此前V8仅首个会话读取成功，后续选择读取因记录器元数据校验停止；失败和运行后清理材料保留。完整Codex授权读取/批准/拒绝/撤权、各平台桌面、发布签名及安装更新继续开放，见[当前MCP记录](testing/records/2026-10-06-codex-mcp-current-attempts.md)。

递归镜像独立候选已通过38项相关组合，8项新TCP覆盖17自有listener场景；后台栈和旧审批复活的原始失败保留。最终694输入语义修复epoch完整门禁实际0，1558普通/8doc/6Python、格式/x.y/严格Clippy通过；前后源码相等、进程组无残留，仅结果文档随后更新。此694作者范围的历史事实见[作者记录](testing/records/2026-10-07-recursive-directory-mirror.md)。后续新的非作者限定复核、当前717输入主副本整合与完整工程检查已完成；新原生及精确CI仍开放，见[组合记录](testing/records/2026-10-07-recursive-mirror-main-combination.md)。它不替代原主树689基线或其原生证据，完整产品和跨平台范围继续推进。

2026-10-07 诊断分支 dd32392 的 Quality 已实际结束：macOS／Windows成功，Linux同ID保存资料审批18秒等待失败；原目录字节用例该次通过但旧失败原因保持未知。新增四阶段test-only观察在独立工作树验证，不改变生产加密、18秒批准或45秒计划，见[CI定位](testing/records/2026-10-07-saved-profile-sync-ci-diagnosis.md)。

主线 9865f1f 的 Quality 已结束：macOS 成功，Windows job 失败于测试 `Command` 导入缺少 Unix 条件，Linux 唯一定时配置同步组合用例在批准后的 18 秒状态等待失败、根因未知。Windows 独立工作树仅补齐导入条件及两个既有夹具调用的模块限定，未改生产、断言或门禁；作者格式／x.y／全工作区严格 Clippy、389 项 session 普通测试和 1 项文档测试通过，16 ignored 未执行，新的非作者复核通过。根已按完整基线导入，整合门禁与新提交级 CI 待完成，原失败日志保留，见[Windows 记录](testing/records/2026-10-07-windows-diagnostic-import.md)。独立诊断分支 0f1a577 的三平台 Quality 已成功；其观察增量尚未整合，该成功不证明主线失败原因或新修复验收。

2026-10-07 当前组合的 macOS 原生递归镜像已实际完成双向各12项，取消及上传后的独立文件核对、下载后的根字节核对及新非作者只读复核通过。两个隔离GUI与所属Linux资源结束，首轮1800秒超时和管理进程wait未知边界保留。中英文纵向审核证据不替代横向长行、最小窗口、辅助技术或其它平台原生，见[原生记录](testing/records/2026-10-07-recursive-mirror-native.md)。

2026-10-07 本地智能体工作目录候选已完成作者完整门禁、精确 Codex 0.160.0/0.160.1 与 Claude Code 2.1.285 的受控适配器验证；默认空隔离、显式绝对目录、目录身份校验、固定 bootstrap、项目 hooks/MCP/tools 禁用、发送前审核和 UI 两轴滚动均有源码及受控证据。1,257 项封包 payload 全部读回，独立复核为 `NO_BLOCKER`；已在主线 `0d64f09` 整合；当时的 Agent 工作流后续已在 a4 主线整合，外部账户/模型、真实桌面窗口、Windows/Linux 原生及新 Agent 原生验收继续开放。对外 MCP 仍只由 KeelShell 提供服务，不添加通用第三方 MCP 客户端。详见[工作目录记录](testing/records/2026-10-07-local-agent-working-directory.md)和[ADR0069](adr/0069-reviewed-local-agent-working-directories.md)。

2026-10-07 父提交 `4c9b14c` 已包含 Windows 导入修复和 MCP 文档方向修正并推送。Quality 37585913129 的 macOS／Windows job 成功，Linux 因保存资料批准后的 18 秒工作区等待失败；日志已保存，busy 状态仍不足以确定根因。Linux 失败专用观察 v2 已经独立复核为 `NO_BLOCKER` 并整合到主副本：它仅在失败时读取有界内存状态，避免复制请求正文，保持既有期限、断言和生产路径。需要新的提交级 Linux CI 来验证诊断信息，不能把本机通过或观察器本身视为根因修复。详见[Linux 观察记录](testing/records/2026-10-07-linux-sync-schedule-observation.md)。

2026-10-07 进程级本地文件测试准入候选：新的双 SSH 夹具反例已证明 existing local writer 会保守占有整个本地侧，独立 UUID 路径不隔离同进程测试。异步方案在新非作者静态审查被确定 P2 拒绝，原 91 通过及全部失败仍保留；新候选只让真实本地传输夹具在同步 setup 有界按组准入，同一测试多夹具显式继承同组，原生产隔离、四线程门禁和所有动作期限/字节断言保留。同步 v4 作者限定91项四线程回归、严格 app Clippy、格式与 x.y 已实际通过；新的非作者复核及精确整合 CI 仍待完成，历史失败具体 owner 未知，见[测试记录](testing/records/2026-10-07-local-mutation-fixture-admission.md)和[ADR0078](adr/0078-process-local-file-fixture-admission.md)。

2026-10-07 测试组同步 v4 随后被新的非作者真实反例拒绝：正常释放 Harness 时原窗口和续传 worker 仍持有本地资源，新夹具却已取得许可；目录同步实际 Busy、零写入、原字节保持。作者正在修复 test-only 生命周期，主树未导入该组；原 91 项作者／独立通过不替代 owner 终态证明，见[生命周期复核](testing/records/2026-10-07-local-mutation-fixture-lifetime-review.md)。

2026-10-07 进程级本地文件测试准入候选：受控双 SSH 已证明 existing local writer 会保守占有整个本地侧；历史 CI 具体 owner 仍未知。异步 v3 及同步 v4 在新非作者复核分别因 GPUI 守卫和真实 retained-window 寿命 P2 被拒绝，全部原通过/失败保留。v5 以实际 App 的 Weak<Group> 保留同 case 原循环与多窗口语义，真实 panel/worker/queue 持准入直至生产 queue.close 真 join；启动失败、超时及额外引用均保持关闭。六项受控寿命/失败控制与95项相关四线程已实际通过；严格 app all-targets Clippy、格式与 x.y 也已实际通过；新非作者复核和精确整合CI仍待完成。原12文件/动作期限/字节断言及生产隔离不变，见[测试记录](testing/records/2026-10-07-local-mutation-fixture-admission.md)和[ADR0078](adr/0078-process-local-file-fixture-admission.md)。

2026-10-08 自动更新策略候选已实现：默认每日延迟后台检查、可关闭／每周频率与可选自动下载、有限失败退避、人工检查优先、Ready继续查新、取消新请求保留旧包及明确弃包；长期服务与乐观保存保留SSH实体和未发送命令草稿。安装仍需明确确认并保留已有回滚逻辑。26项新增测试和最终格式／x.y／严格Clippy、updater41／workspace5通过，新非作者当前源码／专项 `NO_BLOCKER`；作者完整门禁属于取消修复前epoch，原失败反例保留。最终主树组合与原生仍待验收；签名／公证、实际安装矩阵及长阻塞解包的原生退出有界性保持开放。见[产品指南](product/AUTOMATIC_UPDATES.md)与[候选记录](testing/records/2026-10-08-automatic-update-policy.md)。

2026-10-08 自动更新／工具栏组合首轮1722普通／10doc／6Python完整检查通过，后续独立审查发现绘制入口未绑定原安装包／请求；六个旧指针行为反例实际失败，完整同case修后通过，最终9项限定回归通过。完整Release、暂存包与CancelIntent守卫已补齐；最终源码完整门禁／新非作者／新原生仍待完成，原英文小窗口P2未关闭。见[组合记录](testing/records/2026-10-08-update-toolbar-main-combination.md)。


候选封包时，2026-10-08 本地／远程浏览v5候选已完成限定源码与实际136项回归的新非作者复核；原栈失败、两项旧冒泡反例及编码夹具两次失败保留。根导入仍须自己的完整门禁和原生，不继承作者通过为主线验收。见[浏览指南](product/LOCAL_REMOTE_FILE_BROWSER.md)。


2026-10-08 本地／远程文件浏览已精确整合到根开发副本；810输入完整门禁实际0（1,769普通Rust／10rustdoc／6Python与严格工程检查，22ignored未执行），同输入新标准macOS包及57打包实际0。新宽窗口系统picker、隐藏项、目录导航、大小排序及人工审核双向SFTP完成，3×完整30字节内容相等，12原图／AX与实际资源清理保留。新的非作者已完成组合源码／工程／宽窗口原生证据复核，无 P1/P2；精确提交CI、最小原生窗口、辅助技术、其它平台及完整产品继续开放，见[整合记录](testing/records/2026-10-08-local-file-browser-main-integration.md)。
