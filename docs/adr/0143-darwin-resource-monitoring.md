# ADR 0143：macOS 远端只读资源与进程监控

日期：2026-11-16。状态：已实现；专项、全部限定非作者复核及最终集中门禁 r3 通过，
完整桌面与跨架构原生验收保持开放。

## 决策

沿用 session/core/app 分层及当前认证 SSH 连接，通过固定 `uname -s` 分派 Linux
与 Darwin 资源采集。公开 `LinuxMonitor` 名称保留源码兼容性；其资源和进程读取
扩展到 Darwin，监听端口、进程身份核对及 SIGTERM 仍保持 Linux 契约。
不安装远端代理、不提权，不读取凭据或进程环境变量。

Darwin 采集依序选择 `/usr/bin/python3`、`/opt/homebrew/bin/python3`、
`/usr/local/bin/python3` 中已有的可执行普通文件，不使用任意 PATH 解析；
新平台分派依序选择 `/usr/bin/uname`、`/bin/uname`；两处都缺失返回命令缺失，
不将路径布局误判成不支持的操作系统。通过隔离启动 `-I -S -B` 和 stdin 脚本
调用操作系统接口。Python 缺失有独立能力原因；不自动安装或退回猜测的数值。
Mach CPU 累计 tick、连续时钟、物理内存、VM 空闲页和接口实际字节计数来自
内核接口；负载来自系统接口。可选 `df -Pk` 失败不丢失核心样本。
ABI 以本机 Apple SDK 的公开头文件为核对来源，限定实现与真实 SSH 联调分别留证。

采集使用有界、带单位和完成标记的 schema 1 帧。解析拒绝重复/未知字段、接口、
截断、溢出、非有限数和非法时钟；容量独立降级。SSH 返回仍受 session 截止时间
及 2 MiB 输出上限约束，阻塞 I/O 继续由后台 worker 持有。

## 数值语义

- `Snapshot.platform` 标记 Linux/Darwin；跨平台、重启、倒退时间或计数器重置断开速率。
- Darwin CPU 映射为 user/nice/system/idle，其余槽为 0；iowait **未知**，不能显示 0%。
- `MemorySnapshot.free_bytes` 是实际空闲页字节数；不将它当作 Linux `MemAvailable`。
  Darwin 的 available、used 估计和 swap 保持未知。界面展示总量、空闲值及原因，
  内存趋势不伪造已用比例，MCP 的 `memory_used_bytes` 为 null。
- Darwin 磁盘 I/O 保持 `MissingData`；文件系统容量不是吞吐量。
- Darwin 进程列表只读；界面不提供终止入口，操作分派也拒绝绕过入口的请求。

## 集成与验收

三主题继续通过 registry、Palette 和语义 token 使用相同组件；中英文及辅助语义
跟随已有架构，不为 Darwin 新建主题分支。历史图的平台变更必须断段。
能力状态仍绑定认证连接、generation/revision；负缓存到显式重新检测才失效，
传输错误不能变成“工具缺失”。

同批加入独立 Linux 真实 `/proc`、`df`、`ps`、`ss` 的能力降级/缓存/恢复实验；
受控 wrapper 只模拟故障，不生成成功系统数据。容器创建非零、信号或超时保留
UNKNOWN，客户端实际停止和 reap 后才核对自有资源，禁止共享资源清理。
只读容器仅增加 CHOWN、DAC_READ_SEARCH、SETUID、SETGID、SYS_CHROOT；
前两项分别用于自有 tmpfs 移交账户所有权及 root 的只读状态导出，实际内核 mask
必须精确符合 `0x400c5`。不授予 DAC_OVERRIDE、NET_ADMIN，不挂载主机目录。
ForceCommand 只执行精确匹配的 Linux 采样正文；平台包装中 Darwin 文本不执行，
因此实验只证明 Linux 正文与真实工具，不证明 Linux 上完整平台包装的原样运行。

验收按“并行实现、集中集成、窄修、非作者评审、最终一次完整门禁”推进。
真实 macOS SSH、真实 Linux 容器、headless GPUI 和原生桌面分别记账；
它们不能互相替代，完整跨平台桌面与产品目标仍开放。
