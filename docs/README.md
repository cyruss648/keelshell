# 文档索引

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
