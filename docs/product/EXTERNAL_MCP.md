# 向外部智能体提供 MCP

KeelShell 提供 MCP **服务端**。Codex、Claude Code 等外部客户端启动伴随程序 `keelshell-mcp`，它通过受认证的本机 IPC 请求正在运行的 KeelShell；SSH 会话、授权及人工审阅留在桌面应用中。应用内的 API / 本地 CLI Ask 是独立入口，本功能不接入第三方 MCP 服务。

当前实现八项固定工具，第八项为受人工审阅的现有文件替换提案，见[文件提案指南](MCP_FILE_CHANGES.md)。新增工具已通过独立工程复核、主树完整门禁及新macOS自有八工具客户端的实际文件批准/拒绝/已观察并发变化拒绝，见[新增工具记录](../testing/records/2026-10-05-mcp-file-proposals.md)。最新标准包在中英文三种主题的六个独立提案中验证了长正文首尾、固定操作栏、人工拒绝和实际SFTP原字节不变；本轮只协商八项并实际调用五种工具，最小窗口、当前F批准回归、供应商第八工具与其他平台原生仍开放，见[六组合记录](../testing/records/2026-10-05-mcp-file-review-native-matrix.md)。下列供应商实验仅覆盖此前七项工具。此前七项工具、桌面授权及 SSH/SFTP 桥接完成独立代码复审、真实 stdio 进程测试与 macOS 隔离 SSH 服务上的原生操作。实际安装的 Claude Code 2.1.285 已通过标准 macOS 双程序开发包完成七项 schema 协商、13 次真实工具调用与 15 次模型请求的授权流程：明确片段、目录和 UTF-8 文件读取；越界路径与未授权监控返回 `FORBIDDEN`，错误路线返回 `STALE_SESSION`；桌面批准后提案从等待变为成功，另一提案在桌面拒绝后变为拒绝。桌面操作由受控 UI 自动化执行，独立复核已通过该限定范围，无剩余 P1/P2；模型模拟服务与 SSH/SFTP 夹具均自有隔离，不代表云模型质量、任意 OS shell 或客户服务器验收。见[授权客户端记录](../testing/records/2026-10-05-claude-authorized-mcp.md)。

撤权后，同一 Claude 客户端收到 `is_error=true` 的连接已断开结果，未记录第 14 次 tools/call RPC；这证明本次客户端后续访问不可用，不是新请求到达服务端后再次授权拒绝的证据。原默认拒绝及运行中撤权/重启证据继续保留在[前置记录](../testing/records/2026-10-05-external-client-mcp-preflight.md)与[桌面桥接记录](../testing/records/2026-10-04-mcp-desktop-bridge.md)。Codex 0.160.0 的新文本前置在仅为子进程补充回环 `NO_PROXY` / `no_proxy` 后通过；原失败保持；新的实际 MCP 尝试协商七项生产工具后，首笔模型请求缺少 KeelShell 目录并包含额外工具，停止于业务调用之前，两次失败均保留。实际读取和完整审阅场景仍未通过，见[Codex 尝试记录](../testing/records/2026-10-05-codex-authorized-mcp-failure.md)。供应商文件修改提案、其他平台原生窗口及完整发布包仍须单独验收。

最新目录诊断在默认拒绝入口中，实际 Codex 请求已出现七项 KeelShell 名称；没有有效桌面能力或业务调用。原 schema 的若干约束被转换，且记录器缺最终退出回执，完整目录准入仍未通过；这不关闭授权 SSH/SFTP 或桌面审阅。原授权失败保留，见[新目录记录](../testing/records/2026-10-05-codex-catalog-direction.md)。

后续生产八工具目录预检及诊断修复已通过新的非作者限定复审：完整目录定义、32个精确schema投影、实际请求的generated ID/header绑定与recorder错误收尾均已检查。实际新首笔POST仍固定400、0 SSE、0业务；Codex完整授权调用仍开放，不能沿用目录预检作为批准、拒绝、撤权或SSH/SFTP结果。见[八工具预检记录](../testing/records/2026-10-06-codex-mcp-catalog-preflight.md)。

最新 `6b4` macOS 标准开发包完成新界面授权与真实 Codex 八工具目录准入，根和新非作者均已全文复核。第一轮截图格式处理错过准备期限；第二轮目录通过后，首个工具请求的额外元数据被验证记录器拒绝，未到达桌面，仍为 0 个已授权业务 RPC。客户端后续数组输出是连接关闭错误，不能算读取成功。两轮失败与直接实际 wait、所属端口拒绝及文件观察保留；四个间接身份的旧探针仅为无类型 null，完整间接清理未证明。元数据与成功输出 framing 的窄修复已通过纯协议复核，新的三态清理候选已通过134项新独立纯控制；关闭后继续取得配对响应的原严格场景被证明与关闭契约矛盾，新的22项配对业务加实际关闭候选仍待复审和原生绑定。批准、拒绝、撤权和完整 Codex 业务继续开放，见[原生尝试记录](../testing/records/2026-10-06-codex-mcp-native-partial.md)。

## 在应用中授权

1. 在 KeelShell 连接 SSH，核对主机指纹并完成认证。
2. 切到希望授权的活动 SSH 标签，从底部状态栏打开 **MCP**。默认关闭，首次没有任何工具或会话授权。
3. 检查面板显示的目标，并勾选允许的工具。读取终端内容需要先选中内容，再在面板中明确捕获要分享的片段；外部客户端不能抓取整个终端。SFTP 读取需要明确目录范围，并由当前远程会话验证目录。
4. 点击 **授权此会话**。成功后点击 **复制临时启动配置**，交给可信的外部客户端。配置包含伴随程序的实际路径与两项临时环境值。
5. 外部客户端的命令提案出现在同一面板。先查看完整命令、目标和摘要，再选择执行或拒绝；工具调用本身不会执行命令。
6. 点击 **关闭并撤销全部授权** 结束访问。应用退出、会话失效、路线编辑、重新连接或重新授权都会使旧能力失效；外部客户端需停止旧进程，再取得新的配置。

关闭面板本身不会撤销授权，使用面板内明确的撤销按钮。临时启动能力可用于同一授权下的多个客户端连接，持续至授权变更、撤销或应用退出，不是单次连接后自动耗尽。应用重启回到默认关闭，不恢复授权、片段或提案。

发布布局中，macOS 的两项程序位于 `KeelShell.app/Contents/MacOS/`，Linux 位于 `usr/bin/`，Windows 位于包根目录。源码运行时先执行：

```sh
cargo build -p keelshell-app -p keelshell-mcp --locked
cargo run -p keelshell-app --locked
```

若只构建 GUI，面板会提示缺少伴随程序并禁用复制。

## 工具与操作边界

| 工具 | 能力与限制 |
| --- | --- |
| `keelshell_list_sessions` | 只列出授权活动会话的显示名、精确连接/会话/路线 ID、片段 ID 和允许目录；不提供 SSH 凭据 |
| `keelshell_read_selection` | 读取用户明确捕获并授权的有界片段；重新授权替换片段或撤销授权后旧 ID 不可用。仅捕获新草稿尚未改变已有分享 |
| `keelshell_sftp_list` | 在授权目录范围内读取目录，最多 256 项；路径、类型及链接检查在实际 SFTP 边界复核 |
| `keelshell_sftp_read` | 读取授权范围内常规 UTF-8 文件及对应完整 SHA-256，最多 64 KiB；拒绝链接、特殊对象及不完整 UTF-8 |
| `keelshell_monitor_snapshot` | 读取当前会话已有的固定监控缓存；工具不会自行运行远程采集命令 |
| `keelshell_propose_command` | 创建最多 32 KiB、有效期 300 秒的精确命令提案，等待应用用户逐项审阅 |
| `keelshell_propose_file_change` | 提议最多 64 KiB 的既存常规 UTF-8 文件完整替换，旧内容 SHA-256 必须匹配；单独授权目录和能力，等待桌面完整 diff/目标人工审阅，无创建/删除/移动 |
| `keelshell_get_action_status` | 查询同一授权下命令/文件提案的 action_kind 及等待、拒绝、到期、取消、运行、成功、失败或结果未知状态 |

每次操作复核精确会话和路线。客户端没有批准、执行、SSH 登录或解锁凭据库的工具。撤权会关闭 bridge；已发出的远程命令无法据此证明停止，应用显示“远端结果未知”，撤权后的输出不继续发布。

临时启动配置中的 `KEELSHELL_MCP_ADDRESS` / `KEELSHELL_MCP_SECRET` 是当前授权的能力。KeelShell 不把它们写入 profile、命令参数或日志。若外部客户端把复制内容保存到配置文件，它仍会保留这些值；不要放入受版本管理的项目配置、命令行参数、shell 历史或共享日志。用可信的启动器将两项值暂时交给客户端进程，结束后撤销授权。

## Codex 配置示例

以下只保存路径及要转发的环境变量名称。把 `command` 替换为 KeelShell 复制内容里的实际路径，并让启动 Codex 的进程临时提供两项环境值；示例没有有效授权值。

```toml
[mcp_servers.keelshell]
command = "/replace/with/keelshell-mcp"
env_vars = ["KEELSHELL_MCP_ADDRESS", "KEELSHELL_MCP_SECRET"]
```

Codex 使用 `mcp_servers` 表配置 stdio 服务，`env_vars` 转发客户端进程已有环境；配置位置与操作见 [OpenAI 官方 MCP 文档](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)。仅看到配置列表不证明已经连接桌面或调用 SSH 工具。

## Claude Code 配置示例

Claude Code 的 `mcpServers` JSON 支持环境变量插值。以下占位符引用启动 Claude Code 时临时提供的两项环境值；Windows 的 `command` 使用实际 `.exe` 路径及 JSON 路径转义。

```json
{
  "mcpServers": {
    "keelshell": {
      "type": "stdio",
      "command": "/replace/with/keelshell-mcp",
      "args": [],
      "env": {
        "KEELSHELL_MCP_ADDRESS": "${KEELSHELL_MCP_ADDRESS}",
        "KEELSHELL_MCP_SECRET": "${KEELSHELL_MCP_SECRET}"
      }
    }
  }
}
```

配置格式及作用域见 [Claude Code 官方 MCP 文档](https://code.claude.com/docs/en/mcp)。应用复制的是单个服务的启动对象，供客户端相应配置入口使用；不是已写入用户全局配置的结果。授权变更后，停止旧服务进程并使用新能力。

## 连接诊断

| 情况 | 处理 |
| --- | --- |
| 未启用授权或 GUI 未运行 | 打开应用、连接目标并明确授权；伴随程序不会替用户登录 |
| 缺少伴随程序 | 使用包含两项程序的完整包，或按上方源码步骤一起构建 |
| 已撤权、重连或启动配置过期 | 停止客户端旧进程，在当前授权下重新复制；不要复用旧会话 ID |
| `FORBIDDEN` | 检查工具、活动会话、片段 ID 和目录范围；模型文本不能扩大授权 |
| `STALE_SESSION` | 检查当前连接、会话和路线是否仍与授权完全一致；重新连接或编辑路线后须重新授权 |
| SFTP 路径/内容被拒绝 | 使用已验证目录下的常规文件；链接、超额内容和非 UTF-8 拒绝是当前限制 |
| 提案等待审阅 | 回到 KeelShell 查看完整目标及命令；客户端没有批准接口 |
| 结果未知 | 通过独立 SSH 操作核对远端状态，重新审核后再决定下一步；不要自动重试 |

无环境能力时，独立 stdio 服务仍可协商并展示工具 schema，但实际操作保持默认拒绝。协议 stdout 与诊断 stderr 分离；本机能力持有证明不等于 OS 用户或可执行文件身份认证，也不承诺前向保密。
