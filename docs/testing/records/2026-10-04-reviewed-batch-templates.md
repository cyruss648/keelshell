# 审核式批量命令逐目标展开验证记录

日期：2026-10-04

## 覆盖内容

- 核心 `BatchCommandTemplate` 只接受五个受限元数据标记，拒绝未知标记和不支持的 shell 上下文。
- 核心测试验证中文目标名和端点值会作为 POSIX 字面值引用，模板渲染不进行 I/O 或命令执行。
- GPUI 工作区回归在确认前检查 `batch-reviewed-target-commands` 和每个目标的审核节点均可见。
- 同一批次的两个受控 SSH loopback peer 收到不同的最终命令，证明确认时实际 `BatchTarget.command` 使用了逐目标渲染结果；确认前 peer 没有收到请求，终端历史保持为空。
- 普通多行中文命令单独通过 GPUI 回归，确认后按原始字节发送到每个目标，且不进入交互终端历史。
- 逐目标命令列表使用独立有界滚动区域，避免 32 个目标的审核内容覆盖固定操作栏。

## 已执行命令

```text
cargo fmt --all
cargo check -p keelshell-app --locked
cargo test -p keelshell-core --locked batch_template -- --nocapture
cargo test -p keelshell-app --locked batch_template_review_binds_distinct_target_commands_and_separates_terminal_history -- --nocapture
cargo test -p keelshell-app --locked batch_literal_review_preserves_exact_bytes_and_separates_terminal_history -- --nocapture
```

结果：核心模板测试 4 项通过；应用逐目标审核与 loopback 执行回归通过；应用检查通过。完整工作区门禁和跨平台 Quality 需要在父任务合并本切片后重新执行。

## 边界

测试使用受控 loopback SSH peer，不代表生产主机上的 shell 兼容性。模板不支持自定义参数值、远端环境查询、shell 求值、自动重连、重试、调度或依赖图；这些行为仍被明确拒绝或不在当前范围。
