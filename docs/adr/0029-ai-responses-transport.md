# ADR 0029：显式 Responses AI 传输

日期：2026-10-04。状态：接受；实现与验证见[测试记录](../testing/records/2026-10-04-ai-responses.md)。

## 背景

AI 配置模型已经区分 Chat Completions、Responses 和 Anthropic Messages，但
此前只有 Chat Completions 能实际请求。把 Responses 配置标记为可用却仍发送
旧格式会造成目标地址、审核预览和实际请求不一致。

## 决策

1. `keelshell-ai` 增加明确的 `ProviderProtocol::Responses`，由配置快照绑定协议；
   URL 不通过猜测推断协议。
2. Responses 预览只包含 `model`、`stream: false`、固定安全说明
   `instructions` 和经过脱敏/字节限制的 `input`。不启用 tools、function calls、
   远程文件或自动执行。
3. Responses 回复只接受 `output` 中 `type=message` 的 `output_text` 内容；模型
   标识和回复长度仍执行现有边界与脱敏规则。
4. `/responses` 与 `/chat/completions` 都只能按字面后缀推导同源 `/models`；
   其它路径在发起网络请求前失败。
5. 设置页提供中英文协议选择。切换已知后缀时同步替换路径并清除临时密钥；
   自定义未知路径保持原样并要求用户自行审核地址。Anthropic Messages 继续显示
   为尚未接入，不能静默降级为 Chat Completions。

## 验证边界

单元测试、回环 HTTP 测试和 GPUI 设置测试证明了精确预览、协议路径、响应解析、
认证与旧密钥清除。没有使用真实付费服务或上传终端上下文；这不证明任意供应商的
Responses 兼容性、模型能力或计费行为。
