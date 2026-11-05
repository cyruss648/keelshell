# KeelShell 当前交接 — 2026-11-06

本入口供新的 AI 会话接续，时间使用 Asia/Shanghai。先读 [AGENTS.md](../AGENTS.md)，
再按下面的切片记录核实源码、门禁、原生、CI 和交付状态。旧的详细交接已移到
[组合前历史快照](history/2026-11-05-handoff-before-workspace-integration.md)，
其中“当前”、暂停和清理指令只代表历史时点，不构成新的操作授权。

## 当前开发：面板外观与远端诊断

启动恢复切片已提交并正常快进推送main：
`56d5440e4fa260f53ee6ea404fee9223f41d2188`。本地main、origin/main及远端已读回一致，
34路径与r6文档冻结、520工程输入及构建逐摘要匹配；533个提交blob已核对。
本轮所有源码/原件/最终文档限定审查均无开放P0/P1/P2/P3。
提交/推送/绑定原件见`work/input-and-layout-readiness-20261105/`下
`commit-r1.json`、`delivery-r1.json`、`delivery-exec-readback-r1.json`及`committed-inputs-r1.json`。
精确Quality `42063264237 / attempt 1`已三平台success，108项候选各平台通过。
ordinary为macOS2221/23ignored、Linux2216/24ignored、Windows2156/18ignored，
各14rustdoc；Linux/macOS各11项OpenSSH及附件API摘要通过。
唯一观察者Parfit于16:47:34 UTC在原5400秒期限内结束；
exec93298已实际wait、exit0/reaped，84子进程全部回收。三次GET失败原件保留。
`current-ci/final-readback.json`、`exec-readback.json`及`closeout-verification.json`
只签56d5440，不代签本轮dirty源码或新依赖。

当前复用本工作树，分支`feature/pane-surface-polish-20261106`。
`pane_view.rs`已作8增3删的语义画布修整，未连接面板采用主题画布并移除终端内边距，
沿用三主题及原状态组件。首次候选只完成单文件rustfmt和diff检查；
后续组合门禁与新包原生见下，当前提交/交付仍另记。
记录见`work/pane-surface-polish-20261106/applied-r1.json`。
远端审核式ICMP诊断由Cicero在原组合工作树的
`feature/reviewed-icmp-diagnostics-20261105`独立实现，基线dcf7438；
作者已在16:22:23 UTC释放共享Cargo槽位。父按冻结patch逐摘要导入13路径，
已补独立的Workspace活动选择UUID与同步撤权；preview/confirm/结果采用均校验，
不改变Agent共享的session/route语义。真实生产ABA、迟到结果与非活动关闭正控已验证。
协议专项15passed，exec29548 exit0/reaped，525工程输入一致。
来源非作者发现两项P2后，父补ICMP任何错误先停权及六种受控SSH终态负控，
作者另交6路径计数语义修订，已逐摘要导入；原failed审查保持。
原两项ICMP P2已由icmp-r2限定复核批准，开放P0/P1/P2/P3为零。
full-gate-r1因父测试expect_used停止、exec99904 exit1/reaped；
full-gate-r2因remote_only守卫误判新增fake-process夹具停止、
exec17319 exit1/reaped，两个回执525输入分别保持，不回写为通过。
首部inner cfg(test)及守卫LF/CRLF正反例修订已有5项passed。
选择增量的新非作者另指出终端指针focus到绘制后才刷新的一项P2：
父已在正文wrapper捕获阶段同步选择/撤权，2项无中间render真实指针回归passed；
移除handler的同测在预期active0vs1失败、cargo101，源已精确恢复，
负控外层exec50283 exit0/reaped只签预期失败与恢复，不是测试绿灯。
app r3与guard两路径已冻结，full-gate-r3 / exec81606已退出0/reaped：
2236ordinary/23ignored、app970/2ignored、14rustdoc、Python131run/1skip及附加控制器通过。
但selection-r3仍保留原P2的搜索遮挡与键盘子项，不能用旧r3门禁关闭新hook。
父已补TerminalView同步输入所有者、搜索条自身capture和完成Tab动作观察；
只按EntityId更新Workspace，不反读已借用的Terminal/InputState、不移动搜索焦点。
Workspace已有选择的命令聚焦改用raw FocusHandle避免父lease重入，
搜索Input移除祖先重复track_focus，修复真实Tab顺序自循环。
新search-r1编译失败、r2两绿一红保持；r3三项真实搜索专项passed，
exec48788 exit0/reaped，525输入一致。同实体搜索保留审核/焦点；
停用通知的同测实际Cargo101/active0vs1，外层exec85227 exit0只签预期负控和精确恢复。
11个app路径另冻结在review-selection-r4，后续按钮入口复核见下。
新full-gate-r4于17:43:55–17:53:01UTC通过，exec35977已实际wait、exit0/reaped，
525工程输入保持，与build-r1及当时源码逐项相同：
2239ordinary/23ignored、app973/2ignored、14rustdoc、Python131run/1skip，
格式/严格Clippy/依赖策略及附加控制器通过。组合源码/正式文档另作独立复核。
最新官方依赖检查首次发现zip9.4/rmcp3.9，声明保持x.y并更新lock；
dependency-freshness-r2于16:56:19–16:56:34 UTC通过104条，
exec96051 exit0/reaped；后续源修订未改manifest/lock。
最终dependency-freshness-r3于17:51:16–17:51:32UTC再次通过104条，
exec41777已实际wait、exit0/reaped，525输入保持，无额外版本变动。
组合结果另记。共享target由父顺序使用，jobs2、四测试线程。

后续selection-r4及combined-r1保留同一SEL-P2-01：
搜索按钮自身的Tab stop和空格导航可绕过Input-only焦点识别。
父补独立非Tab-stop搜索父焦点范围及导航动作同步所有者发布，
只改变terminal.rs与monitor_tests/protocol_selection.rs；
review-selection-r5及review-combined-r2已冻结，其他20个组合工程路径不变。
search-r4六项passed，exec52073退出0/reaped；两个独立负控各自实际
Cargo101/active0vs1，exec32612退出0只签预期失败与逐字节恢复。
新的非作者selection-r5限定批准关闭SEL-P2-01，开放P0/P1/P2/P3=0。
build-r2于18:19:44–18:19:50 UTC退出0/reaped，exec78221已实际wait，
新包a5261fab…的五成员/41assets已核对，未原生运行/安装/签名/发布。
full-gate-r5于18:20:25–18:29:38 UTC退出0/reaped，父exec80111已实际wait，
525输入保持并与build-r2/当前源码相同：2242ordinary/23ignored、
app976/2ignored、14rustdoc、Python131run（130pass/1skip）及全部工程检查通过。
新的非作者combined-r2批准冻结产品源码/完整门禁/包归属，开放P0/P1/P2/P3=0，
不签正式文档、真实CLI准备或原生。freshness-r4于18:21:23–18:21:39 UTC重新
通过104条要求，exec21640退出0/reaped，无额外版本变化。

新build-r1/exec12666已退出0/reaped，525输入与r4门禁一致；
macOS开发包程序f9b5ac99e109c16de69e497dfa32f3fb47056ff0ea9e1e456764900e251a51b3，
5成员/41assets核对，未安装/签名/发布。
一次离线native run5a650e67ced3483c9cb71122d65756a1完成36对AX/JPEG：
24画布覆盖三家族×明暗×双语的未保存/缺失态，另12设置原件。
合成available未连接态始终保留，不点Connect、不签网络零增量或真实ICMP。
app99109正常UI退出0/reaped，WAITING_RESTART后stop而不restart；
父exec89824已实际wait、exit0/reaped，STOPPED/launches1，原600秒内清理private及已观察进程。
原生原件新非作者surfaces-native-r1限定批准，开放P0/P1/P2/P3=0；
Hallmark只对所验macOS画布映射为ship，仍不签紧凑/输入/VoiceOver/Windows/Linux或产品full finish。
原桌面另有旧组合开发包PID89730，本轮按新包完整路径操作且合成seed匹配，未清理该已有进程。

Finder拖放的新宿主回环准备已冻结，容器路径因无离线SFTP镜像保持HOLD。
唯一实际preflight `1b9beeac25b445039161db570e320414`前三步骤通过，
沙箱内`sftp-requests`以-6退出，具体原因UNKNOWN；canary未执行，没有启动服务或应用。
四子进程均已回收，私有目录删除，0.544秒内清理。没有重试失败准入或放宽沙箱；
作者只在独立诊断目录准备有界stderr采集，新非作者仍复核原冻结准备。
证据见`work/reviewed-file-drop-20261105/preflight-r1-verification.json`。
父已快审并只运行一次相同sandbox/sftp查询的有界stderr诊断，
run`db1ef92b8ca34691b8f8f7b739a1b0ef`仍-6、stderr完整EOF但零字节；
0.146秒内唯一子进程回收/后查不存在，外层同步exit1/reaped。
没有服务或应用启动，原因仍UNKNOWN。原准备独立复核另有响应日志识别、
最后port检查后的TTL两项P2，已由作者在新r2修订，33项离线检查通过；
原非作者preparation-r2限定批准、开放发现零，runtime/native仍HOLD。
精确PID的dyld crash原件已只读匹配，没有具体deny/路径说明，根因继续UNKNOWN。
不改写冻结失败原件。ICMP真实容器夹具另在作者树real-preparation-r2
有界准备官方Ubuntu候选；r2在原期限内HOLD且未启动build/apt，失败保持。
独立首次build的real-preparation-r3于01:30:10–01:30:25（Asia/Shanghai）退出0/reaped，
原240秒预算未延长，仅一build，13.001秒。image35b28f15…的owner为a608bbee…，
无网络查询实际完整ping输出匹配iputils20240117，Python3.12.3；
自有查询容器/context已清理，专用image给后续真实联调准备保留，共享base保持。
尚未启动服务/实际Echo，不允许放宽版本契约或混用本轮离线原生包验收。
双栈CLI回环准备r1仍未执行，非作者指出fixture目录umask权限及回执失败跳过清理
两项P2，作者正在ignored work准备最小r2；不阻断已批准的产品源码，
不得代签真实Rust/UI路径，固定300秒含30秒清理预算保持。
启动恢复作者树工程消费者已释放且32份证据保存，但管理器以primary checkout
保护拒绝归档，没有用shell绕过。新整合入口见
[诊断与外观记录](testing/records/2026-11-06-reviewed-diagnostics-integration.md)。
旧普通输入/IME、完整主题尺寸、Windows/Linux桌面、多窗口及完整产品验收继续开放。

## 已交付：启动恢复与输入补查 — 2026-11-05

本轮开始前，2026-11-05的上一轮组合已通过普通快进推送到main，起点主线为
`dcf7438a13fff737997ee5e8061044c94d4ba53a`，随后远端与主工作副本均已读回。
该提交只跟进8份文档，515项工程输入与e331372相同；r7文档增量的非作者批准
见组合证据中的`independent-review/final-docs-r1.md`。
dcf7438的精确Quality `42032294580 / attempt 1`已三平台success：
ordinary为macOS2,191/23ignored、Linux2,186/24ignored、Windows2,126/18ignored，
各14rustdoc、74项候选通过；Linux/macOS各11项OpenSSH与附件摘要核验通过。
唯一观察器exec16293已退出0并回收，85个子进程全部回收，
13:39:24 UTC结束，未延长原期限。原件见组合证据的`main-ci/final-readback.json`、
`exec-readback.json`与`closeout-verification.json`，只签dcf7438及其范围。

当前复用原Ask诊断工作树，分支为`feature/input-and-layout-readiness-20261105`，
基线dcf7438。文件布局测试将补真实WRITE阻塞和暂停转交的同步条件，保留原断言与预算；
首次四组专项共9项通过，执行会话54030退出0并回收，517项输入保持；
父重算176个唯一布局和16次传输回执。新非作者source-r1批准限定源码与专项，
开放P0/P1/P2/P3为0，两处源码可证的测试时序前提缺口在此限定修订内关闭；
旧Linux失败的确切根因仍UNKNOWN，最终组合完整门禁结果见下。
localhost夹具取消preflight误报成功的P2经同测试修前失败/修后25项通过及r2独立复核关闭；
真实preflight已通过。旧515输入的33be2e68包完成run ec01db306f2d43158d6d8984becf0b83：
22对AX/JPEG，可见x到独立Return才增加1字节，新Return/Right为1/3；
Unicode typeText无可见结果/PTY0，原因UNKNOWN。
限定树Home/Down/End与激活均实际改变选择且PTY/exec零增量，前后Right正控成功。
累计PTY12、拒绝exec24/9,268字节，不能代签新恢复源码或完整输入/IME。
应用及前台exec53146均退出0并回收，私有目录删除/端口拒绝，
548.661秒内清理，原900秒预算保持；新的非作者`input-native-r1`批准限定证据，
P0/P1/P2=0，仍为PARTIAL_OBSERVED。
单窗启动恢复从独立分支的18个冻结路径整合，17个逐字节相同，
terminal仅合并本轮测试注册；61个不同专项最终通过，保留两次首次失败。
恢复分屏拓扑/比例与未连接标签，显式连接才进入当前认证流程；
不恢复OS外框，不创建多个OS窗口。新的非作者`startup-source-r1`批准冻结源码与专项，
P0/P1/P2/P3=0；新的`final-combined-r1`批准21个改动Rust文件与520项输入的组合
源码/门禁/包归属，开放P0/P1/P2/P3=0；正式文档以`final-docs-r2`限定复核为准。
最终520输入的full-gate-r1因三项test-only Clippy失败退出1并回收；
父最小修订后full-gate-r2于14:01:24–14:12:28 UTC通过，
exec88486已实际wait，退出0并回收；520输入前后一致。2,221 ordinary/23ignored、
app963/2ignored、14rustdoc、Python131run/1skip，格式、严格Clippy、依赖策略及
附加控制器均通过。共享target、2jobs、四测试线程和原1500秒预算保持。
build-r1于15:12:27–15:12:57 UTC退出0，exec47106已实际wait/reap；
构建、门禁及当前520输入一致。新macOS开发包程序SHA为
`99cd7f6955735b18686df30626d68688c8de1f2fb3b1ceba2ae6329f3b49b9ae`，
包结构/五个成员/41assets已核对，未安装、签名或发布。
新包run`60c040c7da1a4cadbab99a1a2d78f520`完成同一隔离目录的两次原生启动：
六对AX/JPEG记录初始62%、选择缺失态、调整67%、切中文、显式保存与重启保持。
按合成seed与实际操作重建的完整布局唯一匹配退出后的摘要，UUID/引用/顺序保持；
未选窗口和窗口顺序保持，两次raw state摘要相同。原state只留摘要，已随private删除。
两次应用与exec22504均退出0/reaped，原600秒TTL内完成清理，无本轮活进程。
本次未点击Connect，没有网络审计，不代签真实认证、完整外观/输入或多窗口。
新的`restore-native-r1`为APPROVE_SCOPED_RESTORE_EVIDENCE / PARTIAL_OBSERVED，
开放P0/P1/P2/P3=0，不授予完整native/finish；
冻结及收尾见`native/restore-verification-r1.json`。
本轮官方依赖检查于13:09:23–13:09:39 UTC通过104条要求，无版本变动；
证据位于`work/input-and-layout-readiness-20261105/`。

## 已集成：工作区组合

组合工作副本复用原分屏工作树，分支为 `feature/workspace-integration-20261105`。
组合源码提交 `e3313722b20b0c3f5461ea8a4f194fec51c0adea` 已推送本分支。
它组合以下已审查提交和冻结导航源码；原515输入的本机组合门禁与非作者源码/证据审查通过，
后续测试诊断有独立批准和新的完整门禁，53对原生捕获获限定证据批准，状态仍PARTIAL_OBSERVED。
精确Quality `42025002190 / attempt 1` 已三平台success；
后续文档提交保持515项工程输入不变，CI结果只签e331372，不扩大到其他提交或原生验收：

| 切片 | 精确输入与已验证范围 | 当前边界 |
| --- | --- | --- |
| 原生分屏 | `9db9cbc1dcfd22c05a2e8a5dfb183877c8ff9f65`，父为 `77aa69c`。完整门禁 2,158 ordinary / 23 ignored、11 rustdoc、Python 128 run / 1 skipped；503 输入相同。限定 macOS 原生及新的非作者组合复核通过。 | 已推 feature；Quality `42009970970 / attempt 1` 为 failure：macOS、Windows success，Linux唯一文件布局状态等待失败，根因UNKNOWN；52项分屏测试三平台通过。独立提交绑定批准，两处状态同步P3已在组合中纠正并限定复核；完整原生矩阵未完成。 |
| Ask 诊断与 CI 容量 | `3d8476716ab80f0cec8b8c731a8cde0bcd7601a3`、`72a19ea5b7d47402c79d0ced73d442570c7ae9fd`。容量修订完整门禁 2,126 ordinary / 23 ignored、14 rustdoc、Python 131 run / 1 skipped；506 输入相同。新的非作者源码、组合和不可变提交绑定通过。 | 已推 feature；Quality `42009720436 / attempt 1` 已三平台success。这是新容量修订提交的通过，不回写旧失败，也不解释B的新文件布局失败。 |

源码自动组合没有冲突；冲突仅在 HANDOFF、ROADMAP、CAPABILITIES 的状态段落。
随后以三方patch接入导航的10个冻结源码文件，也没有冲突；7个文件保持导航原字节，
workspace、modal_scope、view三个文件组合了分屏与导航变化，必须重新审查交互。
本分支完整门禁 `full-gate-r1` 已**退出0并回收**，父执行会话 **90038**。
**2,191 ordinary Rust / 23 ignored、14 rustdoc、Python 131 run / 1 skipped**，
app为933/2 ignored；**515工程输入前后一致**。格式、严格Clippy、依赖策略及
附加进程控制器通过；共享target、2 jobs和1500秒预算未改变。
新的官方依赖检查退出0并回收。新的非作者组合复核为
`APPROVE_SCOPED_COMBINED_SOURCE_AND_EVIDENCE`，P0/P1/P2=0；
一项非阻断P3涉及两处仍写C原件待审的状态文字，已由document-followup-r1限定复核关闭。
完整门禁与新的非作者组合复核不继承来源分支结论。
合入前核对的main为 `77aa69c04ad0dd625a5d827552335878de993ee7`；
主线交付与后续文档提交以Git和精确推送回执为准，不以此前CI代签后续SHA。
新的macOS arm64组合开发包已构建，程序SHA-256为
`33be2e685a280b6bfd2094d4707a67c12b8a14ccac6e2b92d34f64a877154831`；
515项冻结输入一致，41assets另行核对；已执行下面的原生子集、未安装，
不能替代来源包或完整平台验收。
组合材料放在 `work/workspace-integration-20261105/`，正式记录从
[组合记录](testing/records/2026-11-05-workspace-integration.md)继续。

## 新组合包的原生子集与资源

run `3f7689e959484901bc4365d98819d8ad` 已结束，原始AX/JPEG共53对，均2880×1866。
最初普通`x`按键可见但PTY0，首次Right0，Escape后Right1而非预期3；
随后两账号粘贴与fresh Right及导航后对照均独立成功。普通字符路径与恢复根因仍UNKNOWN，
不能以粘贴代签，也不把现象归咎IME。
导航/隐藏q/两主题切换/语言段PTY与exec均零增量，但该段End/Down没有移动；
单独Escape后方向、Home/End、搜索Tab导航有实际移动及独立零增量。
分屏建立第三条shell并暂停新监控；点击分隔条+Right未改变比例且PTY增加3，
Tab聚焦后Right由50%变55%且该段为0。合并保留三shell，保存仅有UI确认。
最终累计PTY24字节、32exec/13,440字节，命令与Ask草稿保持、无命令执行或模型发送。

应用由UI退出且exit0/reaped；父wait70792得到final，执行会话已回收、原监督器PID不存在，
外层数值退出码未展示，不代填。11:40:00 UTC清理容器、网络、私有目录，
11:40:39 UTC清理专用镜像，共享base保留。没有仍活的本轮测试应用/容器。
新非作者`native-r1`已核对53对原件、28份原始审计、22个区间和相关源码，
结论APPROVE_SCOPED_NATIVE_EVIDENCE / PARTIAL_OBSERVED，无可确证源码P1/P2，
不授予完整native或finish。见组合记录和
`work/workspace-integration-20261105/native/verification-r1.json`、`parent-close-readback-r1.json`。
原生结束后新增的文件布局测试诊断通过full-gate-r2：2,191 ordinary/23ignored、
14rustdoc、Python131run/1skip，app933/2ignored，修订515输入前后一致；
exec36492退出0/reaped。两项exact专项各1passed，布局176/176组合。
官方依赖freshness-r2在11:59:34–11:59:51 UTC通过104条要求，无版本变动。
原包证据保持r1归属；该来源时点的两处测试时序前提缺口尚未关闭，
本轮已由真实WRITE/暂停交接修订及source-r1限定复核关闭。旧Linux失败确切根因仍UNKNOWN。
测试诊断的`diagnostic-r1`已APPROVE_SCOPED_DIAGNOSTIC_SOURCE_AND_EVIDENCE，
限定补丁/两次exact范围内开放P0/P1/P2/P3=0；已绑定e331372中的测试摘要。
两份报告都不自动签后续更新的最终文档，状态增量另作限定复核。

普通字符输入补查只完成准备，**没有启动第二次原生run**。r2开发包构建/打包均
exit0/reaped，修订后的515输入与full-gate-r2相同，五个包成员与r1逐字节相同；
这不增加原生验收范围。12:25:49–12:28:49 UTC的专用镜像构建在Containerfile内部
`timeout 180s`包下载/安装步骤退出124，外层未超时，exec66413已exit1/reaped。
构建context已删除，本次owner下没有镜像或容器；没有创建run、应用进程或私有凭据目录，
共享base和其他项目资源保留。见组合记录的“输入补查准备”及
`native/input-followup-preparation-r2.json`、`native/package-readback-r2.json`。
普通字符、文本提交和回车补查仍NOT_RUN，不直接重试同一下载或延长预算。

## 导航原始候选与原生边界

`feature/navigation-keyboard-20261105` 的源码与 r2a 开发包仍独立，基线为 `967c588`。
修订完整门禁 2,075 ordinary / 23 ignored、10 rustdoc、Python 128 run / 1 skipped，
499 输入一致；非作者源码与旧证据准确性复核通过。源码已按冻结摘要导入当前组合，
原候选工作树未修改；新原生证据和开发包仍只属于原499输入。

解锁后同一程序 `1f180f13…` 在 run `5c76b54db2de4dd89cff9793187208c6` 取得54对原件。
早期中文树导航、折叠、搜索、已有会话切换和草稿保持有可观察结果；后半段方向键、
Home/End等没有预期导航变化，最后两个终端正控也没有确认服务端输入，重绑定也未恢复，原因 UNKNOWN。
搜索文本输入和鼠标/AX 点击仍可改变界面。不能据此归咎语言或直接认定产品缺陷。
两条真实 SSH 会话在09:49:24–10:02:47 UTC审计窗口内保持身份，raw PTY为0、
新增exec为0；初始化已有22次拒绝/9,240字节另计。正控未成功，完整键盘输入隔离仍OPEN。
原件中29/30仍是B，31选A/32激活才切A；37/39/40没有选到文件名声称的空根；
38缩窗未生效。文件名不是验收结论。

应用和监督器均退出0并回收，10:07:08 UTC清理自有容器、网络、私有目录；
10:07:48 UTC清理专用镜像，共享base保留。新的非作者已逐项复核54对原件、源码路径、499工程输入、包及12份冻结文档，
未发现新增可确证源码P1/P2；只认可PARTIAL_OBSERVED，不授予完整native/finish。
独立记录与设计合同已接入组合，原证据保持。组合源码审查和原生验收另行进行。
接续该树的 `work/navigation-keyboard-20261105/native/r2-parent-close-readback.json`
及 `independent-review/`；旧失败和旧包证据保持，禁止混用分屏包补记导航验收。

## 已审查切片的必要边界

- 分屏：修后程序 `5ce34862…` 的 run `acceb50f…` 有35对2880×1866原始AX/JPEG，
  四个真实Shell、两轴拖动、三个分隔条键盘、工具选择保留、聚焦/合并/关闭和保存读回。
  原四个会话合并后保持，只显式关闭第四个才剩原三个；最后独立输入正控增加1字节，
  监控exec另计。缩窗未生效；后期草稿保持包含人工Tab加入的两个尾空格。
  四叶保存比例6347/5718/5000，单叶保存活动标签为原遍历第二项，后续正控后未再保存。
  新的 `combined-r5` 为 APPROVE_SCOPED_COMBINED，开放P0/P1/P2/P3=0，
  Hallmark finish只签所检子集。详见[分屏记录](testing/records/2026-11-05-native-pane-layout.md)、
  [布局合同](product/WORKSPACE_LAYOUT.md)及[ADR 0101](adr/0101-native-session-pane-ownership.md)。
- CI容量：串行夹具显式关闭，下一夹具检查前一目录已删除；非workspace依赖dev debug=0，
  缓存新分代，检查命令前后记录数值容量。原断言、期限与四线程入口保持。
  旧诊断提交Quality `41994734898`的Linux实际ENOSPC不解释更早`41967541069`的UNKNOWN。
  Windows限定重跑成功，attempt2的Linux/macOS为继承结果，整体仍failure。
  本机两种夹具程序10,078,320/9,921,664字节不是远端或完整target节省量。
  见[容量记录](testing/records/2026-11-05-ci-storage-capacity.md)、
  [诊断记录](testing/records/2026-11-05-local-ask-scratch-diagnostics.md)及
  [ADR 0103](adr/0103-ci-storage-capacity.md)。

## 用户要求与工程约束

- 只管理远程SSH；不接入本地终端、远程桌面或串口。保持真实原生GPUI Kit应用和
  macOS、Windows、Linux目标；编译、受控GPUI与仿真不能替代各平台桌面验收。
- 霜白、曜石、雾紫全部内置，主题通过注册表和语义token扩展；业务组件不按主题ID分支。
  使用既有Hallmark原生映射，见[设计合同](design/HALLMARK_NATIVE.md)。
- UI/core/session/AI分层；阻塞I/O离开UI线程。公共库API有rustdoc，解释不明显约束，
  使用类型化错误和有意义的测试。保持中英文README同步。
- AI上下文发送必须显式审核；生成的命令另行审核执行。主机密钥变化fail closed；
  凭据默认临时，显式保存仅进系统凭据库或加密vault，不能写入普通配置或日志。
- registry依赖要求仅`x.y`，保留Cargo.lock patch，不pin Rust工具链；每轮查官方稳定版本。
  最近容量切片freshness为2026-11-05 17:01:46–17:02:04，104条通过；组合另留新回执。
- 用户已授权公开GitHub、提交推送、标签触发多平台Release和本地Podman真实联调；
  不涵盖遥测上传或覆盖安装。仓库中不写具名专有SSH产品比较。
- 每个功能完成后新开非作者评审。先完成已授权工作，不反复请求确认。
  源码门禁、开发包、原生子集、CI、公开发行与安装分别验收，失败证据不得改写。

## 下一步与资源

1. A/B/C原515组合门禁与非作者源码/证据复核通过，文字P3限定复核已关闭；
   C与新组合原件均仅PARTIAL_OBSERVED，完整输入隔离仍OPEN；两份新限定复核已完成。
2. A的精确CI三平台success，B精确CI因Linux文件布局条件等待失败；失败与原件保留。
   B observer已退出0并回收。文件布局已定位测试前提与诊断缺口，确切根因UNKNOWN；
   阶段诊断、限定验证、新完整门禁及诊断独立复核已通过。
   精确组合CI已三平台success，74项候选三平台通过；该次绿灯本身不关闭旧失败根因。
   两处时序前提缺口已在本轮源码修订与source-r1限定复核中关闭，确切历史根因仍UNKNOWN。
   文档增量限定复核、主线交付和后续精确SHA的CI应分别记录，不扩大源码/原生批准。
3. 单窗启动恢复已有本机完整门禁、限定新包重启与非作者源码/原件批准，
   正式文档、不可变提交与精确新CI分别跟进；
   并行准备真实Finder文件/目录拖放的审核、取消与确认读回。继续紧凑OS窗口、
   完整主题×明暗×语言×尺寸、IME/VoiceOver、Windows/Linux桌面、多窗口、
   真实供应商/CLI和审核目标的完整原生矩阵。
   完整产品目标仍active，具体能力缺口见[路线图](ROADMAP.md)及[能力清单](product/CAPABILITIES.md)。
4. 仅在功能证据与提交保存后归档无消费者的受管worktree，清理自有分支、进程、容器、网络、镜像。
   管理器曾拒绝仍被使用和主工作副本的归档，不能用shell绕过；不删共享target/base image。

当前没有本轮KeelShell或SSH测试容器在等待用户。A的CI observer执行会话34765已退出0并回收，
75个子进程全部回收，macOS artifact补取会话1068也退出0并回收；精确原件188文件已逐摘要复制到组合的source-ci-a目录。
B的observer执行会话48913已退出0并回收，全部启动子进程已回收；两者只读GitHub并有期限。B原收集器6419因错误沿用main筛选已停止并回收，
空查询页保留；修订收集器仍绑定同一run且不延长原观察总期限。资源状态以终态回执为准。
本地`work/`证据被Git忽略，分别位于对应切片的工作副本；公开文档中的路径不代表已发布附件。

组合CI观察器exec79169 / PID29803于20:14:56（Asia/Shanghai）启动，
20:40:44取得终态，在原21:44:56期限内结束收集；75个子进程均已回收。
外层exec79169已实际wait/reap且exit0，后续检查原PID及75个子进程PID均不存在；
回执为`current-ci/exec-readback.json`，未以子进程状态代填外层退出。
它只观察e331372的Quality42025002190，没有重跑或取消CI。
三平台ordinary分别为macOS2,191/23ignored、Linux2,186/24ignored、
Windows2,126/18ignored；各14rustdoc、74项候选通过。
Linux/macOS各11项OpenSSH通过，两份七文件附件匹配API摘要并有资源清理回执；
Windows按工作流跳过OpenSSH，不代表Windows桌面验收。
精确日志、附件与回执在`work/workspace-integration-20261105/current-ci/`。
成功测试未公开内部phase日志，因此旧Linux失败耗时和根因仍UNKNOWN。
依赖工作树归档被管理器以“Another active task still uses this worktree.”拒绝，
没有绕过保护或删除该工作树/分支；当前组合工作树继续使用。
