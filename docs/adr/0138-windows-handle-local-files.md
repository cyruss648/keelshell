# ADR 0138：Windows 句柄绑定的本地文件边界

- 日期：2026-11-15
- 状态：已实现，工程门禁与非作者源码复核通过；Windows 原生验收开放
- 范围：本地审核来源、私有 staging、Zmodem 保存、归档解包

## 背景

本地内容审核必须绑定实际对象及全部祖先；路径字符串、文件长度和时间不足以
识别替换、硬链接、重解析或权限变化。Windows 不能沿用 Unix 的 openat/
renameat 和 mode 位语义，也不能先创建一个继承宽泛 ACL 的对象，再写内容后
才修改权限。不能为兼容平台放宽未知写入、隔离或确认规则。

## 决策

新增独立 `keelshell-platform`，对上层只提供安全的句柄拥有接口。所有 Windows
ABI 调用集中在 `src/windows/`；不向 session/UI 泄漏指针或可替换的原生句柄。
现有 workspace 的 `unsafe_code = "forbid"` 保持不变。平台 crate 单独使用
`unsafe_code = "deny"`，只在 Windows 模块声明处显式允许并逐块解释内存、
对齐、所有权及同步完成契约。禁止 `unsafe impl Send/Sync`。该例外不允许
其他 crate 引入 unsafe。

只接收有界、规范的 Unicode 本地 drive 路径。UNC、设备命名空间、drive-relative、
dot 跳转、ADS、DOS 设备别名、控制字符、尾随空格/点和视觉路径伪装均拒绝。
drive root 打开后检查实际句柄的磁盘设备类型及 remote 标志；不以路径上的
GetDriveType 预检查代替实际 volume 身份。每个 descendant 都以保留的目录句柄
和单个字面名称执行 `NtCreateFile`，禁止 reparsing。

所有进入同步 NT 操作的 capability 均由本模块创建；不能将任意外部 `File`
包装为目录或 publication capability。打开设置 `FILE_SYNCHRONOUS_IO_NONALERT`
及 SYNCHRONIZE，调用期间保留全部参数、缓冲、IOSB 和 security descriptor。
公共 `metadata(&File)` 使用同步 Win32 句柄查询，不能从它推断路径来源。

源文件要求磁盘普通文件、完整 128 位 file ID、volume ID、单一 link、非 reparse/
delete-pending。元数据保留 created/modified/changed、attributes 和有界实际
owner/group/DACL 观察；读取忽略由自身访问产生的 last-access 变化。目录审核
保留 identity/attributes/security，不把兄弟写入造成的时间变化当作祖先替换。
全部已审核祖先和文件句柄保持存活，避免 file ID 被复用。安全观察只作内部
相等比较，公开 Debug 不输出 SID 或 ACL 原始字节。

创建调用直接携带当前进程 TokenUser owner、protected DACL 和唯一 owner
full-control ACE。目录使用 OI/CI，普通文件不继承。创建后通过真实句柄独立
读回 owner、DACL 结构、保护标志和确切 ACE；在首次内容写入前验证。
LocalAlloc 与 token handle 使用 RAII，观察复制有界有效字节，不持久化指针、
padding、ACL 空闲区或 SACL。创建失败或后验失败均不自动删除可疑对象。

普通文件从创建到发布保留独占其他写入/删除 open 的原创建句柄。目录长期
capability 只请求读访问并分享所有访问，不持有 DELETE：Windows 内核为
rename 打开目标目录时，在部分受支持版本上不分享 DELETE。目录作为源发布时，
内部从实际保留对象取得短期同步 DELETE lease，重新核验本地
volume、完整对象观察及私有 DACL；不按路径重新赋予来源身份。这个 lease 仅
在源发布调用期间存活，不替换原 capability。全部 namespace 与内容重验继续
承担竞争检测，不能把目录的读 capability 表示为排斥同账户写入的锁。
2026-11-16 的源码 `4d72e1e` 原生预检已确认原 `ReOpenFile` 路径在四项目录发布
用例中拒绝访问。接续候选改用保留句柄加空 NT 相对名的同步 `NtCreateFile`，
短期请求 `FILE_GENERIC_READ | DELETE` 并只分享 READ/WRITE。
通用 opener 的普通访问和分享参数原样保留；本机 platform 与 Windows 交叉严格
Clippy 已通过，静态复核保留原输入范围。组合完整门禁和真实 Windows 结果
分别记录，不从共享 helper 或已通过的只读游标推断其 DELETE 打开必定成功。

发布针对保留的 `PrivateEntry`，`NtSetInformationFile(FileRenameInformation)`
传入目标目录句柄与单个名称，`ReplaceIfExists=false`。目录枚举使用独立打开
的同步 cursor，并逐条检查有界 UTF-16 返回链；不依赖共享 scan state 的复制
句柄。2026-11-16 的原生预检已确认 NT `"."` 相对名在此 cursor 打开处失败。
后续候选使用保留目录句柄加空 NT 相对名重开，并比较重开前后的
identity/attributes/security；空名仍提供存活的 UTF-16 backing storage，
不把其终止单元计入 UNICODE_STRING 长度。不重建路径，也不按路径回退，
私有 child 名称的公共校验继续拒绝空分量。该机制的实际 Windows 通过状态
由[游标记录](../testing/records/2026-11-16-windows-directory-cursor.md)单独维护。
rename 缓冲满足完整固定结构加名称字节的文档最小长度，传入实际初始化
的对齐缓冲长度，另测单 UTF-16 单元和 255 单元名称。目标预检查拒绝 portable
Unicode case alias；系统调用原子拒绝现有
目标，成功后还需完整 readback、身份、私有 DACL、父链、唯一名称及 staging
缺席验证。竞争产生的不唯一或未知结果继续隔离。

session 在 blocking worker 使用上述接口，保留原 review、取消、mutation pending
与 process-wide reservation。提交 worker 前即登记可能的 mutation；await 取消
不能释放尚存 worker 的保留权。输出只在完整已验证保存后显示成功。

mutation pending 从入队持续到异步所有者实际接收 `JoinHandle` 的结果；后台
闭包返回不等于结果已被接收，不能提前清除。放弃 await、撤销或 panic 保留
pending，scope completion 必须报告 `MutationUncertain`，从而保持 UI 的未知
结果关闭保护。已接收的 staging/publication guard 在验证成功前仍独立持有未知
保留权。共享模块的真实 SSH、有界屏障和实际临时文件回归验证结果交接；
这些宿主执行不替代 Windows NT 文件操作。

平台的保守 Unicode `filename_alias` 同时供本地 namespace 和 process-wide
reservation 使用。现有祖先与缺席路径分量使用同一关系，包括 expanding case
和词尾 sigma，避免使用不同拼写绕过 Busy/Quarantined。此策略不承担文件系统
查找或 Unicode 规范化，也不改变历史未知记录。

## 验收与限制

Windows 交叉编译和宿主 parser 测试只验证工程输入、类型及纯逻辑，不能代替
Windows 原生执行。Windows 原生测试覆盖实际权限创建、拒绝宽泛/无保护 ACL、
句柄身份、硬链接、无覆盖文件/目录发布、目录枚举和来源/目标复查；未经运行
的用例一律标记 `NOT_RUN`。

先 flush 普通文件，再完整读回与 namespace 校验；不把目录 rename 或文件
flush 推断为断电持久性。未知写入保留原路径和隔离状态，不自动重试或清理。
不声明支持远程映射盘、缺少所需 file ID/ACL/NT rename 能力的文件系统，也不
声明 Linux/Windows 桌面、IME、辅助技术或产品整体完成。

本批结果、非作者复核、失败与最终源码绑定由
[集中测试记录](../testing/records/2026-11-15-windows-local-backend.md)维护。
