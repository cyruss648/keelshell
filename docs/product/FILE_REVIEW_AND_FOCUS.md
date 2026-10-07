# 文件完整审核与编辑定位 / Complete file review and editor focus

待确认的文件操作默认使用紧凑审核区，便于继续查看文件列表。选择“展开审核”后，
文件区显示更大的完整审核正文；终端与 AI 侧栏保留原空间。正文保留每条原始路径、
哈希、元数据、内容和空行，可以横向与纵向滚动。展开时正文获得焦点，方向键、
Page Up/Down、Home、End 用于浏览。正文中的 Enter 不批准操作。

确认、取消和“收起审核”固定在正文之外。收起仅返回原视图，审核目标与内容不变；
取消撤销本次待确认操作，保留草稿。每份新审核从紧凑模式和正文开头开始。
文件很长时仍需滚动逐段检查；展开不意味着所有内容同时适配一屏。

打开远程文件后，文件列表上方的固定路径行提供“编辑草稿”和“输入差异”。选择
入口会展开对应输入区，并把键盘焦点置于同一个草稿或差异编辑器。“返回文件”
保留输入；底部可切换草稿与差异、解析差异或发起保存审核，并显示操作状态。
差异解析只改变草稿，不写远端。保存仍须完整审核并显式确认；审核后草稿变化、
会话断连、撤权或暂停继续阻止旧确认，未知写入隔离规则不变。

A pending file operation starts with compact review, leaving the browser available.
“Expand review” gives its complete text the file pane's available space while the
terminal and assistant retain their allocations. Every original path, hash,
metadata row, content row and empty line remains present with horizontal and
vertical scrolling. The expanded body receives focus; arrows, Page Up/Down,
Home and End inspect it. Enter in the body does not approve.

Confirm, Cancel and Collapse stay outside the scrolling body. Collapsing changes
presentation without changing the reviewed operation or bytes. Cancel removes
that pending proposal and retains drafts. Each new review starts compact and at
the top. Long files still require scrolling; expansion does not promise that all
contents fit on one screen.

After opening a remote file, the pinned path row offers “Edit draft” and “Enter
patch”. Each reveals the corresponding input and focuses the same existing
editor state. Back to files preserves both buffers. Fixed lower actions switch
editors, parse a patch or request save review, with the operation status visible.
Patch parsing changes only the draft. Remote saving still requires complete
review and explicit confirmation. Changed drafts, suspension, disconnection and
revocation keep the original guards; unknown mutation isolation is unchanged.

候选源码与回归准备不代表原生验收；目前新布局的 macOS 像素、最小窗口操作、
完整键盘导航、VoiceOver 和 Windows/Linux 桌面仍待复验。详见
[ADR 0076](../adr/0076-expandable-file-review-and-editor-focus.md) 与
[测试记录](../testing/records/2026-10-07-file-review-viewport.md)。

The candidate's source and prepared regressions are not native acceptance. New
macOS pixels, minimum-window operation, full keyboard navigation, VoiceOver and
Windows/Linux desktop checks remain open; see the linked design and test record.
