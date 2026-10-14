# 拖放文件上传 / Drop files to upload

状态：作者候选，2026-10-13。实际编译、回归、独立复核和原生桌面拖放待验证；不作为已验收功能。

Status: controlled GPUI behavior verified on 2026-10-14. Six drop regressions and 162 file-module tests passed. Native file-manager drag-and-drop remains unverified.

## 操作方式 / Interaction

将系统文件管理器中的本地文件或文件夹拖到当前远程文件区域。批量工具栏显示“拖入本地文件或文件夹，审核后上传”，区域可接收时边框随拖入高亮。提示与现有批量控制同行换行，不额外占用文件列表高度。一次最多32项，不要求先选择本地浏览目录。目标是当前实际加载的远程目录，每个来源使用其原文件名；同一次可从不同本地目录取来源。

Drop local files or folders from the system file manager onto the remote file area. The batch toolbar says “Drop local files or folders here, then review to upload” and the border highlights a drag when ready. The hint wraps with the existing batch controls instead of reducing the file-list height. Each drop accepts up to 32 entries and does not require opening a local browser folder. Each source retains its basename under the actually loaded remote folder; sources may come from different local folders.

松开鼠标只开始后台只读准备。完整审核显示每项真实本地路径、远程目标、字节数、覆盖策略、目录数量和拒绝原因。用户点击“确认可传输目标”后，只有审核中可传输的明确子集加入原传输队列。取消审核或只拖入后移出均不会加入任务或写入远端。

Releasing the drag starts read-only preparation in the background. Review lists every exact local source, remote destination, byte count, overwrite policy, folder count and refusal. Only “Confirm admissible targets” queues the displayed admissible subset. Cancelling review or leaving without dropping queues no task and writes nothing remotely.

## 限制与撤回 / Limits and revocation

- 沿用批量传输的32项、16 GiB、30秒只读准备和目录扫描预算。文件保留来源descriptor；目录沿用原完整计划与执行前复核。
- 仅接受绝对路径的普通文件或目录；链接、Windows reparse对象、特殊类型、未知／缺失来源、不安全或不可移植名称、重复／大小写冲突目的明确拒绝。
- 当前操作或审核尚未结束、会话暂停或没有远程目录时拒绝新拖放，并显示说明，保留原任务和审核。超过32项整批拒绝，不默默截断。
- 远程／本地导航、选择或会话变化撤销未执行计划；旧绘制的拖放回调不能把来源送到新的目录。后台只读结果仍受原审核版本撤回保护。
- 普通文件上传按原规则原子发布；目录逐项传输，失败或取消可能留下已完成文件。不会绕过目标隔离、未知结果、暂停／继续／取消规则。

The existing batch limits apply: 32 entries, 16 GiB, 30 seconds for read-only preparation and the established directory scan budgets. File plans retain source descriptors; folder plans retain their complete scans and revalidation. Only absolute ordinary file/folder paths are eligible. Links, reparse objects, special/unknown/missing sources, unsafe or nonportable names, and duplicate/case-alias destinations are refused. A busy, pending-review, suspended or unloaded session refuses new drops while preserving its current work. Oversized drops are rejected as a whole. Navigation, selection and session changes revoke unexecuted plans; a stale painted drop callback cannot retarget a source. Existing isolation, unknown outcomes and queue controls remain applicable. Folder transfers remain non-atomic.

本地访问只为远程SSH的SFTP传输提供来源；本功能不提供本地终端或自动写入。GPUI平台事件回归也不等同于macOS、Windows和Linux系统文件管理器的真实桌面验收。

Local access supplies SFTP sources for remote SSH sessions. This feature provides no local terminal and performs no automatic writes. GPUI platform-event regression is separate from native file-manager acceptance on macOS, Windows and Linux.
