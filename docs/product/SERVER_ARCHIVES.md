# 服务器归档 / Server archives

本批工程门禁和真实 OpenSSH 已通过，独立复核与原生边界分别记录。
设计见 [ADR 0135](../adr/0135-reviewed-server-archive-packing.md)，当前证据见
[集中验证记录](../testing/records/2026-11-15-server-archive-integration.md)。

## 操作

1. 在当前 SSH 会话的文件面板，用远端复选框选择同级普通文件或目录。
2. 展开「传输设置 → 归档传输」，选择 ZIP 或 tar.gz，填写新的远端归档名称。
3. 选择「审核远端打包」。先审核当前主机和完整只读准备命令，确认后扫描源。
4. 再审核完整成员、类型、大小、摘要、新目标、临时目录、完整打包命令和标准输入。
   第二次明确确认后才在服务器压缩。
5. 只有完整读回和成员核对成功后才显示完成。刷新列表后，可另行下载归档或使用
   [下载解包](ARCHIVE_TRANSFERS.md)，不会自动继续其他操作。

需要远端 macOS/Linux 和 Python 3.8+ 标准库，并能核验新建对象的私有权限。
无法清除或验证继承 ACL 时拒绝，源和父目录的权限不修改。
工具 PATH 检测结果不提供打包授权，
也不验证其他 tar/zip/Zmodem 程序的功能。

## 限制与中断

接纳 1–32 个同级源，目标必须是同一父目录中不存在的新归档；不覆盖已有目标。
沿用客户端归档的大小、成员、层级和可移植路径限制。符号链接、硬链接、特殊文件、
大小写别名及不明确的成员全部拒绝，不跳过拒绝项后继续打包。

审核绑定当前实际 SSH 连接、目录、选择、格式和名称。任何变化都需重新准备；
找到同名主机或复用会话显示 token 不能恢复旧授权。标准输入完整展示后按原字节
发送，不通过摘要替代用户尚未看到的执行内容。

失败、取消、断线或未知结果可能保留私有临时目录和新目标。先检查实际状态，
不会自动重试、删除、覆盖或把临时路径写入重启恢复授权。未知写入保留进程内
隔离和关闭保护。SSH 通道关闭不表示远程压缩进程已经终止。

三套主题使用同一注册表和组件 token，正文审核可滚动，确认/取消保持固定。
完整原生平台交互、Windows 安全解包、Zmodem、中继与性能收益仍独立验收。

## English

Engineering checks and real OpenSSH interoperability have passed. Independent
review and native acceptance boundaries are recorded separately.

Select sibling ordinary files or folders using the remote checkboxes. Under
Transfer settings → Archive transfer, select ZIP or tar.gz and enter a new
archive name. **Review server packing** first shows the current host and complete
read-only inspection command. Confirming that review only scans sources.

A second review presents every member, type, size and digest, the new target,
staging folder, complete packing command and exact standard input. Only its
separate confirmation permits compression on the server. Completion requires
full archive readback and member verification. Refresh afterwards and separately
choose download or extraction.

The remote host needs macOS/Linux, Python 3.8+ standard libraries and verifiable
private permissions. Packing fails closed if inherited ACLs cannot be cleared or
verified; source and parent-directory permissions are preserved. One to 32 sibling
sources are accepted within the documented archive budgets; the new target must
be a sibling and remain absent. Links, hard links, special files and ambiguous
or nonportable names are rejected. Existing targets are preserved.

Changing the actual SSH connection, folder, selection, format or name retires
approval. Failure, interruption or an unknown outcome can retain staging and
destination isolation; nothing is automatically retried, removed or resumed
after restart. Closing an SSH channel does not prove the remote process stopped.
Native platform interactions and release acceptance remain separately unverified.
