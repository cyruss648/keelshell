# 远程文本 UTF-8 BOM 与换行保持

日期：2026-11-06。r3/538输入的源码候选已导入开发分支，逐项匹配作者冻结；
组合工程、新包与原生验收分别记录，不把源码候选视为已发布功能。
r2独立审请求修订的三项P2已在r3关闭，必要差分复审与新的完整工程门禁通过。
最终543输入的新包已完成限定macOS原生BOM/CRLF完整审核取消、确认和全部字节核对；
inline合并操作可达性待修，其他格式与跨平台原生仍待验，见
[限定原生记录](../testing/records/2026-11-06-editor-traceroute-native.md)。

## 可用行为

远程编辑仍使用当前已认证 SSH 的 SFTP 通道，只接受完整、检查过的常规 UTF-8
文件。类型、路径、句柄、元数据、无效 UTF-8、NUL 与读取上限的既有拒绝规则保持。
UTF-16/32 文件头不猜测或转换；错误不回显文件正文。

文件头恰好一个 UTF-8 BOM 作为格式外壳保留，不占编辑正文的查找、替换或 patch
偏移。内部或额外的 U+FEFF 仍属于正文。保存审核显示 BOM、换行策略、完整最终
内容、原始与最终字节数以及末尾 LF/CR 状态，使用既有双语和三主题语义 token。

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

读取保留原1 MiB字节上限。保存、合并与 patch 结果的1 MiB / 4096行上限不扩大，
最终字节预算包含 BOM 和 CRLF 扩张。超过4096行但未变动的已读文件不被误标为脏。

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

本片不增加其他编码转换、格式菜单、新依赖、备份历史或自动重试。
r2专项去重42条通过、含新增21条的历史保留；独立审三项P2先由三条新增GPUI回归
实际复现，再修复通过。r3另增一条core策略回归，受影响专项去重37条通过，
累计新增源码用例25条。r3新的完整门禁2338 ordinary passed/23 ignored、
14 rustdoc、Python131 run/1 skipped通过，25个新增名全部实际pass。
该工程结果不代签开发包或原生桌面验收。
最新完整门禁状态见[测试记录](../testing/records/2026-11-06-editor-format.md)；
设计见[ADR0111](../adr/0111-utf8-editor-format-envelope.md)。

The editor preserves one leading UTF-8 BOM separately from the editable body and
retains the original uniform LF/CRLF save policy. Mixed/CR-only drafts stay verbatim.
Exact outbound bytes drive dirty state, review, limits and confirmation. An incompatible
format observation requires an explicit successful reread and a fresh review.
Backup ACK, target publication ACK and complete target readback are distinct:
production does not independently reread the backup; only complete target byte equality
permits baseline adoption. Cancellation is not rollback, and SFTP is not remote CAS.
The existing 1 MiB / 4096-line limits remain. A new macOS package has scoped native
BOM/CRLF cancel/save observations; inline merge accessibility and complete native
acceptance remain open.
