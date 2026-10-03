# AI 诊断计划专项记录

日期：2026-10-03。范围：`keelshell-ai` 诊断计划解析/票据与 `keelshell-app` 助手面板集成。

## 验收目标

- 用户必须明确点击“整理为诊断计划”后才会从回复提取步骤。
- 只接受闭合的 shell 代码块；正文、非 shell 代码块和未闭合代码块被忽略。
- 每个计划绑定主机、活动 SSH 会话、上下文指纹和回复指纹；每个步骤保留回复行号与精确命令。
- 计划和步骤有界；每一步仍需独立命令审阅，不能自动执行或自动发送网络请求。
- 计划审阅票据拒绝不同会话、不同计划或过期票据。
- 语言/上下文/会话变化清除旧计划。

## 已执行测试

```text
cargo test -p keelshell-ai --all-targets
28 passed

cargo clippy -p keelshell-ai --all-targets --locked -- -D warnings
passed

cargo test -p keelshell-app --bin keelshell-app assistant::tests::diagnostic_plan -- --nocapture
2 passed

cargo clippy -p keelshell-app --all-targets --locked -- -D warnings
passed
```

覆盖内容包括闭合代码块提取、非 shell/未闭合过滤、只读风险提示、计划指纹、目标与会话绑定、过期/边界拒绝，以及 GPUI 面板显式生成、逐步审阅和会话失效清理。

## 证明边界

测试使用本地 GPUI 测试上下文和纯内存文本；没有向模型服务或真实 SSH 主机发送请求，也没有执行任何提取命令。只读标签是有限白名单的提示，不是远端权限或命令安全证明。跨平台原生桌面渲染仍需各平台独立验收。
