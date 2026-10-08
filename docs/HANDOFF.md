# KeelShell 开发交接 — 2026-10-08

## 当前状态 — 2026-10-08

最新已推送基线 `f3c8820` 的[Quality 37721737467](https://github.com/cyruss648/keelshell/actions/runs/37721737467)已结束：Linux成功，Windows／macOS失败。新的[Windows诊断 37721774239](https://github.com/cyruss648/keelshell/actions/runs/37721774239)42项1failure，ACK前确实同Job的原child HANDLE在返回后仍LIVE，证明控制器成功终态条件不足；五项CDB／原706仍UNRUN、原应用原因UNKNOWN。macOS为small-stack Codex ProgressAsk取消后连接断言，具体监听者仍UNKNOWN；Linux在未加入SHA2优化的同生产源码上通过，不能归因优化或倒填旧Timeout原因。原完整日志与唯一SDK产物均保留，见[诊断与平台后续](testing/records/2026-10-08-windows-native-crash-diagnostics.md)。

本切片仅导入SHA2 0.11.*开发优化及文档，830输入实际通过1,812普通Rust／10doc／48Python、格式／x.y／严格Clippy和原默认／2MiB各626阶段；两个owner实际wait0/reap/group absent，私有TMP为空并删除，原Rust与期限保持。实际0.11编译opt3/debugassertions-on，0.10.9在macOS图未编译、Linuxflags待验；最终非作者根证据复核已通过，无确认P1/P2；精确新提交CI待完成。Unix cleanup v1因取消／重入／Drop数字PGID重发P2被拒绝，未导入；新的Unix和Windows控制器在独立目录准备，不混入本次源码。详见[优化记录](testing/records/2026-10-08-development-sha2-build-optimization.md)。完整外部MCP、CLI登录复用、最小原生几何／辅助技术、其它平台桌面及发布安装仍OPEN。下文按各自时点保留历史状态。

最新精确 `437f896` 的三平台 [Quality 37717370699](https://github.com/cyruss648/keelshell/actions/runs/37717370699) 已全部失败：Windows 控制器的后代终态测试、Linux 原8秒 CLI目录测试、macOS ProgressAsk取消后连接断言。首次真实 [Windows诊断 37717377052](https://github.com/cyruss648/keelshell/actions/runs/37717377052) 通过SDK inventory，但Python36项回归1failure／1error，未到达五项CDB控制或原706项应用测试；原异常原因仍UNKNOWN。三平台及诊断完整原日志和唯一允许的inventory产物已读回并保留；READY／原句柄身份夹具修正已通过本机48项脚本回归和新非作者限定复核；准确新Windows仍待运行，Linux耗时及macOS连接身份分别保持OPEN。此最新结果优先于下文历史“待运行”状态，不以本机通过覆盖，见[诊断与平台后续](testing/records/2026-10-08-windows-native-crash-diagnostics.md)。

本轮测试修复基于已推送主线 `7c11954`，已包含本地／远程 SFTP 浏览。精确 [Quality 37683724603](https://github.com/cyruss648/keelshell/actions/runs/37683724603) 已结束：macOS 成功，Windows 和 Linux 失败，三份完整 job 原日志均保留。Windows 失败测试的 `PathBuf::join("..")` 在 verbatim 路径上会先归一化；新的测试输入保留原生字面 ParentDir，使原拒绝断言获得正确输入。Linux 失败发生在 120 秒夹具组准入，准确 owner／原因仍未知；新增有界测试观察记录实际持有和清理阶段，原期限、FIFO 与四线程保持。测试修复 `a717400` 已提交推送；其精确 [Quality 37696576523](https://github.com/cyruss648/keelshell/actions/runs/37696576523) 已结束，macOS／Linux 成功；Windows 应用测试以 `0xc0000409` 异常退出，准确触发者未知。新的诊断 v2 已通过独立静态／离线复核并在本次提交精确导入三文件；根 42 项 Python 回归通过，实际 Windows 原源码四线程诊断待运行，见[诊断记录](testing/records/2026-10-08-windows-native-crash-diagnostics.md)。本机此前已实际通过 1,776 普通 Rust／10 rustdoc／6 Python、格式／x.y／全 targets 严格 Clippy，以及 57 项打包测试；新的非作者最终源码／工程证据复核无 P1/P2 阻断；本轮测试修复提交后的精确 CI 另行验收，见[平台记录](testing/records/2026-10-08-file-browser-platform-ci.md)。下文 a940 及浏览整合阶段保留其历史范围。

本次配置恢复提交收录 v9 功能与数量归属修正。首次工程检查的既有 AI 目标测试失败、中断和旧原生数量 P2 均保留；测试改用实际会话导航，领域预览和界面分别标注当前／恢复后数量，不可读当前数量不虚构为零。最终同 824 输入／16,151,246 字节实际通过 1,812 普通 Rust／10 rustdoc／6 Python、格式／x.y／严格全 targets Clippy，以及新 macOS 标准包和 57 打包用例。修后 17 原生观察完成有效／损坏恢复、取消、重启及完整文件比较；另四份中文浅色原图确认当前 2／恢复后 1 并取消保持原字节。两个新 owner 均实际 wait=0、已 reap／group absent，十次 app 均实际收尾，旧 v3 owner exit 仍 UNKNOWN。新的非作者最终源码／工程／包／限定原生复核无 P1/P2 阻断，当前／恢复后数量归属 P2 已关闭；收尾仅更新四份状态文档，其余 820 输入不变。精确提交 CI、最小 OS 几何／VoiceOver／IME、其它平台桌面及断电耐久性仍开放，见[整合记录](testing/records/2026-10-08-configuration-recovery-main-integration.md)。对外 MCP 新验收控制器的账本／进程最终检查修正已通过新的非作者离线复核；当前程序绑定与完整外部 Codex 业务仍未通过。MCP 始终是 KeelShell 向外部智能体提供能力的服务端。

## 历史浏览整合检查点


最新已推送检查点为 `a94038d`（本地智能体准备失败的测试控制器收尾）；父提交 `9669243` 包含自动更新策略与会话栏，其三平台 Quality 已实际全部成功。2026-10-08 后续准备期收尾改动只涉及本地智能体测试控制器及检查入口：新增十项控制、原默认／2MiB各626阶段、85项AI库及严格全target Clippy已在根实际通过；新非作者最终复核和精确远端读回通过；a940 的[三平台 Quality](https://github.com/cyruss648/keelshell/actions/runs/37676683014)已全部成功，三个 job 原日志完整读回；这不替代浏览增量的精确新 CI，见[准备期记录](testing/records/2026-10-08-readiness-cleanup-diagnostics.md)。下文旧219等检查点保留其历史范围，不作为最新主线。

本地／远程文件浏览已精确整合到根 `feature/local-remote-file-browser`：810 输入完整门禁通过 1,769 普通 Rust／10 rustdoc／6 Python、格式／x.y／严格 Clippy，22 ignored 未执行；同输入新 macOS 标准包与 57 打包用例通过。新宽窗口系统目录选择器、隐藏项、子目录／上级导航、大小排序及人工审核上传／下载已实际完成，三份完整30字节来源／远端／下载内容相等；12 原始 JPEG／AX、实际 wait 和所属资源清理保留。新组合源码／工程／宽窗口原生证据的独立复核无 P1/P2；功能提交后的精确 CI、最小原生窗口、辅助技术和其它平台桌面仍独立开放，见[整合记录](testing/records/2026-10-08-local-file-browser-main-integration.md)。MCP 由 KeelShell 向外部智能体提供服务；本轮未授予 MCP 权能或调用外部客户端。

## 2026-10-07 历史主线和整合状态

该历史时点已推送主线为 `219093c95b491692abea1a7d005b2e6eea985967`，根已核对远端main同SHA、ahead/behind为0/0。该提交的[三平台Quality](https://github.com/cyruss648/keelshell/actions/runs/37657109077)已结束：macOS/Linux成功，Windows在本地智能体受控后代端口3秒准备期失败，该case Ask/清理终态未记录，原因UNKNOWN。三个job原日志已读回，不宣称三平台整体通过。此前 `a4c15c04772b11a70a592ae7c95132b4bea4d818` 审核式 Agent 和原 SSH 生命周期绑定的 MCP 授权已整合；本机完整工程检查及 macOS 开发包通过，详见[组合记录](testing/records/2026-10-07-reviewed-ai-mcp-main-integration.md)。这些结果不等于完整产品或新原生流程完成。

此前精确`a4c15c0`的[三平台 Quality](https://github.com/cyruss648/keelshell/actions/runs/37624355451)已结束：macOS、Windows 成功，Linux 唯一应用失败为目录同步被实际 `MutationBusy` 拒绝，零写入，原字节保持。独立双夹具受控流程已证明不同测试目录也可能争用进程级本地写保护；历史 CI 的准确占用者未识别。旧 b54 的 8 秒 Ask 和 e695 的 18 秒同步失败仍是分别保留的未知原因，见[精确 CI 记录](testing/records/2026-10-07-agent-mcp-platform-ci.md)。

本轮43个声明路径已按精确preimage和审核后的三方结果导入、提交并推送主线，完整785输入与准备副本相等，所有未声明基线字节保持。主树完整门禁和新macOS标准包实际通过；对应受控原生文件审核取消/保存流程已完成，新的非作者最终证据复核通过，精确新提交CI另记：

- 文件审核展开视图、固定动作和显式草稿／差异编辑入口，作者及新非作者限定源码与 GPUI 回归通过，见[文件指南](product/FILE_REVIEW_AND_FOCUS.md)。
- 测试专用同步阶段观察和 Argon2 依赖的开发构建优化，作者及新非作者限定复核通过；生产 KDF 参数和原期限保持，见[ADR 0075](adr/0075-development-kdf-build-optimization.md)。
- 本地智能体目录诊断仅在 Ask 返回后尽力输出，新独立 IO 错误反例保持原成功或超时结果；原测试正文和 8 秒 Ask 不变，标准检查将自动执行新增 IO 回归。
- 本地文件测试 v5 以真实App弱组引用和实际panel／worker所有权保持许可，队列close／Join完成后才释放；作者六控制及95项四线程通过，根已完整消费。新的非作者真实App身份、资源终态与组合UI四项限定复核通过；根已完整消费原始材料，旧v3／v4拒绝记录保持，见[生命周期复核](testing/records/2026-10-07-local-mutation-fixture-lifetime-review.md)。生产文件隔离机制不改。

本次785输入正式检查实际0（1691普通Rust/10doc/6Python及严格工程检查），对应macOS标准包/57打包实际0。真实macOS受控SSH/SFTP审核、取消字节保持和明确保存42字节完整读回通过；12原始JPEG/AX、操作顺序、actualwait与清理保留。英文小窗口会话关闭按钮截断P2已确认，下一切片修复；未取得OS逻辑几何、IME/VoiceOver或其它平台桌面证据。整合结果和精确输入见本次记录，不发布完成产品标签。

小窗口会话标签和离线全图标源正在下一独立切片实现：已完成固定格式/x.y/全workspace严格Clippy，四项GPUI及完整内嵌资产回归实际通过。新的非作者静态促成语言/resize旧bounds和开发包资源修正；额外旧绘制close索引实际反例已复现并按EntityId修正，新完整门禁/标准包/原生及最终复核待完成。自动更新策略与本地目录浏览在独立作者树推进，尚未整合，不继承219的通过结论。见[小窗口记录](testing/records/2026-10-08-compact-session-tabs.md)。

## 产品与工程契约

原生 Rust GPUI Kit，macOS、Windows、Linux；只管理远程 SSH。默认简体中文并维护英文，System／Light／Dark 主题和白底跨平台图标。专业界面、完整远程功能、最小窗口和辅助技术目标保持，范围及余项以[能力清单](product/CAPABILITIES.md)、[路线图](ROADMAP.md)和[远程要求](research/remote-ssh-requirements.md)为准。

UI、core、session、AI 分层；阻塞 I/O 不进入 UI 线程。直接 registry 依赖使用 x.y，保留应用 Cargo.lock 与精确工具链。公开 API 有 rustdoc，错误类型明确，行为测试隔离且有界。提交前完成格式、Clippy、测试、依赖策略、相关原生检查和新的非作者审查。

凭据默认临时；明确保存使用 OS 存储或经认证的加密 vault。主密码、明文凭据及客户日志不进入源码、profile metadata 或记录。主机指纹变化拒绝连接。用户已授权公开 GitHub、提交推送和标签触发多平台 Releases，没有授权遥测或覆盖安装。

## AI 和对外 MCP

**MCP 由 KeelShell 向外部智能体提供服务。** 应用内调用 API／本地 Claude Code、Codex CLI 是独立的推理入口。独立代码与文案核对确认了服务端方向，见[方向复核](testing/records/2026-10-07-mcp-direction-review.md)。

对外 stdio 伴随程序经受认证的桌面 IPC 访问用户授权的会话、工具、片段和目录。八项工具读取限定信息、创建待审命令／现有文件替换或查询提案状态；客户端不能自行批准。关闭、结束或重连原 SSH 会话使旧授权失效，已经发出的远程操作仍可能结果未知。见[接入指南](product/EXTERNAL_MCP.md)、[授权生命周期](product/MCP_SESSION_LIFETIME.md)和[审核式 Agent](product/REVIEWED_AGENT.md)。

当前完整外部 Codex 业务尚未通过。旧 V12 完成 20 次配对调用，但后续拒绝状态、原文件再读和撤权未到达；之后启动尝试没有进入 CLI／模型／业务，不能补齐。历史 Claude 七工具限定证据不关闭供应商第八工具；同时活跃的应用内 Agent／MCP 撤权及三平台桌面仍须分别验证。

应用内 CLI 目前使用明确 API 身份，订阅登录复用仍未实现。新的只读研究和六项自有无账号探针确认了有效策略准入缺口；未增加占位功能、未读取用户 token，见[登录研究](research/local-cli-owned-authentication.md)。

## 下一步和证据维护

1. 本地／远程浏览的完整门禁、macOS 标准包、宽窗口审核式双向 SFTP 及新的非作者复核已完成；提交推送后单独核对精确新 CI。
2. 自动更新策略、会话栏和本地目录浏览已完成各自限定整合；继续英文最小原生窗口、更新安装及其它平台桌面的独立验收。
3. 继续完整外部客户端流程、原生 Agent、语言／主题／最小窗口／辅助技术以及 Windows/Linux 桌面验收；不以编译或受控 GPUI 代替。
4. 完成精确提交三平台 CI，继续六目标 Release、签名／公证、安装更新验收。GitHub 链接、变更日志和更新实现已有源码，但发布和安装需要自己的证据。

设计在 `docs/design` 和 `docs/adr`，需求在 `docs/product`，实际成功、失败及未验证范围在 `docs/testing`。完整原始材料留在忽略的 `work/`；归档前保留所需忽略材料，确认没有活跃消费者，再使用可恢复 worktree 归档。旧 UI 和 KDF 候选树已在保留源码、失败材料与实际程序后归档；新的测试组作者树保留。

此前详细交接及各次历史范围保存在[历史快照](history/2026-10-07-handoff-before-ui-runtime-integration.md)，不将旧时点的“当前”状态当作本次结论。研究 Reef／模板、AI 配置和界面资料见[文档索引](README.md)。

本次组合状态与后续实际结果统一记录在[界面与生命周期整合](testing/records/2026-10-07-ui-runtime-main-integration.md)。原生runner中断清理与编译输入绑定的14项自有子进程控制通过，只作为准备验证；实际新GUI/SSH/SFTP审核流程已完成，限定结果和未验证边界见整合记录。

## 自动更新策略与会话栏 — 2026-10-08

core typed policy 和 workspace长期更新服务已接通每日默认／关闭／每周、可选后台下载、启动延迟与有限退避；人工检查可撤销旧后台owner，迟到结果不能接管。已校验旧包只在新包校验成功后替换，取消当前请求恢复旧包，空闲时明确弃包；安装只经用户确认。历史metadata与偏好保存保留SSH实体和命令草稿，state reload即时同步策略。候选26项新增行为、原取消反例与修后专项各自保留；作者1703普通的旧full不继承为最终组合结论。见[策略历史](testing/records/2026-10-08-automatic-update-policy.md)。

工具栏已增加紧凑图标、可滚动标签、标题省略、实体绑定的关闭入口及前后导航；动态更新文字也绑定布局reveal。完整静态图标嵌入用于开发／正式包。四项GPUI和资产回归、新非作者源码复核以及宽窗口双SSH／关闭／草稿保持已有证据；英文原生small P2保持OPEN，不能沿用GPUI900px。

新组合审查发现旧绘制更新动作身份P1/P2，六项原指针反例实际失败、同测试修后通过，最后九项通过；按钮绑定完整Release／暂存包／请求身份，安装测试在current_exe/helper之前安全停止。修正后800输入最终门禁实际0：1,731普通Rust／10rustdoc／6Python、格式／x.y／严格全target Clippy及默认／2MiB各626控制器通过，22ignored未执行。同输入标准macOS双程序包／匹配夹具与57打包用例实际0。新隔离原生完成更新默认、保存／关闭重开、后台无稳定版本提示、双SSH中保存设置、英中／明暗／AI导航及关闭后保留草稿；13原图／AX、三个9,444字节完整文件读回和actualwait／所属组／端口／TMP回收均根读回。新的非作者最终源码／工程／宽窗口证据复核无阻断，收尾后仅九份状态Markdown变更，791其它输入相同。缩小尝试没有达到原生小窗口；精确提交CI、真实更新包下载／签名公证／安装回滚、整个原生退出时阻塞解包有界性、其它平台及完整产品目标继续开放，见[组合记录](testing/records/2026-10-08-update-toolbar-main-combination.md)。MCP保持KeelShell向外部智能体提供服务的方向，应用内API／CLI推理独立。

## 2026-10-08 本地／远程浏览候选整合


候选封包时，`feature/local-file-browser` 以 a4 加根已审核 43 路径／785 输入为基线，只在专用树
实现 files 领域单目录 picker、显式本地单层 metadata、路径填框、remote sorting/hidden。
默认不读本地目录；本地浏览仅供 SFTP。保持人工审核、写入准入、未知结果、原展开审核
及 Textarea/patch；后到旧只读准备版本不会重建已撤回审核，已发出 writes 仍保留真实结果。
v3 静态独立审查的下载类型与 Edit 冒泡 P2 已在 v5 候选修正；实际 v4 默认栈
溢出后，v5 拆分远程列表构建帧，保持全部行为和四十个条件定义。相同 v5 测试旧冒泡
两项断言 actual101、修后两项 actual0；最后 macOS 适用三十八项和原文件流程共
一百三十六项四线程通过，全 workspace 严格 Clippy／格式／x.y 通过。编码夹具实际
创建 EILSEQ92、未创建路径 metadata ENOENT2，最终明确拒绝分支通过；未继承 Linux
非法字节真实创建与读回。新非作者已完成精确候选与专项证据复核，无阻断；root 完整整合／CI 和原生仍独立开放。
原失败日志／scratch 保留，不声称已交付；见 [候选记录](testing/records/2026-10-08-local-remote-file-browser.md)。
勿将 43 继承的 root 输入当作本功能 delta；主树及其它作者范围不可覆盖。
