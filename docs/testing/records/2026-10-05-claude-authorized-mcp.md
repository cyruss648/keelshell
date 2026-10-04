# 外部 Claude Code 的桌面授权 MCP 验收 — 2026-10-05

状态：实际安装的 Claude Code 在 macOS 原生 KeelShell 上完成本轮授权读取、边界拒绝、桌面批准/拒绝及撤权后的客户端结果循环。独立复核已通过该限定范围，无剩余 P1/P2。Codex 的新文本前置已通过，其 MCP 场景尚未执行。本轮没有修改生产源码、用户配置或已安装应用。

## 要求与范围

KeelShell 向外部智能体提供 MCP 服务端。外部客户端读取明确授权的远程信息、提交待审命令，并查询桌面拥有的提案状态；客户端没有批准、执行、SSH 登录或解锁凭据库的工具。内置模型 API / 本地 CLI Ask 是独立入口，不接入任意第三方 MCP 服务。

本轮使用实际安装且签名验证通过的 Claude Code 2.1.285、标准 macOS arm64 开发包中的 GUI 与 `keelshell-mcp`、自有 SSH/SFTP 测试服务，以及本地确定性 Messages/SSE 模型响应。模型只发送工具请求、检查同一个真实客户端回传的结果；不连接 SSH、MCP 或桌面 IPC，不生成工具结果，不操作批准按钮。

SSH PTY 仅回显测试文本；exec 只接受夹具定义的固定命令。本记录不证明任意操作系统 shell 命令、客户机器、云端账户、模型质量或费用行为。

## 标准包与源码绑定

构建开始和结束均为干净 `e16689bcd027a28d8035d271787ad53ab165d0ae`，256 份源码/工程文件的 SHA-256 一致。实际运行构建 GUI/MCP、构建 loopback fixture、标准 `packaging/package.py` 和 `packaging/inspect_native.py`，四步均退出 0。包内五个文件逐 hash 核对，manifest commit 为该精确基线；实际二进制为 arm64，最低 macOS 15.0。

| 实际执行对象 | 字节数 | SHA-256 |
| --- | ---: | --- |
| 标准包 GUI | 179390080 | `26ad9e962f4ebb9ec37b186db89748f9e34f42595f997db22ed3555737180e58` |
| 标准包 MCP companion | 16639744 | `069bbb239b5382f5c5203172ffd9fe080f7dc124c1fe1ab2de2e7d2a3477f53d` |
| SSH/SFTP fixture | 16564064 | `08f65b0f8bd1b457b5d2a3db09124446a336cc57250d1ddd2d1ad6af0667ec86` |
| Claude Code 2.1.285 | 223821616 | `51f09bd1e021d9fa8a1864c179799bd37cb39962a937935c5cf6823398e86db4` |

这是独立开发包，没有替换已安装应用，没有完成签名/公证或正式 Release。本轮文档更新不改变这份包的 commit 归属。

该精确源码的 [Quality 37240943183](https://github.com/cyruss648/keelshell/actions/runs/37240943183) 已实际完成，三个平台均 success。macOS/Linux 各 1031 项普通测试、Windows 1011 项，三者各 8 项文档测试；格式、严格 workspace Clippy、x.y 依赖策略、默认/显式 2 MiB CLI 控制器及各平台打包检查通过。macOS/Linux 各 9 项独立系统 OpenSSH 互操作通过，owned/TMP 清理回执通过。189 份新 CI 文件经根逐字节/hash 核对；这些门禁不证明 Windows/Linux 原生桌面。

## 受限客户端与能力生命周期

客户端使用空的 home/config/workspace、显式子环境、唯一 KeelShell stdio entry、七项精确工具白名单、空内置 tools、关闭 hooks/交互提示/会话持久化。实际进程网络策略仅允许自有模型端口与当前桌面 IPC 端口；没有增加其他网络例外。

本轮三个 fork/exec 代际分别核验模型 HTTP 往返、桌面 IPC connect-only 及八项真实内核 EPERM 负控，共 30 项必要检查通过。额外三项同模型端口 UDP 检查分别观察 sendto 允许、回包 receive 被 EPERM 拒绝，不称为 UDP 往返成功。该策略的 `allow default` 不提供 filesystem 隔离，也不证明所有 XPC/系统委托通道或瞬时后代都经过审计。

临时能力通过 GUI 的主动复制进入启动器内存与子环境，剪贴板随即清空。配置只引用环境变量名称；不保存能力值、模型凭据或认证 header。授权只针对本轮自有会话、明确 42 字节终端选区、canonical `/bin` 目录，以及枚举、选区、目录、文件、待审提案和状态六项能力；监控缓存未授权。

## 实际工具和桌面结果

新的独立 pass3 scope 使用新的 GUI、SSH fixture、会话、选区及临时能力。实际 Claude 协商生产服务的七项 schema/description，并完成 15 次真实 Messages POST、13 次真实 `tools/call`。每个调用都核对 preceding 模型工具名/参数、MCP request id、生产 `structuredContent` / `isError`、CLI stdout，以及同一客户端下一 POST 的精确 `tool_use_id` 和结果对象。

| 场景 | 本轮实际结果 |
| --- | --- |
| 会话枚举 | 返回 GUI 授权的精确会话/路线与选区 ID、目录范围，不返回 SSH 凭据 |
| 终端片段 | 真实 SSH 回显的本轮 42 字节选区完整返回 |
| SFTP 目录与文件 | 列出授权目录；完整读取包含本轮 canary 与中文的 UTF-8 文件 |
| 授权外路径 | `FORBIDDEN`，相同错误对象返回同一 Claude |
| 错误 route revision | `STALE_SESSION`，精确类别与相同错误对象返回同一 Claude |
| 未授权监控缓存 | `FORBIDDEN`，相同错误对象返回同一 Claude |
| 首条固定命令 | 先得到 `pending_review`；根通过原生 CUA 审阅完整目标、命令和摘要后点击执行，GUI 显示成功及固定 stdout/stderr；客户端后续得到同一 action 的 `succeeded` |
| 第二条固定命令 | 先得到 `pending_review`；根通过原生 CUA 明确拒绝，GUI 显示已拒绝；客户端后续得到同一 action 的 `rejected` |
| 撤销全部授权 | 根点击 GUI 撤销按钮后观察 MCP 关闭、授权列表为空及复制按钮禁用；同一旧客户端尝试下一工具，返回 `is_error=true` 和 `MCP server "keelshell" is not connected` |

撤权后的调用没有到达 companion 的第 14 个 `tools/call`。这证明旧客户端无法继续访问、真实错误被带回后续模型请求；不宣称收到新的服务端权限判定、`DISABLED` 或 `FORBIDDEN`。回执显式记录 `fresh_server_authorization_denial_proven=false`。

实际 Claude 在撤权后自动启动第二代 recorder/companion 尝试重连，仍是同一个 CLI 父进程；不是验收控制器重复启动 CLI。两代 companion 的实际退出码均为 1。第二代记录 324 字节未获回复的输入、0 字节 server-to-client 与 `BrokenPipeError`，没有形成第 14 条业务 RPC。这一预期断连接结果完整保留，不写成 companion exit 0 或无自动重连。CLI 初始化还包含 builtin `cc-plugin-agents-md`；实际模型工具 catalog 仍只有七项 KeelShell schema，不称为全部插件或系统委托通道均关闭。

提案摘要绑定实际四个 UUID 与精确命令字节。外部状态工具只返回状态，不返回命令 stdout/stderr 或独立 exit code；批准后的固定输出仅由根在 GUI 中观察。没有独立远端 exec 计数，因此不宣称零次/一次执行计数证明。

三个 marker 均在对应真实 GUI 观察后立即创建，不批准操作，也不替代工具结果。根 GUI 像素证据在本轮 CUA 会话中，附有补写时间/行为记录；独立代理复核协议、摘要、结果与记录，不将其称为代理独立检查了磁盘截图。

CLI 最终精确文本、`result/success`、退出码 0、model `done` 与无失败一致。原生控制器在明确撤权和客户端完成后停止，没有触发本轮期限。

## 失败留存与清理

两个前置失败完整保留，不覆盖为 PASS：

- 首轮实际完成读取/授权外拒绝，wrong route 实际返回 `STALE_SESSION`，验收 helper 错误预期 `FORBIDDEN`，实际 API 400、CLI exit 1。后续只修正该精确断言，授权外路径/monitor 的 `FORBIDDEN` 及期限均保留。首轮 29 文件 manifest 为 `cd1686f8307aa295805a8cac1bf20cc2ad5aef2dd6c15d6c3d2d8283601cb4c4`。
- pass2 实际完成首提案 `succeeded`，第二提案仅 `pending_review` 回到客户端；根观察到 GUI 拒绝，但 marker 在真实客户端超时结果之后补写。实际 13 POST / 12 RPC / CLI exit 1，不能追认拒绝/撤权的同客户端闭环。38 文件 manifest 为 `ddf1a944f1a79cc5558461840e6eaf7bae49454ff7e589d0a76b4fd61814712b`。
- 原 native 生命周期达到原 1200 秒期限后停止 GUI/fixture；4 份最终状态/日志/说明 manifest 为 `16a904ee97c226dd4628ed2738f74e1c60bce397c7d5f008259731674acf0ffe`。这一失败生命周期不冒充 pass3；补写 GUI 记录时间不代表旧应用当时仍运行。

pass3 客户端的已观察 owned PID/birth、stdio/PGID/drain/model/UDP worker、监听端口与 scratch 均清理；原生 GUI/fixture 的原身份消失，fixture 正常退出 0，GUI 经控制器终止退出 -15，fixture root 与 private tree 均删除。只核验已观察身份，不证明未被采样捕获的任意瞬时后代。

根标准包 16 份构建/检查/来源/观察证据 manifest 为 `3ab6b1d77adb23651499e7f3e2b0410b56d11903a97a62dae0b0983c29dd2311`；包内五个产物另由标准 manifest 与二进制绑定核对，stage 保留供后续独立检查。pass3 的 48 份材料 manifest 为 `482befce303edb85185816f1d1ff00a335a5684af9a7e75d82d7dd79b97f8483`，原始 helper、wire、HTTP/SSE、临时身份及失败材料保持 ignored 私有范围，不上传公开仓库。

新独立复核限定范围 PASS，无剩余 P1/P2。16 份复核材料 manifest 为 `e74ed9d5d31599664d566c84dcac515ae76c6c802996c8669925cbe02e88b745`，报告为 `b91df0a19b17575f2e21de6f7ad51cfd67220c9f6231b7f4eed01acf2cf09d99`。复核独立重建 31 个 wire frame、15 个请求 body/SSE，逐一匹配真实 CLI stdout 与下一 POST，重新计算两个精确提案摘要；关联根的 GUI 记录与 marker/结果时序，核对两代 companion、14 个已观察身份的当前消失和 private/fixture root 删除。复核同时确认两次失败及原 native deadline 未被重新分类、九文档范围与 227 个本地链接准确；没有另行调用 CLI、GUI、SSH、网络或 Cargo/Git。

根再次逐 bytes/hash 核验成功 48 文件与复核 16 文件；八组已有 manifest 的 426 条文件记录、256 源码/工程 hash 和 14 个当前 owned birth 回读分别验证，文件条目数不作为测试数。新增提交只修改九份文档：本机格式、x.y、公开文本、链接和 staged diff 检查通过，完整 Clippy/测试/打包依据前述同源码基线的三平台 CI。本次文档提交的新 CI 在推送后独立运行，不预先宣称其完成。

## Codex 文本前置的独立诊断

安装版 Codex 0.160.0 的新 strict 复现保留 22 项 effective false feature、空 MCP、精确模型端口和隔离环境。实际 trace 捕获到它尝试连接模型端口以外的 loopback 代理端口，并被 Darwin EPERM 拒绝，模型收到 0 POST。

仅给子环境增加 `NO_PROXY=no_proxy=127.0.0.1` 的新对照中，实际客户端直接发送 1 次 Responses POST、精确 canary 文本回传、`turn.completed` 和退出码 0。网络策略不放宽，未读取/修改系统代理。该事实关闭新复现的文本连接问题；旧冻结失败没有直接 errno，仍不能回填其当时原因。

官方 `rust-v0.160.0` 中关闭 `respect_system_proxy` 会选择 reqwest 默认行为；该字段为 false 不等同于 `builder.no_proxy()`。核对来源为 [配置装配中的 http_client_factory](https://github.com/openai/codex/blob/rust-v0.160.0/codex-rs/core/src/config/mod.rs) 和 [代理路径中的 configure_builder_for_resolved_route](https://github.com/openai/codex/blob/rust-v0.160.0/codex-rs/http-client/src/outbound_proxy.rs)，不据此宣称已安装二进制与该源码逐字节重现。

新诊断 79 文件 manifest 为 `47272fa5040656e9782bad51b6b6414abc02ece08b11574923106da792b970c9`，14 个已观察 owned 身份及端口/线程/scratch 清理通过。Responses 请求原始字节未留存，仅解析摘要、长度/hash 与 canary 判定；SSE 可按实际事件重建。独立复核已通过该限定范围，无剩余 P1/P2。另有 23 文件 MCP 协议适配准备和 13 项 synthetic integrity 检查，均不等于实际 Codex MCP 调用。

## 未关闭的边界

本轮关闭的是上述 macOS / 实际 Claude / 自有模型与 SSH/SFTP / 明确授权及桌面审批切片。Codex MCP、云端模型和完整 Agent 工作流、其他授权工具动作组合、外部客户端 Running 撤权/重启、Windows/Linux 原生窗口、最终六目标 Release、签名/公证及已安装目录自动更新仍需分别验收。
