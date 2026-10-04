# ADR 0039：桌面持有权威的对外 MCP 与真实 SSH 桥接

- 日期：2026-10-04
- 状态：桌面/协议桥接已实现并独立复审；完整原生窗口、最终整合包与跨平台证据分别记录
- 关联：MCP-01–04、[ADR0037](0037-external-mcp-stdio-server.md)

## 调用方向与权威位置

KeelShell 提供 MCP 服务端给外部智能体，不开发访问其他 MCP 服务的通用客户端。桌面端运行官方 SDK 的服务实例，策略、撤销 lease、生成提案及真实 SSH handle 均由桌面持有；独立 `keelshell-mcp` 可执行文件只转发受认证的协议字节。客户端不能提交授权快照、启用服务、批准命令、登录 SSH、安装主机指纹或解锁凭据。

启动默认关闭，授权不进入 AppState/profile。用户在 MCP 原生面板明确选择当前已就绪的 SSH 会话与七项固定工具；目录读取必须明确输入一个实际 canonical POSIX 根并由后台 SFTP 复核，选区读取必须单独捕获 1–16 KiB 的当前选中片段。最多32个授权会话，不自动共享整屏或终端历史。授权会话枚举可包含已授权片段UUID和根目录，让调用方能够选择正确输入；服务端再次校验这些metadata属于当前grant且对应读取能力已启用。

权限更新撤销全部旧lease并旋转监听端点和密钥，旧客户端不能凭旧配置取得替换后的scope。断线、关闭、重连生成新实体、保存路线或主机信任变更都撤销全部授权，不把旧ID映射到新的session。一次性SSH会话使用独立临时connection/session/route UUID；已保存会话使用profile UUID并绑定不可复用live UUID及路线revision。每次准入在UI原子核对Entity、已就绪终端、已捕获SSH handle和路线/信任快照；异步工作在I/O前与await后检查lease。

## 本机认证与正文保护

只监听 `127.0.0.1` 的随机端口，密钥是显式启用时生成的256-bit随机临时能力。回环地址不是OS peer身份验证，不宣称区分同一用户的进程；拿到密钥的程序具有用户选择的完整scope。独立bridge从两个明确ENV字段读取地址与密钥，不接受argv凭据，不写profile、runtime引导文件、用户CLI配置或日志。

双向认证使用角色/版本domain separation的HMAC-SHA256与新鲜双方nonce；客户端先确认真实服务端证明，才传客户端证明和工具请求。旧端口被其他程序复用不能取得原始bearer或冒充服务端。会话正文进一步由transcript派生的方向密钥以ChaCha20-Poly1305有界record保护，隐式递增nonce拒绝重放、改写与方向混用；仅握手认证但后续明文的本机relay方案不采用。

此方案不提供forward secrecy：如果运行期能力随后泄漏，获得者可结合捕获的握手transcript推导历史方向密钥。它提供本机能力持有证明和传输正文完整性/保密性，没有OS用户、进程可执行身份或长期密钥交换的声明。

连接最多8个（包含认证中的连接），认证截止2秒；每条record明文最多16,384 bytes，含类型/tag的cipher最多16,401 bytes，含header的wire最多16,413 bytes。每个方向最多保留一条待写wire和一条decoded plain；加解密另有短时、有界分配，不能据此声明全进程精确RSS上限。stdio adapter每方向使用Tokio 8,192-byte copy缓冲；服务端仍保持128 KiB输入line、32个尚未实际flush的frame预算及256 KiB完整tool result限制。

启动协商10秒，SDK清理2秒，正常关闭认证EOF flush最多250 ms；host取消的listener drain最多2.5秒，显式close最多等3秒，stdio输入EOF后的输出drain最多2秒，binary runtime shutdown最多100 ms。Drop发出取消并由拥有的任务异步收尾，显式close等待收尾；不为方便清理派生脱离拥有者的加密pump任务。SDK正常close不主动调用AsyncWrite::shutdown，host在SDK收尾后补充认证EOF，不能将真实TCP截断误当成合法EOF。

用户点击“复制临时启动配置”才生成含ENV的JSON；平时界面不显示密钥。配置中command指向应用可执行文件同目录的MCP companion，存在性在后台检查，缺失时禁用复制并提示构建或完整安装。同一能力可供多个客户端连接，持续至授权变更、撤销或应用退出，不因一次连接自动耗尽。任何一次撤权/关闭服务都会使已复制配置失效，需要重新复制；关闭面板本身不撤权。剪贴板内容以及外部客户端显式保存的配置不由应用秘密改写。

## 真实读取与命令审阅

桌面bridge的Tokio有界队列最多8项；UI每次最多处理8项，阻塞/远程I/O均在网络runtime。取消已排队请求会关闭回复receiver，UI准入前拒绝已关闭调用；取消在途读取会drop拥有SFTP subsystem的future。I/O取消不保证远端没有观察到已经发送的读取。

SFTP目录/文件操作先核对canonical路径必须与授权词法路径完全相等，并检查实际父目录链、明确类型与链接。列表非递归最多256项，链接和特殊对象只标unsupported；常规文件完整UTF-8最多64 KiB，溢出、无类型、元数据变化或非UTF-8拒绝，不能静默截断。SFTP v3观察不能排除恶意或并发服务器替换的所有TOCTOU，不能表述为文件系统锁或事务。

监控工具只读取当前已有缓存的CPU/内存固定字段、样本UUID和年龄，不发起shell探针或刷新，不共享进程命令、网络端点、用户名等额外内容。

命令提案最多32项，最多32 KiB原始UTF-8，由桌面服务端生成ID和目标/命令摘要，最多300秒等待人工审阅。原生面板展示目标完整路线、live UUID、摘要与完整命令；控制和方向变换字符显式转义显示，执行仍使用原始不可变字节。只有真人点击“确认目标并执行此命令”才能单次消费批准；同时只执行一项MCP提案，走捕获SSH handle上的独立exec，不注入PTY、不继承terminal cwd、不自动重试。执行有30秒/64 KiB输出界限，UI保留有界结果；未知退出状态、超时、撤权或取消不当作远端已停止。客户端只能读同一目标/仍有效scope下的状态，没有approve/execute工具。

## 证据与未验证边界

对应单元、真实TCP SSH/SFTP、GPUI以及stdio/受认证IPC测试在本切片中补充，包括stdio adapter经加密IPC进入GPUI人工批准、然后捕获SSH handle精确一次exec的闭环。该GPUI测试使用受控窗口与owned loopback peer；完整原生桌面、三平台CI和打包验收依赖最终冻结源码，编译与受控窗口不能当作原生验收。

独立MCP companion进入各平台发布包由整合阶段验证。运行中的环境凭据在同用户/管理员权限下可能被读取；此能力模型不承诺防御已经控制同一OS账户的进程。无真实云账户、客户主机、跨平台原生窗口或供应商MCP客户端配置验收时，不据fixture和编译宣称完成这些边界。

官方实现依据：[RustCrypto HMAC](https://docs.rs/hmac/latest/hmac/)、[RustCrypto ChaCha20-Poly1305](https://docs.rs/chacha20poly1305/latest/chacha20poly1305/)、[MCP stdio规范](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio)。验证见[桌面桥接记录](../testing/records/2026-10-04-mcp-desktop-bridge.md)。
