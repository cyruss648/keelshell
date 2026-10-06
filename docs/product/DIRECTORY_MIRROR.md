# 审核式目录镜像 / Reviewed directory mirror

## 使用流程

文件面板中的普通“合并”默认保留目标独有项。需要让目标与来源一致时，明确选择
“镜像：本地 → 远程…”或“镜像：远程 → 本地…”。远程根目录使用当前成功加载且
重新核实的 canonical 路径，本地目录必须显式填写；选择方向与生成计划只读取文件。

镜像支持常规文件、空目录及多层目标独有非空子树。全部子节点会展开为精确审核行，
先复制或创建来源目录，再按子节点先于父目录的顺序逐项删除。链接、未知对象类型、
类型冲突、大小写重名、不可移植名称、缺失父目录或不完整内容证据，会阻止整份计划。
冲突卡片可以滚动查看全部冲突，不能从冲突卡片执行删除。没有隐式递归删除命令。

查看完整审核：来源与目标根、方向、删除数量、完整指纹，以及每一项复制或删除的
路径、类型、大小和 SHA-256。确认文字明确标记删除不可撤销；使用“确认镜像及删除”
才开始执行。审核正文逐行保留完整内容，使用纵向滚动查看全部项目，使用横向滚动查看不折行的路径和摘要；确认与取消位于滚动区外。新审核从正文开头显示。“取消”不会产生该计划的写入。内容扫描限制为每侧 10,000 项、32 层目录（最后一层可含文件）、
64 MiB/文件与两侧合计 256 MiB，审核文字最多 128 KiB；超限会拒绝计划，不截断批准范围。

执行前重新读取完整计划。每次删除检查来源子树边界仍缺失、目标剩余子树的完整名字
集合与全部剩余文件内容，第二次名字扫描拒绝新增、消失或已删除节点重新出现。观察到
这些变化会废除当前所有者的旧审核，即使外部程序恢复原内容，也需要新比较与确认。
会话、路线、取消和所有者在等待后复核；删除目录之前所有已审批子节点必须完成。
复制使用已有临时文件发布和内容读回。镜像删除只删除本次明确列出的一个文件或空目录，
不会递归展开路径、提高权限、自动重试或重连。SFTP 没有原子的条件删除协议，本地
路径检查也不是全局锁；其它程序在最后检查与删除之间改动文件，仍可能形成竞争。
审核中会显示这一限制，请在可控目录上使用。

## 结果与取消

逐项结果显示“尚未开始”“正在核验”“结果未知”“已完成”“已知失败/拒绝”
“取消前未写入”及“失败后跳过”。取消或失败不回滚已经完成的项目，也不把未知改成
成功。删除回复与删除后的缺失检查均得到确认才显示完成；明确协议拒绝属于已知失败。
未确认的写入或必需读回会沿用已有目标隔离，重新连接不能绕过隔离；通道关闭未确认
会另行提示。保守的“结果未知”有时表示取消发生在提交前，不能反推出远端已经执行。

当前逐项记录只在文件面板内存中保留，不包含重新执行按钮。新计划会替换显示，重启
不会恢复或重放删除，也不能证明旧远端动作已停止。重新执行需要重新比较与完整审核。
任务级持久摘要审计和镜像逐项记录是不同功能。

## Workflow

Ordinary **Merge** preserves destination-only entries. Choose **Mirror local →
remote…** or **Mirror remote → local…** to review a mirror instead. The remote
root is the successfully loaded, rechecked canonical path; the local directory
is explicit. Selecting the direction and preparing the plan only reads files.

The mirror permits regular files, empty directories and nested destination-only
nonempty subtrees. Every child becomes an exact reviewed row. Copy/create parents
precede children; deletion children precede empty parents. Links, unknown types,
type conflicts, case collisions, nonportable names, missing hierarchy or incomplete
content observations block the entire plan. The read-only conflict card shows
every observable conflict. There is no implicit recursive removal command.

Review both roots, direction, deletion count, complete fingerprint and every
copy/delete path, type, size and SHA-256. The irreversible-deletion notice and
**Confirm mirror/deletions** button are explicit. The complete original lines remain inspectable vertically, with horizontal scrolling for unbroken paths and hashes. Confirm and Cancel stay outside the scrolling body, and a new review starts at the beginning. Cancel writes nothing for this
plan. Limits are 10,000 entries/side, depth 32, 64 MiB/file, 256 MiB total content
and 128 KiB for complete approval text. The depth permits 32 nested directories,
with files immediately below the last directory. Exceeding a limit refuses the plan rather
than truncating the reviewed scope.

Execution rebuilds the complete plan. Before each deletion it checks source
absence at the subtree boundary, the complete remaining destination namespace and
all remaining file contents. A second namespace sweep refuses additions, missing
entries and reappearing completed children. Observed changes invalidate that
owner's review; restoring bytes cannot silently restore approval. Session, route,
cancellation and ownership are checked after observations. Directory removal
requires its individually approved children to be completed. Copies retain temporary publication
and checked readback. Deletion addresses exactly one confirmed file or empty
directory, without recursion, privilege expansion, retry or reconnect. SFTP has
no atomic conditional removal; local path observations are not a global lock.
An external rename or writer between final observation and removal can still
race. The approval displays this limitation; use controlled directories.

## Results and cancellation

Each item remains not started, verifying, unknown, completed, known rejected,
cancelled before write or skipped after failure. Cancellation/failure cannot
roll back completed rows or turn unknown into success. Completion requires both
the deletion acknowledgement and checked absence. A valid protocol refusal is
a known failure. Unacknowledged mutation or required readback retains existing
target quarantine; reconnecting cannot bypass it. Unconfirmed channel closure is
reported separately. A conservative unknown row can also arise when cancellation
wins before dispatch, and does not prove the peer executed the operation.

Results are in memory in the file panel. They contain no replay/retry action. A
new plan replaces the display; restarting restores no mirror action and cannot
prove old remote work stopped. A new execution requires a fresh comparison and
complete review. Task-level persistent summary audit is a separate feature.

递归子树是独立候选增量；新的非作者复核、主树门禁及原生流程仍待完成，见
[递归镜像记录](../testing/records/2026-10-07-recursive-directory-mirror.md)。

设计见 [ADR 0068](../adr/0068-individually-reviewed-recursive-directory-mirror.md) 和 [ADR 0066](../adr/0066-reviewed-bounded-directory-mirror.md)，工程证据与未验收边界见
[测试记录](../testing/records/2026-10-06-directory-mirror.md)。隔离 TCP/SFTP 和无窗口 GPUI
检查不等同于真实桌面、客户环境或 Windows/Linux 原生验收。

The subtree extension is an isolated candidate. New independent review, main-tree
validation and native workflows remain pending; see the [recursive mirror record](../testing/records/2026-10-07-recursive-directory-mirror.md).

See [ADR 0068](../adr/0068-individually-reviewed-recursive-directory-mirror.md) and [ADR 0066](../adr/0066-reviewed-bounded-directory-mirror.md) and the
[engineering record](../testing/records/2026-10-06-directory-mirror.md). Controlled
TCP/SFTP and headless GPUI checks do not establish desktop, customer-machine or
Windows/Linux native acceptance.
