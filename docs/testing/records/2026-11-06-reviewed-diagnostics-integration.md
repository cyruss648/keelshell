# 审核式诊断与面板外观整合 — 2026-11-06

基线 `56d5440e4fa260f53ee6ea404fee9223f41d2188`，
分支 `feature/pane-surface-polish-20261106`。证据保存在 ignored 的
`work/pane-surface-polish-20261106/`，公开路径不是已发布附件。

## 来源与增量

ICMP 作者 candidate-r1 为 13 路径，其中 10 个源码/测试、3 个文档；
补丁 SHA-256 为 `30facd49be5cd6accb464754599eacef966cecd98f4cb30b6bf1122e0b28875e`。
对父基线的 preimage 全部相等，应用后 13 项全部匹配候选摘要。
首次后验脚本误将嵌套摘要对象与字符串比较；按 `candidate_sha256` 重算全通过，
补丁只应用一次，来源原件未修改。见 `icmp-integration-r1/import.json`。

父新增选择 UUID 与前台同步撤权，接入全部生产活动选择入口。
预览、确认和结果采用分别校验；不修改其他拥有者共享的会话有效性接口。
同一前台操作内切走再切回需要新审核，关闭非活动标签保留当前审核。
设计见[ADR 0106](../../adr/0106-diagnostic-selection-consent.md)。
未连接面板采用语义画布并移除终端内边距，见[外观记录](2026-11-06-pane-surfaces.md)。

新的非作者 `icmp-source-r1` 对冻结来源提出两个 P2：
未验证监督器终态的环境错误仍能重新执行，以及 quiet 计数被过强描述为实际发送/丢包。
父将 ICMP 所有错误保守停权，增加受控 SSH 非零、仅信号和缺失 Python 的
当前/过期标记反例；工具尝试与未获回复的语义同步修订。来源失败报告保持原字节，
修订复核和组合结论单独记录。

## 已实际执行

`protocol-r1`：`cargo test -p keelshell-app --locked -- protocol_ --test-threads=4`，
15 passed，0 failed，0 ignored；525 项工程输入前后一致。
固定 1,500 秒预算，实际 16:43:38–16:44:41 UTC，子进程与父执行会话 29548
均 exit 0/reaped。使用共享 target、2 jobs、离线依赖。
包含真实生产 ABA、非活动关闭、新审核成功、挂起回复撤权，以及受控 SSH
非零/信号/缺失 Python 帧；这些帧不代表真的杀死远端监督器。
之后仅新增计数限制说明行，该行仍需组合门禁，不由本次专项代签。

作者计数语义 r2 的六路径增量已逐摘要导入，原 Python adapter 的非注释 token
及 AST 保持；新增 quiet 本地发送失败计数反例随离线测试通过。
新非作者 `icmp-r2` 对原两项 P2 为 `APPROVE_SCOPED_ICMP_R2_SOURCE`，
限定范围内开放 P0/P1/P2/P3 均为零；不代签后续焦点修订或组合原生。

首轮 `full-gate-r1` 在测试取消句柄的 `expect_used` Clippy 停止，
父执行会话 99904 exit 1/reaped，525 输入保持；改用既有测试失败风格，
原日志不覆盖。`full-gate-r2` 通过格式/严格 Clippy，随后在
`remote_only::product_sources_cannot_restore_a_local_shell_fallback` 停止：
新 `network_diagnostic/tests/icmp.rs` 的本地 fake-process 测试被文本守卫识别为产品代码。
执行会话 17319 exit 1/reaped，525 输入保持；没有发现产品本地终端回退。
夹具现以首部 `#![cfg(test)]` 显式排除发行代码，守卫只认可首部 inner cfg，
不按目录跳过，也不让 outer 或非首部 cfg 隐藏产品代码。
新增 LF/CRLF 正反例与既有检查共 5 passed，`remote-only-r1` / exec 67792 exit 0/reaped。

新非作者选择复核 `selection-r1` 对 r2 冻结给出一项 P2：
GPUI 的 `on_focus_in` 到绘制末尾才通知，真实终端指针 B→A 可在此之前保持旧选择。
父在终端正文 wrapper 的捕获阶段同步使用 Workspace 选择入口，
在子终端借用/鼠标报告消费之前撤销审核并取消任务。
两项实际 `window.dispatch_event` 回归只在准备时绘制；
后续 MouseMove/Down/Up B→A 没有中间绘制，检查即时选中、撤权、取消、
晚到拒绝和新审核成功。`pointer-r2` 为 2 passed / 0 failed，
exec 72642 exit 0/reaped、525 输入保持。
`pointer-r1` 首次格式/编译的属性位置与 trait 导入错误保持在日志中，已纠正。
只移除捕获 handler、保留测试目标身份后，同一回归在预期 active=0/expected=1
断言失败，cargo 实际 exit 101；负控 harness exec 50283 exit 0/reaped，
该外层零只表示预期失败已确认并将生产文件逐字节恢复，不是测试通过。
负控原件见 `pointer-counterexample-r1/`。

`full-gate-r3`于17:13:46–17:23:01 UTC通过，exec81606已实际wait、exit0/reaped；
525输入前后一致。ordinary为2,236 passed/23 ignored，app970/2 ignored，
14 rustdoc、Python131 run/1 skip及附加控制器通过。
这次绿灯只签当时输入，后续四个app路径的搜索选择修订需要新的门禁。

`selection-r3`认可正文指针与guard修订，但原P2仍开放于遮挡搜索条和键盘入口。
父新增TerminalView输入所有者通知、搜索条本身的捕获处理与已完成Tab动作观察，
只按EntityId同步父选择，不在子实体借用中反读TerminalView/InputState，
不把搜索焦点移回正文。Workspace已有选择时使用原FocusHandle避免反向重入。
搜索输入焦点从祖先移除重复登记，保留Input自身的登记和动作冒泡。
新`search-r1`编译错误与`search-r2`两绿一红的Tab自循环均保留；
修后`search-r3`三项passed，exec48788已实际wait、exit0/reaped，525输入保持。
覆盖遮挡搜索关闭撤权、Root Tab/Shift-Tab与Escape、同实体搜索保留审核/输入焦点、
挂起ICMP回复取消，以及重新审核可真正dispatch成功的正控。
键盘断言在key-down返回后、下一事件前；不把后续按键开始前的GPUI绘制说成零次绘制。
停用输入所有者通知后，同一搜索关闭回归在active0/expected1失败，Cargo101；
外层exec85227退出0/reaped只签预期负控与逐字节恢复，原件在`search-counterexample-r1/`。

11个app路径另冻在`review-selection-r4/`。
相同525输入的新`build-r1`已通过并打包；
`full-gate-r4`于17:43:55–17:53:01 UTC退出0，父exec35977已实际wait、exit0/reaped，
525输入前后保持且与构建一致。2,239 ordinary passed/23 ignored，
app973/2 ignored（已包含在ordinary）、14 rustdoc、Python131 run/1 skip；
格式、严格Clippy、依赖策略和追加控制器均通过。
最终组合源码复核单独核对，原r3不代签新hook。
新包一次macOS离线启动取得24画布/12设置原件，完成三家族×明暗×双语的限定观察，
app和父exec89824均退出0/reaped且原600秒内清理，见[画布记录](2026-11-06-pane-surfaces.md)。
这些原件不代签真实SSH/ICMP、输入或其他平台。
本记录尚不授予 feature complete、完整平台原生或产品 finish。

### 搜索按钮入口的后续修订

`selection-r4`及组合`combined-r1`保留同一SEL-P2-01：
仅识别搜索Input会遗漏三个按钮自身的Tab stop，反向Tab可绕过Input，
空格激活上一个/下一个按钮又直接进入未发布所有者的搜索导航动作。
这是一项选择撤权缺口，不能用r4绿灯关闭。

父增加独立且非Tab-stop的搜索父焦点范围，包含Input及全部按钮；
搜索导航动作也同步发布所有者，回调仍不读取已借用的子实体。
增量只涉及`terminal.rs`及`monitor_tests/protocol_selection.rs`，
冻结在`review-selection-r5/`和`review-combined-r2/`；
其余20个组合工程路径保持字节相同。
`search-r4`于18:16:39–18:17:03 UTC为6 passed，
exec52073已实际wait、exit0/reaped，525输入保持。
新增三项覆盖三按钮×双向Tab/空格、绕过Tab观察者的直接空格导航、
挂起回复即时取消及同所有者审核保留。
焦点检查在key-down返回后、下一事件前，空格激活检查在key-up返回后；
不声称多个按键之间完全没有GPUI绘制。

两个独立负控分别恢复Input-only焦点识别和移除导航发布，
相同测试均在active0/expected1断言失败，Cargo101。
外层exec32612已实际wait、exit0/reaped，只签预期失败与源码精确恢复。
每例固定180秒，实际16.412秒及15.131秒，没有超时或改断言；
原件为`negative-button-*.log`及`search-button-negative-result.json`。
恢复后新`build-r2`通过，exec78221退出0/reaped。
新的非作者`selection-r5`限定批准关闭SEL-P2-01，开放P0/P1/P2/P3均为零，
核对两文件增量、有效11个app路径、525输入及两项负控原件。
新`full-gate-r5`于18:20:25–18:29:38 UTC退出0，
父exec80111已实际wait、exit0/reaped，525输入保持，与build-r2和当前源码相同。
2,242 ordinary passed/23 ignored，app976/2 ignored已包含其中，
14 rustdoc、Python131 run（130 passed/1 skip），格式、严格Clippy、
依赖策略及追加控制器均通过。原r4门禁和r1包保持各自归属；
新非作者`combined-r2`批准冻结产品源码、完整工程门禁及包归属，
开放P0/P1/P2/P3均为零；独立重算上述81个结果块与父计数一致。
该批准不含正式文档、真实CLI准备或原生运行。

## 依赖

官方 `dependency-freshness-r1` 发现 zip 9.4.0、rmcp 3.9.0，
首次 failed 报告、104 条记录及 exec 14443 exit 1/reaped 保留。
声明更新为 `9.4` / `3.9`，lock 仅更新这两个 package；
`cargo update` 固定 300 秒，实际 8.035 秒，exec 49518 exit 0/reaped。
`dependency-freshness-r2` 于 16:56:19–16:56:34 UTC 通过 104 条要求，
exec 96051 exit 0/reaped。后续选择/guard 修订不修改 manifest 或 lock；
完整门禁针对新的冻结工程输入另行记录。
最终`dependency-freshness-r3`于17:51:16–17:51:32 UTC再次通过104条要求，
父exec41777已实际wait、exit0/reaped，525工程输入保持，无额外版本变更。
搜索按钮修订后`dependency-freshness-r4`于18:21:23–18:21:39 UTC重新
通过104条要求，父exec21640已实际wait、exit0/reaped，525输入保持，
没有新增依赖版本变化。

## 真实回环探测准备的独立状态

专用iputils镜像已按原记录完成工具版本核对，真实SSH/ICMP尚未验收。
单次OpenSSH CLI携带原生产Python命令的双栈回环准备r1未执行；
新非作者发现两项准备P2：继承umask077导致fixture目录不可穿越，
以及父回执写入失败会跳过清理。原冻结和拒绝结论保留，r2最小修订另行准备。
这两项属于ignored测试工具，不修改本次525项产品工程输入或其源码批准。
真实执行仍须固定300秒总TTL（含30秒清理预留）、独特owner及精确资源删除；
即使通过，也只签CLI→原生产Python/iputils的probe层，不代签Rust传输/解析、
审核UI或native。不得把工具版本检查记作已发送Echo。

## 文件拖放的独立诊断

原 preflight `1b9beeac25b445039161db570e320414` 保留。
父快审后只运行一次额外查询 `db1ef92b8ca34691b8f8f7b739a1b0ef`：
相同 `sandbox-exec` / `sftp-server -Q requests`，固定 30 秒含 8 秒清理，
最多保留 16 KiB stderr。实际 query 为 -6/SIGABRT，完整 EOF 但 stderr 为零字节，
原因仍 UNKNOWN。0.146 秒内回收唯一子进程，后查该 PID 不存在；
外层同步 exit 1/reaped。没有服务、应用、bind/connect 或新一轮原生拖放。
见 `work/reviewed-file-drop-20261105/diagnostics-r1/`。
原准备的两项独立 P2（响应记录识别、最终检查后的 TTL）已在独立 r2 修订，
原反例两项修前失败、33 项离线检查修后通过；原非作者
`preparation-r2` 为 `APPROVE_SCOPED_PREPARATION`、开放发现零，
仍不批准新的 runtime 或 native。本次 PID 对应的系统 crash 报告已只读匹配，
触发栈位于 dyld，只有原始 termination namespace/code，
没有具体 deny/路径说明，因此具体根因仍 UNKNOWN。
不改写首次失败或用放宽沙箱取得通过。
