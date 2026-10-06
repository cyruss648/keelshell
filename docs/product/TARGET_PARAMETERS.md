# 逐目标参数 / Per-target parameters

批量命令和依赖工作流只使用已经认证、仍在线且明确选择的 SSH 会话。
在命令中填写 `{{path}}`、`{{release}}` 等用户命名参数，再点击同步字段。
同一个名称在不同会话中可有不同值；一个目标的多个依赖任务共享该目标的同名值。
同步字段保留仍被使用的同名值，删除本目标已不使用的字段；不会复制到新连接。
字段为空时默认缺少值，只有人工勾选“明确使用空字符串”才将其作为 `''` 展开。

保留元数据名称 `name`、`host`、`port`、`user`、`endpoint` 保持原义，不能覆盖。
其他名称必须是 1–64 字节的 ASCII 标识符 `[A-Za-z_][A-Za-z0-9_]*`。
每目标最多 32 个用户名称，每条模板的元数据与用户名称总计最多 32 个。
每值最多 4 KiB UTF-8；全草稿所有参与目标的值合计最多 1 MiB。
CJK、撇号、换行和 tab 可作为字面值；NUL、ESC 等其它控制字符被拒绝。
不可见方向/零宽字符在完整命令审核中以转义形式显示。

语法复用参数化片段的受限 POSIX 字面词解析器：参数必须占完整未引用的词，
也可占 `NAME=` 或 `--option=` 后的完整值。引用内参数、词片段、shell 扩展、
here-document 与不支持的复合语法被拒绝，不计算任意表达式或读取环境。
每个值由既有解析器单引号引用；这仅保证该插入点的字面性，`eval`、`sh -c`
以及接收程序本身的语义仍须人工判断。无模板标记的命令保留原有语法接受方式。
源及每条最终命令仍须遵守原 64 KiB 边界，依赖工作流最终命令合计最多 1 MiB。

完整审核显示每个目标/任务的源文本、精确最终命令、路线、会话和依赖。
字段或命令编辑、目标资料/路线变化以及认证连接实例替换都会令旧审核失效。
确认时重算并逐字比较不可变审核，使用原捕获实例派发；不会自动连接、重试或执行。
隐藏/重开保留同一面板草稿/运行；新建草稿丢弃旧映射。取消不能证明远端进程停止。

值、展开命令、输出和它们的派生摘要只保留当前工作区内存。
含用户参数的批量运行不写普通批量审计，原不含用户参数的摘要审计保持。
依赖工作流本身继续不写命令历史、元数据或持久任务日志。

For batch commands and dependency workflows, connect required SSH hosts first,
explicitly select targets, add markers such as `{{path}}`, then sync target fields.
Each authenticated session owns its values; tasks on the same target reuse the same
name. Sync retains still-used values and removes obsolete fields. Empty inputs
require an explicit empty-string selection. New connections never inherit values.

The five metadata names retain their meaning and cannot be overridden. User names
are ASCII identifiers of 1–64 bytes; values support CJK, quotes, LF and tab within
4 KiB, with other controls refused. At most 32 user names per target, 32 combined
names per template and 1 MiB participating values per draft. Existing command and
workflow render limits remain. Invisible format characters are escaped for review.

Every complete final command is reviewed and bound to its captured authenticated
session, route, metadata and exact draft. Edits or replacement connections expire
the review. There is no expression evaluation, inherited environment, automatic
connection, retry or execution. Values and derived commands/output/digests remain
in memory; user-parameter batches skip persisted batch audits. Ordinary batches
retain their existing non-sensitive audit behavior. Hiding/reopening preserves
owned drafts/runs, while new drafts discard old mappings.


后续主副本完整组合门禁已通过，新的 macOS 标准包取得正常执行与真实 Linux SSH 采样的有限原生证据，见[参数与磁盘记录](../testing/records/2026-10-06-target-parameters-disk-native.md)。该后续证据保留原失败，不扩大为其它平台、完整窗口/辅助技术或所有运行分支验收。
