# KeelShell 开发交接 — 2026-10-07

## 当前主线和整合状态

当前已推送主线为 `a4c15c04772b11a70a592ae7c95132b4bea4d818`。审核式 Agent 和原 SSH 生命周期绑定的 MCP 授权已整合；本机完整工程检查及 macOS 开发包通过，详见[组合记录](testing/records/2026-10-07-reviewed-ai-mcp-main-integration.md)。这些结果不等于完整产品或新原生流程完成。

该精确提交的[三平台 Quality](https://github.com/cyruss648/keelshell/actions/runs/37624355451)已结束：macOS、Windows 成功，Linux 唯一应用失败为目录同步被实际 `MutationBusy` 拒绝，零写入，原字节保持。独立双夹具受控流程已证明不同测试目录也可能争用进程级本地写保护；历史 CI 的准确占用者未识别。旧 b54 的 8 秒 Ask 和 e695 的 18 秒同步失败仍是分别保留的未知原因，见[精确 CI 记录](testing/records/2026-10-07-agent-mcp-platform-ci.md)。

本轮43个声明路径已按精确preimage和审核后的三方结果导入主工作副本，完整785输入与准备副本相等，所有未声明基线字节保持。主树完整门禁和新macOS标准包实际通过；对应受控原生文件审核取消/保存流程已完成，新的非作者最终证据复核通过，精确新提交CI另记：

- 文件审核展开视图、固定动作和显式草稿／差异编辑入口，作者及新非作者限定源码与 GPUI 回归通过，见[文件指南](product/FILE_REVIEW_AND_FOCUS.md)。
- 测试专用同步阶段观察和 Argon2 依赖的开发构建优化，作者及新非作者限定复核通过；生产 KDF 参数和原期限保持，见[ADR 0075](adr/0075-development-kdf-build-optimization.md)。
- 本地智能体目录诊断仅在 Ask 返回后尽力输出，新独立 IO 错误反例保持原成功或超时结果；原测试正文和 8 秒 Ask 不变，标准检查将自动执行新增 IO 回归。
- 本地文件测试 v5 以真实App弱组引用和实际panel／worker所有权保持许可，队列close／Join完成后才释放；作者六控制及95项四线程通过，根已完整消费。新的非作者真实App身份、资源终态与组合UI四项限定复核通过；根已完整消费原始材料，旧v3／v4拒绝记录保持，见[生命周期复核](testing/records/2026-10-07-local-mutation-fixture-lifetime-review.md)。生产文件隔离机制不改。

本次785输入正式检查实际0（1691普通Rust/10doc/6Python及严格工程检查），对应macOS标准包/57打包实际0。真实macOS受控SSH/SFTP审核、取消字节保持和明确保存42字节完整读回通过；12原始JPEG/AX、操作顺序、actualwait与清理保留。英文小窗口会话关闭按钮截断P2已确认，下一切片修复；未取得OS逻辑几何、IME/VoiceOver或其它平台桌面证据。整合结果和精确输入见本次记录，不发布完成产品标签。

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
