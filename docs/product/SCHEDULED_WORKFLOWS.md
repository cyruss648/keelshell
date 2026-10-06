# 定时依赖工作流 / Scheduled dependency workflows

依赖工作流默认使用“立即执行”。选择“一次定时”或“有限间隔重复”后，
完整审核将同时包含任务、命令、执行选项、原认证连接以及每次触发的时间。
点击“确认并启用定时计划”会授权审核表中的整个有限序列；到时再次核对
同一审核和原连接，只有仍有效且处于宽限窗口内的触发才能放行。

定时计划只在本次应用工作区的内存中存在。返回工作区会隐藏面板并保留计划；
通过“依赖工作流”入口可重新打开。关闭应用后不恢复计划、触发令牌或任务输出。

## 时间与次数

| 配置 | 约束与含义 |
| --- | --- |
| 首次日期时间 | ASCII `YYYY-MM-DD HH:MM:SS`，有效 Gregorian 日期，年份 1–9999；启用时不得早于当前时刻 |
| 固定 UTC 偏移 | ASCII `+HH:MM` 或 `-HH:MM`，范围 `-14:00` 至 `+14:00`，十四小时只允许零分钟 |
| 一次定时 | 一个绝对 UTC 触发时刻 |
| 有限间隔重复 | 每次间隔至少 60 秒，共 1–32 次 |
| 每次宽限 | 1–60 秒；可放行窗口包含触发时刻与最后宽限边界 |
| 最终结束 | 末次触发加宽限；从首次触发到最终结束不超过七天，实际启用到最终结束也不超过七天 |

审核前，五个原始字段各最多 32 字节，包括切换模式后暂不使用的字段；数字字段只接受 ASCII 数字，不接受符号、空白、Unicode 数字或表达式。

默认偏移明确填写为 `+00:00`（UTC），首次时间为当前 UTC 加五分钟；用户可以显式修改偏移和日期时间。
日期时间使用用户明确填写的固定偏移转换为 UTC。固定偏移不随夏令时、
系统时区或后来修改的本地设置变化；日期不接受隐式本地时区、Unicode 数字、
控制字符、闰秒或前后空白。审核同时显示固定偏移日期时间、UTC 日期时间和 epoch 秒数，需核对每次
触发及最终结束时间。

重复序列始终从原始首次 UTC 时刻计算：第 `k` 次时间为
`first_utc + (k - 1) × interval`。运行或完成耗时不改变后续计划时间。
例如首次为 `10:00:00`、间隔 60 秒、共三次时，审核的序列为
`10:00:00`、`10:01:00`、`10:02:00`，每次分别使用自己的宽限窗口。

## 完整人工审核

1. 先连接所需 SSH 主机，为每个任务显式选择已经认证且仍在线的会话。
2. 填写任务、前置关系、命令、并发、每任务超时及单次执行的失败策略。
   命令中的用户 `{{release}}` 等名称先通过“同步全部目标参数字段”建立输入，
   再为每个已认证目标填写字面值；同一目标的同名值由该目标的任务共享。
   可以与 `{{endpoint}}` 等保留元数据混用，仍遵守[参数契约](TARGET_PARAMETERS.md)。
3. 选择一次或有限间隔模式，填写首次时间、固定 UTC 偏移和每次宽限；
   重复模式还需填写间隔和次数。
4. 点击“下一步：完整审核”，核对全部源命令、最终展开命令、目标、路线、
   原认证连接、前置关系、执行选项以及整个触发序列。
5. 人工点击“确认并启用定时计划”。每次触发只授权执行这一份完整快照。
   任务、参数、日期时间、次数、偏移或执行选项变化需要新的完整审核。

审核还绑定工作流身份、修订、依赖计划指纹和原连接实例。相同主机或端点的
重新连接不能继承旧授权；目标资料、路线、模板上下文或认证连接变化会停止
后续触发。程序式输入变化同样需要通过最终快照核对，不能借没有输入事件
保留旧审核。定时功能不会自行连接主机，也不会替换原连接。

返回工作区后，只有来自已启用计划的待放行令牌可以触发隐藏面板中的工作流。
每次放行前仍重新核对完整审核、当前目标资料和原认证连接，并在最新时钟样本
下领取一次性令牌。AI 建议仍只能成为需要人工审核的草稿；模型输出不拥有
启用、修改或执行定时计划的权限。

## 错过、占用与停止

每次触发的窗口为 `[触发时刻, 触发时刻 + 宽限]`。超过最后边界即记为
`Missed`（已过期），不会追补。系统休眠或界面延迟后，只会考虑仍处于各自
窗口的触发，过去的窗口不会形成待执行积压。

已经预留或正在运行的前次占用本次窗口时，本次记为 `SkippedBusy`（占用跳过）。
每次时钟检查至多预留一个触发，预留与领取不可重复；占用跳过的触发不会在
前次完成后补发，也不会产生重叠执行。

授权时记录 UTC 壁钟和同一进程起点的 monotonic 时钟。任一时钟相对上一样本
倒退，或两者相对授权样本的累计经过时间差超过 2000 ms，会使计划永久失效。
这也是挂起或时钟调整造成异常时的停止规则，需要人工重新审核新计划。
相同经过时间的正常正向推进会按各个绝对窗口记账；已领取的在途任务仍按
实际回执收尾。

单次工作流只有完整回执匹配原依赖计划和执行选项、所有任务都有明确零退出
状态时才记为 `Succeeded`。明确失败或拒绝记为 `Failed`；缺失、不匹配或
未能确认的完整结果记为 `Unknown`；取消记为 `Cancelled`。失败、未知或取消
会停止余下触发。单次工作流的“继续独立分支”策略只影响本次任务放行，
不会授权失败后的下一次重复。

“取消工作流”撤销未来触发并停止在途任务的本地等待，不能证明远端进程已经
停止，也不能撤销已完成的工作。取消、失效或完成后的授权不能恢复；继续执行
需要新建工作流并完成新的审核。已领取任务在停止后仍可记录实际完成回执，
该回执不会恢复未来授权。

## 输出与验收边界

计划摘要和各次触发状态最多保留 32 项，全部在内存中。任务输出只保留最近
一次执行的完整有界回执；两次触发间仍能选择任务查看最近一次输出，下次
实际放行会替换上一轮任务输出。参数输入实体和值在有限序列内保留；
每次实际触发重新读取当前值并比较完整审核，无输入事件的程序式改值同样
拒绝后续执行，恢复原值不能复活已失效序列。它们不进入连接
metadata、命令历史或现有按目标统计的普通批量审计，不保存命令、凭据或
可恢复的授权令牌。

当前范围是应用存活期间的单次和有限间隔触发。持久调度、后台 daemon、cron、
重启恢复、无限重复、自动重连、自动重试以及任务级持久审计仍需独立设计和实现。
领域状态测试、GPUI 行为测试、隔离 SSH 夹具、真实 loopback 程序和原生桌面
操作分别提供各自范围的证据；编译或夹具结果不能当作三平台原生或生产主机
验收。本文件不记录测试通过结论，验收状态以对应测试记录和
[路线图](../ROADMAP.md)为准。

The dependency workflow defaults to **Run now**. **Schedule once** and
**Bounded interval** add an explicit finite schedule to the complete human review.
Confirming the schedule authorizes every listed occurrence of the same complete
plan, execution options and original authenticated sessions. Each occurrence
checks those bindings again and claims a one-use token within its own inclusive
grace window before dispatch.

Intervals are at least 60 seconds, occurrence counts are 1–32, and grace is
1–60 seconds. The first-to-final-expiry span and the authorization-to-final-expiry
horizon are each limited to seven days. Input uses an explicit fixed UTC offset
within ±14:00; it does not follow daylight-saving changes. Occurrences remain
anchored to the original first UTC time. Expired and busy occurrences never catch
up, overlap or retry. Backward clock movement or cumulative wall/monotonic drift
over two seconds invalidates future authorization. Failure, unknown outcome and
cancellation stop the remaining sequence.

**Back to workspace** retains the in-memory plan. Replacement SSH connections
require new review, even at the same endpoint. Closing the application does not
restore schedules. Only the latest run's bounded task output is retained; slot
summaries remain transient. Persistent scheduling, daemon/cron operation,
restart recovery and task-level durable audit are outside this slice. Separate
engineering, fixture and native evidence is required for each acceptance claim.

See [ADR 0063](../adr/0063-scheduled-workflows.md) and the existing
[manual dependency workflow](DEPENDENCY_WORKFLOWS.md) contract.


User-named parameters are filled through the actual per-target controls after explicit
field synchronization. A target shares same-name values across its tasks; values may
contain Unicode, apostrophes and newlines under the existing shell-literal contract,
and may be mixed with reserved metadata such as `{{endpoint}}`. One confirmation
binds the rendered commands for the whole finite sequence. The actual input entities
and values remain in memory between occurrences and after hiding/reopening. Every
due-time check reads them again: a silent programmatic value change invalidates future
authorization, and restoring the old bytes cannot revive it. Recent task output remains
inspectable between occurrences and is replaced at the next actual admission. Neither
values, rendered commands nor outputs are written to profile metadata, history or audit.

## 与加密配置同步共存

隐藏已批准的内存计划后可以打开加密配置同步。同步只修改保存的连接元数据，
不会替换已认证 SSH 的捕获路线或把原审核改投新端点。参数和输出仍只在当前
工作区内存，不进入同步、历史或审计。等待中的完整输入每 250 ms 复核；即使
程序化编辑没有 Change 事件，也会撤销原审核。实际连接/捕获路线失效会撤销
尚未执行的审核并反馈；已执行的回执继续保留，失效计划不补跑或因恢复原值复活。

当前组合是生产 GPUI 与自有 TCP-SSH 测试候选，新的非作者组合审查、根整合和
桌面原生仍待完成，详见[组合记录](../testing/records/2026-10-06-sync-schedule-combination.md)。
