# 本地与远程文件浏览 / Local and remote file browsing

状态：2026-10-08 已整合到主副本。v5 候选新非作者限定复核、根完整门禁和新 macOS 标准包通过；新 macOS 宽窗口已验证系统目录选择器、隐藏项、子目录导航、大小排序及经人工审核的双向 SFTP。新的非作者组合源码／工程／宽窗口原生证据复核无 P1/P2；精确提交 CI、最小原生窗口和其它平台桌面仍开放。详见[主树整合记录](../testing/records/2026-10-08-local-file-browser-main-integration.md)。

Status: integrated into the main working copy as of 2026-10-08. Fresh independent
candidate review, the full main gate and a new macOS package pass. A fresh native
macOS wide window exercised the folder picker, hidden entries, folder navigation,
size ordering and reviewed bidirectional SFTP. Fresh independent review of the combined source, engineering results and wide-window
native evidence found no scoped P1/P2; exact-commit CI, the minimum native window and other target
desktops remain open. See the [main integration record](../testing/records/2026-10-08-local-file-browser-main-integration.md).

本地栏只为 SFTP 选择传输来源和目的，不是本地终端或默认磁盘扫描器。
点击文件夹图标打开系统目录选择器，或输入本地绝对目录后按 Enter／刷新。
启动不读取 HOME。打开普通目录和上级按钮只读取明确导航到的目录的一层元数据。
链接、特殊类型和无法读取的目录均明确处理；超过预算不显示部分成功列表。

The local pane selects SFTP sources and destinations. Choose a folder with the
system picker or enter an absolute local folder and press Enter or Refresh.
Startup does not read HOME. Open and Up browse only the explicitly navigated
folder's immediate metadata. Links and special types cannot be opened or used
as transfer sources; a failed or over-budget listing is not partially adopted.

文件条目的“使用”仅填入传输路径。选择普通目录后可点“使用本地所选”，也可点
“使用本地目录”填入当前目录。选中远程条目后，“选作下载目录”填入当前本地目录
与远程名称连接的目标；已有文件的续传仍需用户明确选择已有路径。
下一步由用户点击上传／下载，再审核准确来源和目标，最后确认。
远程 Edit 打开另一文件前，未保存草稿仍需确认放弃；取消保留原草稿。
普通下载不覆盖已有文件；目录和续传继续使用原计划检查和完整审核。

Use on a file only fills the transfer input. Use local selection also accepts an
ordinary selected folder, and Use local folder selects the current folder.
With a remote entry selected, Use as download folder fills a destination formed
from the loaded local folder and that remote name. Resuming an existing file
still requires choosing its existing path explicitly. Click Upload or Download,
review the exact source and destination, then confirm. Editing another remote file
requires approval to discard an unsaved draft; Cancel preserves that draft. Ordinary downloads do
not overwrite existing files; folder transfers and resumes retain their full
plan verification and review.

名称、大小和修改时间表头切换排序；隐藏文件按钮切换显示。
本地识别 dot-name／Windows hidden 属性；远程隐藏项按 SFTP 名称是否以点开头判断。
关闭隐藏项时清除不可见选区和未执行审核，但不取消已经发出的传输。
编辑路径或另选条目会撤回旧审核；
后到的旧只读准备结果不会重建该审核，正在运行的写入仍报告原任务真实结果。
非 UTF-8 本地路径可保留原生编码浏览，当前文本型传输输入不能安全表示它，因此
不会把 lossy 显示名称当成传输路径。

Name, Size and Time headers toggle ordering; the eye button toggles hidden entries.
Local filtering recognizes dot names and Windows hidden attributes; remote
filtering recognizes dot names, without inventing unsupported SFTP attributes.
Hiding a selected entry clears that selection and an unexecuted review,
without cancelling an issued transfer. Editing paths or choosing another entry
withdraws the old review; a late obsolete read-only plan cannot recreate it.
Running writes still report their original outcomes. Native non-UTF-8 paths may
be browsed, but cannot populate the current text transfer input through lossy
conversion.

本地读取预算为 4096 项／1 MiB 文件名和协作式 5 秒检查；单个阻塞文件系统调用不能
硬取消。仅支持单选，拖放／多选和跨平台原生布局及系统 picker 仍待后续开发／验收。
设计见 [ADR 0079](../adr/0079-explicit-local-and-remote-file-browsing.md)，
实际运行边界见 [测试记录](../testing/records/2026-10-08-local-remote-file-browser.md)。

Local reads are limited to 4096 entries and 1 MiB of encoded names, with
cooperative five-second checkpoints. A blocked filesystem call cannot be
forcibly cancelled. This milestone is single-selection; drag-and-drop,
multi-selection and target-native layout/picker acceptance remain open.

## 后续批量候选 / Batch candidate

上面的单项浏览是已整合历史范围。当前[多选／审核式批量传输](FILE_BROWSER_BATCH_TRANSFERS.md)已实现checkbox、Ctrl/Cmd／Shift选区和逐目标审核；最终842完整工程、同源macOS开发包及受控原生三上传／三下载（每方向106字节）通过，首轮完整失败及同源目录续传复现保留。[固定队列入口](TRANSFER_QUEUE_ACCESS.md)默认摘要、详情、返回保持的英文深色可发现性P2由新非作者现场复核关闭，次级工具卡片仍按真实滚动访问。拖放、最小OS逻辑几何／IME／VoiceOver、System原生主题、其它平台桌面、OpenSSH桌面GUI和发行安装独立OPEN，不能以本功能验收扩大成全产品通过。详见[实际记录](../testing/records/2026-10-08-file-browser-batch-transfers.md)。

The single-item browser above is historical scope. Current [reviewed batch transfers](FILE_BROWSER_BATCH_TRANSFERS.md) implement checkboxes, Ctrl/Cmd/Shift selections and per-target approval. The final 842-input snapshot passed complete engineering checks, a matching macOS development package and a controlled native three-upload/three-download flow totaling 106 file bytes in each direction; first complete and same-source directory-continuation failures are retained. A fresh independent live review closed the English/Dark [fixed queue entry](TRANSFER_QUEUE_ACCESS.md)/counts/details/return discoverability issue; secondary cards still use actual tools scrolling. Drag-and-drop, minimum OS geometry/IME/VoiceOver, native System-theme switching, other desktops, OpenSSH desktop GUI and release installation remain independently open. This feature is not whole-product acceptance. See the [actual record](../testing/records/2026-10-08-file-browser-batch-transfers.md).
