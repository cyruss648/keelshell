# MON/NET-02：显式审核式 ICMP ping / Reviewed remote ICMP

切片始于 2026-11-05，基线 `dcf7438a13fff737997ee5e8061044c94d4ba53a`。
此指南描述已合入代码的能力契约；来源历史见
[作者记录](../testing/records/2026-11-05-icmp-diagnostics.md)，
组合门禁、真实probe及仍未完成的Rust/UI/原生边界见
[整合记录](../testing/records/2026-11-06-reviewed-diagnostics-integration.md)。

## 操作与授权 / Interaction and authority

在已认证远端 SSH 的协议诊断中选择 **ICMP ping**，输入主机名或 IP，先预览，
核对执行主机、目标和固定计数为四的 Echo 操作，再点击确认。展开面板、输入、切模式、
预览、切换语言或主题均不启动 ICMP。DNS/TLS/HTTP 保留各自语义；
TCP 连接测试不会被称作 ping。没有定时、批量、AI 或遥测入口。

Choose **ICMP ping** in an authenticated SSH host's protocol diagnostics, enter
a host or IP, preview the exact host/operation with the fixed tool count of four,
then confirm. Opening, editing,
previewing and changing language or theme do not dispatch ICMP. This operation
has its own meaning alongside DNS/TLS/HTTP and the existing TCP connection test.

审核绑定捕获的真实 SSH 连接、保存路线/信任、操作与原始输入快照。
规范化后等价的输入变化以及改回原值也必须重新审核。输入变化会撤销正在进行的
结果采用；执行前与结果采用前再次校验，取消、关闭、重连和路线撤权不能采用晚到结果。
调用方提供的凭据仍只属于原 SSH 连接，不能出现在 ping argv、原因或普通日志中。

2026-11-06父侧已接入独立的活动叶/标签选择UUID和同步撤权，切走再切回不能复活
旧审核或迟到结果；再次选择同一标签和关闭非活动标签保留当前审核。
确认和结果采用另查当前选择，普通切换保留SSH连接供新预览。
不改变Agent等拥有者共享的会话有效性规则；设计与组合验收分别见
[ADR0106](../adr/0106-diagnostic-selection-consent.md)及
[整合记录](../testing/records/2026-11-06-reviewed-diagnostics-integration.md)。

## 明确支持边界 / Explicit capability boundary

| 项目 / Item | 契约 / Contract |
| --- | --- |
| 执行位置 | 只在用户审核的已认证远端 SSH；没有本地网络替代结果 |
| 远端系统 | Linux，POSIX，Python 3.8+ 及现有标准库 probe 前置条件 |
| 工具 | 仅 `/usr/bin/ping`、其次 `/bin/ping`；不搜索用户 PATH，不安装，不提权 |
| 实现 | 只接受 `ping from iputils 20240117` 及该 release 的完整六项 `-V` 配置行 |
| 地址 | 系统解析器的首个数值 IPv4/IPv6 地址；只执行一次，不回退到其他地址或工具 |
| argv | 固定 `ping -4/-6 -n -q -c 4 -i 1 -s 56 -W 1 -- NUMERIC_ADDRESS`，`shell=False` |
| 环境 | ping 使用固定 `LC_ALL=C`、`LANG=C`、`LANGUAGE=C` 和系统 PATH |
| 预算 | 远端总 8 秒，运行到第 7.5 秒即收尾，最多 0.4 秒实际 wait；SSH envelope 10 秒 |
| 输出 | 各子阶段 stdout+stderr 累计 32 KiB；SSH 响应另有既有 32 KiB 合计上限 |

其他 iputils 版本、发行版自定义版本串、BusyBox、BSD/macOS、远端 Windows、
未知版本/配置输出都明确返回 Unsupported，不使用 `--help` 推测参数是否安全。
版本串属于已认证远端的能力声明，不能证明 executable 的密码学身份；
此适配器不是对恶意远端二进制的沙箱。IPv4/IPv6 文本契约都有离线夹具。
2026-11-06另在自有Linux容器完成OpenSSH CLI携带精确生产Python命令的
双栈回环probe：127.0.0.1和::1各四次工具尝试/四个回复，实际工具为iputils20240117。
这只覆盖CLI→生产Python/iputils，不代签Rust传输/解析、审核UI、外部路由、
真实错误/取消矩阵或各平台原生验收。

主机输入只接受受限 DNS 名称或完整数值 IP。拒绝 flags、空白、控制字符、shell
片段、IPv6 zone、URL、userinfo/query/fragment。传给工具的目标只能是解析后的数值地址。
`-n` 禁止 PTR 查询。一次解析选一个地址；结果标明该地址，不能当成整个 DNS 地址集的健康。

## 结果语义 / Result semantics

| 状态 / State | 含义 / Meaning |
| --- | --- |
| Success / Completed | 工具退出与已验证 summary 一致，工具计数发送尝试 4、唯一 Echo 回复 4，且没有报告 ICMP 错误或损坏回包 |
| IcmpPartial / PacketLoss | 工具计数发送尝试 4、唯一回复 1–3；差额是未获回复的尝试，可能包含静默本地发送失败，不能称为已测得线上丢包 |
| IcmpNoReply / NoEchoReply | 工具计数发送尝试 4、回复 0；不能证明这些尝试成功发送，不等同于监督器超时，也不证明主机离线 |
| Timeout / Deadline | 某阶段到达总预算；没有 summary 就没有计数，不能编造“100% 丢包” |
| UtilityUnavailable | 固定位置缺少工具；没有安装或 TCP fallback |
| PermissionDenied | 启动拒绝，或已识别实现的精确 C-locale errno；不回显 stderr，不使用 sudo |
| UnsupportedEnvironment | 系统、Python 前置条件或版本/配置契约不受支持 |
| IcmpFailure | 工具错误、未知/矛盾输出、输出上限或 ICMP 错误；固定原因，不反射任意 stderr |
| CleanupUnknown | 有界 kill/wait 没有确认退出；停用此诊断权限，检查目标进程后重连 |

`transmitted` 是工具计数的发送尝试，不是成功发送或线上报文数量的证明。固定 `-q`
下，上游 `hard_local_error` 也会增加 `ntransmitted`；本地错误的 quiet 分支可能在
增加 `nerrors` 前返回。因此 `errors=0` 不能排除本地发送失败，差额
`transmitted - received` 只能称为**未获回复的尝试**，不能归因为线上丢包。
quiet summary 不能恢复已被工具抑制的错误，也不能据此把静默 EPERM 分类为 PermissionDenied。

The utility's transmitted counter means counted send attempts. Quiet iputils
can count hard local send errors without reporting them in its error counter.
Unanswered attempts therefore prove neither successful transmission nor network
packet loss. The existing `packet_loss` wire reason is retained for compatibility;
it identifies the partial-reply category without attributing the cause.

RTT 是工具 summary 的 min/avg/max/mdev，转成整数微秒，只描述已观测的 Echo 回复。
summary 内的丢包百分比仅用于检查其与工具计数的算术一致性，不是独立的线上丢包测量。
stderr 只用于精确白名单错误分类，永不作为显示文本。完整 quiet 输出必须符合该版本
的 header、数值目标、计数、丢包率和 RTT 契约；额外行、未知格式、缺失 RTT、计数与退出
状态矛盾均不构成成功。这里没有设置 `-w`，因此退出状态的解释只适用于本表的固定 argv。

监督器直接拥有版本检查、DNS worker、ping 三个顺序执行的进程；每个有独立进程组，
在取消信号、期限或输出上限时杀掉其组并实际 wait。信号处理只记录意图，不在 Popen
创建与归属登记之间抛异常。正常退出、杀后确认退出和未确认退出严格区分。

SSH 取消或通道故障不能证明立即远端退出；已有独立监督期限仍有效。若 ICMP 没取得可验证
退出结果，UI 保守停用该诊断权限，避免直接重试不确定的进程。强杀监督器、主机失联或内核
拒绝清理不能伪造为已回收。历史源码、离线测试、受控 GPUI 和真实原生/ICMP 验收分别记录。

## 官方依据 / Primary sources

- [iputils 20240117 ping.c](https://github.com/iputils/iputils/blob/20240117/ping/ping.c)：
  版本报告、固定选项、IPv4 header。
- [iputils 20240117 ping_common.c](https://github.com/iputils/iputils/blob/20240117/ping/ping_common.c)：
  `main_loop` 的计数停止条件，`finish` 的 summary 和退出条件。
  同时指定 count 和 deadline 会改变计数停止条件；因此此实现不用 `-w`。
  `pinger` 中 `hard_local_error`（423–431 行）在发送失败后仍推进发送计数，
  且 quiet 模式可不输出 sendmsg 错误；summary 的 transmitted 不能被提升为成功发送证明。
- [iputils 20240117 ping6_common.c](https://github.com/iputils/iputils/blob/20240117/ping/ping6_common.c)：
  IPv6 header；不能用 BSD 的输出语法推断。
  IPv4 `ping.c` 的 1435–1445 行与此文件的 540–550 行均有本地错误 quiet
  提前返回路径，位于 `nerrors++` 之前。
- [iputils 20240117 git-version.h.meson](https://github.com/iputils/iputils/blob/20240117/git-version.h.meson)、
  [iputils_common.c](https://github.com/iputils/iputils/blob/20240117/iputils_common.c)：
  `IPUTILS_VERSION` 与完整配置行。构建配置可为 yes/no，不接受额外内容。
- [Python subprocess](https://docs.python.org/3/library/subprocess.html#subprocess.Popen.communicate)：
  超时不会自动 kill；必须显式终止并 wait。本实现边读边限制输出，没有无限 communicate 缓冲。

新的 experimental JSON 模式不在依赖契约中。扩展版本/系统必须另行核对上游、
增加输出与进程回收反例，并完成隔离真实工具验收，不能放宽到未知输出也算成功。
