# NET-01 / AI-04 独立代码与功能复核记录 — 2026-10-24

> 本记录冻结在 2026-10-24 的复核范围；AI-04 单条 StateStore 持久化、完整 Workspace capture/save 集成和本轮独立复核属于后续切片，不能沿用本记录的“保存按钮禁用”表述作为当前状态。

## 复核范围

本记录覆盖已合入主线的 NET-01 隧道运行时流量统计／命名规则和 AI-04 本地知识预览审核，以及它们对应的 ADR、能力矩阵、路线图和交接文档。隧道切片经过两轮独立只读复核；AI-04 UI 与敏感信息修复经过独立只读复核。所有 reviewer 均未修改、暂存、提交或推送文件。

## 首轮发现与修复

- remote forwarding 原先把本机目标 socket 作为计数第一侧，导致 `sent`／`received` 与“listener 绑定侧”契约相反。现在以远端 listener 侧为第一侧，测试使用 3 字节请求和 11 字节响应验证方向。
- remote forwarding 原先只撤销 route 和请求远端取消监听，没有等待已建立 relay task。现在 route 在锁内完成 active relay admission，关闭先撤权，再等待 active relay 归零；relay 也响应 route cancellation，因此停止行的快照有明确 quiescence 边界。
- AI-04 搜索原先使用折叠字符串的 byte offset 切分原文，Unicode 大小写展开可能造成错误列号或 panic。现在保留折叠位置到原文字符起点的映射，并增加 `İ`／组合字符回归。
- unknown、not started 和 cancelled 原先仍可能把普通标题放进时间线。现在只有已知 completed／failed entry 才保留标题，排除项和 Markdown 只保留安全来源、时间戳及稳定状态。
- 敏感标记补充带空格的 `api key`；空白文本在构造阶段拒绝；snippet 截断为省略号预留字节，保持公开上限。
- AI-04 领域产物移除通用 `Deserialize`，保留受控 `Serialize` 预览，避免后续调用方直接反序列化出绕过来源绑定或 `DoNotPersist` 的不变量。
- `DoNotPersist` 原因聚合对 `SensitiveContent` 采用更高优先级，避免未知结果掩盖敏感内容阻断原因。
- 文档明确区分来源标签构造阶段直接拒绝与标题／摘要构建阶段生成不可保存时间线，同时把 FILE-04 的前一轮完整门禁和本轮 NET/AI 门禁分开记录。

## 合入前新增修复与复核

- 普通 AI 请求的 `Redactor` 增加带空格的 `api key` marker，并加入整行阻断、后续普通行保留和计数回归；这使普通请求与 AI-04 知识敏感内容闸门在该 marker 上保持一致。
- AI-04 测试记录收窄为真实 `AssistantPanel` GPUI 控制和确认按钮证据，明确当前专项测试直接驱动 `set_context`，没有把 `context-selection`／`context-screen` 到 workspace capture event 的完整路径写成已验证。
- 命名规则测试记录修正过时的 494 项全量测试表述；新增集成测试后的 497 是测试目标枚举，不被写成通过数。
- 最新 AI reviewer 对 `fb6226a` 给出 `NO_BLOCKER`：marker 逻辑、回归测试和文档边界均无 P0/P1/P2 阻断。隧道 reviewer 对 stale rule、批量删除保护和文档修正无新增 P0/P1。

## 修复后专项验证

以下命令在当前工作树实际通过：

```text
cargo test -p keelshell-core --locked --offline ai_knowledge -- --nocapture
cargo test -p keelshell-session --locked --offline forwarding::tests -- --test-threads=4
cargo test -p keelshell-session --test ssh_loopback --locked --offline local_forward_transports_bytes_and_closes_listener -- --test-threads=1
cargo test -p keelshell-session --test ssh_loopback --locked --offline remote_forward_transports_bytes_and_is_cancelled -- --test-threads=1
cargo test -p keelshell-app --locked --offline dynamic_panel_starts_real_proxy -- --test-threads=1
```

结果分别为 7、2、1、1、1 项通过。remote loopback 额外断言停止后等待完成，并在短暂等待后确认 `3 sent / 11 received` 不再变化；app GPUI 回归在停止完成后确认 `8 sent / 8 received` 仍保留。

## 提交级完整门禁

前一轮 NET/AI 修复工作树运行：

```text
python3 scripts/check.py
```

最终退出码为 `0`。关键结果如下：

- Python 工程检查 67 项通过、1 项跳过；受控进程清理场景通过。
- `cargo fmt --all --check`、`git diff --check` 和 workspace 严格 Clippy 通过。
- app workspace 单元／集成测试 755 项通过、2 项 ignored、0 项失败。
- core 单元测试 148 项通过、0 项失败；session 及其他 workspace 测试、rustdoc 和 doctest 均通过。
- AI local-agent readiness-error 场景全部通过；测试没有调用供应商 CLI 或真实模型。

门禁输出仍有两类已有环境或传递依赖警告：传递依赖 `block 0.1.6` 的 future-incompatibility 提示，以及 macOS linker 的 `__eh_frame section too large` 警告；本轮没有把它们当作失败吞掉。该门禁证明当前源码和受控测试范围通过，不代表三平台原生桌面、真实客户 SSH、安装签名或发布验收。

## 合入后主线门禁

在主线合并文档更新后运行：

```text
cargo fmt --all -- --check
git diff --check
cargo clippy -p keelshell-core --all-targets --all-features --locked --offline -- -D warnings
cargo clippy -p keelshell-app --all-targets --all-features --locked --offline -- -D warnings
cargo clippy -p keelshell-ai --all-targets --all-features --locked --offline -- -D warnings
cargo test -p keelshell-core --locked --offline
cargo test -p keelshell-ai --locked --offline
cargo test -p keelshell-app --locked --offline explicit_terminal_context_builds_reviewable_knowledge_preview_and_keeps_source_binding -- --nocapture
python3 scripts/check.py
```

结果：格式、差异、三个包的严格 Clippy、core 497 项测试、AI 176 项测试、AI GPUI 专项 1 项测试和完整 `scripts/check.py` 均通过。完整脚本报告 67 项 Python 检查通过、1 项跳过；workspace app 测试 756 项通过、2 项 ignored，core 152 项通过；其余 session、MCP、集成测试与 10 项 doctest 在同一脚本中通过。受控网络/原生夹具仍按各测试的 ignored 条件保持未运行。

## 当前合入状态与边界

本轮修复和专项验证不代表 Windows/Linux/macOS 原生窗口、IME、VoiceOver、语法颜色像素、Finder 拖放、安装签名、公证、Release 或真实客户 SSH 主机验收。NET-01 已有命名规则持久化和审核计划，但仍未实现规则编辑 UI、真实 forwarding executor、listener 启停、运行状态持久化、重启恢复或重连绑定失效。AI-04 已接入 `keelshell-app` 的 GPUI 本地知识预览和明确完成/失败审核按钮；StateStore 持久化、条目编辑/删除、模型发送和完整 workspace capture 集成测试仍未完成，保存按钮保持禁用。受控 GPUI 与源码门禁不能替代原生跨平台验收。
