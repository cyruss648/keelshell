# ADR 0156 — Windows 远端只读监控

日期：2026-11-17。状态：源码整合、本机完整门禁与两位非作者限定复核通过；真实平台／原生／发行分开接续。

## 目标

监控不再以资源采集成功作为远端平台的唯一证据。认证连接可独立识别平台与执行器，
再分别读取资源、进程、容量和端口；一项失败不清除其他有效数据。Windows 首批提供
只读观察，既有 Linux 审核式连接探测、进程核对和 SIGTERM 不因此扩大权限。

## 平台、执行器与连接权威

每个 monitor 在首次需要时去重执行固定 POSIX 身份探针，严格核对成功退出、完整帧
及平台／runtime；不能建立身份时，有界尝试固定 Windows PowerShell 探针。
两种探针均不能证明身份时保留“平台与执行器未确认”，不猜测 Windows 或命令缺失。
SSH 传输失败不触发平台推断。初批只接受实际观察到的 Desktop Windows PowerShell
5.1 及四段版本号；其他 runtime 没有因此取得支持声明。

平台和逐项能力共享认证连接、generation、revision 与报告失效机制。clone 共享缓存，
新 monitor 重新建立观察；显式重检清除平台和能力缓存。暂停／退役撤销进行中操作，
保留此前已完成的观察；旧探针不得启动后续命令，也不得写回 reset 后的缓存。
每条新 SSH 命令发送前和收到结果后检查 generation，绘制不执行探针或采集。
发送前检查包含异步 channel-open 获准后的边界；撤销时关闭获准但未使用的通道，
不发送 exec。已经发出的 SIGTERM 继续保留实际回执，不能把撤销或通道关闭当作
远端进程终态。审核时效也在动作发送前重新核对。

已确认 Windows 路由下，类型化的非传输资源失败等待显式重检；
缓存读取不更新原检查时间或 revision。既有 POSIX 瞬时资源失败的重试行为保持，
不把未知身份或 SSH 故障转成命令缺失负缓存。

## 数据语义

| 项目 | Windows 来源与呈现 |
| --- | --- |
| CPU | Win32_Processor 的 LoadPercentage，按有效逻辑处理器数加权；独立点观测，首样本可显示。缺失时未知，不伪造累计 ticks。 |
| 内存 | 同次 CIM 的 TotalVisibleMemorySize／FreePhysicalMemory，显式从 KiB 换算字节；free 不推算 available 或 used。 |
| 运行时间 | 同一 CIM 对象 LocalDateTime 减 LastBootUpTime，标为系统时钟估计；不用于推算网络速率。 |
| 网络 | .NET GetIPStatistics 的实际累计收发字节，接口计数和名称有界；本片速率未知。 |
| 容量 | 已就绪固定卷的 DriveInfo；TotalFreeSpace 与 AvailableFreeSpace 区分，used 从 total 减 total-free 得出。失败独立降级。 |
| 进程 | Get-Process 的 PID、ProcessName 和可读 WorkingSet64。owner／CPU%／MEM% 未取得即未知，名称不是命令行。PID 0 可显示，不能操作。 |
| 端口 | Get-NetTCPConnection 的 Listen 与 Get-NetUDPEndpoint 的绑定地址。两接口均成功才发布完整成功；不承诺整个主机可见性、握手或应用健康。 |
| 未实现指标 | load average、swap、iowait、磁盘 I/O、网络速率保持未知。 |

CPU 来源用 `CpuSample` 区分累计计数、提供程序百分比和不可用。负载及进程可缺失
字段用 Option 表达。趋势将 gauge 作为当前点保存，暂停、旧时间、重启和断采样仍
断线；缺失内存 used 不生成零值曲线。MCP 继续只分享已有、带年龄的缓存样本。

## 固定命令与严格协议

命令使用固定 `powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand`，
UTF-16LE Base64，不拼接用户内容。编码后整个命令小于 8000 ASCII 字节，
并以实际默认 shell 的执行留证；编解码单测不能代签该执行边界。

版本 1 JSON 帧要求 operation、platform、runtime、四段版本、单位、状态和数据，
使用拒绝未知字段及重复键的类型化反序列化。宽整数使用规范无符号十进制字符串。
成功要求完整单帧、成功退出及空 stderr；失败原因只来自固定类型化帧，
不依赖本地化错误文本。拒绝缺失退出码、截断、溢出、非有限值、杂文、重复身份、
错误行数、坏端点或控制字符。LF 与 CRLF 均有明确边界。

合并输出上限沿用 2 MiB；进程及端口最多各 20,000，接口最多 256，固定卷最多 512。
超过预算失败，不截断成成功。容量失败保留核心样本；TCP 或 UDP 其中一个接口失败
不能把另一个接口的结果标为完整端口列表。只有完整成功的显式空数组可显示空观察。

## 工程与验收

会话层拥有探针、固定采集、解析和缓存；界面只采用验证后的样本与认证报告，
保留三主题 token、双语、分页和 Linux 动作门槛。沿用私有分区 builder 的栈边界。
不自动安装工具、不提权、不修改 SSH 配置、服务、注册表或防火墙。

验证分开记录：纯解析、受控 SSH／缓存／迟到、GPUI 模型／几何、真实 Windows API、
真实 Windows OpenSSH、原生桌面、发行。Windows 工装以实际生产脚本读取自有短命
进程与端点，并核对自有资源终态；原始 stdout 可另做 Rust 认证路由 replay，
该 replay 仍不能代签真实 Windows SSH 的默认 shell 或认证链。
关闭 SSH 通道不证明远端 PowerShell 已退出；缺少终态证据保持 UNKNOWN／HOLD。

非作者源码与功能复核、受影响专项后，在同一冻结输入上集中执行一次最终完整门禁。
依赖只使用 x.y 要求并保留 patch 锁文件；本批观察到 zip 9.6.1 补丁后更新锁文件，
在线完整新鲜度查询的失败与重试单独保留，不能把部分成功合并成一次完整通过。
