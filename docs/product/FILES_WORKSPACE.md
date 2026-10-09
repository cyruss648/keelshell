# 文件工作区 / Files workspace

文件操作使用当前已认证 SSH 会话的 SFTP 通道，远端路径与终端工作目录独立。
上传、下载、目录创建、重命名、删除、权限修改和内容保存均保留已有审核流程；
目录比较只读取当前成功加载的 canonical 远端目录与显式填写的本地目录。

名称、权限和本地传输路径草稿在语言切换时保留。文件与传输操作按可用宽度换行，
AI 侧栏打开后可以纵向滚动访问全部完整标签。文件列表保留表头和至少一个真实文件行，
编辑、比较和传输卡片在操作区内滚动，不会挤出列表。待确认时审核内容与确认/取消按钮
暂时替换操作工具栏；完整审核文字可滚动检查，两个按钮保持在操作滚动区之外，取消
会恢复原有草稿。提示文字在已展开的状态下也按当前语言重新绘制。
默认中文/跟随系统外观规则继续沿用应用设置。

Files use the active authenticated SSH session's SFTP channel. The remote path
is separate from the terminal working directory. Upload/download, creation,
rename, deletion, permissions and content saving retain their explicit review
flows. Directory comparison reads the last successfully loaded canonical remote
directory and the explicitly entered local directory.

Language changes preserve name, permissions and local-path drafts. File and
transfer controls wrap to the available width and remain reachable by vertical
scrolling with the assistant open. The browser reserves its header and at least
one real file row while editor, comparison and transfer cards scroll in the
action area. A pending review temporarily replaces the action toolbar with its
complete scrollable text; Confirm/Cancel stay outside the action scroll area.
Cancellation restores the drafts. Visible
application hints read the live language when redrawn. Chinese and System
appearance remain the defaults.

实现与验证边界见 [ADR 0043](../adr/0043-responsive-files-and-live-tooltip-translations.md)
和 [测试记录](../testing/records/2026-10-05-files-responsive-tooltips.md)。布局/协议测试
不等同于 Windows/Linux 原生桌面、真实客户 SSH 或所有模态可访问性验收。

## 并行传输候选 / Parallel transfer candidate

传输审核后加入当前已认证 SSH 会话的内存队列；可以继续浏览和审核其它传输。
文件区提供 1–4 个并发槽位，默认 2，最多 32 个未结束任务。每项任务独立显示
目标、已确认字节数、等待/运行/暂停/取消/错误/完成状态；暂停保留槽位和路径锁，
调低并发不会中断已有任务。任务列表有独立的有界滚动区，选择任务可查看完整路径。

普通文件上传通过独占临时文件和 POSIX 原子重命名发布；不同已有目标也能并行。
缺少原子重命名能力或目标为链接时拒绝写入，不降级为直接截断。下载仅新建目标；
目录和续传仍可能保留部分内容。源/目标重叠、目录父子目标和同名目标保守互斥。
等待中的写入未收到确认时显示“结果未知”，本地目标在同应用跨连接隔离；远端目标
按已验证 host/port/host-key 范围跨连接隔离，其它独立目标继续工作。重新连接不会解除
隔离。“检查隔离目标”只读显示完整目标和精确记录 ID，之后需显式审核解除隔离的
未知风险；检查无法证明迟到写入已停止，原任务仍显示结果未知。撤权、连接变更、
观察快照或隔离记录改变会拒绝确认。隔离为内存状态，重启不证明旧远端工作停止。
队列不会自动恢复、重连或重放；取消不代表远端写入已被撤销。

After review, transfers join an in-memory queue bound to the authenticated SSH
connection. Files offers 1–4 concurrent slots (default 2), at most 32 unfinished
jobs, individual acknowledged progress/control/results and a bounded scrollable
job list. Pausing retains a slot and path locks; reducing concurrency leaves
already active jobs running. Selecting a job shows its complete paths.

Ordinary file uploads use exclusive temporary staging and POSIX atomic rename,
so distinct existing destinations can run concurrently. Unsupported atomic
rename or symlink targets fail closed. Downloads create new local targets;
directory transfers and explicit continuations can retain partial output.
Overlapping source/destination paths, parent/child trees and the same target
exclude concurrent writes. An unacknowledged mutation displays an unknown result
and isolates local destinations across this application and remote destinations
within the verified host/port/host-key scope, including fresh connections. Other
targets continue. “Inspect isolated target” shows complete paths, exact record IDs
and read-only observations before explicit unresolved-risk consent. Inspection
cannot prove late writes stopped; old jobs remain unknown. Revocation, connection
changes, changed observations or isolation records reject confirmation. Isolation
is in memory; restarting does not prove old remote work stopped. Jobs do not
restore, reconnect or replay automatically;
cancellation cannot imply rollback.

普通保存、创建、重命名、删除、权限、目录同步和 MCP 人工批准的文件替换，
与传输队列共用实际写入准入；其它面板不能绕过活跃或未知目标。目录同步持有源与
目标两棵树，子文件临时写入复用同一所有权。独立目标仍可并行；直接冲突操作立即
显示忙碌或隔离错误，不会自动等待后重放。普通文件操作的未知目标也可从
“检查文件隔离”只读查看完整目标/ID，并另行确认未知风险；MCP 提案批准不是风险确认。
临时文件清理无确认同样保留最终目标及临时路径隔离；原任务状态不会因迟到清理改变。

Ordinary save/create/rename/delete/permissions, directory synchronization and
human-approved MCP file replacement share actual mutation admission with queues.
Other panels cannot bypass active or unknown targets. Synchronization owns both
trees and reuses one owner for child temporaries. Independent targets remain
parallel; direct conflicts return busy/isolation errors without delayed replay.
“Inspect file isolation” reads complete paths/IDs for ordinary file operations
before separate risk consent. MCP proposal approval is not risk acknowledgement.
Unacknowledged temporary cleanup isolates both final and temporary paths; late
cleanup cannot revise an old unknown job.

已确认的字节进度不证明可写句柄关闭或最终发布完成。可写 CLOSE 尚无确认时，
取消或丢失结果仍保留“结果未知”和相同目标隔离；明确 STATUS 拒绝则是已知失败。
只读来源与计划扫描的 CLOSE 不产生远端写入隔离。普通原子上传保持独立目标并行，
原地写入与已有目标续传保留已有的保守整侧锁。临时文件 CLOSE/删除清理缺少确认
同时保留最终目标与临时路径记录，不重试已发送 CLOSE 的失效句柄。

Acknowledged bytes do not prove writable handle closure or final publication.
Cancellation or loss of an unacknowledged writable CLOSE remains unknown with the
same destination isolation; a valid STATUS refusal is a known failure. Read-only
source/planning CLOSE does not create remote-write quarantine. Ordinary atomic
uploads retain distinct-target parallelism; in-place writes and existing-target
continuations keep their conservative filesystem-side locks. Missing temporary
CLOSE/removal cleanup replies retain both final and temporary records, without
retrying an already-sent CLOSE on an invalidated handle.

v1 普通写入绕过隔离和 v2b 可写 CLOSE 未确认提前释放的独立实证均保留；v3 修复
已通过作者完整门禁、OpenSSH 与 macOS 开发构建，全新独立审查与主线整合仍待完成。原记录保持历史范围，见
[并行传输记录](../testing/records/2026-10-06-parallel-transfers.md) 与
[写入隔离修复](../testing/records/2026-10-06-parallel-transfers-mutation-repair.md)。

新关闭边界见 [可写 CLOSE 修复](../testing/records/2026-10-06-parallel-transfers-writable-close.md)。

传输等待采用每个任务独立的“确认 I/O 后重置空闲期限”：上传、下载、原子上传及
目录/续传可以持续超过一个连接超时间隔。已确认暂停不计入活动空闲时间；界面
刷新或其它任务进度不能替本任务续期。全内容/全树只读核验保持固定 30 秒期限。
卡住的 WRITE、可写 CLOSE 或发布仍有界失败并保留未知目标隔离，迟到确认不自动
释放隔离。设计与作者验证见 [空闲期限设计](../adr/0059-confirmed-transfer-idle-timeouts.md)
和 [测试记录](../testing/records/2026-10-06-transfer-idle-timeouts.md)。新的非作者复审及
主树组合门禁已通过；macOS受控双上传、暂停/继续、同时运行、完成、完整路径、
队列可达性及精确内容见 [原生记录](../testing/records/2026-10-06-parallel-transfers-native-v5.md)。
其它传输模式、最小窗口和其它平台仍待验收。

Each transfer renews its own idle interval only after confirmed I/O. Uploads,
downloads, atomic uploads and directory/continuation jobs may progress longer
than one connection timeout. Acknowledged pause excludes active idle time;
presentation activity and another job's progress cannot renew it. Full-content
and full-tree read-only checks retain fixed 30-second limits. Unanswered WRITE,
writable CLOSE or publication remains bounded and preserves destination
quarantine; a late reply cannot silently release it. See the
[idle deadline design](../adr/0059-confirmed-transfer-idle-timeouts.md) and
[test record](../testing/records/2026-10-06-transfer-idle-timeouts.md).
Fresh independent review and the combined-main gate passed. The controlled
macOS two-upload/pause/continue scene is recorded in the
[native record](../testing/records/2026-10-06-parallel-transfers-native-v5.md); other
transfer modes, minimum-window and other-platform acceptance remain open.

## 审核式目录镜像 / Reviewed directory mirror

普通合并默认保留目标独有项。额外的两个镜像方向入口只生成只读计划，完整审核后
可精确删除常规文件和目录树；全部子节点逐项审核、子先父后删除，链接和冲突阻止整份计划。逐项结果保留
已完成、已知拒绝、取消及未知边界，既有树所有权和隔离继续适用。详见
[镜像指南](DIRECTORY_MIRROR.md)，递归子树候选已实现，新的非作者复核、主树整合及原生桌面验收仍开放。

Merge keeps destination-only entries by default. The two explicit mirror
directions prepare read-only plans before full confirmation can remove exact
regular files and fully expanded directory subtrees. Children are removed before
empty parents; links and conflicts block the whole plan. Per-item results preserve completed, rejected,
cancelled and unknown boundaries under existing tree ownership and quarantine.
See the [mirror guide](DIRECTORY_MIRROR.md); new independent review, main integration and desktop
acceptance of the recursive extension remain open.

测试准入不改变产品所有权：已有本地写入仍保守排斥整个进程本地侧的竞争访问；有界 setup 分组仅用于共享该真实资源的受控夹具；同实际测试 App 的窗口/运行时继承弱组，真实工作线程及队列 join 结束后才放行下一组。Historical CI ownership is not identified by a local fixture pass. 详见[ADR0078](../adr/0078-process-local-file-fixture-admission.md)。
