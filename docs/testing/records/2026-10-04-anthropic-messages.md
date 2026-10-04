# Anthropic Messages 传输验证记录

日期：2026-10-04

## 覆盖内容

- `ProviderProtocol::AnthropicMessages` 绑定到不可变 provider 配置和人工审核预览。
- 预览只包含 `model`、固定 `system`、一个 `user` 文本消息、`max_tokens` 和 `stream: false`；默认上限 4096，额外 API 只接受 1–1,000,000 的边界。
- 请求在有 key 时使用 `x-api-key`，永远不把 Anthropic key 放入 Bearer `Authorization`；无 key 时仍发送 `anthropic-version: 2023-06-01`。
- 回复只读取 assistant 的 `text` blocks；tool use、thinking、redacted thinking 和未知 block 不会成为 UI 文本，只有非文本回复会失败。
- 模型发现从同源 `/messages` 推导 `/models`，按 `after_id` 跟随最多 64 页和 4096 个 ID；缺失或重复游标会失败。
- 设置页的协议切换会把地址后缀改为 `/messages`，认证切换到 `x-api-key`，并撤销旧临时密钥和旧预览。
- 加密凭据绑定保留 endpoint、协议和 profile ID，Anthropic 使用独立的 `header:x-api-key` 绑定值。

## 已执行验证

```text
cargo fmt --all
cargo test -p keelshell-ai --locked
cargo test -p keelshell-core --locked
cargo test -p keelshell-app --locked
```

结果：AI crate 的 30 项单元测试、19 项 discovery loopback 测试、11 项 provider loopback 测试、1 项 doctest 通过；核心库 70 项单元测试和集成测试通过；GPUI 应用 269 项测试通过。

## 边界

loopback fixture 不调用商业 AI 服务，也不证明供应商账户、模型能力、计费、真实响应质量或 Windows/Linux 原生桌面交互。Anthropic 的高级请求头、代理、推理参数、服务端工具和 Agent 工作流仍由配置校验显式拒绝；当前助手只展示文本并把命令交还给人工审核。
