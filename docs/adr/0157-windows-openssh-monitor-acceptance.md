# ADR 0157 — Windows OpenSSH 监控的隔离验收

日期：2026-11-17。状态：工程已实施并通过本机门禁与独立源审；
当前没有本批真实Windows SSH成功回执。
基线：`bfbea7bcc84ff433d28a635e85daed9cca337fb0`。

## 目标

真实验证已实现的Windows只读监控在OpenSSH认证、默认shell和远端进程生命周期上的行为。
既有Server2025 AMD64直接API与认证peer replay已经通过；这些原成绩保持各自范围，
不能代签本次真实OpenSSH。源码实现与工装存在本身也不是Windows验收。

## 隔离服务器与身份

使用目标已有的System32/OpenSSH二进制，通过独立配置在回环高位端口运行短命sshd。
不安装工具、不创建账户、不修改已安装服务、注册表、系统默认shell或防火墙。
主机与客户端Ed25519密钥只在受保护临时目录存在，私钥和私有客户端配置不进入上传目录。
只允许当前登录用户、公钥认证，关闭转发与PTY；没有ForceCommand或继承配置。
Windows不支持的Unix配置选项不放入这份实际配置。

Rust真实测试使用精确主机指纹，先执行错误主机密钥拒绝控制，再通过公开
`SshSession::connect`与`LinuxMonitor`调用生产路由。所有采集命令来自现有生产代码，
不从测试注入替代采集器。对照四个自有双栈端点、两个connected-only反例、进程与工作集，
同时检查资源、未知字段和缓存／撤权语义；本批不发送信号或Linux主动诊断命令。
默认shell只读取配置并留证，实际子进程对象负责证明其执行路径。

## 原对象、接纳与终态

sshd在挂起创建并加入自有Windows Job后才继续运行，另以DEBUG_PROCESS接收继承子进程事件。
每个CREATE_PROCESS_DEBUG_EVENT必须在首次Continue前保留独立原对象句柄，
绑定PID、创建时间、image与command line，确认属于自有Job。
进程标识、调试事件与Job累计接纳计数必须一致；缺项、复用歧义、异常或无法绑定均失败／HOLD。
调试器不吞真实崩溃，不把PID轮询、管道EOF或通道关闭当作原对象终态。

客户端完成并断开后，有界等待非master原进程自然终止，再关闭新子进程接纳，
核对四条固定PowerShell命令的原对象和真实exit0。
随后终止本工装sshd master，核对最终Job空、原对象signaled、调试事件继续、
管道排空与自有句柄关闭，且清理前后的接纳身份集合不能多出遗漏对象。
服务master的受控终止与采集PowerShell的自然终止分开记载。

Job与调试继承不是恶意程序沙箱，也不能自动证明不存在由外部broker独立创建的进程。
实际Windows OpenSSH的token/broker执行链必须能被本次接纳与身份协议完整解释；
若原件显示逃逸、漏接纳或无法对应，保持HOLD，不能通过放宽census签署成功。

## 独立对照与来源

沿用已验证的独立Win32 oracle，自行枚举稳定接口和固定卷，
不从生产结果选择对照对象。资源、进程和端口由公开Rust路由结果还原协议数据供对照，
还原规则逐字段记录，缺失值不能伪造。CPU不同观察窗口的差值仍只作信息。

原件保存精确Git来源、实际native测试二进制与独立库存、固定生产命令装配摘要、
Rust结构化回执、独立前后读数、调试接纳和清理回执。Git工程输入不干净时不签验收。
新的Windows工作流自动运行本次实际用例；缺少条件或终态证据以非零HOLD结束。
本机离线专项、Mac编译与完整门禁、Windows实际运行、原生桌面和发行分别记录。

## 参考依据

2026-11-17通过官方MicrosoftDocs GitHub内容API读取；learn.microsoft.com直连失败原件保留。
只保存出处、固定Git blob和原文摘要绑定，不把未成功抓取的网页引用当作已核实来源：

- [Windows OpenSSH配置](https://github.com/MicrosoftDocs/windowsserverdocs/blob/main/WindowsServerDocs/administration/OpenSSH/openssh-server-configuration.md)；
  内容API返回的该文件blob为`2f0ddff2bae02ac695823232e3dcdacdaa2ff0df`。
- [Debugging Events](https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/Debug/debugging-events.md)；
  文件blob `0bc26e176a4dd7de03a9230cb09c9fcc1f1409e8`。
- [Process Creation Flags](https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/ProcThread/process-creation-flags.md)；
  文件blob `cfca8378a46be11f56b2661976576a1d4892239f`。

上述blob是文件对象身份，不冒充仓库提交；最终测试原件另行绑定本项目精确提交。

## 首轮真实观察与有界settle修订

工程`41671be`的真实Windows首轮已完成认证与生产采集，整体因census／清理失败。
旧原件缺失第一次坏census的分项读数，不以事后累计21等于接纳21推定当时通过。
顺序执行的Job accounting／PID list／accounting并非原子快照；
自然退出或异步终止期间可能变化，候选在原有限settle内重泵事件、重取完整读数，
最终严格匹配和封接纳后的累计不变规则继续保留。
每次不完整观察保留前16条及最后读数；不一致不能提前返回零active。
真实cmd.exe还会在argv0后加入空格，只有这一参数边界允许归一化，
生产选项／Base64仍保持逐字节绑定。四文件修订已获新非作者限定批准，
49项独立控制通过；冻结后的完整门禁r2通过，765工程输入前后无漂移。
修订后的真实Windows验收尚待新精确提交，原FAIL保持。
