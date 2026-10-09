# 多选与审核式批量传输 / Multi-selection and reviewed batch transfers

状态：2026-10-09已完成本功能限定验证：最终842完整工程、同源macOS开发包和受控原生三上传／三下载（每方向三文件106字节）通过。新的非作者现场复核关闭英文深色默认队列入口／分类计数／详情／返回保持的可发现性P2；其它原生范围仍独立开放。原首轮完整失败和同源布局复现保留，次级操作卡片按真实工具区滚动访问，不承诺默认可见。仅用于远程SSH会话的SFTP，不提供本地终端管理。详见[测试记录](../testing/records/2026-10-08-file-browser-batch-transfers.md)。

Status: this feature completed scoped validation as of 2026-10-09. The final 842-input snapshot passed complete engineering checks, a matching macOS development package and a controlled native flow of three uploads and three downloads, totaling 106 file bytes in each direction. A fresh independent live review closed the discoverability issue for the English/Dark default queue entry, category counts, details and preserved return state. Other native scopes remain open. The first complete failure and same-source layout reproduction are retained; secondary cards use actual tools scrolling without promising default visibility. Local browsing supplies sources and destinations for remote SSH SFTP transfers.

## 选择与导航

1. 明确选择本地目录并浏览远程目录。启动不扫描本地磁盘。
2. 勾选条目前的checkbox。普通行点击单选；Ctrl／Cmd行点击切换；Shift行点击按当前可见排序选范围。每侧最多32项，超限保持原选区。
3. 工具区显示两侧选择数量，提供清空选择。排序保留路径身份；隐藏文件从选区移除，显示回来不恢复选择。刷新或进入另一目录清空该侧选区。
4. 多项选中时，单项编辑／权限／改名／删除／填框没有唯一目标，保持停用。单项与现有人工审核、目录／续传／文本草稿流程继续使用原入口。

Choose the local and remote folders explicitly. Check individual rows, use Ctrl/Cmd to toggle row selections, or Shift-click a row to select a range in the current visible order. Each side allows up to 32 entries. The tools area shows both counts and Clear selection. Sorting retains path identity. Hiding entries removes them from selection; showing them again does not select them. Refreshing or navigating clears that side. Actions requiring one target are disabled for a multiple selection.

## 审核批量上传与下载

点击“审核批量上传”使用所选本地条目和实际已加载的远程目录；“审核批量下载”使用所选远程条目和明确已加载的本地目录。准备仅后台读取metadata／目录快照，不创建目标、不写文件。

完整审核逐项展示来源、目的、字节数、覆盖策略、目录文件／子目录数量，以及拒绝项原因。普通上传允许审核时已存在的普通远端文件被原子替换；目录目标必须不存在，并按项传输；整棵目录非原子，失败或取消会保留已经完成的项。普通下载目标必须不存在，不覆盖本地已有文件。链接、特殊或未知类型、路径越界、不可移植名称、重复目的等明确拒绝。

“确认可传输目标”仅加入本次审核中可传输的明确子集；所有拒绝项都不会执行。取消不加入任何任务。改变目录、选择或会话会撤销未执行计划，旧绘制按钮不能批准新计划。批准后的精确目标各自显示在原传输queue，继续提供暂停／继续／取消和逐目标结果。部分失败不会使成功目标变成失败，也不会自动重试。断线或取消后，已发出请求可能保持未知结果，仍隔离目的并要求新的检查／审核。

Review batch upload uses selected local entries and the actually loaded remote folder. Review batch download uses selected remote entries and the explicitly loaded local folder. Preparation reads metadata or directory snapshots in the background and creates no destination. Every review row shows exact source/destination paths, confirmed bytes, overwrite policy, directory counts or its rejection reason. Regular-file uploads atomically replace an existing reviewed regular file. Folder transfers are not atomic: they create items individually and retain completed items after failure or cancellation. Folder destinations and all normal local downloads must be absent. Links, special/unknown types, unsafe paths/names and duplicate destinations are rejected explicitly.

Confirm admissible targets queues exactly the admissible subset shown in that review. Cancel queues nothing. Folder, selection or session changes revoke unexecuted plans. Old painted approval controls cannot approve a replacement plan. Each target retains the existing queue's pause/resume/cancel and result controls; partial failures are shown individually, with no automatic retry. Issued requests may have unknown outcomes after disconnection or cancellation; their destinations stay isolated and require fresh inspection/review.

## 预算与验收边界

一次32个选择项、累计16 GiB、只读准备30秒预算；目录继续受原深度／项数预算限制。普通文件上传保留实际来源descriptor并在后台执行／原子发布前复核；下载在检查已打开远端句柄后才创建本地文件。metadata不代表内容锁，SFTP v3无法提供对其它进程的原子版本检查。阻塞文件系统调用仍不能强制结束。

The batch is bounded to 32 selected entries, 16 GiB and a 30-second read-only preparation budget. Existing directory depth/count limits remain. Regular-file uploads retain their actual source descriptor and revalidate before execution and publication. Downloads inspect the opened remote source before creating a local file. Metadata observations are not content locks; portable SFTP v3 cannot atomically exclude other writers. A blocked filesystem call cannot be forcibly terminated.

固定队列入口和返回行为见[交互指南](TRANSFER_QUEUE_ACCESS.md)。本次完整工程、同源macOS开发包和限定受控SFTP原生已验证；拖放、最小OS逻辑几何／IME／VoiceOver、System原生主题、其它平台桌面、OpenSSH桌面GUI、精确新提交CI和发行安装仍独立OPEN，不能以本功能验收宣称完整产品完成。

See the [queue guide](TRANSFER_QUEUE_ACCESS.md). Complete engineering checks, the matching macOS development package and the scoped controlled native SFTP flow are validated. Drag-and-drop, minimum OS geometry/IME/VoiceOver, native System-theme switching, other target desktops, OpenSSH desktop GUI, exact-commit CI and release installation remain independently OPEN. This feature is not whole-product acceptance.
