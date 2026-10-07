# KeelShell 开发交接 — 2026-10-07

最新已推送检查点为 `9669243`（自动更新策略与会话栏）。2026-10-08 后续准备期收尾改动只涉及本地智能体测试控制器及检查入口：新增十项控制、原默认／2MiB各626阶段、85项AI库及严格全target Clippy已在根实际通过；最终独立复核、提交与精确CI另记[准备期记录](testing/records/2026-10-08-readiness-cleanup-diagnostics.md)。下文旧219等检查点保留其历史范围，不作为最新主线。

## 当前主线和整合状态

当前已推送主线为 `219093c95b491692abea1a7d005b2e6eea985967`，根已核对远端main同SHA、ahead/behind为0/0。该提交的[三平台Quality](https://github.com/cyruss648/keelshell/actions/runs/37657109077)已结束：macOS/Linux成功，Windows在本地智能体受控后代端口3秒准备期失败，该case Ask/清理终态未记录，原因UNKNOWN。三个job原日志已读回，不宣称三平台整体通过。此前 `a4c15c04772b11a70a592ae7c95132b4bea4d818` 审核式 Agent 和原 SSH 生命周期绑定的 MCP 授权已整合；本机完整工程检查及 macOS 开发包通过，详见[组合记录](testing/records/2026-10-07-reviewed-ai-mcp-main-integration.md)。这些结果不等于完整产品或新原生流程完成。

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

1. 本次完整门禁、macOS标准包及限定原生文件流程已完成；完成功能提交和精确新CI（独立证据复核已通过）。
2. 修复英文小窗口会话关闭入口；继续独立候选自动更新策略、本地目录浏览，分别完成工程检查、新非作者复核与相应原生。
3. 继续完整外部客户端流程、原生 Agent、语言／主题／最小窗口／辅助技术以及 Windows/Linux 桌面验收；不以编译或受控 GPUI 代替。
4. 完成精确提交三平台 CI，继续六目标 Release、签名／公证、安装更新验收。GitHub 链接、变更日志和更新实现已有源码，但发布和安装需要自己的证据。

设计在 `docs/design` 和 `docs/adr`，需求在 `docs/product`，实际成功、失败及未验证范围在 `docs/testing`。完整原始材料留在忽略的 `work/`；归档前保留所需忽略材料，确认没有活跃消费者，再使用可恢复 worktree 归档。旧 UI 和 KDF 候选树已在保留源码、失败材料与实际程序后归档；新的测试组作者树保留。

此前详细交接及各次历史范围保存在[历史快照](history/2026-10-07-handoff-before-ui-runtime-integration.md)，不将旧时点的“当前”状态当作本次结论。研究 Reef／模板、AI 配置和界面资料见[文档索引](README.md)。

本次组合状态与后续实际结果统一记录在[界面与生命周期整合](testing/records/2026-10-07-ui-runtime-main-integration.md)。原生runner中断清理与编译输入绑定的14项自有子进程控制通过，只作为准备验证；实际新GUI/SSH/SFTP审核流程已完成，限定结果和未验证边界见整合记录。

## 自动更新策略与会话栏 — 2026-10-08

core typed policy 和 workspace长期更新服务已接通每日默认／关闭／每周、可选后台下载、启动延迟与有限退避；人工检查可撤销旧后台owner，迟到结果不能接管。已校验旧包只在新包校验成功后替换，取消当前请求恢复旧包，空闲时明确弃包；安装只经用户确认。历史metadata与偏好保存保留SSH实体和命令草稿，state reload即时同步策略。候选26项新增行为、原取消反例与修后专项各自保留；作者1703普通的旧full不继承为最终组合结论。见[策略历史](testing/records/2026-10-08-automatic-update-policy.md)。

工具栏已增加紧凑图标、可滚动标签、标题省略、实体绑定的关闭入口及前后导航；动态更新文字也绑定布局reveal。完整静态图标嵌入用于开发／正式包。四项GPUI和资产回归、新非作者源码复核以及宽窗口双SSH／关闭／草稿保持已有证据；英文原生small P2保持OPEN，不能沿用GPUI900px。

新组合审查发现旧绘制更新动作身份P1/P2，六项原指针反例实际失败、同测试修后通过，最后九项通过；按钮绑定完整Release／暂存包／请求身份，安装测试在current_exe/helper之前安全停止。修正后800输入最终门禁实际0：1,731普通Rust／10rustdoc／6Python、格式／x.y／严格全target Clippy及默认／2MiB各626控制器通过，22ignored未执行。同输入标准macOS双程序包／匹配夹具与57打包用例实际0。新隔离原生完成更新默认、保存／关闭重开、后台无稳定版本提示、双SSH中保存设置、英中／明暗／AI导航及关闭后保留草稿；13原图／AX、三个9,444字节完整文件读回和actualwait／所属组／端口／TMP回收均根读回。新的非作者最终源码／工程／宽窗口证据复核无阻断，收尾后仅九份状态Markdown变更，791其它输入相同。缩小尝试没有达到原生小窗口；精确提交CI、真实更新包下载／签名公证／安装回滚、整个原生退出时阻塞解包有界性、其它平台及完整产品目标继续开放，见[组合记录](testing/records/2026-10-08-update-toolbar-main-combination.md)。MCP保持KeelShell向外部智能体提供服务的方向，应用内API／CLI推理独立。
