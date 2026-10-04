# ADR 0037：对外 MCP stdio 服务端与桌面授权契约

- 日期：2026-10-04
- 状态：服务端基础已实现；受认证桌面 IPC、原生审核与真实 SSH 工具桥接待实现
- 关联：MCP-01、MCP-02、MCP-03、MCP-04；[设计与智能体计划](../product/DESIGN_AND_AGENT_PLAN.md)

## 决策

新增独立 `keelshell-mcp` 库与同名 stdio 可执行文件，供外部智能体调用。产品不新增访问任意第三方 MCP 服务的客户端。该模块不依赖 UI/core/session/AI 实现，也不包含 SSH 登录、凭据解锁或任意执行器。

采用官方 Rust SDK `rmcp = "3.5"`，锁定解析版本 `3.5.0`；依赖均保持直接 registry `x.y` 要求。只启用 server、stdio transport，未启用 SDK client、HTTP、OAuth 或第三方连接功能。支持 2026-07-28 的 `server/discover` 和逐请求 `_meta`，并明确支持 2025-11-25、2025-06-18 的 `initialize` 兼容路径。未声明支持其他版本。

独立可执行文件默认 `AccessPolicy::default()`（关闭）及 `DisconnectedBackend`。工具发现可用，调用返回 `DISABLED`；即使可信调用方配置授权，只要没有桌面后端，仍返回 `NOT_CONNECTED`。客户端没有启用/安装授权的方法。此阶段不能宣称外部 Claude Code/Codex 已读取真实桌面 SSH 会话。

## 固定工具与权限

工具 schema 只接受声明字段，不接受多余控制字段。七个固定名称为：

| 工具 | 行为与固定边界 |
| --- | --- |
| `keelshell_list_sessions` | 只返回允许枚举的活动会话身份/标签，最多32项；不含地址、用户名或凭据 |
| `keelshell_read_selection` | 只读取明确选中的片段UUID，完整UTF-8最多16 KiB |
| `keelshell_sftp_list` | 授权canonical远程目录、非递归最多256项；链接/特殊对象仅标为unsupported |
| `keelshell_sftp_read` | 授权canonical常规UTF-8文件，完整内容1–64 KiB，溢出拒绝，不静默截断 |
| `keelshell_monitor_snapshot` | 固定缓存监控字段，不接受shell输入 |
| `keelshell_propose_command` | 最多32 KiB精确UTF-8命令，仅创建待桌面人工审核提案 |
| `keelshell_get_action_status` | 读取同一目标和授权下提案的桌面状态，无审批能力 |

`SessionIdentity` 同时绑定connection UUID、不可复用live session UUID与route revision UUID。授权按会话、工具、canonical远程根及selected-fragment UUID独立配置。每次调用重新读取权威策略；同一连接的旧会话/路线返回 `STALE_SESSION`，不同连接和越权工具返回 `FORBIDDEN`。

`PolicyController::replace/disable` 在新版本可见之前撤销旧lease，取消已进入后端的读取及后续准入；返回数据前再次检查lease。更换授权保守地撤销所有旧请求，不尝试复用之前已批准的权限。路径词法校验拒绝相对路径、`.`/`..`、重复/末尾分隔符、反斜线及控制字符，根路径匹配必须完整组件；它不是远程防symlink或TOCTOU保证。

`CommandProposal` 由服务端生成UUID和SHA-256摘要，摘要绑定版本、proposal ID、精确目标与命令字节。桌面未来必须在最多300秒内由真人审核这些内容，单次消费批准，再复核授权/活动session；客户端不能自行选择ID、延长有效期、approve、execute、写文件或解锁vault。首次提案只允许返回匹配ID/摘要的 `PendingCommand`。取消/超时不能证明已经入队的提案被删除，也不能撤回已发生的远端I/O。

## 传输与资源边界

stdio stdout只输出SDK JSON-RPC；binary诊断仅stderr静态消息，不拼接参数、请求正文、SSH输出、路径、命令或底层错误。独立binary未安装tracing subscriber，SDK内部记录不进入输出；嵌入已有全局logger时须过滤rmcp的原始client metadata/cancel-reason记录，不能仅依靠本模块的静态错误保证SDK内部日志脱敏。格式错误JSON遵循SDK忽略行为；合法JSON但错误请求形状返回Invalid Request，多余/非法参数返回Invalid Params。

- 单条输入含newline最多128 KiB，超过即关闭transport；scratch每次8 KiB，不在报错时修改调用方ReadBuf。
- 启动协商10秒；binary runtime shutdown最多100 ms，避免不可取消blocking stdin阻止进程退出；backend默认5秒，可信构造范围1 ms–30秒。
- backend默认并发8，可信构造范围1–8；满时返回 `BUSY`，不排队扩容。
- SDK调度前的framing另有32项预算：输入frame占一项，只有实际stdout/stream flush成功才归还一项，backend完成/响应排队不能归还。超额直接关闭，不继续生成BUSY响应扩容；限制也覆盖tools/list、畸形/空JSON、notification。当前固定服务不发送unsolicited progress/subscription；将来扩展这些消息须改为按request ID关联准入。无响应通知、客户端放弃的请求保守保留槽位，因此累计耗尽预算会关闭此连接；响应被及时读取的正常客户端可持续调用。
- 完整序列化tool result最多256 KiB，包括legacy文本兼容副本及JSON转义，超量拒绝。
- EOF、读失败与reader析构撤销此transport的生命周期token，在service返回前释放owned backend future，不影响其他连接的策略。writer也监听同一token，使已blocked的write/flush退出并释放SDK writer mutex；完整SDK收尾另有2秒deadline。断开时可能丢失或截断最后在途响应，不承诺已发生I/O回滚。
- RPC取消通过SDK cancellation token释放拥有的backend future。2026规范禁止继续发送该取消请求的消息，SDK会丢弃其迟到响应；库直调仍有typed `Cancelled`，后续协议请求保持可用。

失败结果使用固定枚举，覆盖关闭、越权、未连接、旧会话、撤权、取消、超时、繁忙、非法参数、超量与私有后端失败。不转发可能含敏感数据的底层错误。

## 未来桌面 IPC 契约与缺口

`DesktopBackend::dispatch(AuthorizedRequest)` 是未来受认证本地IPC的契约，当前没有具体实现。IPC凭据只由用户明确开启后的桌面产生；必须验证本地peer、绑定当前authority，而不能信任客户端自报identity/策略。Bridge的桌面端应原子捕获已连接SSH handle并核对三个identity字段，before admission/before I/O/after await反复检查lease。断线、路线编辑、重连及撤权不能把旧ID映射到新session。

SFTP后端必须在真实transport重新canonicalize与检查授权根、拒绝symlink/特殊对象、验证类型/完整长度并保持取消/超时cleanup。固定监控只能返回允许的缓存字段。提案只进入原生人工审核机制；get-action-status必须核对提案归属与当前scope。不自动代登录SSH、信任变更host key或解锁凭据。已发出的远程读取没有事务取消承诺。

尚未完成：桌面启用/授权UI、IPC认证与进程身份、active-session映射、真实terminal/SFTP/monitor桥接、人工提案批准与单次消费/到期、外部真实CLI配置及互通、打包独立binary、Windows/Linux native进程验收。测试中的受控后端没有远程操作能力。

## 来源与验证

实现时检查了[官方Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)、[rmcp文档](https://docs.rs/rmcp/latest/rmcp/)、[2026-07-28传输](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports)及[stdio取消/生命周期](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio)。SDK源代码来自锁定公开registry，另核对了metadata、cancel响应丢弃和stdio解析行为。

测试与失败修复记录见[服务端基础记录](../testing/records/2026-10-04-mcp-stdio-server.md)。受控后端、真实macOS stdio子进程协议证据、真实SSH业务验收分别管理。
