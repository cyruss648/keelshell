# 定时工作流独立审查 / Independent schedule review

审查者从精确 `24a19b954b2634c3f54d9554e5ce9b04a0062e9b` 基线和作者只读冻结补丁
重建独立 managed worktree。作者冻结包的 742 份 payload 正文、572 个完整工程输入与
新工作树逐文件相等；本轮不使用作者的可变源码，也没有操作根工作树或原生应用。
本记录补充[作者记录](2026-10-06-scheduled-workflows.md)，不覆盖其失败或验收边界。

## 审查与独立反例

未发现需要修改生产代码的确定缺陷。人工完整审核绑定原任务、最终命令、执行选项、
原始定时字段及原认证 SSH 连接。纯领域账本只产生一次性触发，生产工作区在领取前
重新核对审核和原连接，不能自动重连、重叠、追补或复用终态授权。

新增六项公开领域 API 集成测试实际通过：

- 全部 32 个授权触发各领取和完成一次，最终耗尽后不能重新触发。
- 已过期的第一张令牌不能领取或完成正在运行的下一次触发。
- 领取时壁钟倒退、monotonic 倒退或超出累计偏差均永久撤销后续授权。
- 已占用跳过的时刻在前次失败或未知后仍不可恢复；未来时刻失效。
- 七日授权边界包含最终宽限，超出一毫秒或一秒均拒绝。
- 公历时间与独立 Python `datetime` 精确 epoch 向量一致，覆盖负 epoch、世纪闰日、
  非整小时偏移和 UTC 年份边界。

新增四项生产工作区 GPUI/真实 loopback SSH 协议反例实际通过：

- 审核后的人工确认晚于首次触发时刻，计划不启用、没有远端请求。
- 首次任务收到 exec 确认但未收到完成回执，超时记为未知并使后续时刻失效。
- 隐藏面板时同一认证连接的目标 endpoint 资料变化，撤销原授权并保持零请求。
- 隐藏且已启用的计划被无输入事件地增加次数，最终快照拒绝，原三次授权均失效。

这些 SSH peer 记录实际协议请求并返回受控响应，不通过 shell 运行接收到的命令。
独立领域六项通过，整个定时 GPUI 专项十一项通过；GPUI 执行时间为 105.80 秒，
包含真实两次相隔 60 秒的有限序列、隐藏/重开、取消、明确失败、认证连接替换及
900×580 的中文/英文与 System/Light/Dark 控件检查。
这是 GPUI 自动化与隔离协议证据，不是原生桌面操作或真实主机命令执行验收。

## 工程检查与完整证据

领域专项实际 exit 0，外层 19.051 秒；GPUI 专项实际 exit 0，外层 224.097 秒。
两次各 574 个完整工程输入前后相等，全部输入正文保存并实际读回；进程 reaped、
原 kernel 身份和进程组实际不存在、独占临时目录已移除。构建使用审查者独立的
APFS copy-on-write 缓存，不包含 incremental，不是干净重建，也没有共享可写 target。

完整门禁实际 exit 0，560.742 秒：格式、依赖 `x.y`、严格 Clippy、1274 普通测试、
8 doc tests 与 6 Python 测试通过；13 ignored 不计为通过。默认与额外 2 MiB 控制器
各实际记录 626 个连续事件（sequence 0–625，最后为 controller/end）、38 个 socket
事件，future 为 5176 字节。575 个完整输入前后相等，日志 366890 字节，SHA-256
`6674227d5b0a1f22d23f63cbd1c92f8a9afe28005ed611dc38c84a2fd2701590`。

自有 loopback OpenSSH 九项互操作测试实际 exit 0，27.502 秒（外层 27.623 秒），
180 秒总期限。575 个完整输入前后相等。原六个主进程的 kernel 身份当前不存在，
测试端口实际拒绝连接，临时 key/config 目录及空的自有父目录已移除。harness 收据
记录 56 个累计观察的出生身份已清理；其原有“观察之间完全脱离的未知后代不在
可证明范围内”限制保持，不能表述为全系统清理。该互操作检查验证既有 SFTP/exec
适配器，不能替定时功能提供原生桌面或生产主机验收。

本结果 Markdown 更新在门禁后进行；仅本记录正文变化，其余 574 个输入、完整
namespace、全部 Rust/Cargo/scripts 与通过的门禁相等。最终依赖策略另外重新检查。
冻结包保留原作者失败、完整作者源正文、独立门禁的每个输入正文、日志及收据。
独立日志汇总器首次误将从零开始的事件序号按从一开始验证的失败继续保留，修正
只涉及汇总器。工程门禁和业务用例没有因该汇总失败重跑或修改。

## 未关闭的验收

此独立候选基线没有根工作树新增的用户自定义参数实现。根整合时必须保留逐目标
参数、完整最终命令审核和临时值生命周期，并独立验证“自定义参数 × 有限定时”组合。
本轮不能替该组合、根完整整合门禁、原生桌面定时/挂起/系统时区、Windows/Linux
桌面、持久调度/重启恢复或任务级持久审计提供通过结论。没有提交、推送或发布标签。

The reviewer rebuilt the exact candidate from a read-only frozen patch and the
specified base, then checked all frozen source bodies. Six new public-domain API
tests and four new production-workspace GPUI regressions passed; the full schedule
GPUI suite passed eleven tests, including two real wall-clock occurrences sixty
seconds apart. No proven production defect required a code change.

The complete strict gate passed 1274 ordinary tests, eight doc tests and six Python
tests, with formatting, the dependency policy and strict Clippy. Thirteen ignored
tests are not passes. Default and additional 2 MiB controllers each recorded 626
ordered events, numbered 0–625, and 38 socket events. Nine isolated OpenSSH tests
also passed; current principal process, port and private-directory cleanup were
checked without extending the harness's observed-descendant ownership boundary.
Only this results Markdown changed after the full gate; the remaining inputs and
all production sources are identical. GPUI and controlled protocol peers remain
separate from native desktop acceptance. The author's baseline lacks the main
worktree's custom user parameters, so the combined parameters-and-schedule feature
requires fresh integration evidence.
