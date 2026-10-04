# 向外部智能体提供 MCP

KeelShell 提供 MCP **服务端**。Codex、Claude Code 等外部客户端启动伴随程序 `keelshell-mcp`，它通过受认证的本机 IPC 请求正在运行的 KeelShell；SSH 会话、授权及人工审阅留在桌面应用中。应用内的 API / 本地 CLI Ask 是独立入口，本功能不接入第三方 MCP 服务。

当前已实现七项工具、桌面授权及 SSH/SFTP 桥接，完成独立代码复审、真实 stdio 进程测试与 macOS 隔离 SSH 服务上的原生操作。下方客户端示例按官方文档核对；实际 Codex/Claude Code MCP 互通、其他平台原生窗口及完整发布包仍须单独验收，见[桌面桥接记录](../testing/records/2026-10-04-mcp-desktop-bridge.md)。

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
| `keelshell_sftp_read` | 读取授权范围内常规 UTF-8 文件，最多 64 KiB；拒绝链接、特殊对象及不完整 UTF-8 |
| `keelshell_monitor_snapshot` | 读取当前会话已有的固定监控缓存；工具不会自行运行远程采集命令 |
| `keelshell_propose_command` | 创建最多 32 KiB、有效期 300 秒的精确命令提案，等待应用用户逐项审阅 |
| `keelshell_get_action_status` | 查询同一授权下提案的等待、拒绝、运行、成功、失败或结果未知状态 |

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
| SFTP 路径/内容被拒绝 | 使用已验证目录下的常规文件；链接、超额内容和非 UTF-8 拒绝是当前限制 |
| 提案等待审阅 | 回到 KeelShell 查看完整目标及命令；客户端没有批准接口 |
| 结果未知 | 通过独立 SSH 操作核对远端状态，重新审核后再决定下一步；不要自动重试 |

无环境能力时，独立 stdio 服务仍可协商并展示工具 schema，但实际操作保持默认拒绝。协议 stdout 与诊断 stderr 分离；本机能力持有证明不等于 OS 用户或可执行文件身份认证，也不承诺前向保密。
