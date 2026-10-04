# AI Responses 传输验证记录

日期：2026-10-04

## 覆盖范围

- `ProviderProtocol::Responses` 绑定到不可变 ProviderConfig 与审核预览。
- `instructions`、`input`、模型和 `stream: false` 的请求体；没有 `messages`、tools
  或自动操作字段。
- 回环服务使用真实 `/v1/responses` 路径，验证精确请求字节和 `output_text` 回复解析。
- 异步 ProviderClient 的连接测试使用 `input`，并读取可选服务端模型标识。
- 设置页 Chat Completions/Responses 切换、已知 URL 后缀替换和临时密钥清除。
- `/responses` 模型目录推导与未知后缀拒绝。

## 命令与结果

```text
cargo test -p keelshell-ai --locked
29 unit tests passed
9 provider HTTP tests passed
16 discovery HTTP tests passed
1 doctest passed

cargo test -p keelshell-core --locked ai_profiles -- --nocapture
7 ai profile unit tests passed

cargo test -p keelshell-app --locked ai_settings::tests -- --nocapture
10 AI settings GPUI tests passed
```

覆盖使用本机回环 fixture，不读取真实密钥，不连接付费服务，也不代表 Windows/Linux
原生桌面验收或所有供应商的协议兼容性。完整工作区门禁（应用 266、核心 69、会话库 64、
批量集成 14、SSH loopback 95，OpenSSH 外部互操作 6 项因缺少环境而忽略）已通过；跨平台
Quality [37176372172](https://github.com/cyruss648/keelshell/actions/runs/37176372172) 已在 macOS 26、Ubuntu 24.04 和 Windows 2025 成功；这证明构建、测试和打包路径，不等于 Windows/Linux 原生桌面交互或真实供应商兼容性验收。
