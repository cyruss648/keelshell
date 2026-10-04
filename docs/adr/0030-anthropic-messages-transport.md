# ADR 0030：Anthropic Messages 传输

日期：2026-10-04

## 背景

AI 配置目录已经允许声明 Anthropic Messages，但此前该值只能保存为元数据，设置页和助手会在请求前拒绝它。这样会让配置界面看起来支持一种协议，却不能完成可审阅的连接测试、模型发现或请求预览。

Anthropic 的 Messages API 使用 `POST /v1/messages`，请求包含 `model`、顶层 `system`、`messages` 和必填的 `max_tokens`；认证使用 `x-api-key`，请求还需要固定的 `anthropic-version`。模型列表位于同源 `/v1/models`，并使用 `after_id` 游标分页。实现依据[官方 Create a Message 文档](https://platform.claude.com/docs/en/api/messages/create)和[官方 List Models 文档](https://platform.claude.com/docs/en/api/models/list)。

## 决策

1. 在 `keelshell-ai` 中增加显式 `ProviderProtocol::AnthropicMessages`。协议由不可变的 `ProviderConfig` 绑定，不能根据模型名或响应内容推断。
2. Anthropic 请求只发送 `x-api-key`（如果用户选择认证）和固定的 `anthropic-version: 2023-06-01`，不发送 Bearer `Authorization`。没有认证时仍发送版本头。
3. 请求预览包含固定安全系统提示、一个 `user` 文本消息、`model`、`max_tokens` 和 `stream: false`。默认输出上限为 4096；独立的 `prepare_with_max_tokens` 只接受 1 到 1,000,000 的有界值。设置页本轮继续拒绝尚未接入的高级 Token 配置，避免保存后静默丢失。
4. 回复解析只接受顶层 `role=assistant`，只把 `text` 内容块展示给用户。`tool_use`、`thinking` 和未知块不会转成文本；没有文本的回复失败。工具调用不会进入命令审阅或执行路径。
5. 模型发现只替换显式 `/messages` 后缀为同源 `/models`，最多跟随 64 页、累计 4096 个模型 ID。服务端声称还有下一页但没有新游标、重复游标或超过边界时失败，不返回静默截断的目录。
6. core 配置只接受 Anthropic 下的 `x-api-key` header 和无认证，持久引用仅允许凭据库引用。Bearer、环境变量引用、其它 header、代理、自定义 header 和未实现高级参数继续拒绝。
7. 凭据库将 Anthropic 绑定编码为 `header:x-api-key`，并将 endpoint、协议和配置 ID 纳入既有加密绑定。改地址、改协议、改认证或解除关联会清除当前进程的临时引用。

## 验证

- `keelshell-ai` 单元测试验证精确预览、默认/自定义 token 上限、协议路径和文本块解析。
- loopback HTTP 测试验证请求体等于预览、`x-api-key` 与版本头、无认证仍发送版本头、tool/thinking 不会变成文本、分页游标、缺失游标、大小上限、取消和超时。
- `keelshell-core` 测试验证 Anthropic profile 的激活、认证矩阵和 legacy projection 拒绝。
- `keelshell-app` GPUI 测试验证协议切换、`/messages` 后缀、x-api-key 配置、助手预览和加密凭据保存/解锁。

这些测试不证明商业账户可用性、真实模型质量、供应商计费行为或 Windows/Linux 原生窗口交互。
