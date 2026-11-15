# 自有 SSH 中继与审核式路径比较

KeelShell 使用你管理的 OpenSSH 跳板建立到目标的 SSH 连接。每一跳先核验
主机密钥，再进行对应账户的认证；目标在转发通道内独立完成 SSH 握手和认证。
跳板获得的是转发目标与流量信息，目标 SSH 会话仍有自己的端到端加密。
中继不能保证更快，新增的一跳也可能增加时延、拥塞和运维成本。

## 部署一个受限 OpenSSH 跳板

使用专用、非管理员的跳板账户和独立公钥。私钥保留在客户端，不复制到跳板，
不启用 agent forwarding。不要把无认证的 SOCKS/HTTP 代理公开到网络。
以下是由服务器管理员审查后合入 `sshd_config` 的示意配置；实际账户、地址、
端口和已有 `Match` 块需要按环境调整：

```text
Match User relay-user
    AuthenticationMethods publickey
    PubkeyAuthentication yes
    PasswordAuthentication no
    KbdInteractiveAuthentication no
    AllowTcpForwarding local
    PermitOpen target.internal.example:22
    AllowStreamLocalForwarding no
    AllowAgentForwarding no
    PermitTunnel no
    X11Forwarding no
    PermitTTY no
    MaxSessions 0
```

`AllowTcpForwarding local` 允许客户端发起 `direct-tcpip`，配合 `PermitOpen`
只允许连接明确列出的目标。这里的 `local` 是 SSH 转发方向，不能理解为只监听
localhost。`MaxSessions 0` 拒绝 shell、exec 和 subsystem 会话，保留转发通道；
仅禁止 PTY 不能达到同样效果。不要设置 `DisableForwarding yes`，它会覆盖
转发许可。用防火墙再限制谁能访问跳板、跳板能访问哪些目标。

为跳板账户的这一把公钥设置单独的 `authorized_keys` 条目：

```text
restrict,port-forwarding,permitopen="target.internal.example:22" ssh-ed25519 AAAA... user-managed-relay
```

示例公钥是占位符。`restrict` 先关闭附加能力，`port-forwarding` 再开放受服务器
与 `permitopen` 共同约束的转发权限。该 hostname/port 应与客户端最终请求
完全一致；不要依赖别名、通配符或自动 DNS 转换扩大权限。管理员应检查
OpenSSH 版本所附 `sshd_config(5)` 和 `sshd(8)`，用 `sshd -t` 验证配置，
并检查最终生效的账户规则；通过独立管理连接完成变更与回退。

## 信任与连接

通过已认证的管理通道获得跳板和目标的 SHA256 主机密钥指纹，例如管理员在
对应主机读取公钥后使用 `ssh-keygen -lf`。网络上扫描到的密钥不能单独作为信任
依据。分别核对并保存每一跳的信任；未知或变更的密钥必须停止连接并核实，
不能为了实验关闭 host-key 检查。

保留两个连接配置：A 到目标，B 通过受限 SSH 跳板到目标。两者最终请求的
host、port、username 和已核验主机密钥必须完全一致。A 可以显式使用受信的
SOCKS5 或 HTTP CONNECT 出口；B 的跳板也可以配置自己的显式出口。最终目标
相同并不证明 DNS/集群一定落到同一物理机器，比较对象是请求相同逻辑端点的
两条实际路径。用不同目标端口或不同用户比较，不能算同目标路径实验。

审核时检查两条实际连接封存的完整路径，包含每个跳板和显式代理。比较不会
自动建立第二条连接、选择赢家、重试或替你切换路由。仅在你明确确认本次审核后
启动探测；换连接、重连或相关路由/信任变化后应重新审核。

## 结果能说明什么

每次比较固定三轮、18 个样本，每轮 A/B 交替，第二轮反转先后顺序。每条路径
分别执行固定 SSH exec、256 KiB 内存上传、256 KiB 内存下载，合计 3 MiB
合成载荷，另有 SSH、命令及确认开销。目标需要可用的 POSIX shell 和 Python 3。
探测不读用户文件、不写远端文件、不请求 PTY、sudo 或工具安装。

每个样本最多八秒，整次比较最多六十秒。计时包含通道打开、远端命令及 Python
启动、传输和完成确认；不是 ICMP RTT，也不是裸链路带宽。只有完整成功的三次
样本才有该路径/操作的中位数；有失败、取消、缺失或未知清理时不要补零或宣称
加速。清理未确认时先核对连接并重连，再做新的比较。

观察差异需要结合真实业务、负载、运行时段和多次独立实验判断。一次本地受控
实验的较小耗时不能推断互联网线路或生产体验必然改善。

## 开发者的独立 localhost 实验

`scripts/ssh_path_lab.py` 使用 Python 3.11+ 和已安装的 OpenSSH `sshd`、
`ssh-keygen`。适用 macOS/Linux，需要当前账户能运行临时 sshd，checkout 祖先
目录满足 OpenSSH `StrictModes` 的所有权和权限要求。它不安装依赖，
不调用 Cargo，不修改用户 SSH 配置，不使用用户凭据。由父集成流程先构建
`ssh_path_interop` integration test，核验来源和测试库存后传入其可执行文件。
共享 target 的归属、失效与库存要求见
[SHARED_TARGET.md](../testing/SHARED_TARGET.md)。

```sh
python3 scripts/ssh_path_lab.py \
  --test-binary /absolute/path/to/prebuilt/ssh_path_interop-TEST_HASH \
  --output work/ssh-path-lab/run-001
```

`--output` 必须是新目录，已有目录会被拒绝，保留早先失败证据。默认总上限
300 秒，包含清理，不能通过参数延长。默认顺序为 `baseline`、
`direct-impaired`、`relay-impaired`；`--scenario` 可以单独选择其中之一。
`--delay-ms 20 --rate 524288` 为默认损伤参数；`--rate 0` 禁用速率等待，
`--delay-ms 0` 禁用附加等待。不要把两者都设零后称为损伤实验。

实验拓扑：

```text
A: Rust test -> localhost CONNECT gate A -> target sshd (one final port)
B: Rust test -> localhost CONNECT gate B -> jump sshd
                                       -> direct-tcpip -> same target sshd
```

两个 sshd 分别持有独立的临时 host key、client key、authorized_keys、配置和日志。
target 禁止转发，jump 仅允许转发到这一个 target 且禁止 exec/PTY。两条连接的
最终 host/port/user/pin 相同。所有监听都是 `127.0.0.1`，gate 只能 CONNECT
各自唯一的固定目标；临时密钥位于日志目录之外的独立私有目录。

基线两 gate 都不加等待；direct 受损只改变 gate A，relay 受损只改变 gate B。
每个方向独立转发至多 16 KiB 的一次读取结果，在每次写前等待
`delay_ms/1000 + chunk_bytes/rate` 秒，然后受下游背压约束。缓冲与两方向传输
可以流水重叠，TCP 读取边界、操作系统调度也会影响实际耗时。这个模型明确是
**用户态流延迟与限速**，不能解释为固定链路 RTT，更不能解释为 IP 丢包、
内核 netem、TCP 重传或拥塞恢复测试。

输出保留每场景 `conditions.json`、两条身份/路线的 `review.txt`、
`report.json` 中全部原始样本顺序、轮次、耗时、字节、失败原因与中位数，
以及独立测试日志。基线另跑取消、同路径拒绝和双层错误 pin 回归。
顶层 `result.json` 包含二进制/runner 摘要、测试库存、各进程退出和实际 reap、
按内核 birth identity 跟踪的清理、私有目录删除回执；差异使用有符号的
`B_minus_A_ns`，不自动命名为“加速”。失败场景保留失败，后续成功不能改签它。

正常完成、异常、超时和 SIGINT/SIGTERM 都进入 finally 清理自己的进程、通道与
临时密钥。清理无法证实时整次退出失败。SIGKILL、机器断电等无法执行 finally
的情形应由调用者核对回执与记录身份后处理，不能据此猜测清理成功。

## IP 层实验的验收边界

该 runner 在回执中将 netem/IP 丢包明确标为 `HOLD / NOT_TESTED`。
如果另行具备 Podman/Linux `tc netem` 条件，应仅在独立自有容器与网络命名空间
配置、读取实际 qdisc 及计数器，分别执行基线、A 受损、B 受损，保留原始样本与
完整容器/网络清理回执。不要对宿主或其他任务容器应用 qdisc，也不要用关闭流、
丢弃应用字节或引入等待来冒充 IP 层随机丢包。未取得该证据，NET-03 的 IP 层
损伤及性能收益验收仍开放；本地 OpenSSH 互操作也不替代三平台原生 UI 验收。
