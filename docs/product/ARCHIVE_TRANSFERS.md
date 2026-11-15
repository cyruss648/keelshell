# 审核式归档传输 / Reviewed archive transfers

客户端实现已接入；集中工程验证、真实 OpenSSH 协议测试及独立复核结果见
[测试记录](../testing/records/2026-11-15-reviewed-archive-transfer.md)。
这项能力属于 FILE-05 的客户端部分。[服务端工具观察](ARCHIVE_TOOL_INSPECTION.md)
另行审核；服务端实际打包、版本与功能验证、Zmodem、中继加速和三平台完整
原生验收另行追踪。

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
安全解包已接入 macOS、Linux 与 Windows 后端。Windows 逐组件保留祖先句柄，
在创建时设置私有 DACL，完整验证私有目录树后以无覆盖方式发布；只接纳满足
对象身份、权限和 NT 发布契约的本地 drive 文件系统。UNC、设备路径、重解析、
多硬链接与不可移植名称均拒绝。界面已启用 Windows 下载解包入口，但 Windows
原生文件系统和桌面流程尚未执行验证，实际工程与 Linux 本地运行范围见
[本地后端记录](../testing/records/2026-11-15-windows-local-backend.md)。
取消或丢弃 await 后，后台 mutation worker 继续持有保留权直到确实退出；
失败及未知 staging 保留隔离，不按路径自动删除。

## 限制与恢复

归档最多 64 MiB，展开最多 128 MiB，2048 项，32 层，单成员路径最多
4096 字节，所有成员名累计最多 48 KiB。超过限制直接拒绝，不能省略成员后执行。
压缩与解包在客户端完成，不需要远端安装 ZIP 或 tar。
这不表示服务器端压缩加速已经实现。
ZIP 支持单盘 ZIP32 的 stored/deflate 成员，不接纳 ZIP64 和多盘格式。
tar.gz 支持普通成员、有界 GNU 长名称及严格 PAX 元数据子集。
PAX 允许 `path`、`size` 和验证后丢弃的时间/所有者字段；不恢复这些元数据。
单个 PAX 头最多 8 KiB，累计元数据最多 2 MiB + 48 KiB，元数据头最多
4096 个。整体 tar 流上限不增加。局部属性优先于全局默认，再到普通头。
完整原始大小先解析并校验，拒绝负值、溢出高位或终止符后隐藏的数值。
重复键、空值删除、字符集/厂商/稀疏扩展、链接和歧义扩展链均拒绝。
普通备用路径同样需要满足可移植规则，因此 Python 以 `?` 替换 Unicode
普通头的归档仍可能拒绝；合法 UTF-8 PAX path 搭配安全备用路径可以接纳。
设计见 [ADR 0134](../adr/0134-bounded-pax-archive-metadata.md)；本次增量的集中
门禁和非作者源码复核已通过，原始 size 截断 P2 已修复并复核关闭。
实际结果及原生边界见
[集中验证记录](../testing/records/2026-11-15-archive-tools-pax-integration.md)。

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
Server-side packing, version/feature verification, Zmodem, relay acceleration
and full native platform acceptance remain separate open work.
Extraction integrates no-replace directory backends for macOS, Linux and
Windows. Windows retains every ancestor handle, creates objects with a private
DACL and verifies the complete staged tree before publication. Only supported
local drive filesystems are admitted; UNC/device paths, reparse points, multiple
hard links and nonportable names are rejected. Windows extraction controls are
enabled, but native filesystem and desktop execution remain unverified. See the
[local backend record](../testing/records/2026-11-15-windows-local-backend.md)
for actual platform-specific results. A cancelled await does not release an
active mutation worker's reservation; unknown staging remains quarantined.
The codec
admits single-disk ZIP32 stored/deflate and regular TAR members with bounded GNU
long names and a strict PAX subset. ZIP64 and multidisk ZIP are not admitted.
PAX accepts `path`, `size` and validated but discarded time/owner fields, with
local values overriding global defaults and the ordinary header. Each metadata
header is limited to 8 KiB, cumulative payloads to 2 MiB + 48 KiB, and metadata
headers to 4096, within the existing whole-stream allowance.
The complete raw size is checked before overrides or budgets; negative values,
overflowing high bits and hidden digits after a terminator are rejected.
Duplicate keys, empty-value deletions, vendor/encoding/sparse extensions, links
and ambiguous extension chains fail closed. Ordinary fallback paths must remain portable;
Python archives whose Unicode fallback contains `?` can therefore be rejected
even with a valid PAX path. Safe fallbacks with UTF-8 PAX paths are supported.
The current increment passed the combined gate and independent source review;
the raw-size truncation P2 was fixed and independently closed. See the
[integration record](../testing/records/2026-11-15-archive-tools-pax-integration.md)
for executed results and the remaining native acceptance boundaries.
