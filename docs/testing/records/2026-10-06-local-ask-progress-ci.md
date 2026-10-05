# 本地 Ask 进度的精确源码 CI

日期：2026-10-06。[Quality37367209582](https://github.com/cyruss648/keelshell/actions/runs/37367209582) 的 attempt1 对应 `cfac02eab96d1791660bfafeb9bda9552c7f4cfe`，已结束整体 failure。Linux success、Windows failure、macOS cancelled。不能用本机通过或历史三平台结果关闭本次失败。

## 实际平台范围

| 平台 | 原日志与 API 支持的结果 |
| --- | --- |
| Ubuntu24.04 | 1171普通通过、12原ignored、8doc、6脚本；默认与2MiB控制器均完成、future5040字节；57打包通过；9项真实OpenSSH通过 |
| Windows2025 | 1152普通通过、11原ignored、8doc、6脚本；默认控制器完成、future5400字节；打包57项中53通过、4按平台跳过；额外2MiB控制器在45秒总期限超时，未完成 |
| macOS26 | 没有取得运行器，steps为空、runner_id为0、job日志实际404，原ZIP没有Mac日志；不证明checkout或任何macOS CI执行 |

Windows 原日志明确额外 `--controller-small-stack` 入口在 `local_agent_process.rs:73:34` 返回 `Elapsed(())`，之后主线程收到join错误，cargo exit101、检查脚本exit1。最后已知标记为future5400字节；未记录具体停滞场景或进程出生身份，**具体根因仍UNKNOWN**。默认控制器及16项MCP IPC测试实际通过，不把新小栈总期限失败归因于原MCP2秒启动测试、堆缓冲或历史预算TCP断言。有限只读诊断另行记录，不增加原期限或改写原结果。

macOS check-run 的一条failure明确托管运行器多次领取失败，另三条notice说明arm64容量受限及排队可能延长。actor/triggering_actor不是取消人；取消人仍UNKNOWN。这个状态属于未执行，不能称为源码测试失败或原生通过。

## 证据与未关闭事项

原证据封包125payload/126归档成员已由根全部逐bytes/hash读回，原ZIP23成员另与保留日志相等。根首次两次读回因假定归档路径及manifest大小写而断言失败，原收据保留；按实际目录前缀和小写manifest读回通过，未改变封包或CI结果。attempt1原日志ZIP232218字节，SHA-256 `77f170890a92db12c785656d62cd1f3ca8a6996d69f314339f571fe02e770b19`；23个成员已逐bytes/hash读取。原run/job/check-run API、错误日志404、平台日志、Linux OpenSSH实际7个产物成员与清理回执保留在ignored目录。CI读取者是MCP test-only守卫作者，其原日志读取不当作该代码的非作者审查；源码独立审查证据另见[主树整合](2026-10-06-local-ask-main-integration.md)。

本机第二完整门禁1171普通+8doc通过保持其本机范围；新版macOS中文浅色Ask的[部分原生验证](2026-10-06-local-ask-progress-native.md)另记。Windows新超时、macOS未执行及完整桌面/供应商/Release验收继续开放。任何后续重跑使用独立attempt与新收据，保留本attempt failure。

原attempt1冻结及根完整读回后，仅对未取得运行器的macOS job提交重跑请求，API实际接受。新attempt2的原API显示exact cfac和新macOS job queued；Windows/Linux虽有新job ID，完成时间和结果沿用attempt1，不能当作重跑执行。API接受与排队不等于通过，也不关闭Windows超时或原attempt1整体failure。

## 后续终态快照

2026-10-05 20:57 UTC 的新只读API快照确认：旧cfac的attempt2已completed/cancelled；Mac check-run明确说明同main并发组有更高优先级等待请求而取消，并附arm64容量notice。Windows/Linux的attempt2步骤、时间及结论与attempt1相等，不作为新执行。取消操作者仍未知。

后续文档检查点ad0912c的Quality37370415166 attempt1为completed/failure，但三个job均completed/cancelled、runner_id=0、steps为空，没有checkout或测试执行。该run的取消原因未读取，保持UNKNOWN，不能将顶层failure称为产品断言失败，也不代表任何平台通过。18份新原始API/收据材料已由根核对，独立读取仅4个GET，未改变旧失败。后续源码提交CI须按自己的精确head单独检查。

## 2f7ca1e 的新阶段证据

[Quality37377310693](https://github.com/cyruss648/keelshell/actions/runs/37377310693) attempt1对应精确2f7ca1e，三个job实际执行。Linux/macOS成功；Windows默认完整控制器完成626条记录、38次TCP、future5440字节，73.497253秒。26次实际ConnectionRefused/10061耗时52.451005秒，已超过额外小栈45秒整体预算；小栈在序号370记录期限到达，整体failure。新Windows预算候选与全部原日志hash见[独立记录](2026-10-06-windows-small-stack-budget.md)，尚待新Windows执行。这组新阶段事实不追溯证明早期cfac未采集pending的根因。


## 5b6b7b4 的预算修正验证

[Quality37381858939](https://github.com/cyruss648/keelshell/actions/runs/37381858939) attempt1精确5b6b7b4三平台实际成功。Windows默认/2MiB入口分别71.242069/69.283210秒完成626阶段、38原TCP及5440字节future；90秒整体预算已实际覆盖小栈完整控制器。新非作者复核通过该test-only范围，源码按精确提交快进整合。全部原失败、原日志hash与各平台计数见[预算记录](2026-10-06-windows-small-stack-budget.md)；早期cfac的未采集pending仍UNKNOWN，不追溯改写其根因，也不将此源码CI升级为桌面/供应商/发布验收。
