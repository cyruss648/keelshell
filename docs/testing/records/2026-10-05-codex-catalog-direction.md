# 对外 MCP 方向与 Codex 目录诊断 — 2026-10-05

状态：MCP 仅由 KeelShell 向外部智能体提供能力。新的实际 Codex 0.160.0 默认拒绝入口实验已观察到七项工具名称，但完整目录准入、授权桌面读取及审阅闭环仍未通过。生产代码没有在本次诊断中修改；原失败保持原始字节和结论。

## 产品方向

应用内 API / 本地 CLI Ask 为 KeelShell 提供推理后端；对外 MCP 服务让外部客户端访问用户明确授权的 KeelShell 会话能力。两者独立，不开发通用第三方 MCP 客户端。默认关闭，终端片段、目录和工具范围由桌面用户授予，命令由客户端提交提案、桌面人工审阅。当前七项能力见[接入指南](../../product/EXTERNAL_MCP.md)。文件修改提案在独立工作树开发，尚未整合或验收，不把本次七项目录实验当作新工具验收。

## 只读诊断与启动前复核

原 pass2 请求的 46,780 字节原始 body SHA 为 `44368b3080b15e69e9f437c31f4be3c607449882cc9b47792258ad1ef5fb0753`。没有顶层 `tools`，工具定义位于 `input` 的 `additional_tools`；该请求没有 KeelShell namespace。旧脚本仅检查顶层工具的假设不适用于这个载体。

版本锁定的 [Codex rust-v0.160.0 源码](https://github.com/openai/codex/tree/rust-v0.160.0) 显示：该模型的 ToolMode 优先于普通 feature fallback；单独禁用 `code_mode` 不覆盖模型模式。诊断采用 typed `features.code_mode` 的 `direct_only_tool_namespaces` 和 `agents.enabled=false`，保留其余 21 个禁止开关、只读文件权限、网络限制、独立 HOME、七项工具过滤及 `required=true`。这些字段的公开说明见[官方配置参考](https://learn.chatgpt.com/docs/config-file/config-reference)。原 `required` 已开启，未把 startup grace 当作旧目录缺失原因。新组合的结果不证明哪一项单独造成旧缺失。

探针只接收首笔自有回环模型 POST，始终返回固定 HTTP 400，不发送 SSE，不提供有效桌面能力。记录器在转发前拒绝 `tools/call` 及其他非目录请求。CLI 退出 1 是这类探针的预期模型中止，仍须另行满足 schema、记录器生命周期及清理检查。

| 范围 | 实际检查与结论 |
| --- | --- |
| A 启动前 | 23 项作者离线回归通过；独立两反例证实 Python `false == 0`、`true == 1` 导致 schema 误判。限定 P2 / FAIL，未运行真实客户端 |
| B 启动前 | canonical JSON 类型比较修复；27 作者、原两断言原样重放、11 独立边界，共 40 项离线通过；限定启动前 PASS |
| B 实际 | 1.495 秒，实时二进制/签名及三代端口控制通过，features 命令不支持 `--strict-config`，未进入完整 exec。缺少无条件最终 HTTP snapshot，不能凭缺文件写成精确 HTTP0 |
| C 启动前 | 仅 features 命令去 strict，真正 exec 保留；补无条件 POST 尝试/捕获/错误计数。31+2+11+9，共 53 项离线通过；限定启动前 PASS |
| C 实际 | 4.170 秒，features 22 项 false，启动真实 strict exec。假 IPC 端点没有认证应答，companion 认证 deadline 后退出；仅 initialize 请求，没有服务端响应。实际 POST0 / capture0 / handler-errors[]，PROBE_ABORTED |
| D 启动前 | 仅实际 companion 子环境移除三项能力/模型令牌变量，父与记录器保留生成 sentinel，使用既有默认拒绝入口。34+原2+旧11+旧9+根6，共 62 项离线通过；根为非作者复核，未取得 fresh 子 agent D 结论 |
| D 实际 | 1.903 秒，initialize 和 tools/list 真实完成，POST1 / capture1 / handler-errors[]，实际七名称出现。仍 PROBE_ABORTED：缺最终 owned_exit，六项原 schema 与请求不完全相同 |

独立 A / B / C 证据分别为 31 / 38 / 59 项 indexed 文件，manifest SHA 为 `90ff808901f3110844f507b22d0aca2533fb7040fc7961283ccacc08efb2e3f0`、`419bc5b566781fbf5f8f5f47ca7e96ec65804f0aba03ae06e1d23f7c70e8f538`、`27f72bf1b5ac9e940d7b3a9b4e3d170595992168557ab1e205b1fc4deed903a4`。根逐项核验并复制。D 根预审 40 项 indexed 文件，manifest SHA `8d4c04007770c0801c05618fecaf679f603ae2d702e047a064870e01303a7d31`；该根复核不冒充 fresh 子 agent。协调工具的 thread limit 阻止了安排新 reviewer，不是自动审批拒绝或测试失败。

## D 的实际观察

Codex 为已安装 0.160.0，241,555,024 字节、SHA `112fae7a5a1223e673c8a1791d32338f37df8b527ff1159bb8adac6c4dbf1b4b`，每次实际运行重新核验严格签名。companion 为旧 a19 MAC15 开发包，16,651,968 字节、SHA `6bb5ae18a752cbf7ef1d38dee8f47160254705f11356d879d9136c43e17c6823`，没有宣称新 233 原生包或 Release 验收。

D 原始首笔 body 40,352 字节，SHA `fea6667b82a8126f4668c0123a5a996db3402ec4d9838c159ad399a9dd09aa44`。七项名称均在 `mcp__keelshell` namespace 中，以非 deferred function 出现；另五项客户端内置定义只作名称/schema SHA 观察，没有批准或调用。模型始终收到固定 400，SSE0、业务 `tools/call` 0，没有 SSH 会话或桌面 grant。

记录的单调时间：tools/list 响应 `311741803677666`，首 POST `311741817830125`，目录响应先于 POST。不能由此声称观察到了 CLI 内部 finalcatalog。严格原 schema 比较中，只有会话列表相同；其余六项的 UUID `format`、数值 `minimum/maximum`、字符串 `minLength/maxLength` 被移除，属性/类型/required 保留。本次没有为得到通过而放宽比较，后续必须核对版本对应的上游转换，分别绑定原 MCP schema 和模型可见 schema。

Codex 实际 exit1，观察身份、进程组、端口、私有 TMP 和 HTTP 线程的清理通过；wire 没有记录器最终 owned_exit，因此完整生命周期准入未通过。立即清理可能抢先于自然 EOF 收尾只作代码支持的候选解释；后续须有界观察和新证据，不追溯修改 D。

## E 的准备与独立拒绝：上游转换、进程出生身份

E 仅完成离线准备，尚未执行真实客户端。版本对应的官方源码证实转换链：MCP schema 经过解析进入 typed `JsonSchema`，再序列化为 Responses 参数；该类型没有上述五个约束字段，而 Serde 默认忽略未知字段。证据为 [MCP 解析](https://github.com/openai/codex/blob/rust-v0.160.0/codex-rs/tools/src/mcp_tool.rs)、[typed schema](https://github.com/openai/codex/blob/rust-v0.160.0/codex-rs/tools/src/json_schema/types.rs)、[Responses 转换](https://github.com/openai/codex/blob/rust-v0.160.0/codex-rs/tools/src/responses_api.rs)及 [Serde 官方说明](https://serde.rs/container-attrs.html)。这解释 D 已观察到的六项原 schema 差异，不改变 D 的失败结论，也不证明旧授权请求为何缺少工具。

新比较只允许已证实的五项字段转换，保留原始与适配后 schema 的独立 SHA、删除路径和值、原始精确比较结果。UUID 格式、有限数值、非负整数长度仍作类型检查；同名属性、类型、required、名称、重复项和未知变化保持严格验证。适配匹配使用独立状态 `SCHEMA_ADAPTED_CATALOG_OBSERVED`，不记作 raw exact。未证实的 sanitizer、裁剪、compaction 或其它关键词转换仍拒绝。

自然 EOF 函数最多等待两秒，只用于没有先前错误、预期 exit1 的目录调用，计入原十秒清理预算；其它执行默认不等待。纯 grace 函数的出生身份检查通过，但不能弥补调用方的所有权缺陷。作者 47 项选中离线测试和 12 个 Python 语法解析通过，36 个冻结输入的 manifest SHA 为 `17580cc871f1832d1973b63bd37264ffbdaf9f72020fd1c81f2f5bbd99cd63a8`。

新的非作者独立预审结论为 **FAIL，剩余 1 项 P1 / 0 项 P2，E 没有实际执行**。进程扫描以历史 PID 传播所有权，并在 executable 路径变化时替换原 identity，未先核对 birth。两项原函数 mock 反例令 PID456 被不同 birth/PPID/PGID 的进程复用且产生 child789，默认 grace0 和新 grace2 均记录 TERM 到这两个不属于测试的身份，最终 errors 仍为空。没有发送真实信号；不据此断言旧实际尝试发生过误杀，也不把新增 grace 当作根因。

独立实际离线检查共78项选中、76通过、2项上述反例失败；原47、旧2/11/9及新其余7通过。reviewer 初次选择空 required 数组的自有假失败另行保留，不列为产品缺陷。36项E输入和旧B/C/D的31/45/44证明逐bytes/SHA前后不变。71项独立proof经根核验复制，manifest SHA为 `c1a1422ad5fe0fe2eedb1beba388754fe5302a0b20c8703775a7a2bea566bda0`。这是工程预审拒绝，不是自动审批拒绝或缺少用户授权。新F只修固定出生身份、有效关系种子与复用PID不认领，必须原样重放两项反例并重新独立预审，才考虑限定目录探针。

## 保留与未验收边界

原诊断 29 项文件已核验复制。实际 B 的 31 项和 C 的 45 项输入/结果清单保持；原 A schema 失败、B features 错误、C 认证 deadline 和 D 原记录均不覆盖、不重跑同一 scope。材料在 ignored `work/codex-mcp-*20261005*`，目录 0700、文件 0600，记录不包含有效能力、API key、模型 Authorization header 或用户配置。自有监听及临时数据已清理，未改变系统代理、用户 CLI 配置、现有应用或容器。

新的实际七名称观察不能关闭完整目录准入、Codex 授权 SSH/SFTP、桌面批准/拒绝、运行中撤权、新文件修改提案或其他平台原生验收。原 Claude 的有限授权通过仍按其[独立记录](2026-10-05-claude-authorized-mcp.md)报告，旧 Codex 的失败仍见[原记录](2026-10-05-codex-authorized-mcp-failure.md)。
