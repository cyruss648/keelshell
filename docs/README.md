# 文档索引

- [本地智能体准备期失败收尾](testing/records/2026-10-08-readiness-cleanup-diagnostics.md)：取消与实际 Join、诊断脱敏、独立控制及未验证边界。

- [小窗口会话标签设计](adr/0080-scrollable-session-tabs.md)：固定关闭入口、可发现的导航、布局后reveal与离线图标。
- [小窗口会话标签修复记录](testing/records/2026-10-08-compact-session-tabs.md)：真实缺陷与新功能各自的检查状态。

- [展开文件审核与输入焦点](product/FILE_REVIEW_AND_FOCUS.md)：完整两轴正文、固定确认区和显式草稿／差异入口。
- [精确 a4 三平台 CI](testing/records/2026-10-07-agent-mcp-platform-ci.md)：macOS／Windows 成功；Linux 目录夹具冲突及其证据边界。
- [开发 KDF 构建优化](adr/0075-development-kdf-build-optimization.md)：开发构建效率、加密参数保持和限定回归。

- [审核式 Agent](product/REVIEWED_AGENT.md)：逐轮请求、单项SSH操作、固定审批、停止与原会话绑定。
- [MCP会话授权生命周期](product/MCP_SESSION_LIFETIME.md)：向外部智能体提供能力，原始结束、关闭及重连撤权。
- [Agent与MCP主树组合](testing/records/2026-10-07-reviewed-ai-mcp-main-integration.md)：独立审查、精确导入与组合验收边界。
- [文件合并／patch限定macOS原生](testing/records/2026-10-07-text-merge-patch-native.md)：两次完整远端读回与UI证据限制。
- [精确b54三平台Quality](testing/records/2026-10-07-text-main-platform-ci.md)：macOS/Windows成功，Linux首次目录Ask超时原因未知。

- [产品与交互](product/PRODUCT.md)：用户任务、界面规则、AI 边界
- [远程 SSH 能力要求](research/remote-ssh-requirements.md)：产品目标与逐项验收标准
- [工作区观察摘记](research/remote-workspace-reference.md)：历史匿名研究与证据限制
- [路线与状态](ROADMAP.md)：每阶段范围及验收状态
- `research/`：来源、版本、核验日期；只保存公开材料与脱敏研究结论
- `adr/`：架构决策、备选方案与结果
- `testing/`：验收方案、实际执行结果、失败证据与待验证范围

文档随功能提交更新。设计状态不等于实现状态，测试计划不等于测试通过。正式发布前需满足逐项验收矩阵。

- [Capability ledger](product/CAPABILITIES.md): what runs, what is tested, and what remains.

- [开发交接](HANDOFF.md)：迁移后的当前状态、未验证模块与接续顺序。

- [远程工作区、AI 与图标验收](testing/records/2026-10-03-remote-ai-icons.md)：181项门禁及本机受控交互证据。
- [命名 AI 配置与视觉系统](adr/0005-named-ai-profiles-and-visual-system.md)：当前协议、凭据、取消、视觉与平台边界。

- [GitHub 与发布流程](RELEASING.md)：六平台标签构建、资产完整性校验和无发布演练。
- [凭据库认证设计](adr/0006-authenticated-credential-vault.md)：整库认证、身份绑定、保存冲突与安全边界。
- [SSH 凭据库与终端搜索集成验收](testing/records/2026-10-03-vault-search-integration.md)：全仓检查与 macOS 原生回环交互证据。

- [连接组织模型](adr/0007-connection-library-organization.md)：目录树、迁移、软删除、成功历史及导入边界。
- [连接组织与代理集成验收](testing/records/2026-10-03-library-socks-integration.md)：工作区焦点、最近记录、全仓门禁及 macOS 实机证据。
- [动态 SOCKS5 验证](testing/records/2026-10-03-dynamic-socks.md)：协议、资源限制、停止及异常清理边界。

- [Windows SOCKS 拒绝响应修复](testing/records/2026-10-03-socks-windows-rejection.md)：跨平台 TCP 关闭差异与严格响应回归。

- [README 呈现参考](research/readme-presentation.md)：开源项目首页结构参考与双语维护约定。
- [凭据库维护](adr/0008-credential-vault-maintenance.md)：已认证条目、主密码轮换和清理边界。
- [AI 加密凭据](adr/0009-ai-encrypted-credentials.md)：目的地绑定、显式解锁与草稿应用。
- [递归目录传输验证](testing/records/2026-10-03-recursive-transfer.md)：扫描审核、路径限制、取消与无覆盖行为。
- [SFTP 暂停与内容校验续传验收](testing/records/2026-10-03-sftp-resume.md)：确认后暂停/继续、文件及目录显式续传、重连/重启后新建计划；三平台 Quality 通过，macOS/Linux 另通过 4 项真实 OpenSSH 互通。
- [SSH 通道所有权验证](testing/records/2026-10-03-session-channel-ownership.md)：迟到确认、关闭背压及 SFTP 生命周期。
- [凭据与目录集成验收](testing/records/2026-10-03-credentials-tree-integration.md)：整仓门禁、原生回环操作与最终构建证据。
- [传输任务的小栈回归](testing/records/2026-10-03-transfer-stack.md)：Windows 失败证据、Future 内存布局修复与小栈取消测试。

- [命令片段与本地建议](adr/0010-command-snippets-and-local-suggestions.md)：显式保存、多行审核、后台匹配与目标绑定。
- [片段领域存储回归](testing/records/2026-10-03-snippet-domain.md)：稳定身份、失败原子性、边界与真实磁盘冲突。
- [命令工作区集成验收](testing/records/2026-10-03-command-snippets.md)：键盘与输入法事件、原文保留、会话目标和原生交互。
- [专属 SSH 跳板路线](adr/0011-owned-ssh-jump-routes.md)：逐跳身份与认证、凭据路线绑定、取消与资源所有权。
- [跳板领域回归](testing/records/2026-10-03-jump-domain.md)：依赖图、导入映射、删除恢复及作用域。
- [跳板协议回归](testing/records/2026-10-03-jump-transport.md)：远端解析、多跳通道、绝对截止时间及异常关闭边界。
- [跳板集成验收](testing/records/2026-10-03-jump-integration.md)：真实 SSH 工作区测试、双语布局、原生操作与清理。
- [双语 README 首页整理](testing/records/2026-10-03-readme.md)：开源项目结构参考、链接与内容核对。
- [SSH 上游网络代理](adr/0013-upstream-ssh-proxies.md)：逐跳 SOCKS5 / HTTP CONNECT、代理与 SSH 认证隔离、路线身份及取消边界。

- [SSH 上游代理验收](testing/records/2026-10-03-upstream-proxies.md)：配置、协议、真实代理转发 SSH、独立认证、取消及原生操作。
- [远端命令与路径补全](adr/0015-remote-command-completion.md)：明确目录、固定只读探针、光标处局部替换、取消与会话隔离。
- [远端补全集成验收](testing/records/2026-10-03-remote-completion.md)：词法/真实 shell、SSH/SFTP、GPUI、OpenSSH 与原生交互验证。

- [参数片段与批量 exec 设计](adr/0016-parameterized-snippets-and-batch-exec.md)：显式模板、字面引用、审核快照、有界调度与未知结果。
- [参数与批量命令验收](testing/records/2026-10-03-parameterized-snippets-batch-exec.md)：已通过领域/协议/组件专项，最终整仓、OpenSSH、原生与 CI 结果分层记录。


- [界面设计与智能体计划](product/DESIGN_AND_AGENT_PLAN.md)：系统明暗主题、专业视觉、本地Claude Code/Codex和对外MCP服务端，明确尚未实现的范围。
- [设计资料库](design/README.md)：参考选型、语义token/组件规范的维护方式和可用技能。
- [对外 MCP 使用指南](product/EXTERNAL_MCP.md)：桌面授权、八项工具、临时启动配置、Codex/Claude Code 示例与撤权边界。

- [远程协议诊断](product/REMOTE_PROTOCOL_DIAGNOSTICS.md)：明确远端 DNS、验证 TLS、HTTP(S) HEAD、能力缺失与取消边界。
- [协议诊断设计](adr/0067-remote-protocol-diagnostics.md)：分层请求、远端进程监督、实际等待和结果权限。
- [协议诊断候选测试](testing/records/2026-10-06-remote-protocol-diagnostics.md)：原 DNS 阻塞失败、真实自有服务、GPUI与待验证范围。

- [当前主副本递归镜像／文件呈现／协议组合检查](testing/records/2026-10-07-recursive-mirror-main-combination.md)
- [递归镜像 macOS 原生与实际 Linux 文件核对](testing/records/2026-10-07-recursive-mirror-native.md)
- [诊断分支保存资料同步 CI 定位](testing/records/2026-10-07-saved-profile-sync-ci-diagnosis.md)
- [本地智能体工作目录候选](testing/records/2026-10-07-local-agent-working-directory.md)：显式目录隔离、发送审核、版本绑定和剩余验收边界
- [Linux 定时同步失败观察](testing/records/2026-10-07-linux-sync-schedule-observation.md)：18 秒批准后等待的有界失败诊断、v2 复核和未确定根因

- [本地 CLI 登录身份研究](research/local-cli-owned-authentication.md)：实际 parser／配置探针、执行前策略缺口和仍开放的验收。

- [File review and runtime integration](testing/records/2026-10-07-ui-runtime-main-integration.md)

- [自动更新策略](product/AUTOMATIC_UPDATES.md)及[主副本组合记录](testing/records/2026-10-08-update-toolbar-main-combination.md)：源码整合与专项结果，完整门禁／原生各自记录。

- [本地／远程文件浏览](product/LOCAL_REMOTE_FILE_BROWSER.md)：显式目录选择、双栏元数据、排序／隐藏项和原有审核；v5 新独立限定审查、根完整门禁、新 macOS 标准包和宽窗口审核式双向 SFTP 通过，最终组合源码／工程／宽窗口原生独立复核无 P1/P2；精确 CI／其它原生范围见[整合记录](testing/records/2026-10-08-local-file-browser-main-integration.md)。
