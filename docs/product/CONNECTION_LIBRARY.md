# 连接库批量组织与回收站 / Connection library organization and trash

在连接行中勾选需要组织的配置，然后使用选区工具栏。活动连接支持移动到现有文件夹或未归档根目录、添加/移除/替换标签、统一收藏状态及移入回收站。搜索或在活动连接筛选之间切换不会改变明确选中的 ID，审阅面板始终展示全部选中连接，包括当前筛选中隐藏的连接。切换到回收站或从回收站返回活动连接会清空选区。

Select profiles using the selection control on each connection row, then use the
selection toolbar. Active profiles support folder moves, tag add/remove/replace,
uniform favorites and recoverable deletion. Search and active-profile filters
retain explicit selections; review lists every selected profile, including those
hidden by the current filter. Crossing between active profiles and trash clears
the selection.

每次批量修改先编辑操作，再点击“审阅影响对象”。审核显示完整名称、端点、ID、原目录/标签/收藏及修改后的内容，还显示跳板、凭据引用和相关活动会话。点击“返回修改”继续编辑；点击“取消”放弃本次修改。只有确认后才将完整候选保存一次。语言/主题变化保留草稿；连接库、选区或活动会话变化要求重新审阅，外部配置写入冲突保留草稿并提示重新加载。

Edit the operation, then choose “Review affected profiles.” Review shows exact
names, endpoints, IDs, original and resulting organization, jump references,
credential references and related active sessions. “Back to edit” retains the
draft; “Cancel” discards the proposed change. Confirmation performs one complete
metadata save. Locale and appearance changes retain drafts. Library, selection
or active-session changes require review again; an external configuration write
conflict preserves the draft and requires reloading the configuration.

回收站保留原连接 ID、文件夹与凭据引用。可以单项恢复，也可以勾选整组连接后审阅恢复。上一批软删除的“撤销”同样打开新的恢复审核；重启后仍可从回收站恢复。若活动连接仍依赖某个跳板，软删除该跳板会阻止整批操作，需一并选择其活动依赖或先改变依赖路线。

Trash retains original profile IDs, folder memberships and credential references.
Restore one profile directly, or select a group for reviewed restoration. Undoing
the last batch trash opens a new restore review; after restart profiles remain
recoverable from trash. Trashing a jump host is blocked while an unselected active
profile depends on it. Select the active dependent too, or change its route first.

永久清理有单项、选中和“清空回收站”三个入口，均需审阅精确对象后点击“确认永久删除”。永久清理无法撤销；选区外的活动或回收站跳板依赖会阻止整批操作。整条已选跳板链可一起软删除、恢复或永久清理，不受选择顺序影响。

Permanent cleanup supports one profile, selected profiles or the entire trash.
Every entry requires exact review and “Confirm permanent deletion.” Permanent
cleanup cannot be undone. Unselected active or trashed jump dependents block the
whole operation. A fully selected jump chain can be trashed, restored or purged
together regardless of selection order.

这些操作仅修改本机连接配置。现有 SSH 会话保持打开，不执行命令或删除服务器文件；已回收的路线不会在后续自动重连。永久清理只移除配置和目录关联，加密凭据条目、其他配置的共享凭据引用及服务器信任记录继续保留。若需维护凭据，请另行打开凭据库管理并审阅对应操作。

These operations change local profile metadata only. Existing SSH sessions stay
open; no commands run and no server files are deleted. Trashed routes stop future
automatic reconnection. Permanent cleanup removes profiles and folder membership,
while encrypted vault entries, other profiles' shared references and host trust
records remain. Review credential maintenance separately in vault management.

实现和验收边界见 [ADR 0045](../adr/0045-reviewed-connection-library-transactions.md)
与 [测试记录](../testing/records/2026-10-05-connection-library-bulk.md)。
