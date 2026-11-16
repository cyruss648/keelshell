# 文件列表键盘操作 / File browser keyboard

状态：2026-11-16 工程门禁与非作者源码复核通过。实际 macOS Frost 中文窗口已
观察下述浏览、审核和正向 Tab，见
[本批记录](../testing/records/2026-11-16-file-browser-keyboard.md)。
后续在两侧各一文件的同主题场景，完成从命令框正向及反向到两列表根的原生路径，
完整草稿保持，见
[焦点补验](../testing/records/2026-11-16-archive-diagnostics-and-native-focus.md#原生焦点补验)。
原始多条目反向尝试仍不完整；其他主题/平台、IME/VoiceOver 和完整焦点矩阵尚未验收。

点击文件行、复选框或列表空白处后，键盘焦点位于相应列表；也可使用 Tab 到达列表。
游标边线与选中背景分别表示当前落点与选区。目录在前、当前列排序及隐藏项过滤
始终参与键盘顺序，游标移动后自动滚动到可见区域。

| 按键 | 当前聚焦的文件列表 |
| --- | --- |
| ↑ / ↓ | 单选上一行/下一行，首尾不循环 |
| Shift + ↑ / ↓ | 从保留的锚点连续选择，可缩短或反向扩展 |
| Page Up / Page Down | 按实际列表高度移动一页，可结合 Shift 扩展选区 |
| Home / End | 移到首行/末行，可结合 Shift 选择范围 |
| Space | 切换当前游标项，保留其他选中项 |
| macOS Cmd+A；Windows/Linux Ctrl+A | 选择全部可见项；超过 32 项整次拒绝 |
| Escape | 清空该列表选择，不批准或执行文件操作 |
| Enter | 远程打开单选目录/文件；本地打开普通目录或填写普通文件路径 |
| Backspace / Alt+↑；macOS 也支持 Cmd+↑ | 浏览当前已加载目录的上级 |
| F5 | 刷新当前已加载目录 |
| F2（远程） | 展开文件操作并聚焦已填入的单选名称；编辑后 Enter 提出重命名审核 |
| Delete；macOS 也支持 Cmd+Backspace（远程） | 提出单选项的完整删除审核，仍须独立确认 |
| macOS Cmd+↓ | 与 Enter 相同 |

列表选择最多 32 项。超限范围与全选保留原选区及游标，不截断、不选隐藏项。
F2 与删除只接受单选，多选时不默默操作第一项。本地列表不提供本地重命名或删除。
名称输入框中的 Enter 仅在明确按 F2 后提出重命名审核；普通新名称输入不自动操作。
审核中的 Enter 或重复删除键不确认，取消保留远端文件。
重命名、删除或新建完成后的状态会提示刷新列表；按 F5 重新读取目录后，再选择新名称。
旧行不会被当作重命名后的新对象继续操作。

Only the focused list handles these keys. Arrow/Page/Home/End navigation follows
the current visible order and scrolls the cursor into view. Shift extends from
the retained anchor; Space toggles the cursor, Escape clears selection, and
Cmd+A on macOS or Ctrl+A on Windows/Linux selects the whole visible set within
the 32-entry limit. Oversized selections preserve the previous complete state.
Engineering and independent source review have passed. Scoped macOS Frost
observations include browsing and forward Tab. A later Frost check with one
file in each list completed forward and reverse native navigation from the
composer to both list roots while preserving the full draft. See the
[focus follow-up](../testing/records/2026-11-16-archive-diagnostics-and-native-focus.md#原生焦点补验).
The original larger-list reverse attempt remains incomplete. Other
themes/platforms, IME, VoiceOver and the complete focus matrix remain open.

Enter opens a single remote entry, preserving dirty-draft review. In the local
pane it browses an ordinary folder or fills a file transfer path. It never
starts a transfer. Backspace/Alt+Up browse the loaded parent; F5 refreshes the
loaded folder. Unsubmitted path drafts are not used. Native non-UTF-8 paths
remain native values and cannot be converted lossily into transfer text.

Remote F2 focuses the existing name editor. Enter after that explicit intent
proposes a reviewed rename. Delete, or Cmd+Backspace on macOS, proposes a
reviewed single-item deletion. Repeated keys and Enter in the review body never
confirm it; remote mutation requires the separate confirmation control.
After a rename, deletion or creation, the result asks you to refresh the list.
Use F5 to reload before selecting a new name; an old row is never reinterpreted
as the renamed object.
Other inputs, child controls, the terminal and hidden tables retain their own
key handling. Local rename/delete are outside this pane's source-selection role.
