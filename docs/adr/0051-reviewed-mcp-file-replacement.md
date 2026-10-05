# ADR 0051：外部 MCP 的受审阅文件替换提案

- 日期：2026-10-05
- 状态：A 独立审查 P2 保持；B 作者/全新非作者限定复核、主树完整门禁与新 macOS 八工具文件审阅通过；供应商第八工具、其他平台原生及新提交 CI 待记录

KeelShell 继续向外部智能体提供固定 MCP 服务，不作为第三方 MCP 客户端。外部智能体可以提议修改一个已存在的远程文件，批准和写入只属于桌面人工审阅。

`keelshell_propose_file_change` 需要独立勾选的工具权限及经过真实 SFTP 验证的 canonical 目录范围。参数是精确连接/会话/路线身份、完整路径、完整旧内容 SHA-256 和完整 UTF-8 替换正文。正文上限 64 KiB，空替换合法；不存在、链接、目录、特殊对象、非 UTF-8、不完整或超额内容不进入待审。既有请求帧总长度 128 KiB 仍有效，JSON 转义后的内容也受该总上限约束。没有创建、删除、移动、批准或直接写入工具。

服务生成不可复用的提案 ID，以及绑定版本、ID、三个目标 ID、路径、预期旧内容 hash 和精确替换 bytes 的摘要。桌面后台读取旧正文和 size/mode/mtime 快照，匹配预期 hash，生成有界线性完整替换 diff；没有在 UI 线程上执行二次方 LCS。未授予读取能力的提案调用者只收到 pending ID/digest，失败只有固定错误码，不返回旧正文或当前 hash。`keelshell_sftp_read` 显式读取授权独立，增加精确内容 SHA-256。

文件审阅是独立模态：目标路径、完整 SSH 路线、三个身份、提案 ID/digest、前后 hash 和全部原文删除/替换正文加入的 diff 在可滚动正文中；批准、拒绝、暂不处理按钮固定在页脚。其它授权按钮不会与审阅同时挂载。隐藏/重开保留待审提案；到期、拒绝和撤权阻止执行。正文、diff、授权和提案不持久化。

人工批准在 UI 线程消费一次 PendingReview，绑定当前租约和原 SSH handle；同一 EntityId 换成新连接不能沿用授权。写入使用现有独占创建的同目录临时文件 owner，写完并关闭后才以 OpenSSH `posix-rename@openssh.com` version 1 替换。没有该扩展时拒绝，无 delete/write 或非原子回退。完整正文、size/mode/mtime、canonical 路径在 staging 前及 rename 前再次复核，提交后读取精确 replacement 确认。失败不自动重试或重放。撤权、取消或丢失确认后的远端结果可能未知；旧租约的迟到 success 不被发布。

这不是远程文件系统原子 compare-and-swap：SFTP v3 没有 inode 锁、原子 no-follow 或与条件检查一体的 rename。诚实服务器上的这些检查可以拒绝已观察到的并发变化，不能排除恶意服务器或最后检查后的服务端竞争。POSIX rename 只保证整体内容可见性，不保证 crash durability；沿用现有 writer 的 rwx mode 保留，不保留 ownership、ACL、special mode bits 等元数据。独立审查、原生 UI、供应商客户端和跨平台验收必须分别记录，不从测试或编译推断。

A 的独立复核实证发现：文件准备期间，失败的重新授权改变 UI revision，但保留旧 authority；旧完成帧先因 revision 不同被丢弃，计数没有释放。32 次重复可使没有活动准备任务的应用仍报告 Busy。A 的源码、门禁与失败证据保持冻结，作者门禁通过不能覆盖该 P2。

B 为每个准备任务记录 revision 与独立 preparation ID。回调只释放匹配的所有权项，再执行原 revision/lease/会话审阅准入检查；成功替换授权和撤权清空旧所有权。旧帧不能释放新 generation 的槽或进入新审阅，闭合 IPC 回复也释放自己的项。新修复、原断言重放、完整工程门禁、新非作者及原生验证分别记录。
