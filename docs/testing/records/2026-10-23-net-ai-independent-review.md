# NET-01 / AI-04 独立代码与功能复核记录 — 2026-10-23

## 复核范围

本记录覆盖本轮未提交的 NET-01 隧道运行时流量统计和 AI-04 本地知识领域核心，以及它们对应的 ADR、能力矩阵、路线图和交接文档。三名独立只读 agent 分别检查了 forwarding 生命周期与方向、AI-04 安全边界和文档证据；agent 没有修改、暂存、提交或推送文件。

## 首轮发现与修复

- remote forwarding 原先把本机目标 socket 作为计数第一侧，导致 `sent`／`received` 与“listener 绑定侧”契约相反。现在以远端 listener 侧为第一侧，测试使用 3 字节请求和 11 字节响应验证方向。
- remote forwarding 原先只撤销 route 和请求远端取消监听，没有等待已建立 relay task。现在 route 在锁内完成 active relay admission，关闭先撤权，再等待 active relay 归零；relay 也响应 route cancellation，因此停止行的快照有明确 quiescence 边界。
- AI-04 搜索原先使用折叠字符串的 byte offset 切分原文，Unicode 大小写展开可能造成错误列号或 panic。现在保留折叠位置到原文字符起点的映射，并增加 `İ`／组合字符回归。
- unknown、not started 和 cancelled 原先仍可能把普通标题放进时间线。现在只有已知 completed／failed entry 才保留标题，排除项和 Markdown 只保留安全来源、时间戳及稳定状态。
- 敏感标记补充带空格的 `api key`；空白文本在构造阶段拒绝；snippet 截断为省略号预留字节，保持公开上限。
- AI-04 领域产物移除通用 `Deserialize`，保留受控 `Serialize` 预览，避免后续调用方直接反序列化出绕过来源绑定或 `DoNotPersist` 的不变量。
- `DoNotPersist` 原因聚合对 `SensitiveContent` 采用更高优先级，避免未知结果掩盖敏感内容阻断原因。
- 文档明确区分来源标签构造阶段直接拒绝与标题／摘要构建阶段生成不可保存时间线，同时把 FILE-04 的前一轮完整门禁和本轮 NET/AI 门禁分开记录。

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

修复后在当前工作树运行：

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

## 边界

本轮修复和专项验证不代表 Windows/Linux/macOS 原生窗口、IME、VoiceOver、语法颜色像素、Finder 拖放、安装签名、公证、Release 或真实客户 SSH 主机验收。AI-04 仍未接入 `keelshell-app` UI 审核、StateStore 持久化或真实模型发送；NET-01 仍未实现命名隧道规则、规则持久化、审核后启停、重启恢复或重连绑定。
