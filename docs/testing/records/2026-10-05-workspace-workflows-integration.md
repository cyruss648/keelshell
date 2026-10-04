# 工作区、连接库与依赖任务整合 — 2026-10-05

状态：连接库、文件响应布局及依赖流程UI经整合分支合入 `main`，生产提交 `40824f0` 已推送并核对精确远端SHA，各自与冻结整合范围的独立复审通过。标准双程序macOS开发包完成受控SSH/SFTP、标签审核/保存及两任务退出0/7，但在900×580英文/Light/AI/真实Files组合发现终端约37px的新P2。紧凑工作区修复已实现，完整Workspace/Files和候选场景、根新1031普通+8文档+6脚本/57打包门禁通过；fresh紧凑独立复审与新macOS包八组合/补全原生闭环PASS，新源码Quality37232614315已结束：Linux/Windows成功，macOS的SSH测试在认证阶段超时，整次失败。先前通过结果不覆盖后续源码变化，见[紧凑原生记录](2026-10-05-compact-workspace-native.md)。

## 范围与来源

三个增量从 `3a2df35f88466c650e86056c20bf67f23f92a7f4` 独立开发：

| 增量 | 初始冻结作者提交 | 初始独立结论 |
| --- | --- | --- |
| 文件响应布局与实时 tooltip 翻译 | `85eb16a20ccc5d79d667942e197c05077a259847` | 333 项应用测试、app strict Clippy、fmt/版本策略通过；真实首行与次级操作探针发现一项 P2，需重新分配垂直空间 |
| 连接库批量组织与永久回收清理 | `eabd37f1c91041ea0ac0f452878cf0f5487ab435` | 无可复现 P1/P2；321 普通 core + 4 文档、24 连接库 GPUI 及 5 项补充探针通过 |
| 审核式依赖任务编辑与执行 | `2834122b0417368b8e32e2daf4155fbf9d1cc08f` | 27 项应用工作流、12 项 session 工作流、9 项系统 OpenSSH 通过；最小英文窗口的新入口横向越界为 P2 |

初始工作流的独立源码副本另完成 128 次真实任务输入、32 个目标选择及全部 128 终态收集，确认前没有 exec。900×580 双语/明暗四种审核保持固定操作可达。其 240 秒外层测试时限失败及后续 600 秒总时限通过都保留，产品 deadline 与完成等待断言没有放宽。该探针属于原冻结提交，不能代表作者后续修复的验收。

独立工作流审查 cutoff 为 `2026-10-04T18:57:18.517615+00:00`。438 个跟踪文件前后 SHA 一致，随后作者获准恢复其工作树，探针继续使用冻结源码副本。文件与连接库的初始审查分别核对了 432、437 个跟踪文件，均保持原 HEAD 和 clean 状态。根任务已逐项核对 26、39、31 项独立证据哈希，原始失败与 probe 源码保留。

## 初始发现及表述校正

文件区在 900×580 中只剩 28px 浏览高度时，全部高度由表头使用，真实首行位于视口外。完成上传或进入目录比较后可进一步降至 0px；比较的次级按钮也可越过底边。初始断言只检查浏览高度，未检查真实文件行，因此不足以证明可用性。

初始文件探针的 `comparison-plus-transfer` 和 `suspended-comparison-transfer` 标签不准确：探针实际完成上传并读回内容后再执行 Compare，没有手动插入卡片；生产 `FilesPanel::run` 会清退旧 transfer。准确场景为“传输完成后进入比较”和“挂起比较快照”。原始标签、报告与哈希保留，根补充生命周期校正；不再声称两张卡片同时存在。真实初始、完成传输和比较状态的布局缺陷仍成立。

工作流入口在最小英文窗口且 AI 开启时有 106px 越过命令列边界，进入后绘制的不透明 AI 栏。渲染器的按钮中心点击仍能打开流程，因此结论限于关键入口及文字的越界与绘制重叠，未声称完全无法打开。

修复分别需要真实首行、生产主题、有界滚动与固定审核按钮，以及四个命令动作的列内边界与不重叠检查。根还将复核新增工作流与连接库审核的互斥、隐藏/重开、关闭快捷键、语言/主题保存及真实 SSH 所有权。

## 新冻结修复与独立复审

| 增量 | 最终冻结提交 | 新独立复审结论与准确范围 |
| --- | --- | --- |
| 文件响应布局 | `15c00b3e10189af96997d45f71ee3caecdb87a95`，父提交 `85eb16a` | 原垂直预算 P2 关闭，无剩余可复现 P1/P2；21 项文件 GPUI/TCP 专项、fmt、x.y 与 app/all-targets strict Clippy 通过；恰好 176 个生产主题场景、11 个真实状态，最小浏览区64px、工具区50px；独立源码副本补充固定按钮位置、真实28px首行和长草稿保持探针也通过 |
| 依赖工作流入口 | `cdaa20210dcd639c93e4324e7e8a882877090004`，父提交 `2834122` | 原动作越界 P2 在渲染器几何与实际鼠标事件层面关闭，无剩余可复现 P1/P2；29 项应用工作流、fmt、x.y、全工作区/all-targets strict Clippy 通过；两种尺寸×中英×明暗×AI开关的16种布局，包含两种历史策略标签与运行中标签、真实 held SSH 隐藏/重开/取消且不重放 |
| 连接库批量 | `eabd37f1c91041ea0ac0f452878cf0f5487ab435` | 维持初始独立 PASS：321 普通 core+4文档、24 项库 GPUI 及5项补充探针；两项显式2 MiB布局探针不等于整套 GPUI 小栈测试 |

文件新复审核对433个跟踪文件前后 SHA、HEAD 和 clean 状态不变；独立 probe 只在自己的源码副本增强 `files/workspace_layout_tests.rs`，未改作者源码。场景使用生产 FilesPanel/AssistantPanel 和工作区预算，不是完整 Workspace、监控传输、原生屏幕像素或原生键盘/IME 验收。真实流程包括选择首行、读文件、审核/执行768 KiB上传、暂停/继续并读回、权限审核取消、比较/差异/保存审核与挂起草稿；比较明确清退旧传输，没有虚构同时存在的比较/传输卡片。

工作流新复审核对438个跟踪文件前后 SHA、HEAD 和 clean 状态不变，cutoff 为 `2026-10-04T19:21:45.615735+00:00`。修复的生产变化限于命令动作行约束、换行、间距和观察 ID，处理器、输入构建、执行和授权未改变；独立测试仍验证输入实体/原文/目标/revision保留、无批准前请求、两条捕获 SSH 上各一次真实请求且无 PTY 写入。公开 textarea 设置器的输入保持检查不等于原生键盘输入。旧冻结的128任务/32目标、OpenSSH、完整门禁和2 MiB控制器未被复用为新修复的独立运行结论。

文件新独立报告 `work/file-workspace-fix-review-20261005/REVIEW.md` 的 SHA-256 为 `88280426ab00b1206d19a67593f497909e1cc6cf69743214a6770a2d0787f015`，回执为 `e401b4c6ee6d8c72aa0c4e1d8896099d86a8b252479bc7169bc63a116d46e902`。工作流新独立 `work/workflow-ui-fix-review-20261005/review-report.json` 的 SHA-256 为 `6351871975bab109fad5bfabf1f7ef6bb6a57bfd2c42583ae2df0310990aa6ef`。完整日志、源码前后清单、补充 probe 与失败证据在各 ignored review 目录保留；作者门禁和构建结果仍按[文件记录](2026-10-05-files-responsive-tooltips.md)、[工作流记录](2026-10-05-dependency-workflow-ui.md)单独归属。

## 合并后的冻结范围与后续验收

三个已复审提交分别经 `3558fa8`、`6f0ed48`、`d9eeaef` 合入整合分支。当前已实现：连接库明确选区的批量组织/回收/恢复/永久清理；手工1–128任务、最多32个已认证目标、32个直接前置的完整审核与逐任务结果；文件操作换行、64px真实首行预算、有界工具滚动与固定审核按钮。任务正文与输出只保留在当前工作区，不进入普通批量摘要审计，不自动重连/重试/重放；取消只停止本地等待和后续放行，不能证明远端进程停止。

| 根整合验证项 | 已审查冻结源码的结果与后续边界 |
| --- | --- |
| 连接库/依赖流程审核互斥及捕获 SSH 保持的混合真实 TCP 回归 | 已先复现 `workflow must exclude library review`；加入 `show_workflow` 守卫后1项通过（0.45s），真实两条TCP/SSH与手动放行保持，无PTY写入 |
| 剩余静态 tooltip 调用点与合并后的共用模态/焦点守卫 | 5个静态调用点已转换为live-locale helper；剩余两处为动态完整目标/目录文字。独立实际控件探针完成静止指针 EN→ZH→EN，审核/偏好保存/关闭快捷键与捕获SSH保持通过 |
| 整仓格式、严格 Clippy、Rust/文档/脚本测试、x.y 与默认/2 MiB控制器 | 紧凑预算修复前通过：1030普通+8文档、6脚本，应用361项属于1030普通；默认/显式2 MiB完整CLI控制器各通过，RUST_MIN_STACK未设置。修复后1031普通+8文档+6脚本/57打包的独立新范围在末节记录 |
| 独立整合审查、单独 OpenSSH 与打包校验 | 冻结范围PASS，无剩余可复现P1/P2；独立126工作区、30工作流和1混合测试的集合有重叠，不相加；另2补充探针通过。根9项系统OpenSSH（4.15s）和57打包通过 |
| 标准双程序 macOS 包与真实小窗口、文件/库/工作流 SSH/SFTP | 开发包commit=null，精确256生产hash与独立冻结快照一致；实际标签审核前不写盘、确认后恰好2目标改变；两条受控SSH及SFTP、完整两任务审核、成功0/失败7及输出通过。900×580实际Files+AI发现终端约37px的新P2，后续新包八组合/补全闭环通过，另在末节与原生记录归属 |
| 新合并源码远端 Quality | 生产提交40824f0已推送，Quality37232614315已结束：Linux/Windows成功，macOS的SSH测试在认证阶段超时，整次失败；最终三平台及OpenSSH结果另行追加 |

原生 Windows/Linux、背景模态 AX 隔离、完整屏幕阅读器路径、真实系统样式变化、任务级持久化审计、定时/自定义参数映射、供应商 MCP、签名/安装更新与最终 Release 仍开放。本轮成功的 GPUI 或 TCP 证据不关闭这些边界。

## MCP 方向

本次重新只读核对了现有服务端与计划：`ServerHandler` / `RoleServer` 提供 KeelShell 的对外服务，`DesktopIpcClient` 仅用于受认证的内部桌面桥接；它不是访问其他 MCP 服务的通用客户端。固定本地 Ask 明确清空 Codex / Claude 的 MCP 配置并拒绝工具事件。当前七项对外工具只提供明确授权的读取和待审命令提案。实际供应商 MCP 客户端互通与文件修改提案仍待单独实现/验收，见[对外指南](../../product/EXTERNAL_MCP.md)。

独立方向审计检查29个指定文档/生产入口/协议边界文件，结论PASS：未发现通用第三方 MCP 客户端的产品承诺、配置或生产入口，无需方向性代码修复。读取时 HEAD 为 `3558fa8c2b7840a7b00da347636ea5fe947b6919`，根文档已有未提交改动；它只记录逐文件读取时间与哈希，没有声称整个工作区冻结。`work/mcp-direction-audit-20261005/report.md`、`checked-files.json` 与 `SHA256SUMS` 保留定向证据，根只读核对11个指定文件的回执在 `work/workspace-integration-evidence-20261005/mcp-direction-root-verification.json`；两者不是后续新源码全局冻结或供应商互通证明。审计未构建、测试、操作 GUI/客户系统或调用供应商客户端。测试里的 MCP 客户端只验证 KeelShell 自己的服务端，内置固定 Ask 与对外服务端的授权契约各自独立。

## 证据与清理

ignored `work/workspace-integration-evidence-20261005/` 保存作者日志的 SHA 核对副本、初始独立报告核对、生命周期校正及后续整合回执；原独立源码副本与失败日志各在对应 review 目录保留。

三个作者增量均已合入整合分支。连接库 managed worktree 已可恢复归档，已合本地分支以非强制方式删除；独立连接库 target 经进程检查后清理，probe 源码、失败日志及哈希清单继续保留。文件与工作流作者/复审源码、原始失败及哈希副本仍保留，两个作者managed worktree已由根可恢复归档，确认checkout移除后以非force方式删除已合分支；最终回执在 `author-cleanup.json` 保留。

修复前的标准双程序 macOS 实际窗口与 SSH/SFTP 原生证据见[原生基线](2026-10-05-native-file-workspace-baseline.md)。它证明旧问题，不能作为新布局、合并后程序、Windows/Linux 桌面或正式 Release 已通过的证据。

## 根新增混合回归

新测试 `dependency_workflow_and_library_reviews_are_exclusive_and_retain_captured_ssh`
使用两条真实TCP SSH连接和不关联它们的本地连接元数据。workflow审核已打开时，
先前 `open_library_batch` 缺少 `show_workflow` 检查而允许第二个审核层；原日志
`mixed-before-guard.log`（SHA-256 `3fe0b1fe1dcfe86f6ef280ed3717630ea59e27514f702488e5d4f399dd1ca0d2`）
保留实际失败。补上专门守卫后通过，未使用包含连接管理本身的通用阻塞函数。

通过日志 `mixed-after-guard.log` SHA-256为
`23f89c353dc50b442499ae9e1e28e8f82eaa2df7682d3f59a0a4872e30a89cb2`。
测试经真实点击审核/确认保存收藏，等待5秒以内保存完成；确认之前磁盘元数据不变，
无exec和PTY写入。关闭快捷键只隐藏原流程审核并保留标签，连接库审核阻止流程/普通批量
反向进入，元数据保存保留两条活动SSH；重新打开同一流程Entity保持完整旧审核，
再人工确认才让目标收到一次精确命令，另一条SSH无exec，两条PTY均无写入。
该GPUI/TCP回归不证明原生窗口或客户主机。

## 紧凑预算修复前的根源码门禁

`final-gate-1.json`/`.log`：完整 `scripts/check.py` 通过（350.455s），
1030项普通测试、8项文档、6项Python脚本，应用361项；11项默认忽略为两项
供应商显式选择测试和9项OpenSSH，后者另行执行。默认和显式2 MiB的完整本地CLI
控制器都通过；未设置 `RUST_MIN_STACK`。门禁包括格式、workspace/all-targets严格
Clippy和x.y策略。日志SHA-256：
`37526bec36c1eee77afb1cd50c1cb452b614b3384e269b675ff386b3053c5759`。

新独立系统OpenSSH9项通过（测试4.15s、外层43.426s），owned进程/临时目录
清理通过且 `ancestry_unverified=[]`。原始 `root-openssh/result.json` 与测试日志保留。
新57项打包回归通过（1.416s）；`final-packaging.log` SHA-256：
`353dc4b8234e7cf74b8013f857e0d8fd151c7ea3d7c662d6923fdd0b1e06e7f8`。
这两项均来自本次合并后源码，不代替新的原生桌面/Release验收。

上述门禁从HEAD `d9eeaef`及四个根Rust修正文件运行；聚合文档在同步，生产
代码/测试/配置在独立整合审查期间冻结，非全根目录冻结。实际patch摘要和命令期限
保留在每份回执中。GUI与MCP两程序的native build通过，macOS最低版本设为15.0；
构建日志SHA-256为 `95e3c9206cec27b18780807d4637b542b221af0b8fa70ad1681f5712418740f8`，
随后标准双程序开发包已完成打包、原生结构检查及有限真实交互，但暴露了新的
完整工作区终端垂直预算P2；包与回执见[原生记录](2026-10-05-compact-workspace-native.md)。
本节通过结果属于修复前的冻结范围，不代表紧凑预算修复后的最终门禁。

## 独立整合复审

新独立审查读取HEAD `d9eeaef28ca54c611086f4355273967d24a11924`及四个根Rust修正，
生产patch SHA-256为 `b39a0a97d873fec2ed849d377dd7f27126cf74484c16b7de863dc379ad3799d6`。
256个生产/测试/工程文件的根快照在审查前后完全一致；文档明确不属于冻结范围。
门禁cutoff为 `2026-10-04T19:40:01.325975+00:00`，最终报告时间为
`2026-10-04T19:45:25.767141+00:00`。独立源码副本的新增探针只修改自己的测试文件，
没有回写根源码。

独立格式、x.y策略及app/all-targets strict Clippy通过；真实混合TCP测试1项（0.46s）、
工作区126项（23.70s）、工作流30项（9.28s）通过。这三组筛选重叠，不能作为157项
唯一测试总数。另两项独立补充探针通过（1.35s），验证语言/主题保存保留完整旧审核与
标签草稿、模态输入不写PTY、关闭快捷键保留两条SSH并恢复焦点、同一流程人工确认
只向捕获目标发送一次命令，以及真实收藏/批量目标tooltip在静止指针下EN→ZH→EN。
它们是GPUI/TCP证明，不是原生键盘、OS焦点/样式或背景AX隔离验收。

初次源码副本误用外层Git导致patch未进入副本的失败，以及两次首次帧未稳定导致
hover指针被测试平台重置的失败，均保留在ignored review目录。副本初始化独立Git后，
256项源码hash才匹配；hover探针只先稳定生产首帧再悬停，未改生产代码或放宽断言。
这些修正不抹除原失败，也不证明原生hover行为。

独立报告 `work/combined-workspace-review-20261005/review-report.json` SHA-256为
`dbc195d650d02bf5c464c6fddf3a4e142963ded6e8fed88f125cafda5ac8c32b`；36份证据清单
`evidence-sha256.json` SHA-256为
`d779ef9dacae13a997c68ff33986c3001797fab119614c2a5da430b521c30f1d`。
根已逐SHA/bytes核对全部36份，原生文档整理任务又只读复核一次，未重新运行测试。
该PASS仅关闭上述冻结的整合审查范围。随后实际屏幕发现的约37px终端新P2需要独立
修复和新原生包证据，不能由本次PASS或先前文件面板176场景关闭。

## 紧凑工作区修复后的门禁（新范围）

新冻结范围为HEAD `d9eeaef`及六个根Rust变化文件，256生产文件清单在
`work/compact-workspace-native-20261005/source-freeze.json`；生产patch SHA-256为
`d0cf9487277ba8c830783957839fd4f711ad3db7b570847ee8466a7a83635b09`。
相对上一标准包只有完整工作区视图、补全视图和依赖流程测试三文件发生变化；
原根审核守卫/tooltip改动保持，session/MCP传输与打包生产文件hash不变。
文档继续独立同步，不属于这256项冻结。紧凑布局和精确输入/实体保持规则见
[ADR0046](../../adr/0046-compact-workspace-terminal-budget.md)。

作者7项依赖工作流专项、fmt/x.y/app strict Clippy通过；完整Workspace使用真实
TCP SSH/SFTP加载生产Files和监控组件，32场景包含两尺寸×双语×明暗×AI×历史策略，
终端89.5–284.5px、浏览/工具85–139px、真实首行28px、命令输入64px。
另16场景实际显示13项文件候选及满144px列表，终端100.5–403.5px，并以平台滚轮
到达最后候选；查询期间折起Files，关闭后恢复同一面板，未虚构二者同时绘制。
这些是GPUI/TCP场景，不另算48项测试或原生像素/键盘验收。原72.5px回归失败、
编译错误及duplicate module/caret诊断保留；根核验作者43份证据SHA/bytes，回执为
`compact-author-verification.json`，作者证据清单SHA-256为
`ee8a63545fa474a627fa56001c9c203e892c156142bc7142c64c83ce6185f2c7`。

根`compact-final-gate.json`通过（185.853s）：1031普通（含362项应用）、8文档、
6脚本，格式、workspace/all-targets严格Clippy、x.y和默认/显式2 MiB完整CLI控制器
均通过，controller future4624字节，`RUST_MIN_STACK`未设置。11项默认忽略分别为
2供应商显式选择与9OpenSSH。日志SHA-256为
`56471da604e2a4a0dfde2ce6092eca2cfef04c107ad9728e3a4bd056dd1ffb26`。
新57打包回归通过（外层1.463s），日志SHA-256为
`76d56a30fb77cdfd7c0a73b26a03ba8ccdba70e02a4de16d2b71925ee3235bf4`。

前一冻结范围的9项系统OpenSSH仍归属前述独立运行；因本次传输和打包源码未变，
没有重复运行，不能列作紧凑修复后新执行结果，后续新源码CI另行核验。
GUI/MCP新native build通过（4.869s），日志SHA-256为
`191d6648b0d8ad04ef71ed568830c4c43d8a3b5ed8440ea7ef61d9c1ed16b679`。
fresh紧凑独立复审已完成PASS；新标准macOS包实际900×580八组合与显式补全闭环也通过。
构建与作者/根门禁不关闭Windows/Linux桌面、OS样式/背景AX或正式发布边界，本次有限macOS原生范围见下节。

新独立紧凑复审在上述256文件冻结快照上通过：127工作区、31工作流、12补全及1项
完整工作区回归的筛选集合重叠，不能相加；格式/x.y/app全targets严格Clippy通过。
私有1项补充probe覆盖长完整目标与900×580、900×699/700/701、1440×900，共80个
正常态和40个实际候选态，验证700px布局阈值、最后候选滚轮可见、两个图标说明在
静止鼠标EN→ZH→EN即时更新，以及SSH/Input/Files实体和审核目标/revision保持。
布局切换没有创建查询，捕获终端输入通道为空；真实SFTP不等于真实网络PTY或主机监控。
首个私有probe访问sibling私有worker/ticket导致编译拒绝，随后仅改用既有busy()/visible()
观察接口，通过前后业务条件未放宽，原日志/源码保留。

独立 `work/compact-workspace-review-20261005/review-report.json` SHA-256为
`41131c7815f7be992510c66a9b878765afb9edc41c5408c54073956377c5fcf1`；39项证据清单
SHA-256为 `299d22d47aa37dc9982657f66383b312000f92ccd7545a20195e4eb173c2b65a`。
文档任务只读核验全部39项SHA/bytes。结论为冻结生产/GPUI/TCP范围无剩余可复现P1/P2，
该审查未将GPUI当原生证明；根随后用新标准包实际屏幕关闭约37px的受控macOS边界，证据见下节。

## 新标准包原生闭环与最终清理

`work/compact-workspace-native-20261005/`的新开发包保持`build.commit=null`，
GUI SHA-256为 `1bfd5e6a0b448abff6545130e3e4832efd47af7a9c77b94d125562b1252ecd84`，
MCP仍为 `069bbb239b5382f5c5203172ffd9fe080f7dc124c1fe1ab2de2e7d2a3477f53d`；
256生产文件与上述新freeze一致。实际900×580、双语×明暗×AI开关八组合通过，
终端人工从原始Retina像素估读为英文AI约89px、其他组合约121px；这是约数，
不混用GPUI89.5/100.5精确断言。实际SFTP起点读取/、7候选查询、滚轮最后项
`报告 draft.txt`、Dismiss恢复Files和`cat `草稿/目录/目标、Files directory回填/通过，
候选状态终端约100px且Files暂时折起。仅连接fixture0，监控正确报server rejected exec；
没有新文件写入/传输/工作流执行、真实主机健康、供应商AI/MCP或其他平台验收。
原37pxP2在这个受控macOS紧凑范围关闭，其余产品/平台边界仍开放。

19原始JPEG/AX、程序/source元数据、日志、controller、结果与清理逐SHA/bytes登记，
排除data凭据、stage二进制/资源和tmp；新清单52项经文档任务和根分别核验通过。
`native-result.json` SHA-256为 `96954fc8a20cd564949e2fc3f624a8d32420166eb7bcd0709c818b3710422235`，
清单为 `802827059486ab2aee71e1f7125d7a9e693651ba3fdb6065d6267521c775e43d`，
核验回执为 `4523775fa3fc0fd23ca2122a47a0b594cd053b89a159802ae2d99fdaed9545e3`；
根52项核验回执另在 `compact-native-verification.json`。新04误标minimum仍为宽图，
05仅缩高度，06才是真900×580；全部失败原图保留，细节见[原生记录](2026-10-05-compact-workspace-native.md)。

根实际核验controller/app/两fixture四PID消失、两监听消失、两临时root移除，进程均退出0。
新39项独立复审也经根逐SHA/bytes核验，回执为 `compact-review-verification.json`。
两个独立review的target/private-tmp在无owned进程后删除，source/probe/失败日志保留，
追加cache-cleanup回执不改原36/39冻结清单；新cache-cleanup SHA-256为
`a7c9ad174e19348a7d88bb7fdd064d0a786b4f05f1bf514a359e27388a436596`。
本切片本地门禁/独立审查/有限原生验证完成；生产提交40824f0已推送main，其Quality最终失败见下节，
未沿用前一提交CI，也未创建或证明新的Release/安装更新。

## 提交、远端绑定与新 Quality

生产修复提交为 `40824f00ae9385f380b36c0abecf41b043c11a4e`，已以fast-forward
合入并推送 `main`。根逐项读取提交tree的256个生产文件，与新原生
`source-build.json`完全相同；原开发包仍保留`build.commit=null`，不能重写成
正式提交构建或Release。`git ls-remote`确认远端main精确指向此提交，
当次tracked工作区clean、main/origin ahead/behind为0/0；已合整合分支
以非force方式删除。回执在`commit-push-verification.json`保留。

[Quality37232614315](https://github.com/cyruss648/keelshell/actions/runs/37232614315)
以此精确生产提交运行并已结束failure；此前提交的成功CI不覆盖此范围。
Ubuntu成功：1031普通+8文档、6脚本、57打包，默认/显式2 MiB控制器均4624字节；
普通12项ignored另含Linux /proc手工采样，独立9项OpenSSH通过13.96秒，
owned进程与临时根清理通过、ancestry_unverified为空。Windows成功：1011普通+8文档、
6脚本列出/1跳过、57打包列出/4跳过，两控制器均4968字节；OpenSSH按平台跳过。
macOS部分1018普通通过、1失败，文档和显式2 MiB未执行，OpenSSH被跳过。
失败为`directory_limit_failure_and_timeout_release_remote_handles`在
`SSH connect/authenticate`返回Timeout，未进入其目录超时/清理断言；
套件94通过/1失败，workspace返回101。原API日志183750字节，SHA-256
`320320f78ee4186c2ec08c13b13278383a852caa017f3c39b4cbb9fe4ddf7df6`。
根核验旧run49项SHA/bytes；其清单SHA-256为
`873ae54625acddbc4c2bf79ab2be89f221cb02a5610324c95fe84ab1de8470a6`，
报告为`ebc12b8feda15877a052b9585226843c997a1c35b0506d3d6e2f435ede32ca50`。
原失败不重跑、不覆盖，测试夹具修复和新提交CI另行记录。此处后续文档提交独立于生产源码
提交，不能把开发包或CI视为签名、安装、更新或Windows/Linux GUI验收。

后续两文件SSH夹具修复完成最终作者95项/12专项、根整仓工程门禁、9项本机OpenSSH及GUI/MCP构建。新独立复审无剩余P1/P2；修复提交CI继续按[夹具记录](2026-10-05-ssh-timeout-fixture-stability.md)归属；原408失败与各次原生包快照保留，不用旧候选或中断gate替代最终结果。
