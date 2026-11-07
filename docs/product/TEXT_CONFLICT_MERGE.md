# 远程文本合并与差异应用 / Remote text merge and patch

文件编辑使用当前已认证 SSH 的 SFTP 通道。打开文件时读取完整字节，并检查父目录、
叶类型、打开句柄、大小、权限和修改时间；只接受明确的常规 UTF-8 文件，拒绝 NUL、
链接、目录和缺少必要类型/大小信息的对象。编辑保存、合并与 patch 的每侧文件/结果上限为 1 MiB、
4,096 行，patch 输入另限 4 MiB / 17,408 行；读取不会把截断内容认作完整成功。

截至 2026-11-08，`merge_adopt_save_keeps_late_newer_draft_and_rebases_next_save`
已通过定向 GPUI/SFTP 和完整工程门禁；当前源码仍需固定提交、独立复核、精确包和
跨平台原生验收。

“读取远端并合并”是只读操作。应用保留打开时基线、本地草稿和新读取的远端版本，
组合独立修改；重叠修改、同位置插入和有歧义的插入边界成为冲突。可切换查看完整基线、草稿、当前远端及已解决的合并全文；每个冲突显示基线、
草稿及当前远端区域文本，可选择草稿、远端或手工输入的区域替换。上一项/下一项不会
丢失未采用的手工输入；已经采用的手工内容再次编辑后需要重新选择。取消/关闭合并
保留原草稿。“采用合并草稿”只改变本地编辑缓冲区并采用此次远端观察作为新基线，
不会写远端。即使没有冲突，也必须显式采用草稿再审核保存。
重新读取会撤销旧版本的冲突审核和选择，保留编辑器草稿；后台操作期间不能采用旧审核。
迟到结果必须同时匹配读取时的文件路径、完整基线字节和草稿，完整基线视图始终使用
该次计算捕获的基线，不能由后来变化的编辑器基线替代。

## Adopt、保存审核与迟到草稿

组合回归
`merge_adopt_save_keeps_late_newer_draft_and_rebases_next_save`
使用真实 GPUI、loopback SFTP 和受控原子写入停顿覆盖完整时序：读取远端并生成合并
审核，采用合并草稿，进入完整保存审核，在已审核内容发布过程中编辑器产生较新草稿，
随后完成目标读回。回归同时核对远端采用已审核的合并字节、编辑器保留较新草稿、
编辑基线切换到成功读回的合并版本，以及下一次保存使用该新基线并按原 CRLF 插入
策略生成精确字节。

迟到保存结果不会覆盖较新的编辑器内容，也不会把一次成功的发布读回误当成较新
草稿的基线。只有下一次明确审核和确认才会写入较新草稿；如果目标读回失败，仍沿用
既有失败/未知结果隔离合同。

“应用差异到草稿”接受粘贴的单文件 unified diff。两个文件头必须精确填写当前完整
远端路径；hunk 按序排列，行号、删除和上下文逐字节匹配当前草稿。不接受时间戳、
`a/`/`b/` 前缀、跨文件、二进制、重命名、命令或模糊匹配，不搜索路径或调用 shell。
解析在后台执行，仅将精确结果放入草稿；解析期间草稿变化会拒绝迟到结果。
现有差异预览用于观察逻辑行，将 CRLF/LF 视为相同并明确说明字节仍可能不同；
预览包含观察用途的文件头，不能直接作为精确 patch。patch 必须使用原字节与精确路径。

```diff
--- /srv/app.conf
+++ /srv/app.conf
@@ -1 +1 @@
-enabled=false
+enabled=true
```

“审核并保存”展示目标、审核基线元数据、完整最终全文、字节数和末尾 LF 状态；
文字可按两轴滚动，确认/取消按钮固定在滚动区外。确认后编辑内容改变会废除旧审核。
内层文本滚动不同时推动外层；文本区侧边留有真实间距，最小工作区中仍可从外围滚动
到其它工具。紧凑确认文本保留 48 px 高度预算，所有原始行仍可按两轴查看；
“展开审核”提供更大的正文与固定动作，详见 [完整审核指南](FILE_REVIEW_AND_FOCUS.md)。
保存前完整读取并比较基线，远端变化只生成新的合并审核，不覆盖或自动重试。
写入使用既有共享目标占用/未知结果隔离，以独占同目录临时文件 staging，原子发布前
再次核对路径、完整内容及元数据、检查当前授权，然后完整读回发布内容。只有读回
完全一致才更新编辑基线；收到发布确认但读回失败时保留旧基线与草稿并提示检查。
原子替换只保留普通 rwx 权限，不保留所有者、ACL、特殊模式或崩溃耐久性。SFTP v3
提供观察，不能保证恶意/并发服务端在最后复核与 rename 之间不更换对象。

取消、断连、撤权或重连不得恢复旧确认。归档面板保留草稿，但没有远程操作授权；
正在发送的请求可能已生效，取消不声称回滚。未确认写入继续沿用共享隔离与显式风险
审核。模型输出和 patch 都不能自行批准。此功能不增加第三方 MCP 客户端；KeelShell
仍向外部智能体提供 MCP 服务，MCP 文件提案保留既有 64 KiB 预算。

## 内联合并审核的紧凑布局

内联三方合并审核使用固定 `220 px` 卡片。卡片正文拥有独立的纵向滚动句柄，底部
动作栏固定在卡片内部并分成两行：第一行提供完整基线、完整草稿、完整远端、合并全文
和当前冲突；第二行提供展开审核、采用合并草稿和关闭并保留草稿。进入完整审核表面后，
展开按钮隐藏，完整版本与保存动作继续使用独立的审核滚动区。

正文滚轮由当前 `review_id` 绑定的处理器消费，按正文句柄的实际最大偏移截断，并阻止
事件继续推动外层 `file-tools-scroll`。旧卡片在新远端观察替换后仍可能收到 GPUI 保留
帧事件；旧 `review_id` 会直接失败关闭，不能修改新审核或外层滚动。切换冲突、完整
版本或冲突选择方案时，正文滚动复位到顶部，避免旧内容的滚动位置污染新内容。

这一布局只改变审核内容的可达性，不改变“采用草稿”和“保存”之间的授权边界。正文
和动作区都由主题 registry、语义 Palette 与组件 token 驱动，Frost「霜白」、
Graphite「曜石」、Lavender「雾紫」无需视图按主题 ID 分支。

## Compact inline merge review

Inline three-way merge review uses a bounded `220 px` card. Its body owns an independent
vertical scroll handle, while the fixed action area is split into two rows: complete base,
draft, remote, merged result and current conflict on the first row; expand review, adopt
merged draft and close while retaining the draft on the second row. The expanded review
surface hides Expand and keeps complete-version inspection separate from its save actions.

The body consumes wheel input through a handler bound to the current `review_id`, clamps the
body handle to its measured maximum, and stops the event before it moves the outer
`file-tools-scroll`. GPUI may deliver an event to a retained old frame after a newer remote
observation replaces it; the old review ID therefore fails closed and cannot mutate the new
review or the outer scroll. Switching conflicts, complete versions or conflict choices
resets the body to the top.

This layout changes review reachability while preserving the separate adopt and save
authorization steps. The body and action area use the theme registry, semantic Palette and
component tokens; Frost, Graphite and Lavender remain data-driven choices without
theme-ID branches in the view.

The combination regression
`merge_adopt_save_keeps_late_newer_draft_and_rebases_next_save` uses real GPUI, loopback
SFTP and a controlled atomic-write hold. It reads a remote change, creates a merge review,
adopts the merged draft, opens a complete save review, edits the editor while the reviewed
bytes are being published, and waits for full target readback. It verifies that the remote
file receives the reviewed merge, the editor retains the newer draft, the successful
readback becomes the editor baseline, and the next explicit save prepares bytes from that
new baseline while retaining the original CRLF insertion policy.

A late save result cannot replace newer editor content or silently rebase that newer draft.
The newer draft is written only after a new explicit review and confirmation; a failed
readback continues to use the existing failure and unknown-result isolation contract.

The editor reads complete checked regular UTF-8 files on the captured authenticated
SSH/SFTP session. Text operations admit at most 1 MiB and 4,096 lines, preserve exact
CRLF/EOF bytes, and refuse links, directories, NUL and ambiguous metadata.

“Read remote and merge” preserves the opened baseline and local draft while obtaining
a checked current remote version. Independent edits combine; overlapping changes and
ambiguous insertion boundaries require a separate Draft/Remote/Manual choice for each
conflict. Complete base/draft/remote and resolved result views remain available. Conflict navigation retains manual drafts. Closing retains the original
draft; adopting changes only the buffer and its observed remote baseline. Complete
save review remains mandatory, including merges without conflicts.
Rereading retires the previous review/choices while retaining the editor draft;
an old review cannot be adopted while another operation owns the panel. Late results
must match the captured path, complete baseline bytes and draft. The full baseline
view uses exactly the captured version used to calculate that plan.

“Patch draft” strictly parses an ordered single-file unified diff against the exact
current buffer. Both headers must use the complete selected remote path. Context,
removal bytes and both coordinates must match. No fuzzy search, cross-file/binary/
rename operation, timestamps, prefixed paths or shell is supported. Parsing occurs on
the worker; changed drafts reject late results. Only the buffer changes.
The existing observation-only diff normalizes CRLF/LF and labels logical equality
separately from byte equality. Its descriptive headers are not directly applicable
patch headers; exact patch input must use original bytes and the selected path.

Save review exposes complete final contents, metadata, size and final-LF state, with
two-axis text inspection and fixed Confirm/Cancel. Changed drafts invalidate consent.
Inner gestures do not simultaneously move the outer list; a real side gutter keeps
other tools reachable in a small workspace. Confirmation keeps its compact 48 px
height budget while every original row remains inspectable along both axes.
“Expand review” offers a larger body with fixed actions; see the
[complete review guide](FILE_REVIEW_AND_FOCUS.md).
Current remote content and metadata are read before staging and again before POSIX
atomic publication under the existing mutation exclusion/quarantine and authorization
checks. A changed remote observation creates a fresh merge review. Successful full
readback alone updates the baseline; acknowledged publication with failed readback
retains the old baseline/draft and asks for inspection. Only ordinary rwx permissions
are preserved; ownership, ACLs, special modes and crash durability are not promised.
SFTP observations are not an atomic compare-and-swap lock against server-side races.
Cancellation is not rollback; reconnect cannot restore old authority or release
unknown write isolation. AI output cannot approve itself. MCP continues to be the
KeelShell server for external agents; its existing 64 KiB file proposal budget remains.

See [ADR 0072](../adr/0072-reviewed-text-conflict-merge.md) and
[author test record](../testing/records/2026-10-07-text-conflict-merge.md).
Controlled GPUI/owned TCP checks and compilation do not establish native desktop,
customer SSH, Windows/Linux acceptance or complete product delivery.
