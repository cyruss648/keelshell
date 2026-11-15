# 远程文本编码、BOM 与换行保持

## 保存审核的末尾字符 — 2026-11-16

审核中的「末尾 LF / CR」表示按保存格式编码并无损回解后的最后一个字符。
不能直接检查 UTF-16 的最后一个字节：真实换行可能以零字节结束，其他字符也可能
恰好以 `0A` 或 `0D` 结束。原始与最终字节数、完整正文、备份和目标读回仍使用
准确出站字节。本片回归覆盖大小端、LF、CR、CRLF、无换行、空正文和容易误判的
非换行 Unicode 字符；实际门禁和原生范围见
[本批记录](../testing/records/2026-11-16-editor-keyboard-native.md)。

## 显式旧编码读取 — 2026-11-15

文件区远程标题旁的**读取编码**提供 Unicode 自动、GBK、GB18030、Big5、
Shift_JIS 和 Windows-1252。选择后，再明确点击文件的编辑入口。
Unicode 自动只接受 UTF-8，以及带 BOM 的 UTF-16 LE/BE；不会猜测旧编码。
选项不写入连接配置，不发起网络请求，也不修改已经打开的正文、草稿或保存格式。
当前文档实际保持的编码单独显示。

重新打开脏草稿先审核放弃草稿，取消保留正文、完整原始字节和格式。
审核后文档、草稿、编码选择或实际连接变化，旧确认失效；读取期间发生变化时
迟到结果也不能覆盖当前文档。更改下一次读取选项不会将已打开的文件转码。

| 显式选择 | 读取及保存合同 |
| --- | --- |
| GBK、GB18030、Big5、Shift_JIS、Windows-1252 | 无替换解码，同编码重新编码必须逐字节等于完整输入；保存继续使用该编码 |
| 任意旧编码遇到 Unicode BOM、非法序列或不能严格往返的字节别名 | 拒绝打开，保留当前草稿，不猜测、不写回 |
| 草稿包含所选编码无法无损表示的字符 | 在生成保存审核前拒绝；保留草稿，不创建备份或远端临时文件 |
| 编码器会静默近似映射的字符 | 回解必须与规范化草稿完全相同，否则拒绝，例如 Shift_JIS 的日元/反斜杠近似映射 |

旧编码使用 `encoding_rs` 的 WHATWG 映射，不承诺全部历史厂商扩展或国家标准
修订版本。不存在有损保存、自动猜测、数字字符引用替代或自动转码。
查找/替换、diff、patch 和冲突合并处理解码后的 Unicode 正文；
保存仍审核同一格式产生的精确出站字节，并保留原始字节备份。
远端旧编码观察出现非法序列或 Unicode BOM 时，锁定显式重读要求；
远端恢复原字节不会自动解锁。无 BOM 的字节无法可靠证明远端改用了另一编码，
因此不宣称自动检测编码变化。

原始字节、解码后 UTF-8 正文及最终字节分别最多 1 MiB；保存最多 4096 行，
包括 CRLF 和编码扩张。空内容、无换行版本及连续保存保持当前文档的编码和已知
插入策略。明确重新打开后才采用新观察到的策略。

本片工程、真实 OpenSSH、独立复核和三平台原生验收分别记录在
[旧编码批次记录](../testing/records/2026-11-15-legacy-editor-encodings.md)；
源码接入不等于原生桌面验收。设计见
[ADR 0140](../adr/0140-explicit-legacy-editor-encodings.md)。

### Explicit legacy reads

The file header offers Unicode auto, GBK, GB18030, Big5, Shift_JIS and
Windows-1252 for the next explicit open. Changing this transient choice sends
no request and leaves the current document, draft and save format intact.
Reopening a dirty draft needs a new discard review bound to the exact document,
draft, read choice and authenticated connection.

Legacy reads must decode without replacement and reproduce every source byte.
Saves must decode back to the exact normalized Unicode draft; unmappable and
silent best-fit mappings are rejected before approval or mutation.
These are `encoding_rs` WHATWG mappings, not a claim of all vendor extensions.
Unicode BOMs are never reinterpreted as legacy text. There is no automatic
encoding detection or lossy conversion. Byte, decoded-body and save-line
budgets, original backups, isolation and complete target readback remain.
The original Unicode transport API and MCP's 64 KiB limit are unchanged.

## Unicode 验收历史

日期：2026-11-06。r3/538输入的源码候选已导入开发分支，逐项匹配作者冻结；
组合工程、新包与原生验收分别记录，不把源码候选视为已发布功能。
r2独立审请求修订的三项P2已在r3关闭，必要差分复审与新的完整工程门禁通过。
最终543输入的新包已完成限定macOS原生BOM/CRLF完整审核取消、确认和全部字节核对；
inline合并操作可达性待修，其他格式与跨平台原生仍待验，见
[限定原生记录](../testing/records/2026-11-06-editor-traceroute-native.md)。

截至 2026-11-08，连续无换行保存组合回归已通过定向 GPUI/SFTP 和完整工程门禁；
源码实现已固定为 `9134cfaa771057a0f246ead47479be3706248415`，第二次独立复核已
返回 `APPROVE`（P0/P1/P2/P3 均为 0）。精确包和跨平台原生验收仍未完成。

## 可用行为

远程编辑仍使用当前已认证 SSH 的 SFTP 通道。默认 Unicode 自动只接受完整、检查过的常规 Unicode
文本文件。类型、路径、句柄、元数据、无效文本、NUL 与读取上限的既有拒绝规则
保持。UTF-8 可带或不带 BOM；UTF-16 只接受明确的 UTF-16 LE/BE BOM，不猜测
无 BOM 的 UTF-16，也不转换 UTF-32。错误不回显文件正文。

文件头恰好一个编码 BOM 作为格式外壳保留，不占编辑正文的查找、替换或 patch
偏移。UTF-16 的字节序和 BOM 在保存时保持。内部或额外的 U+FEFF 仍属于正文；
BOM-less UTF-8 开头的 U+FEFF 由已观测格式解释，避免在合并或迟到读回时被误删。
保存审核显示编码、BOM、换行策略、完整最终内容、原始与最终字节数以及末尾
LF/CR 状态，使用既有双语和三主题语义 token。

| 观测到的编码 | 编辑器行为 |
| --- | --- |
| UTF-8，可带 BOM | 以 UTF-8 正文编辑，保存时只保留原观测到的 BOM 状态 |
| UTF-16 LE，带 `FF FE` | 以 Unicode 正文编辑，保存时使用小端序和 `FF FE` |
| UTF-16 BE，带 `FE FF` | 以 Unicode 正文编辑，保存时使用大端序和 `FE FF` |
| UTF-16 无 BOM、UTF-32、非法代理项或奇数字节 | 拒绝打开，不猜测、不写回 |

| 打开时观察到的换行 | 保存策略 |
| --- | --- |
| 没有换行 | 新插入的换行使用 LF |
| 单一 LF | 保存时使用 LF |
| 单一 CRLF | 保存时使用 CRLF，包括 Enter/粘贴产生的 LF，不产生 CRCRLF |
| 混合换行或仅 standalone CR | 草稿原样保存，不按多数风格归一化 |

不强制增加或删除文件末尾换行。dirty、保存预览和最终确认都使用同一份精确出站
字节；逻辑行差异预览不代替字节审核。查找、替换和严格单文件 patch 只改变草稿，
不能授予写入权限；普通合并仍先显式采用草稿，再单独审核保存。
同一打开文档连续保存为空或无换行版本时，保留已知插入策略；明确重新打开一个
无换行文件才重新采用LF默认值。合并中新加入的头部U+FEFF不被误删。

读取保留原1 MiB原始字节上限，并额外限制解码后的 UTF-8 正文不超过1 MiB。
保存、合并与 patch 结果的1 MiB / 4096行上限不扩大，最终字节预算包含 BOM、
UTF-16 编码扩张和 CRLF 扩张。超过4096行但未变动的已读文件不被误标为脏。

## 连续无换行保存回归

当前 GPUI/SFTP 组合回归
`independent_editor_format_no_newline_keeps_lf_policy_across_empty_and_plain_saves`
从远端无换行内容开始，使用真实编辑器 Enter 插入首个换行，然后依次保存带 LF 的
内容、空内容、再次无换行内容和后续带换行内容。每次审核都核对原始基线与精确出站
字节，最终还核对四份远端原始备份。

该回归确认：首次插入使用 LF；保存空内容或无换行内容后仍保留 LF 插入策略；后续
编辑不会因为空内容或无换行读回而自动增加 BOM 或改成 CRLF。它只证明当前受控
GPUI、loopback SFTP 和本地测试夹具路径，不代签 Windows/Linux 原生编辑器、IME
或真实客户 SSH。

## 格式变化与授权

原始完整字节、大小、普通 mode 和修改时间持续作为审核基线。新观察到不兼容
BOM/EOL 时，保留草稿、撤销旧合并与保存路径，必须显式成功重读，再重新审核。
远端恢复原格式不会解除重读闸门。无换行与 LF 共用 LF 插入策略，二者兼容。

取消保存审核不产生远端写入。审核后草稿或相关身份变化会拒绝旧确认；较新草稿
不被迟到保存结果覆盖，并继续使用与其正文配对的格式解释，避免下一次保存自动
增加BOM。已发送操作的取消、断连或撤权不声称回滚，也不自动重放。

## 备份、发布和完整读回的三个边界

| 阶段 | 当前保证与限制 |
| --- | --- |
| 原始备份 | 复核原 snapshot 后，把原始字节写入同目录 `.{name}.keelshell-backup-{uuid}.txt`；备份写入成功 ACK 后才继续目标替换。生产流程没有独立完整读回该备份，ACK不等于备份内容或崩溃耐久性验证。 |
| 目标发布 | 沿用独占 staging、当前授权及发布前完整基线复核，使用 POSIX 原子替换。发布 ACK 只表示服务端确认，不能直接更新编辑基线。 |
| 目标完整读回 | app 在发布 ACK 后完整读取目标，并将全部字节与已审核出站字节逐字节比较；只有完全一致才采用保存基线。失败或任一字节不符时保留旧基线与当前草稿，提示检查，已完成的备份留在远端。 |

测试会独立读取合成备份并断言原始字节正确；这项测试观察不能扩大生产备份 ACK
合同。备份不是版本历史、自动恢复或事务回滚。未知目标写入继续保留既有隔离；
普通 SFTP 观察不是远端 CAS，最终复核与 rename 之间仍有服务端竞争窗口。
只保留普通 rwx，不承诺 owner、ACL、特殊 mode 或崩溃耐久性。

## 验收与范围

2026-11-06 的 Unicode 片没有增加其他编码、格式菜单、新依赖、备份历史或自动重试；
2026-11-15 的显式旧编码读取范围以上方当前合同为准。
r2专项去重42条通过、含新增21条的历史保留；独立审三项P2先由三条新增GPUI回归
实际复现，再修复通过。r3另增一条core策略回归，受影响专项去重37条通过，
累计新增源码用例25条。r3新的完整门禁2338 ordinary passed/23 ignored、
14 rustdoc、Python131 run/1 skipped通过，25个新增名全部实际pass。
该工程结果不代签开发包或原生桌面验收。
最新完整门禁状态见[测试记录](../testing/records/2026-11-06-editor-format.md)；
设计见[ADR0111](../adr/0111-utf8-editor-format-envelope.md)。

The editor preserves the observed UTF-8 or BOM-qualified UTF-16 envelope separately from
the editable Unicode body and retains the original uniform LF/CRLF save policy.
Mixed/CR-only drafts stay verbatim.
Exact outbound bytes drive dirty state, review, limits and confirmation. An incompatible
format observation requires an explicit successful reread and a fresh review.
Backup ACK, target publication ACK and complete target readback are distinct:
production does not independently reread the backup; only complete target byte equality
permits baseline adoption. Cancellation is not rollback, and SFTP is not remote CAS.
The existing 1 MiB / 4096-line limits remain, including the decoded-body and UTF-16
encoded-byte budgets. A new macOS package has scoped native
BOM/CRLF cancel/save observations; inline merge accessibility and complete native
acceptance remain open.

The GPUI/SFTP regression
`independent_editor_format_no_newline_keeps_lf_policy_across_empty_and_plain_saves`
starts from a remote file without line breaks, inserts the first break through the real
editor, then saves LF content, empty content, another no-break body and a later
line-containing body. It verifies exact reviewed bytes after every step and reads back all
four original backup versions. The regression confirms that the first inserted break uses
LF, the known LF policy survives empty and no-break saves, and later edits do not acquire a
BOM or CRLF. It is controlled GPUI/loopback-SFTP evidence and does not establish native
Windows/Linux editor, IME or customer-SSH acceptance.
