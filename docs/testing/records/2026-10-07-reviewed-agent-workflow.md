# 审核式 Agent 作者记录 — 2026-10-07

后续当前状态：epoch3已通过新的非作者限定复核并与MCP生命周期修复精确导入主副本；组合源码审查、根1677普通/10doc/6Python完整门禁和新macOS包／57打包实际通过。精确提交CI与新原生仍需各自证据，见[主树整合记录](2026-10-07-reviewed-ai-mcp-main-integration.md)。下方“未提交/未推送/未整合”保留作者冻结时点，不代表后续当前状态。

状态：作者epoch3同步失效冻结候选，基线 `3873dacb9fe6608a9aec40136bb1d82200da9a51`。旧freeze-v3权威退休及epoch2停止传播均经fresh非作者复核发现已复现P1，阻塞整合；epoch3限定回归、精确新输入的完整门禁与macOS开发包检查均实际0，新一轮非作者复核待完成。未提交、未推送、未主线整合。原生窗口、实际供应商 CLI 新工作流及 Windows/Linux 仍未验收。MCP 始终为 KeelShell 对外提供服务，没有增加第三方客户端。

## 行为与证据

领域包含真正的有限推理回合、严格单动作决策、人工审批、桌面结果、新上下文审核以及明确终止。实际后台路径支持精确 SSH 命令、有界 canonical 常规文件读取和先读原文再批准的已有文件原子替换。目标绑定实际 SSH 句柄并复核路线／信任，不以标签重新查找替代连接。用户停止与未知结果不自动重试，迟到结果按 run/action 身份丢弃。

全部命令使用所属 worktree 的独立 target、私有 `TMPDIR` 和 `CARGO_BUILD_JOBS=1`；完整退出码、前后输入映射、输出 SHA-256、进程组与临时目录记录在忽略目录 `work/reviewed-agent-20261007/`。初轮 app-check 实际101，暴露作者借用和测试语法／辅助 trait 失误，原日志与收据保留；第二轮实际101修正遗漏的一处 profile 引用，原日志保留。第三轮 app 全 targets check 实际0、输入相同、进程组结束和空临时目录删除。早期领域过滤共43测试通过，其中8项新 Agent 领域测试，包含原有 local_agent 过滤；不是完整工程门禁。

新增8个受控 GPUI 闭环已通过，含真实 HTTP 三协议两轮、TCP SSH 人工批准／拒绝／非零退出结果、SFTP 常规文件读取及先审原文再批准替换与独立完整读回、停止已发起 hold 命令为 unknown、原 SSH 句柄替换拒绝、篡改动作拒绝、畸形决策及到期审批无执行，以及缺原文的程序化写入拒绝后独立下一轮继续。中英三主题的900×580控制窗口确认固定批准／拒绝／停止可见，独立横纵 wheel 均有真实负 offset；这一结果属于受控 GPUI，不是原生像素／AX。

两次最初闭环夹具因空 profile.name 被正常准入拒绝；随后两轴证据加强的等量双轴 wheel x=0，查 GPUI 源码确认默认轴锁会选纵轴，不将这次失败当作已证实产品布局缺陷。后续以真实字体宽度设置自然内容宽度，并分别发送横向与纵向 wheel 验证；所有失败保留。首次自然宽度改动有一处 helper 签名遗漏导致101，也保留。严格Clippy初次发现作者未使用的本地化helper和可折叠if，限定修正后重新检查。受控 TCP peer 记录或提供协议回复，不运行真实 shell 命令，不代表客户机器、供应商云端或原生平台验收。

## 第一完整门禁与后续防御修正

`full-gate-v1` 在尚为7项闭环的精确候选上运行 `python3 scripts/check.py`，真实退出0，651.245秒。x.y依赖策略、6项scripts测试、fmt、workspace/all-targets严格Clippy、1627项普通Rust测试、8项doc-test，以及默认和显式2 MiB控制器均通过；22项既有忽略测试未被记为验收。完整日志SHA-256为 `ce60f00864a757cf50ed9f3b6ab71e439a6db2928c049b484c0ba67fd41bb695`。741项tracked及非ignored新增文件前后SHA相同，直接进程leader被wait/reap，原numeric进程组不再存在，所属空private TMP删除。该观察不证明未观察的逃逸后代。57项packaging回归实际1.471秒退出0，输入相同，日志SHA-256为 `14b52d0a90aa419f1ca87a6a39394e69d0f533b1d285c0899388c694d2ef0f04`。

之后作者只读检查发现跳过原文准备的程序化写入拒绝会保留旧pending。正常UI具有原文准备条件；仍修正后台防御分支，在准入失败时撤销精确旧提案，并新增“拒写→完整上下文审核→下一轮独立命令批准→finish”的闭环。`agent-gpui-v9` 对修正后8项闭环实际22.327秒退出0，输入相同，日志SHA-256为 `ba718e8c094ef7effc7e1ceee38bacec82ab9308d0f70dada21e89185d513991`。原第一门禁只证明它的输入映射，不能替代修正后的第二完整门禁；其结果及本机原生产物构建、独立复核待追加。

## 修正后精确候选

`full-gate-v2` 对上述防御修正与8项闭环的冻结候选运行相同标准脚本，真实514.451秒退出0。依赖策略、6项scripts测试、fmt、workspace/all-targets严格Clippy、1628项普通Rust测试、8项doc-test、默认及2 MiB本地进程控制器均通过。22项既有忽略测试继续未被计为验收。完整日志SHA-256为 `e81c203cb610d9f16b84cb2eae2c0323f1f6029569162306d57f45cf63b95335`，741项文件前后输入映射相同。直接leader已wait/reap、原numeric进程组不再存在、空private TMP删除；不扩大为未观察后代的证明。

同一741项输入上以显式最低macOS 15.0及jobs1运行 `cargo build -p keelshell-app -p keelshell-mcp --locked`，**dev** profile 的arm64双程序实际52.828秒退出0，日志SHA-256为 `e6e139c91daf497fe54b57d5da5b1bdddb48fca035e3de4dcc1fea350e767761`。57项packaging回归在修正后实际1.684秒退出0，日志SHA-256为 `dc3010863e9ce766b4780f075185cbc2cc325f7c9f88e2526dcb9df76fca4a4a`。标准macOS staging实际0.376秒退出0；native inspection实际1.128秒退出0，架构、15.0 minimum、动态依赖、execute bits、plist与清单检查通过。五项包文件SHA及两程序与原构建字节相等，包清单SHA-256为 `1685baf61acc764367a0b185dace406e21e4e42d1974a544907b65826bc5e4f2`。这是一份未安装、未签名、未启动的本地dev包，不是Release或原生Agent窗口验收。

最终只追加本节结果文档，未更改冻结运行时代码、测试、指南或ADR；独立复核需以最终文件摘要及第二门禁输入映射核对该文档差异。原始失败和所有收据仍保留。没有提交、推送、发布标签或操作共享原生窗口。

## 独立P1与epoch2退休修正

fresh非作者在完整独立snapshot上运行保留的生产反例：真实HTTP写提案、原文读取及第二次批准后，暂停精确预写canonical请求，调用生产 `close_tab` 删除捕获页签与SSH映射，释放后台并延迟前台维护。旧取消令牌未同步撤销，完整SFTP读回成为 `changed after target tab closed`。实际断言101，测试0.58秒，并非超时；原wrapper40.021秒、numeric PID/PGID 88198、所有741作者输入相同、owned组已消失且TMP删除。原2200字节日志SHA-256为 `f8ea907d35c7390cce99c54fef8aca7c0d16f9b60347c4315461cb7519c87166`，完整独立生产case SHA-256为 `e50ab74a6560c2dd772af680253d5322afacea1ee7ff4e9ad8baf7a5c69f1da8`。原freeze-v3与作者完整门禁0不能证明没有此缺陷，旧候选被BLOCKING_P1阻塞。相关自然End缺口最初只有源码证据，不能称为审查者已执行反例。

窄修正生产close与reconnect移除权威映射后的同步Agent维护，以及终端观察回调的即时维护。审查了全部生产SSH映射变更：close退休、finish_reconnect旧映射退休／新Entity替换、open_remote新Entity准入；新连接创建不撤掉原捕获目标，重连不能继承旧授权。原八项闭环不变，完整独立close反例保持原8秒等待及2秒读回／原字节断言；新增无保存reconnect binding的普通Exited及typed End两种终端观察路径，明确禁用200 ms备用维护，并新增实际生产finish_reconnect安装的退休验证。已发操作均保持Unknown/TargetLost，迟到成功无法替换Unknown或发送新回合。

epoch2限定第一轮101是作者测试缺少类型import及误用TestAppContext直接read；第二轮独立close和生产reconnect通过，但typed End在raw watch已经结束、Terminal.poll尚未发布通知时，取消仍为false，断言实际101。该失败及其输入完整保留：它证明只增加前台观察回调不能关闭raw结束／前台延迟边界；raw后台边界需单独源码约束及实际预写验证，不能仅凭改为等待published End就宣称关闭。此轮取消断言本身没有执行旧版本自然End后的文件发布反例。最初相关观察测试改为同8秒内等待生产poll发布End状态，仅证明观察回调这一较窄路径。第三轮101是一处Message import遗漏。第四轮11个闭环实际30.573秒退出0、测试17.47秒，包括原精确close反例 owner_retained=false 与完整原文读回，以及较窄的published End路径；它没有独自证明raw后台边界。第四轮日志2399字节SHA-256为 `540364d11fd1a3180d884bd3a7768c27ad71f240e498d6fbd2f3ab49b00ac750`，producer3597实际wait/reap leader3609／PGID3609，组实际不存在，空private TMP删除。

随后增加真实SSH生命周期共享源：任务捕获原Terminal的只读watch receiver，后台准备／执行select监听源结束，所有SFTP写授权检查直接要求同一源仍为Ready且producer未关闭。生产ssh_bridge始终附带该源，不靠前台定时器重新获取状态。新增第12个闭环把raw typed End、Terminal.poll未发布、前台整个回合阻塞、精确canonical预写hold与2秒独立完整SFTP读回放在同一窗口更新中；后台谓词当场false，完整原文不变，后台取消实际true。此时前台owner仍存在的事实也显式断言，避免把后台拒权混同UI已经退休。随后仅UI Unknown／TargetLost收据等待真实queued callback，精确run/action迟到成功不能覆写Unknown、不能恢复或发送下一轮。

第五轮12项中11项通过，raw保护／原字节读回／后台取消均通过，但作者在释放前台后立即断言owner不存在而queued callback尚未运行，实际101，日志3128字节SHA-256为 `64411528edd1b95813bc791a4eeb8b33d52793d9d93789f9bebf4ba67a12b944`，producer9355、leader／PGID9367。第六轮只为后续UI收据加入原8秒范围内的真实回调等待；原2秒前台阻塞读回、即时后台谓词、原close反例及所有字节断言没有变化。12项闭环实际31.423秒退出0、测试20.17秒，742项输入前后相同；日志2528字节SHA-256为 `58dae45f70279ed9aacbad0d58e07232c892ea844057d8766a13e9bc66cf4a89`，producer11680、leader／PGID11693，actual wait 0、leader已reap、原组不存在，所属空private TMP删除且路径不存在。所有期限与原反例的字节断言均保留。此结果只覆盖受控协议与GPUI，不扩大到供应商或原生窗口。

旧作者epoch收据未保存numeric leader/PGID，历史终态因此是producer报告，无法据其重新核对numeric身份；epoch2新wrapper额外保存启动与最终numeric leader/PGID、真实wait返回、reap、组终态、TMP剩余内容与删除状态，并保留全量输入映射及每个失败日志。新的完整门禁、macOS双程序dev构建／包检查和再次独立复核待追加；此前所有证据保留。

## epoch2精确完整门禁与开发包

16项候选文件在完整验证期间冻结，742项tracked及非ignored输入映射相同。`full-gate-v1` 的标准 `python3 scripts/check.py` 实际506.581秒退出0，依赖x.y策略、6项scripts测试、fmt、workspace/all-targets严格Clippy、1632项普通Rust测试、8项doc-test，以及默认和显式2 MiB进程控制器均通过；22项既有ignored测试仍未计入验收。日志412434字节SHA-256为 `6faaa0a3a9a89322ed2ea6a4566489f062f604e26e04764f4105b588f4adb92f`。producer16040，leader／PGID16052，actual wait 0、leader已reap、原组实际不存在，所属TMP为空并删除、路径实际不存在。完整门禁包含全部12个Agent GPUI闭环及8个领域测试。

相同742项输入上，显式macOS最低15.0、jobs1的 `cargo build -p keelshell-app -p keelshell-mcp --locked` arm64 **dev** 双程序实际9.002秒退出0；日志428字节SHA-256为 `687f3d96876059dee5aaf0840edd3bc8b4de00b6c64aae05340aabf04e59ea2b`，producer26858，leader／PGID26871。标准macOS staging实际0.350秒退出0，producer27694／leader及PGID27706；native inspection实际0.276秒退出0，producer27834／leader及PGID27846；五项包文件摘要、架构、最低系统版本、动态依赖、execute bits、plist和清单全部检查通过。包清单SHA-256为 `7f937471ee684aea69a04c75e2ec3ba78ea2576ca1f1928420d1c42ad752397f`，包内app SHA-256为 `625cd5f04b91fd2557420d75bcca693818b9e4da179d2eed4a97becce3ddd54a`，MCP SHA-256为 `abed072525e86dd627189f0dd754150787138ebe8cc979473e12228578d34d9c`，两程序与上述构建逐字节相同。独立字节检查实际0.184秒退出0，producer27928／leader及PGID27940。

57项packaging回归实际1.386秒退出0，日志248字节SHA-256为 `e349eb553a202ea7f6f3a0f9dde7904cae00dc4df2dc351a67d16b9c1720690c`，producer27943／leader及PGID27954。以上所有owned leader都有真实wait 0、reap，原numeric组不存在、所属空TMP删除且路径不存在，输入摘要全量一致；没有宣称未观察的逃逸后代。未签名、未安装、未启动应用或MCP进程，没有操作共享VM／原生窗口，没有推送／发布。包和结构检查不是原生Agent UI验收。

最后仅更新本测试记录的状态、准确区分raw旧取消断言与未执行的旧自然发布反例，并追加本节实际结果；运行时代码、测试、指南及ADR均保持精确验证输入不变。最终冻结及全量收据包记录这一个文档差异。旧独立P1失败、全部作者失败和旧包原封保留；新的fresh非作者复核仍是整合条件，作者门禁0不能替代独立复核。

## epoch3独立停止P1与同步失效修正

同一fresh非作者在epoch2独立snapshot上确认原close和raw producer关闭均已修正，十二闭环0，但又加入同一真实写提案／原文审核／第二批准／精确canonical预写hold的生产 `assistant-stop-agent` 按钮反例：同前台回合点击后domain已为OutcomeUnknown，workspace owner仍保留；释放hold并阻塞前台事件交付，2秒范围内独立完整SFTP读回出现新内容。不是已发送写回滚承诺，而是停止后的新写准入。三case运行actual101、62.733秒，raw3265字节SHA-256 `af815cb4f33a94e87567ae6c7568b75814be64720630e61599c6641fe364661b`；stop-only实际101、0.743秒，raw1195字节SHA-256 `d44deeb9fe4c1a8f4806e679f3ea7b63defddca22b6241975279641c23f9ba27`。numeric leader／PGID33440和35999消失，私有TMP删除，全部742作者输入及snapshot输入不变。epoch2 freeze SHA-256 `5480c2608332d1238ec031af206d20f54e870b7fb8c3b5614e83eafe54828579` 被BLOCKING_P1_CONFIRMED_STOP_PROPAGATION阻塞；原作者工程0不能覆盖这个缺陷。

在作者树新增独立模块而不改原十二case，复制同Stop function并先以旧runtime实际复现101：wrapper21.877秒，测试0.68秒，2099字节日志SHA-256 `84f882ab864bf0e6bce5e09c9cafb1f0e67efef167318de83e6fd838ae3ef4ed`，producer40461、leader／PGID40472。8秒等待、2秒前台阻塞读回和原字节断言不变。修正Panel共享精确run后台取消令牌并在当前动作内cancel；AgentStop只负责随后同run的owner清理。两种Capture按钮先直接失效，事件绑定previous_run；新run前核对queued Start身份，旧停止／执行／接受／结果及旧Change不能退休替代任务。

实际Kit 0.7源码显示Textarea Change异步、set_value静默、validator不保证多行／空文本路径，未假称同步hook。活跃run创建时直接把问题锁为只读，复制仍有效，固定“编辑问题并停止”先cancel后解锁聚焦；终态界面自动解除只读。实际窗口键盘输入、backspace和clipboard paste在活跃任务下被拒绝、问题字节不变，copy有效；点击后相同真实输入及空值编辑成功。其它覆盖选择文本、当前屏幕、实际set_context、配置菜单选择、catalog apply、凭据apply和Ask模式，共八种暂停写失效入口，均在前台整回合阻塞下同步backend false并保持2秒完整原文，Unknown及精确迟到IDs不恢复任务。

首次修正编译101因作者遗漏FluentBuilder import，1051字节日志SHA-256 `14451e99110b2fda37e52d16b93f67b29b2e72b47e643e0340f767b04bdc541d`；补充closed producer case的下一轮101因遗漏AgentOutcome import，1070字节日志SHA-256 `4a46cc97b349bd5eb6a3e019fb01c4e9cd2999853b7e6739afdc4c3e6172a225`，均保留原收据。14闭环先实际0，70.332秒；17闭环第三轮16通过／1失败，实际101、68.244秒，900×580新编辑按钮在滚动正文中不可见，4291字节日志SHA-256 `cae0d1615145a68ceecd61e93a057d1f3b83dff9c11d3f5c557b6fa82055f982`。将它移到固定request bar，没有放宽可见或字节断言。

第四轮17项实际0、74.528秒，3309字节日志SHA-256 `51665908fd6c0623ad445cdb9af134b896c98ad28a211a71b3d03a5238a42324`；producer53859、leader／PGID53871 actual wait 0、reap、原组不存在，所属空TMP删除且路径不存在，743项输入摘要前后相同。原lifecycle文件整体SHA与epoch2相同，原八case所在父文件除新增mod外SHA相同。包括同Stop反例owner_retained=true但原文不变，证明后台同步取消先于排队owner清理；同非作者closed producer case通过，旧事件后新的独立审批命令实际唯一执行。900×580中英／系统亮暗六组合编辑按钮可见、实际点击和后续输入通过，原固定批准／拒绝／停止测试仍通过。它们是受控GPUI／HTTP／TCP SSH/SFTP，不是native像素／供应商或Windows/Linux验收。

第五轮17项实际0、73.244秒，3309字节日志SHA-256 `1f47f8b80a8201d85687fb2d4fcba2075516f7f93e8cc091852a43a7808566d6`；它先验证新增问题容器不破坏已有闭环。随后把编辑动作接到Kit公开ScrollAnchor，并加强六组合测试：先真实滚到历史底部，固定编辑按钮仍可见，点击后题目容器完整bounds位于900×580视口，再实际编辑。最新该限定测试实际11.198秒退出0、六组合3.79秒，1070字节日志SHA-256 `204fd61f54f0fe505a8d07f2808cc9bd4b616af3d85d4dfc11791555d8a8254c`；producer72222／leader及PGID72234，wait 0、reap、原组和所属TMP路径不存在、743输入相同。最终完整门禁需包含这个最新定位输入，而不能只继承早先按钮可见证据。

完整新工程门禁、arm64 macOS dev双程序构建／包检查和再一次非作者复核待追加。旧epoch1、epoch2、两个独立P1及所有失败日志／收据／包保持原样。MCP由另一工作流独立处理，本候选没有增加第三方client或修改MCP授权实现。

## epoch3精确完整门禁与开发包

17项候选文件在正式验证期间冻结，743项tracked及非ignored输入摘要全部相同；包含最终ScrollAnchor定位源码，而不是继承第五轮的旧定位边界。`full-gate-v1` 标准 `python3 scripts/check.py` 实际518.531秒退出0，依赖x.y策略、6项scripts测试、fmt、workspace/all-targets严格Clippy、1637项普通Rust测试、8项doc-test、默认及显式2 MiB本地进程控制器均通过；22项既有ignored继续未计为验收。应用套件600通过、0失败、2忽略，187.05秒，含完整17个Agent GPUI闭环与原八case及四个生命周期case；Agent领域八项测试通过。日志413103字节SHA-256为 `84a042b628167cb12737e2fe04727a7326367ceb4ba9c071496b2ccd479ae311`，producer73065，leader／numeric PGID73077，actual wait 0、reap、原组不存在，所属空TMP删除且路径不存在。

同一743项输入、显式macOS最低15.0与jobs1的arm64 **dev** 双程序构建 `cargo build -p keelshell-app -p keelshell-mcp --locked` 实际8.822秒退出0，日志428字节SHA-256为 `c603a24aa6771110d4fdd59f65c0d90e45a112e6255f4dd499470c262a97e6c0`；producer81617、leader／PGID81628。标准macOS staging实际0.383秒退出0，producer82135／leader及PGID82147；native inspection实际0.273秒退出0，producer82271／leader及PGID82282。架构、最低系统、动态依赖、execute bits、plist和清单检查通过。五项包文件SHA在独立记录中，包清单SHA-256为 `ffedd323011ae435558009db3b0201b41894cce3c514a194fca65a1e28870001`，包内app SHA-256为 `3be7ea7beccd54f4b4e531d0a578b0aa2720fb70be86c18d973bd6d012d3f12c`，MCP SHA-256为 `abed072525e86dd627189f0dd754150787138ebe8cc979473e12228578d34d9c`，两程序与本轮构建逐字节相同。独立字节检查实际0.227秒退出0，producer82384／leader及PGID82397。

57项packaging回归实际1.683秒退出0，248字节日志SHA-256为 `d76ef9ce49c83f74d7d7c8b2f0246e7050a7cc74ec15757abeea875322d82a6f`，producer82491／leader及PGID82503。每一owned leader均保存numeric身份、真实wait 0与reap；原numeric组实际不存在、所属空TMP删除且路径不存在，六个正式步骤的743项输入摘要完全相同。不扩大为未观察逃逸后代的证明。包未签名、未安装、未启动；没有共享原生／VM操作或推送发布，结构检查不是原生Agent UI验收。

门禁后仅更新本测试记录的状态和本节实际结果，运行时代码、测试、指南及ADR保持正式输入完全相同。最终冻结逐文件记录唯一文档差异；所有epoch1、epoch2旧候选／独立P1、epoch3原停止101和其它作者失败及原日志／收据／包均保留。新一轮fresh非作者复核仍是整合条件，作者没有作出无阻塞结论。

## 保持开放

- 精确新候选的独立非作者代码／功能复核和主线消费。
- 原生 API 三协议、真实 Claude Code／Codex 已安装版本的新工作流与账户边界。
- 最小／宽窗口、中英文和三主题的原生像素与辅助技术。
- Windows／Linux 原生窗口，远端 Windows 文件语义及更广工具能力。
- 未知远程效果需用户独立核查；没有自动回放、审批复用或解除隔离的路径。
