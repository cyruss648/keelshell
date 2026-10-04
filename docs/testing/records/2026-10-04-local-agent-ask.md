# 本地智能体 Ask 进程验证记录

日期：2026-10-04。环境：macOS / Apple Silicon、项目固定 Rust 工具链。切片范围仅为 `keelshell-ai` 后端；[ADR 0038](../../adr/0038-reviewed-local-agent-ask-processes.md) 说明准入、权限与生命周期边界。

## 已执行检查

```text
cargo fmt --all -- --check
cargo clippy -p keelshell-ai --all-targets --locked -- -D warnings
cargo test -p keelshell-ai --locked
python3 scripts/check.py --policy-only
```

结果：45 项 AI 单元测试、20 项 provider discovery 集成测试、12 项 provider HTTP 集成测试、2 项文档测试均通过；独立的真实子进程 custom harness 全部断言通过；2 项安装版 CLI 测试在普通运行中显式 ignored。严格 Clippy、格式与直接依赖 `x.y` 政策通过。新的 registry 依赖为 `process-wrap = "10.0"`（锁定 `10.0.1`）以及复用已解析的 Unix `nix = "0.31"`；无私有 Git 依赖。

custom harness 由同一份编译后的 Rust 测试可执行文件充当 CLI，不经过 shell、不依赖 Python/Node 或供应商账户。它在两种协议下验证：版本/help/feature 准入、固定 argv 与空环境、问题只经 stdin、中文 UTF-8 分割、完整回答与成功回执、未知工具、乱序/重复/截断、非零退出、stderr 秘密不回显、输出洪水、已知密钥再次遮蔽、预取消、活动取消、超时、leader 提前退出留下后代、future 被丢弃后的停止与目录清理、凭据出现在上下文时拒绝。后代创建受控 loopback listener；测试确认操作前可连接，操作后不可连接，而非仅检查父进程状态。

## 安装版 CLI 的独立 opt-in 验证

以下入口默认不运行。操作者显式指定绝对原生可执行路径；测试自行建立 loopback Responses / Anthropic SSE 服务与临时 HOME/CLI 配置，不连接供应商推理服务，不使用真实 API key 或既有账户。示例中的可执行路径是占位符。

```text
KEELSHELL_CODEX_EXECUTABLE=<absolute-native-codex>
KEELSHELL_CLAUDE_EXECUTABLE=<absolute-native-claude>
KEELSHELL_CLI_FIXTURE_RECEIPT_DIR=<absolute-owned-receipt-directory>
cargo test -p keelshell-ai --locked --test local_agent_cli -- --ignored --nocapture --test-threads=1
```

以上变量必须设置在运行测试的进程环境中。`KEELSHELL_CLI_FIXTURE_RECEIPT_DIR` 仅用于测试保存不含 headers、密钥或用户路径的 receipt，不是应用配置。

本机已通过 2 项 opt-in 测试。其结果为：

| 安装版 CLI | 版本 | 已捕获请求 | 模型 tools | JSONL 结果 |
| --- | --- | --- | --- | --- |
| Codex | `0.160.0` | 一次 `POST /v1/responses`，模型 `gpt-6-sol` | 字段缺省（receipt 中为 null） | 4 帧，完整固定回答与成功完成 |
| Claude Code | `2.1.285` | `HEAD /api/hello` 连通性探针；一次 `POST /v1/messages?beta=true`，模型 `claude-sonnet-4-6` | 空数组 | 4 帧，完整固定回答与最终 success |

所有实际推理请求都包含显式问题；放在独立工作目录外、测试 scratch 父目录中的 `AGENTS.md` / `CLAUDE.md` canary 均未进入请求。每次 owned scratch 目录均被移除，测试服务线程已停止，测试控制器临时目录由 RAII 清理。原生 CLI 报告的 usage 或成本估算是固定 SSE fixture 的本地元数据，不代表供应商调用或账单。

Claude 初始化实测 `tools/mcp_servers/skills/slash_commands=[]`、`analytics_disabled=true`、`product_feedback_disabled=true`、凭据来源为 `ANTHROPIC_API_KEY`、默认权限；只保留 `cc-plugin-agents-md@builtin` 与固定内置 agent 元数据。解析器将安装插件、额外 agent、工具、hooks、MCP、非默认权限、订阅凭据来源或未关闭的分析/反馈 typed 拒绝。没有用空列表假装所有内置元数据不存在。

另执行 Codex 0.160.0 的本机 Seatbelt 权限探针：独立工作目录中的自有文件可读，目录外的自有文件读取拒绝，工作目录写入拒绝且文件未产生，退出码为 0，没有模型调用。限制环境中 Python/Xcode 的 cache 与 file watcher 诊断被保留在本地 receipt，未被当作权限失败。有效 feature 探针显示 18 项要求的能力为 false；`unified_exec` 仍为 true 的差异保留，未假称全部配置开关都生效。

本地原始证据保存在 ignored `work/local-agent-validation/`、`work/cli-research/` 及 `work/cli-research/adapter/`，包括命令日志、版本与 feature 探针、权限 receipt、初步 wire 结果和最终适配器 JSON receipt。这些位置不是发行内容。

## 发现并修复的问题

- 初次编译因现有 `AiError` 不支持 Clone 失败，去除不必要的派生；新增测试接口命名与内部匹配分支的编译错误已修复。
- macOS zombie-only group 清理可发生 EPERM，改为有界回收 leader 后重新发送 signal；只接受进程组不存在，第二次权限错误仍失败。正常 leader 退出后明确清理后代；不会先在 Windows Job 等待后代自然退出。
- 可控 fixture 的问题字符串检查曾将 `tool` 与固定 `shell_tool` 参数误判，改为准确输入边界检查；stderr-only 非零退出与缺失最终协议结果分开分类。
- Claude 本机 help 不公开 `--max-turns`，先前将它作为 help 准入导致拒绝；移除该错误准入，并保留实际执行固定单轮及最终回执的单轮校验。
- 原生 Claude 先做 HEAD 连通性探针；初版测试服务仅接受有正文的 POST，导致测试失败。服务现明确接受该探针，并修复 macOS accepted socket 的 nonblocking 继承。此修复针对 fixture，没有放宽生产输出协议。
- 严格 Clippy 指出 fixture 故意不等待后代，局部以理由标注：该场景专门验证适配器接管并停止后代，未在生产代码抑制 lint。

## 证明边界与剩余验收

已证明 macOS 后端协议、可控真实子进程生命周期及这两个具体安装版本与自有 SSE fixture 的互操作。没有验证 Windows/Linux 原生 CLI、图形界面的配置/发送/停止流程、已有订阅登录、真实获授权推理服务、服务端取消或计费上限。普通 CI 的同一套进程 fixture 将覆盖各平台代码，但编译、仿真或 macOS 结果不能替代其他平台的原生验收。

Unix group / Windows Job 管理正常所属后代；没有把主动脱离进程组的恶意程序视为被沙箱约束。丢弃 future 只有尽力 kill；显式取消并 await 才能得到 typed 清理结果。升级到其他 CLI 版本须重新核对官方协议、权限、metadata 与实际 wire，再扩展兼容集合。
