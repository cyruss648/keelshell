# Codex 授权 MCP 尝试 — 2026-10-05

状态：两次实际尝试均失败，业务读取闭环未通过。失败证据保留，不能使用文本前置、工具目录协商或清理成功替代 MCP 业务验收。

## 范围与结果

测试使用源码 `e16689b` 的标准 macOS arm64 GUI/MCP 开发包、自有 SSH/SFTP 服务与本机模型模拟服务。当前 `40d092c` 只改文档，256 个源码/工程输入与三个二进制的前后 SHA 保持。根通过真实 CUA 核对测试主机指纹、连接并选取本次 42 字节随机标记；只授权枚举会话、读取明确选区、列目录和读取 UTF-8 文件，canonical 目录为 `/bin`。监控、提案与状态查询没有授权。

| 独立尝试 | 实际观察 | 结论 |
| --- | --- | --- |
| 首轮 | 已安装 Codex 0.160.0 的功能表 exit0；脚本把多词阶段列误解析为布尔值，停止于执行前检查；0 模型 POST，未运行 MCP exec，0 业务 RPC | 验收工具失败，旧记录保持失败 |
| 新范围 pass2 | 修正末列解析并通过对应离线检查；真实严格 CLI exit1、1 次模型 POST；生产 companion 完成 initialize 与 tools/list，返回七项工具；0 业务 RPC | 首次模型请求不符合工具目录约束，停止并保留失败 |

pass2 的原始 Responses 请求为 46,780 字节，已在私有证据保存 base64 原字节，并核对长度、SHA 与解析内容一致。顶层缺少 `tools`；developer `additional_tools` 包含 `functions`、`clock`、`collaboration` 三组共 11 项额外工具，其中没有 KeelShell 工具。没有过滤目录、自造工具结果、批准或调用额外工具。该请求被拒绝后没有发送 SSE；400 响应原字节未单独采集，实际 CLI 输出保留了错误文本。

没有经过 Codex 的选区、SFTP 文件、提案、批准、拒绝或旧能力读取。因此这些功能的 Codex 验收仍开放，不能用之前独立完成的 Claude 场景替代。

## 诊断与证据边界

保存的官方 `rust-v0.160.0` 源码解释了 `use_responses_lite` 将 `prompt.tools` 序列化进 `AdditionalTools` 的载体，但这 11 项额外工具的来源尚未证明。实际功能表中 `code_mode_host=true` 独立于 `code_mode=false`，仅作为后续候选；没有将其认定为根因，也没有继续第三次供应商执行。

实际启动使用明确子环境、新建私有 HOME/CODEX_HOME/config/cache、忽略用户配置和规则、关闭继承文件描述符；已安装 Codex 的 Developer ID 与二进制 SHA 核验通过。每轮 30 项必要网络检查与三项另记 UDP 诊断通过，限定子进程只能访问本次本机端点。这些证据不证明全部文件系统、OS、XPC、全局配置或未观察后代完全隔离，不代表云模型或实际客户主机验收。

客户端已清理观察到的 Codex、recorder、companion 出生身份、自有模型端口/线程及 scratch。根随后通过 CUA 撤销全部授权，面板显示 MCP 关闭且授权会话为空，再停止自有 GUI/SSH；两出生身份消失，fixture 根与 private 删除。此次退出后撤权没有再测试同一客户端请求，不能称新的服务端拒绝。GUI 资源 strict codesign 失败保留为开发包边界，不称签名 Release。

私有证据位于 ignored `work/external-codex-mcp-native-20261005` 与 `work/external-codex-mcp-native-20261005-pass2`。两份最终 manifest 分别绑定 62 / 59 个文件，SHA 为 `9484eeed8f296a8877cdad53f3fcc948801206d044769485852ed200d2da7654` / `6cb80ffa953f27ebfcb3d9b3b304e5e747ecaf4779d33b1626c7257bb463992f`；原先冻结的 50 文件第一轮记录逐字节保持。根逐条回读 bytes/SHA/0600 通过，原生最终九项 manifest 与撤权观察另行保留。

新的非作者只读复核确认两轮证据与失败表述一致，无剩余限定范围 P1/P2，结论为 `EVIDENCE_CONSISTENT_FAILURES_REMAIN_OPEN`，不是业务通过。复核逐项核验原 manifest、原请求字节、schema 目录、工具/模型次数及已观察身份清理；首轮功能表误解析与第二轮自有模型 schema gate 拒绝保持，工具来源未知。15 份独立证据由根复制核验，manifest SHA `cd2d0fa23ce79554462cb06f5147447f81cb9d080d3e4f57960270301ef34ec1`。独立范围没有启动客户端、网络、模型或 GUI，原体只读核对而未复制；复核辅助脚本的解析错误也保留，没有把原失败改写为成功。

## 最新 CI

新精确 `a19d0c17f2e0e298b97805991f01e2329d494d4f` 的 [Quality 37253864420](https://github.com/cyruss648/keelshell/actions/runs/37253864420) 已结束 failure：macOS/Windows 成功，Linux app 文件布局场景等待 12 秒失败，尚未执行 MCP/OpenSSH。新 macOS/Windows 的 EOF 回归通过，但本次 Linux 结果不能关闭下方旧 EOF 失败；详见[三平台记录](2026-10-05-integration-ci.md)。这也不改变本文的 Codex 业务失败。

精确 `40d092ca61c4c229acb4a004d2852dd858e86aed` 的 [Quality 37244888512](https://github.com/cyruss648/keelshell/actions/runs/37244888512) 最终为 **failure**：macOS、Windows 成功，Linux 在 MCP 未读 stdout 后 EOF 的成功退出断言失败。完整日志已保留；没有该子进程 stderr，具体原因不能追溯断言。本次 Linux OpenSSH 步骤未执行，旧 `e16689b` 三平台 success 不改变新运行的 failure。

后续继续查清客户端工具来源，验证实际 Codex 读取及审阅闭环。新的 EOF 修复已通过独立代码审查及根整合门禁，见[整合记录](2026-10-05-ai-modal-mcp-integration.md)；Linux 新文件布局失败及后续 CI 分别确认。Windows/Linux GUI、完整授权组合、文件修改提案、六目标签名发行与已安装更新仍独立开放。
