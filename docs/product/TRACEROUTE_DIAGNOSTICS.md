# 审核式有界远端 traceroute / Reviewed bounded remote traceroute

本文描述 2026-11-06 冻结 r3 的源码契约。旧 r2 曾获
`APPROVE_FOR_SOURCE_ONLY`，随后真实工具发现版本输出流失配，r2 不作为发布候选。
r3 修正该流契约，并在 2026-11-06T04:04:14Z 获独立
`APPROVE_FOR_SOURCE_ONLY`，零 open findings。父最终 r3 工程门禁、
正常应用构建及 macOS 包生成/静态检查均已退出 0 并实际回收。
同包真实 macOS UI 已分别完整审核并确认 UDP IPv4/IPv6 回环，观察到目的地址一跳回复；
native-r1 非作者复核批准该观察子集；原生 ICMP/取消、Windows/Linux 桌面、
Release 与安装仍待验，完整原生保持 HOLD。
见[限定原生记录](../testing/records/2026-11-06-editor-traceroute-native.md)；
这不表示完整产品验收完成。实际专项与失败历史见
[验证记录](../testing/records/2026-11-06-traceroute-diagnostics.md)，设计见
[ADR 0112](../adr/0112-reviewed-bounded-traceroute.md)。

## 操作与授权 / Interaction and authority

在已认证的远端 SSH 会话中打开网络诊断，选择 traceroute，输入主机名或 IP，
明确检查 UDP/ICMP、IPv4/IPv6、跳数、每跳次数和单次等待，再准备审核。
审核显示执行主机、目的地、全部请求选项、固定工具/路径、argv 模板和预算。
所选 IP 族的首个远端解析地址在确认后产生，预览对此明确标注。
展开、编辑、准备审核、切换主题或语言都不派发该诊断；确认只派发一次审核请求。

Choose traceroute in an authenticated remote SSH session, enter the destination,
review the protocol, IP family and limits, then confirm the complete request.
The SSH host performs resolution and probing. Previewing and editing do not
dispatch the diagnostic. There is no autonomous AI execution.

审核绑定真实活动 profile/SSH connection、保存路线/信任、pane/session 选择令牌
和草稿快照。执行前和采用结果前再次校验；改变任一输入、切走再切回、重连或路线撤权
均不能恢复旧审核。取消、关闭或失去授权后的迟到结果不成为当前结果。
未取得可验证终止回执或收到 CleanupUnknown 时停用本诊断权限，不能直接重试不确定操作。

## 支持契约 / Capability contract

| 项目 / Item | 本片契约 / Contract |
| --- | --- |
| 执行位置 | 已审核的远端 SSH；没有本地结果替代或其他地址回退 |
| 远端前置 | Linux/POSIX，沿用现有 Python 3.8+ probe 前置条件 |
| 工具 | Dmitry Butskoy Linux traceroute 2.1.5，`--version` 退出 0、stdout 为空、stderr 精确匹配固定两行 |
| 发现 | 先 `/usr/bin/traceroute`，不存在时才检查 `/bin/traceroute`；不搜索用户 PATH |
| 协议 | 显式 UDP（传统 default 方法，初始端口 33434）或 ICMP Echo |
| 地址族 | 显式 IPv4 或 IPv6，只运行所选族解析结果的首个数值地址 |
| 跳数 / 每跳次数 | 1–16 跳；每跳 1–3 次 |
| 单次等待 | 100–1000 ms |
| 累计请求等待 | `hops × probes_per_hop × wait_ms ≤ 6000 ms` |
| 调度与报文 | `-N 1`，固定总包长 60 字节，数值输出，不做 PTR 查询 |
| 运行预算 | 远端总 8 秒；阶段共用 `started + 7.5s` 工作期限，最多 0.4 秒实际终止/wait；SSH envelope 10 秒 |
| 输出预算 | 版本、解析、探测各阶段 stdout+stderr 累计 32 KiB；SSH 响应另受既有合计上限约束 |

实际 traceroute 进程通过 `shell=False` 参数数组启动：

```text
traceroute -4|-6 -n -M default|icmp -f 1 -m H -q Q -N 1 -w S.MMM -z 0 -- ADDRESS 60
```

上式是固定分支模板。用户不能输入 flags、命令片段、URL、空白/控制字符或 IPv6 zone；
SSH 包装只携带固定脚本和编码请求，工具目标重新验证为数值 IP。UDP 端口按该工具传统
方法逐探测递增，没有可配置端口范围、批量目标、通用扫描或工具框架。

只承认该精确版本的输出契约。缺失、其他版本/实现、选项不兼容及权限不足均明确报告；
不安装、不提权、不更改配置、不尝试另一工具。版本输出是已认证远端的能力声明，
不证明二进制密码学身份。IPv6/ICMP 的真实可用性取决于远端工具、内核和权限，
不能由源码分支或离线夹具推定。

## 结果语义 / Result semantics

| 状态 / Reason | 含义 / Meaning |
| --- | --- |
| Success / DestinationResponded | 最后一个完整跳中有目的地址回复，且完整结果没有已识别错误标记 |
| TracerouteHopLimit / HopLimit | 请求的全部跳已完成，没有目的地址回复；不表示主机离线 |
| TracerouteUnreachable / ReportedUnreachable | 完整跳包含已识别 ICMP 标记，如 `!H`；保留已验证观测 |
| Timeout / Deadline | 共享阶段期限到达；只保留已完成的完整跳 |
| Interrupted | 取消停止操作；迟到结果不能恢复授权 |
| UtilityUnavailable / PermissionDenied | 工具缺失或已识别权限拒绝；没有自动安装或提权 |
| UnsupportedEnvironment | 系统、实现版本或选项不满足固定契约 |
| TracerouteFailure | 缺失、畸形、矛盾或超限输出，以及未识别进程失败 |
| CleanupUnknown | 有界终止/wait 未确认退出；撤销该诊断权限 |

星号是未知探测槽，地址和 RTT 均为空，不是零 RTT、线上丢包率或离线结论。
只采用顺序、完整、数值地址/RTT/已知标记均有效的跳；不完整末跳丢弃。
Rust 另核对请求、状态/原因、族、时序和目的地址。任意 stderr 不作为显示文本。

监督器直接拥有版本检查、Python resolver 和 traceroute 子进程，各有独立进程组；
取消、期限和输出上限触发终止及实际有界 wait。SSH 断开或取消不证明远端立即退出；
主机失联、监督器被强杀或内核拒绝清理时不能伪造回收成功。

## 最小受控回环预期 / Minimal controlled loopback expectation

在已有、受控且满足精确工具契约的 Linux SSH 夹具中，使用目标 `127.0.0.1`、
UDP/IPv4、1 跳、每跳 1 次、等待 1000 ms；累计等待 1000 ms。
先准备/取消审核应不派发，再重新审核并确认。实际工具 argv 应为：

```text
/usr/bin/traceroute -4 -n -M default -f 1 -m 1 -q 1 -N 1 -w 1.000 -z 0 -- 127.0.0.1 60
```

路径可为固定发现规则选出的 `/bin/traceroute`。在夹具 UDP 33434 未占用、
回环未被过滤的条件下，预期一个完整 `ttl=1`、一个 `127.0.0.1` 回复槽、
真实测得 RTT、无错误标记，状态为 Success/DestinationResponded。
不得固定或编造 RTT。星号、拒绝、工具缺失等结果必须按真实状态报告。

这是 SSH 审核链的受控运行预期，作者没有执行该真实 SSH 回环。
父另有下述生产 Python 回环原件；它不代签 SSH 审核链、外部路由、
完整取消回收或原生 UI。

## 上游与验收 / Provenance and acceptance

参数和输出依据为 SourceForge 上游 `traceroute-2.1.5.tar.gz`：
SHA-256 `9c6c260d96eaab51e3ce461b0a84fe87123ebc6dd6c9a59fab803f95b35a859e`。
保存的 `traceroute.c`、`mod-udp.c`、`mod-icmp.c`、`traceroute.8` 和版本文件用于
核对固定选项、header、probe、错误标记和版本输出；原件及来源在本地
`work/reviewed-traceroute-20261106/upstream-provenance.json`。

r2 通过的 17 个新增 Rust 测试及 37 个旧回归包含受控 loopback SSH、
离线真实子进程和 GPUI 控制器；4 个既有环境测试保持 ignored。
这些是历史 r2 回执，不是新 r3 Rust 门禁。r3 离线 Python 专项 14 项通过，
包括 stderr 版本正控及 stdout-only、双流、跨流拆分、多余字节、非零退出、
其他版本/实现负控；不拼接两条流、不做 substring 或宽松版本匹配。

父的受控真实工具原件确认：`--version` 退出 0、stdout 0 字节，
stderr 为固定两行共 110 字节。另有父 raw-tool 回环观测：UDP IPv4/IPv6 各一跳，
ICMP 在非特权夹具中为已识别权限拒绝；这只证明该镜像的原始工具行为，
不是 SSH→生产适配器→UI 或原生验收。r3 作者未运行 Cargo、真实工具或原生界面。
父最终组合门禁和 macOS 包结果如下；后续原生及发布验收由父另行续记。

父后续用精确生产 `SCRIPT = icmp + trace + probe` 和实际 worker，在同一受控
Linux 镜像的 network none、UID 1001、只读且无 capabilities 环境完成 red/green：
旧版本流判定返回 UnsupportedEnvironment；r3 的 UDP IPv4/IPv6 均为一跳
Success/DestinationResponded，ICMP 为 PermissionDenied。进程实际退出并回收，
容器已清理。该证据覆盖生产 Python 适配器与 worker，仍不代签 Rust 解析、SSH、
审核 UI 或原生验收；精确 RTT、期限、hash 和原件位置见
[验证记录](../testing/records/2026-11-06-traceroute-diagnostics.md#parent-production-script-redgreen-follow-up)。

父最终 `full-gate-r3` 绑定组合冻结
`1b5c7cf42d8a4d9ede5de6fcca29caf11da5daeaef59fc8e580ea4a1172590bd`：
2355 个 ordinary 测试通过、23 ignored，14 个 rustdoc 通过；
门禁 Python 运行 131 项，130 通过、1 skipped。app 子集为 1047 通过、2 ignored。
42 个组合新增用例（含本片 17 个）及 204 个选定基线用例均按具体名称确认通过，
这些是门禁总数的子集。上述 14 项 traceroute Python 专项不另累加进门禁总数。
父正常 debug app 的 SHA-256 为
`9e83d83af253ffe3c3aba29997450ea75f4c70a6dbe9311e61469228bd0158b9`；
macOS aarch64 包清单记录同一 hash。构建、打包和 `inspect_native.py`
均为退出 0/实际回收，静态包检查不代签原生交互或安装。

The final combined r3 engineering gate, normal application build and macOS
package generation/inspection passed with actual exit and reap receipts.
The isolated source approval remains source-only. Native UI, Windows/Linux
platform acceptance, Release and installation remain pending in this record.
