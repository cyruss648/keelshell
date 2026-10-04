# 0038 — 审核后的本地智能体 Ask 进程

- 日期：2026-10-04
- 状态：后端小切片已实现；应用配置与发送界面另行接入
- 范围：`keelshell-ai` 的本地 CLI Ask 适配器，不创建本地终端

## 决策

本地智能体仍通过所配置的推理服务发送上下文。首个适配器只接受用户审核过的单次问题与显式选择的上下文，返回完整文本建议，没有 SSH、文件、Shell、MCP 或其他动作执行入口。`PreparedLocalAsk` 绑定 CLI 类型、绝对可执行路径、独立临时目录策略、推理地址、模型、输出界限与准确 stdin；显示审核后，显式发送操作消耗唯一的 `ApprovedLocalAsk`。审核超过五分钟失效。

使用原生可执行文件与固定 argv，不经过 shell 或 PTY。拒绝 Windows `.cmd` / `.bat` 启动器。每次请求创建空工作目录、独立 HOME、CLI 配置目录与临时目录；环境从空集合建立，只提供运行所需路径、禁用非必要流量的固定开关与显式提供的临时 API 凭据。不会继承用户代理、SSH agent、Node 配置、hooks、插件目录或账户登录环境。构造配置与准备审核不读取 CLI 配置或账户文件。

此版本不静默复用订阅登录、OAuth、钥匙串或现存会话。`LocalAgentCredential` 在内存中归零，不写入配置与 argv，Debug 隐去值；已知凭据出现在待发送上下文时拒绝。应用后续通过既有凭据服务解析引用，不能将密钥保存到配置元数据。没有自动安装、升级、登录、重试或恢复会话入口。

## 两种固定协议与准入

| 适配器 | 实证版本 | 非交互协议与限制 |
| --- | --- | --- |
| Codex CLI | `0.160.0` | `exec --json --ephemeral --ignore-user-config --ignore-rules`，stdin 输入，自定义 Responses 推理地址与显式 env key；18 项涉及工具、hooks、插件、扩展、子智能体与远程能力的 feature 必须有效为 `false`，禁用 web、MCP、历史、分析与反馈。 |
| Claude Code | `2.1.285` | `--print --bare --output-format stream-json --verbose`，stdin 输入；`--tools "" --disallowedTools "*"`、strict 空 MCP、禁止 slash commands、空 setting sources、关闭 hooks、默认权限、禁止权限提示、无会话持久化、单轮。 |

每次调用重新执行版本、help 与适用的有效 feature 检测，未知版本、缺失必需公开参数或有效禁用状态不成立时 typed 拒绝。这两个 patch 版本是受核对的 CLI 协议兼容集合，不是 Cargo 依赖版本要求，不能推广到其他版本。Claude 的单轮参数在本机版本有效但未列入公开 help，因此不把该隐藏参数当作 help 准入证明；结构化最终结果还必须报告恰好一轮。

Codex 使用明确的 permissions profile：`:root=deny`，`:minimal=read`，`:workspace_roots=read`，命令侧 network 关闭、权限审核者固定为用户。目录根是新建空目录，模型没有工具可以发起读取或动作。`unified_exec` 在本机即使传 disable 仍报告 stable true，因此不假称此键可禁用；独立 `shell_tool=false` 以及实际 Responses 请求无 tools 才是这版实证。应用服务器 schema 也被检查过；本切片选择已经验证的 exec JSONL，未建立 app-server 会话。

Claude 的 bare 模式仍报告内置 agent 与插件元数据，不能把这一事实误判为安装插件被继承。解析器只接受固定内置 agent 名称及 `cc-plugin-agents-md@builtin` / `cc-plugin-telemetry@builtin` 的精确内置路径与来源，不接受任意安装插件；tools、skills、slash commands、MCP 列表必须为空。初始化版本、API key 来源、默认权限、分析关闭与产品反馈关闭必须符合准入。实际启用非必要流量开关后，本机仅报告 `cc-plugin-agents-md@builtin`。空 `commands_changed` 元数据允许，其他 hook / tool / child assistant 事件拒绝，`api_retry` 作为推理失败终止。

Claude 的固定环境关闭非必要流量、反馈调查、官方 marketplace 自动安装、telemetry、错误上报与更新。实际版本还向已配置的推理端点发送一次 `HEAD /api/hello` 连通性探针；这项行为被独立测试记录，不宣称网络请求只有一次。

## 输出与生命周期

stdout 按 JSONL 有界增量解析，支持分割的 UTF-8 和 CRLF。总输出、单行、完整回答与帧数分别设界限；stderr 有独立上限，边读边丢弃，不回显供应商错误、凭据或上下文。完整 assistant 文本和最终成功回执必须同时成立，非零退出、未知工具事件、乱序、重复、末行缺失换行与截断均拒绝。思考内容或工具结果不会成为答案。已知推理凭据在回答中被再次遮蔽。

进程使用 `process-wrap` 的 Unix Process Group / Windows Job Object。所有已等待的终态都先停止所属进程树，再等待有界清理，随后移除 scratch。只等待 leader 的退出后才进入外层容器清理，防止 Windows Job 在正常退出时先等待仍存活的后代。macOS zombie-only group 可能短暂返回 EPERM：先回收 leader 再重新发送信号，第二次 EPERM 仍失败，只有已不存在的进程组可以被接受。未来被丢弃时发出尽力而为的 group/job kill；调用方必须用取消信号并 await 结果，才能得到清理确认。

这里的进程容器不是任意不可信可执行文件的安全沙箱。恶意程序或主动脱离 Unix group 的后代不在普通子进程清理保证内。局部取消、超时与输出字节上限也不证明服务端取消或推理计费上限。

## 实证与后续

普通 CI 使用同一个已编译 Rust 集成测试可执行文件充当 CLI 与后代 fixture，覆盖真实管道、字节分割、固定 argv、环境、异常输出、取消、超时、leader 先退出与 future 被丢弃后的进程停止，不要求安装供应商 CLI。

另有显式 opt-in 测试，将实际安装的两种固定版本 CLI 指向自有 loopback SSE 服务，使用纯 fixture 凭据；检查准确协议地址、完整回答、实际请求 tools 缺省或空、显式上下文、独立临时目录清理与外部规则 canary 排除。该测试不接触真实云端模型或账户，也不是 GUI 或 Windows/Linux 原生验收。结果见 [测试记录](../testing/records/2026-10-04-local-agent-ask.md)。

后续配置界面、可执行文件选择与验证、凭据引用、审核与停止交互、中英文错误展示以及真实获授权服务的验收应单独开发；本模块不宣称这些流程已完成。

## 官方依据

- [Codex 非交互执行](https://learn.chatgpt.com/docs/non-interactive-mode) 与 [配置参考](https://learn.chatgpt.com/docs/config-file/config-reference) 定义 JSONL、临时执行及 feature/config 行为。
- [Codex 权限](https://learn.chatgpt.com/docs/permissions) 与 [app-server](https://learn.chatgpt.com/docs/app-server) 用于核对文件系统范围、权限 profile 与协议选择。
- [Claude 非交互运行](https://code.claude.com/docs/en/headless) 与 [CLI 参考](https://code.claude.com/docs/en/cli-reference) 定义 stream-json、bare 和工具限制。
- [Claude 环境变量](https://code.claude.com/docs/en/env-vars) 与 [数据使用](https://code.claude.com/docs/en/data-usage) 用于核对凭据来源、推理地址与非必要流量禁用开关。
