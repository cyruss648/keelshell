# Windows 远端只读监控

状态：2026-11-17 源码整合、本机完整质量门禁与独立复核通过。实际通过范围从
[测试记录](../testing/records/2026-11-17-windows-read-only-monitor.md)读取；
受控 SSH、Windows API、真实 Windows SSH、原生桌面和发行分别验收。

## 使用方式

在已认证的远端 SSH 会话中打开主机监控。首次读取独立确认平台及运行时，
资源、容量、进程与端口分别报告状态。当前实现接受目标已有的 Windows
PowerShell 5.1；不会自动安装工具、提权或修改目标配置。
端口可以先于资源读取，不需要先取得成功的 CPU 或内存样本。

普通刷新复用已经确认的平台。Windows 的已确认接口失败等待显式“重新检测”后
重试；传输故障和身份未知不会被缓存成“命令缺失”。重新检测清除本连接的平台
与能力观察；重新连接从新状态开始。暂停保留已完成的观察，撤销进行中结果的
发布权限。绘制、主题切换、语言切换和缓存翻页不读取远端。

## 指标说明

| 显示项 | 含义与限制 |
| --- | --- |
| CPU | CIM 提供程序的百分比观察；有有效值时首样本即可显示。未知与真实 0% 分开，不伪造累计 CPU 计数。 |
| 内存 | 分别显示总量和空闲字节。没有取得 used 或 available 时保持未知，内存趋势和 MCP 不从空闲量推算 used。 |
| 运行时间 | 同次 CIM 对象的系统时钟估计；时钟跳变会使趋势断段，不据此计算网络速率。 |
| 网络 | 接口名称和实际累计收发字节；当前网络速率未知。 |
| 容量 | 已就绪固定卷的总量、总空闲量、当前账户可用量。used 由总量减总空闲量计算，不把配额限制算成磁盘占用。容量失败不丢弃其他资源。 |
| 进程 | PID、进程名称及可读工作集；名称不是命令行。未取得的用户、CPU% 和 MEM% 为未知；PID 0 只用于显示。 |
| 端口 | 当前账户可见的 TCP 监听和 UDP 绑定端点，PID 只作显示。只有两个接口都完整成功才显示成功列表；完整成功的空数组与读取失败分开。 |
| 其他指标 | load average、swap、iowait、磁盘 I/O 和网络速率保持未知。 |

列表不保证覆盖其他账户或整个主机。UDP 绑定不表示已握手或可达，TCP 监听不表示
应用协议健康。Windows 不提供连接探测、进程核对或终止入口；保留的旧 Linux
样本或审核对象不能获得当前 Windows 或未确认环境的执行权限。

## 趋势与 AI 上下文

CPU 曲线复用同一份已验证的当前点，不增加轮询。暂停、断采样、重启、时间回退或
异常时间跨度会断段；恢复后不跨缺口连线。没有有效 used 内存时不生成零值曲线。
三主题、明暗模式和中英文共享主题注册表及语义 token。

已有 MCP 授权机制只分享当前缓存中固定的 CPU、内存字段及样本年龄，不因为读取
上下文而发起新的远端采集。AI 输出仍需审阅，不能绕过命令或文件动作审核。

## 验证边界

真实OpenSSH隔离联调已取得精确工程`660e90d6`的两次成功工作流：
`43997403565`与`43998970900`。隔离回环sshd、固定主机指纹、
公开Rust SSH／监控接口、独立Win32对照及采集原进程自然终态分别核验，
不连接客户主机或改变已安装SSH服务。新Quality43997403533三平台已成功，
macOS／Linux真实OpenSSH interop各18项通过；新非作者对两次SSH与API原件的
4018项离线检查通过，限定批准、P0–P3全0并已关闭。Quality不在该reviewer范围；
完整日志、Git来源与digest由[精确CI封存](../testing/records/2026-11-17-windows-openssh-monitor-evidence/ci-completion/README.md)独立重算；
见[本批记录](../testing/records/2026-11-17-windows-openssh-monitor.md)。
原首轮`43990824640`的census／清理FAIL保持，首次坏读数精确子条件UNKNOWN；
后续成功不改写原失败，也不构成恶意程序沙箱或其他Windows版本的运行证明。

2026-11-17 精确工程 `5d2df2f` 的 Windows Server2025 AMD64 hosted 验证已通过
生产采集、独立 Win32 对照、五类 cmd／PowerShell 原对象自然终态和认证路由 replay。
同一工程的Windows／macOS／Ubuntu完整质量门禁均成功，普通Rust分别3394／3407／3400通过、
失败均0。Windows工装Python111 run中108通过、3跳过；非作者离线回执复算102项通过。
这只覆盖记录中的平台、运行时与自有资源，完整原件及精确结果从
[CI记录](../testing/records/2026-11-17-windows-read-only-monitor-evidence/ci-completion/README.md)读取。

Windows 工装执行实际生产采集命令，经 `cmd.exe` 启动，并用独立 Win32 API 对照
自有进程和双栈 TCP/UDP 对象。原始帧可以通过认证 monitor 路由回放；
该回放不证明真实 Windows OpenSSH 的认证、默认 shell 或远端进程终态。
前批手动isolated-ssh job只做前置检查并保持HOLD；
新的真实OpenSSH工作流独立执行认证及原进程终态，不使用这个前置job补签。
任何 API 或 runner 成绩均不能代签原生窗口、键盘、IME 或辅助技术。

协议、分层与取消不变量见 [ADR 0156](../adr/0156-windows-read-only-monitor.md)，
跨平台按项能力见[监控能力](MONITOR_CAPABILITIES.md)。
