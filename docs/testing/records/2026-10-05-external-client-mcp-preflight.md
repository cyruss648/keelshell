# 对外 MCP 的真实客户端前置与未授权拒绝 — 2026-10-05

状态：本记录的真实 Claude Code 配置/schema/未授权默认拒绝闭环已执行通过，fresh独立复审通过，无剩余P1/P2；证据范围措辞P2由追加更正关闭，原材料不改。下文 Codex 失败与未授权范围属于历史冻结切片。后续新的 Codex 文本对照已成功，Claude 授权 SSH/SFTP、桌面批准/拒绝及撤权客户端结果循环也完成；新独立复核已通过该限定范围，无剩余 P1/P2，精确范围见[授权原生记录](2026-10-05-claude-authorized-mcp.md)。其他平台、Codex MCP 与完整供应商/云模型互通仍开放。本记录不修改生产程序，也不把确定性回环模型当作云模型验收。

## 用户要求与服务方向

KeelShell 提供 MCP **服务端**，让其他智能体访问用户在桌面中明确授权的能力。
应用调用本地 CLI 作为 Ask 推理后端与对外 MCP 是两个入口；内部桌面 IPC 客户端只连接 KeelShell 权威端。
不开发应用内访问任意第三方 MCP 服务的通用客户端。
七项已实现工具、目录/会话授权与人工审阅见[指南](../../product/EXTERNAL_MCP.md)，后续任务见[正式计划](../../product/DESIGN_AND_AGENT_PLAN.md)。

本轮验收只使用已存在的生产 companion，没有 GUI、SSH、SFTP、桌面授权或临时能力。
未设置 `KEELSHELL_MCP_ADDRESS` / `KEELSHELL_MCP_SECRET`，真实主程序进入默认 disabled/disconnected 服务。
模型 fixture 只发送确定性的工具请求并检查实际客户端回传，不能自己调用 MCP、伪造结果或替用户批准命令。

## 源码与进程绑定

源码基线为 `896073ad81e5f9868e6f5eb6fe8c4fcc0f7df348`；其三平台源码 CI 成功见[观察记录](2026-10-05-windows-forward-observer.md)。
此次新增支持代码、模型/stdio 记录器和结果均为 ignored 私有验收材料，不改 Cargo、依赖、生产源码、用户配置或应用安装。

| 实际执行对象 | SHA-256 |
| --- | --- |
| Codex 0.160.0，241555024字节 | `112fae7a5a1223e673c8a1791d32338f37df8b527ff1159bb8adac6c4dbf1b4b` |
| Claude Code 2.1.285，223821616字节 | `51f09bd1e021d9fa8a1864c179799bd37cb39962a937935c5cf6823398e86db4` |
| 既有 KeelShell companion，16639744字节 | `069bbb239b5382f5c5203172ffd9fe080f7dc124c1fe1ab2de2e7d2a3477f53d` |

两个安装版 `codesign --verify --strict` 通过。companion 在现有开发 target 与标准开发包中逐字节 hash 一致，执行前后保持。
这证明所用 artifact 的身份；没有重新编译标准包，也不将旧包的 source/commit 归属改写成新包或 Release。

## 精确回环端口与文本前置

每个场景采用自己的空 workspace/home/config/cache、显式 child env、匿名 stdio pipes、无继承代理和明确 dummy model credential。
credential 只在内存与子环境中使用，HTTP header/auth值不保存；CLI 自写临时状态结束后删除。
进程网络策略只允许连出该场景自有模型端口；不增加 bind/inbound/Unix socket 例外，不调用公网推理。

新端口每次重新完成三代真实 fork/exec 负控，第三代另建 session：每代一项自有 HTTP/TCP 往返及八项其他回环端口、文档v4/v6 TCP/UDP、显式loopback/wildcard bind 的内核 `EPERM`，必要27项通过。
额外三项同模型端口 UDP 动作逐阶段记录为 sendto允许、实际回包recv得到EPERM，不并入27项，也不称为UDP往返通过。
超时、连接拒绝或配置声明均不当作内核拒绝证据。

| 安装版文本前置 | 实际结果 |
| --- | --- |
| Claude Code | 严格空MCP、内置tools为空、七项精确allowedTools参数、dontAsk、无permission prompts与session持久化；唯一一次Messages POST包含自有nonce、tools=[]，完整SSE与精确答复回到客户端，exit0 |
| Codex | 网络限制下实际版本与22项effective false feature通过；保留strict和工具/配置限制的exec失败，exit1，模型fixture收到0个POST，stdout为五条有限reconnect及generic Connection failed/turn.failed；没有controller重跑 |

Codex缺少直接socket errno或到达模型的请求，不能把generic连接错误归因为OS拒绝、具体参数或产品缺陷。
`features list` 实际不支持strict；该非推理子命令只读回有效配置，strict仍留在exec。
此修正与原UDP预期/阶段解释错误分别冻结；前者不被扩大成所有子命令的约束结论。

文本前置108份文件经根逐bytes/SHA与0600权限核验，manifest为
`1d6c08983c4dbe3630131a3007a4e48b87155e8aa7f532d27c6e4315f71396a1`，
报告为`3b998f86073a251ba4d479e01d6b3241e35c04716ad30b91b561729349857114`。
最终文本场景profile为`273468b0a654d16c462ff9308e5aff4d23989859901f0f57fcbbc3171a3b3676`。
26个唯一已观察owned PID/birth全部消失，七个scratch实际删除，监听/线程/stdio与PGID清理。
这一步尚没有任何MCP入口或工具循环；Claude前置成功与Codex失败分别保留。

## Claude 的真实未授权工具循环

另一个新端口/私有scope只执行已通过文本前置的Claude。
临时strict MCP配置只有一个KeelShell stdio entry与七项工具白名单；透明记录器只在真实转发后记录byte/JSON，不生成RPC或响应。
实际记录器父进程的CLI路径/SHA、companion进程SHA与执行对象相符。

1. 真实Claude发送一次initialize、一次tools/list，协商实际生产server的七项工具。
2. 第一Messages请求中七项input_schema与真实tools/list的inputSchema逐对象相等，模型据实际schema选择list_sessions。
3. 真实Claude发出唯一一次 `keelshell_list_sessions({})`，MCP request id为2。
4. 真实companion返回isError=true及structuredContent.error.code=DISABLED，没有读取成功。
5. 同一Claude的第二Messages请求携带相同tool_use_id、is_error=true，正文与server structuredContent逐对象相等。
6. 模型只在实际看见该回传后给固定最终答复；CLI最终result/success、精确答复及exit0一致。

恰好两次模型POST、一次MCP调用，CLI0.833秒。这里只执行list_sessions的默认拒绝，七个工具的schema协商不等于七个工具动作均验收。
本场景profile为`290756bb1f89babedfdcad4f49389b678ccf8c28fb59617b7b01f58336f16b22`。
真实CLI的init元数据包含builtin `cc-plugin-agents-md`；模型请求的实际工具仍恰好为七项KeelShell schema，没有额外shell/file/browser或其他MCP schema。
不能因此宣称所有插件不存在或它们的任何外部能力已被证明关闭。

作者80份冻结材料经根逐bytes/SHA与0600权限核验，manifest为
`2c1d5c470751a6da26749cffea5e3d9cb08d10edc083fef638c87d91142dfaff`，
原报告为`da53f2843cf116a588eee65bd9c064d2e87bc27c5b489f6475f2b652ce01f5a9`。
原报告的插件范围措辞由独立复审指出P2；原80份保持冻结，另加范围更正与新manifest，根再次核验。
addendum为`da5251113ffbd5c48630c70049b1601dbc2aae7c10507df88a1effdf00e4e38a`，
amendment manifest为`3db3ec7fdbe210dd3aa92808f77f7b0362f2e0c4c1fdf4b91bbc4962c334b6e5`。
更正说明优先于原报告，不改原80份或重复执行；P2据此关闭。

fresh独立只读复审无剩余P1/P2，13份材料经根逐bytes/SHA与0600权限核验；
manifest为`47c88e40f94eb0fa3519bea8f22ac32578fc7152af6180478c52fd66c7fbf5d6`，
报告为`14068dfba84d0e5e3c5aa478e070ea9050361f15a1f823e0bb9e6f8e33de256e`。
复审由原JSON+newline逐SHA/bytes重建7条实际MCP frame及两条真实HTTP body/SSE，
验证initialize/list/call各一次，唯一request id2、同tool_use_id与同DISABLED对象在server、CLI stdout和下一模型请求中一致，
两请求的14个schema/description逐对象匹配真实catalog（七项每请求各一次，不相加为14项不同工具）。
原source/config256 hash、七份helper与两个binary保持；首次失败及范围更正独立保留。

## 失败保留、清理与证明边界

首个未授权controller遗漏自有UDP reply canary：必要27项通过，但额外UDP观察超时，未启动CLI/MCP。
下一独立attempt只补回该canary，保持profile、断言和期限；原失败不覆盖。
另一次credential扫描误把源码generation prefix当成真实值，保留诊断，按完整生成形状扫描实际值为零；未改冻结源码或执行结果。
五项synthetic helper负控另记，不相加为真实客户端场景。

未授权场景两个attempt的11个唯一已观察owned PID/birth最终全部消失，四个TCP旧端口连接拒绝、四个UDP端点可复绑；thread/drain/PGID与scratch已清理。
policy scratch名称未持久化，其清理由controller的实际scratch_remaining=[]回执支撑；独立当前逐路径回查仅覆盖已记录的CLI scratch根，不能称已逐路径复查全部policy临时根。
wire没有完整owned_exit尾帧，不能推断companion退出码；清理以外层PID/birth实际回读为依据。
50ms有界census只能核验已观察到的身份，不能证明任意瞬时后代不存在。
网络profile的allow default不提供filesystem隔离；这里没有XPC或全部系统委托通道审计。

本记录可关闭的范围只有真实Claude的配置/schema/未授权默认拒绝循环。
Codex MCP未执行；真实GUI授权SSH/SFTP、七工具授权动作/越权拒绝、待审提案与人工批准/拒绝、撤权/重启及Running撤权继续验收。
Windows/Linux原生、真实云模型效果、客户环境、六目标正式Release、安装与自动更新也保持开放。
