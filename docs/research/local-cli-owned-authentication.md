# 本地 CLI 登录身份研究 — 2026-10-07

状态：尚未实现 CLI 登录身份复用。已实现的本地 Codex CLI／Claude Code 调用
使用明确配置的 API 身份。它们与 KeelShell 向外部智能体提供的 MCP 服务
分别设计、分别验收；本研究不增加通用第三方 MCP 客户端。

## 明确的最小设计

默认保持 API 模式。登录候选只由用户明确选择非秘密状态目录与凭据存储
类型，原版 CLI 自己读取和刷新认证。KeelShell 不读取、复制、记录 token，
不自动登录；CLI 可能更新自己的状态需在审核中说明。认证模式、目录、
存储、模型或可执行文件改变，撤销旧审核与运行。目录身份不代表精确账号
绑定，Ask 与 Agent 共同遵守请求、上下文和远端动作的人工审核。

## 已观察的精确版本与缺口

本机 Codex 0.160.1、Claude Code 2.1.285 的九次限定版本／帮助调用已实际
结束。随后六次 Codex 探针只使用自有空目录、无认证文件与 key，运行于
网络拒绝 sandbox；单次 4.5 秒加最多 0.5 秒 reap，输出各限 64 KiB。
每个自有进程组最终不存在、临时目录移除，没有模型、登录或真实账号调用。

| Codex 自有探针 | 实际退出码 | 可以证明的范围 |
|---|---:|---|
| 全局 ignore-user-config 与 features list 的 parser | 2 | 当前版本拒绝该组合 |
| 非法自有配置下的 features list | 1 | 此路径会读取用户配置层 |
| 空配置与合法 marker 配置的固定 features list | 各 0 | 当前 18 项均 false、输出相同、marker 未执行 |

这些成功观察不证明 exec／SessionStart 的副作用隔离。官方说明 exec 的
ignore-user-config 跳过用户配置而继续使用 CODEX_HOME 认证；管理配置还会
结合账号策略，并能归一化 pinned features。空状态的独立 preflight 不能
替代同一真实执行进程的有效策略准入。[CLI 命令说明](https://learn.chatgpt.com/docs/developer-commands)，
[管理配置](https://learn.chatgpt.com/docs/enterprise/managed-configuration)。

目前缺少已验证的契约：在实际认证／策略范围内，SessionStart hook、MCP、
插件或工具副作用发生前，拒绝所有与 Ask 隔离要求冲突的能力。没有因此
添加占位 UI 或放宽发送准入。后续需精确版本的执行前策略接口或同等证据；
这里不推断供应商永远无法支持。App-server 可另行评估，不能直接移植 exec
标志或借架构变化省略隔离验证。[App-server 说明](https://learn.chatgpt.com/docs/app-server)。

Claude 的原版 CLI 用户身份场景与第三方产品登录／转发场景须分别确认。
官方同时描述原版程序的用户自有身份和第三方集成约束；本项目不据此概括
禁止全部本地订阅调用，也不声称内置 Ask 已获适用确认。现有 API 路径保持，
登录候选继续开放。[官方集成范围](https://code.claude.com/docs/en/legal-and-compliance)。

## 证据与后续验收

初始只读包为 44 个 payload／569,479 字节，manifest SHA-256
0b31c2e23e28502189466eae229ee4c4ae133a58e45e1406c154791a37d57e99。
隔离补充为 20 个 payload／74,695 字节，manifest SHA-256
841c93d1ce8200d9ebd34f3568edbbc8f06f90ccd5721ffd31d15a752e5cff74。
根完整读取并保留源码、实际输出、wait 与清理记录；原解析拒绝不改写为
产品故障。无生产代码变更。完整材料位于忽略的整合 work/ 证据。

执行前隔离成立后，再验证旧 API 资料兼容、模式变更撤权、无登录／错误
存储／账号变化、实际供应商请求、取消与迟到结果，以及三平台 credential
store 和原生桌面。当前帮助、非会话 canary、API loopback 或外部 MCP
历史成功均不能关闭这些项目。
