# 审核式归档传输 / Reviewed archive transfers

客户端实现已接入；集中工程验证、真实 OpenSSH 协议测试及独立复核结果见
[测试记录](../testing/records/2026-11-15-reviewed-archive-transfer.md)。
这项能力属于 FILE-05 的客户端部分，服务端打包与能力探测、Zmodem、
中继加速和三平台完整原生验收另行追踪。

## 打包上传

1. 在本地文件列表中选择文件或目录，最多 32 个根选择。
2. 展开「传输设置」，在「归档传输」中选择 ZIP 或 tar.gz。
3. 填写新的远端归档名称，分别以 `.zip` 或 `.tar.gz` 结尾。
4. 点击「审核打包上传」，后台只读扫描并生成完整归档。
5. 核对源路径、新远端归档路径、格式、字节数以及全部成员后确认。

目标放在当前已成功加载的远端目录。路径输入框中的未提交导航不改变目标。
成员保留所选根的名称、相对目录和二进制内容，包括空目录。
准备和审核取消不发布远端文件。确认后重新校验源，变化时要求重新准备。
已有远端目标保留，不静默替换。

## 下载并解包

1. 选择一个普通远端归档文件。
2. 在「本地传输路径」填写一个尚不存在的新目录的绝对路径。
3. 选择与文件内容一致的格式，点击「审核下载解包」。
4. 核对源归档、新本地目录、格式、全部成员和展开字节数后确认。

后台完整读取和严格解码后才能进入审核。确认后重读远端源校验，在同父私有目录
完整准备，再发布新目录。不会恢复归档中的可执行权限、所有者或特殊权限。
不接纳链接、特殊类型、路径穿越、绝对成员路径、不可移植名称或冲突成员。
已有本地目标不覆盖。
本批安全解包后端面向 macOS/Linux；Windows 的原子新目录发布与权限处理尚待
实现，界面明确停用下载解包并说明原因，打包上传仍可使用。

## 限制与恢复

归档最多 64 MiB，展开最多 128 MiB，2048 项，32 层，单成员路径最多
4096 字节，所有成员名累计最多 48 KiB。超过限制直接拒绝，不能省略成员后执行。
压缩与解包在客户端完成，不需要远端安装 ZIP 或 tar。
这不表示服务器端压缩加速已经实现。
ZIP 支持单盘 ZIP32 的 stored/deflate 成员，不接纳 ZIP64 和多盘格式。
tar.gz 支持普通成员及有界 GNU 长名称，PAX 元数据和其它扩展暂不接纳。

审核绑定会话、浏览目录、选择、格式和路径草稿；任何变化均需重新准备。
进行中的操作保留原目标与真实结果。取消或断线后，应按状态检查新目标；
未知结果不自动重放。临时归档不写入持久传输恢复日志，重启后重新准备。
已确认完成的归档释放自身窗口关闭保护；更早的未决目标仍保留保护和隔离。
协议和文件系统竞争边界随 ADR 与测试记录公开。

## English

Expand Transfer settings to choose ZIP or tar.gz. For a packed upload, select up
to 32 local files or folders and enter a new archive name in the current remote
folder. For extraction, select one remote archive and enter an absolute, new
folder in the local transfer path. Preparation reads and validates the complete
archive without publishing a destination. Review all source and destination
paths, sizes and every member, then explicitly confirm.

Confirmation revalidates the source. Existing targets are preserved. Extraction
does not restore archived executable modes, ownership or special permissions.
Links, special files, unsafe paths, nonportable names, conflicting entries,
corrupt streams and oversized archives are rejected. Limits are 64 MiB compressed,
128 MiB expanded, 2048 entries, 32 levels, 4096 bytes per member path and 48 KiB
of accumulated member names.

Changing the session, folders, selection, format or path drafts invalidates a
review. Inspect the destination after interruption or an unknown publication
result; operations are never replayed automatically. Prepare again after restart.
Verified completion releases the archive's own window-close hold while preserving
any older unresolved destination and quarantine.
Server-side packing and capability detection, Zmodem, relay acceleration and full
native platform acceptance remain separate open work.
Extraction currently requires the macOS/Linux no-replace directory backend.
Windows extraction is disabled with an explicit explanation pending a safe
publication and permissions backend; packed uploads remain available. The codec
admits single-disk ZIP32 stored/deflate and regular TAR members with bounded GNU
long names. ZIP64, multidisk ZIP and PAX metadata are not admitted.
