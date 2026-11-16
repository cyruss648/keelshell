# Windows 归档缺审诊断回执

结论：精确源码 `76b8253acd9bfa62180563f482e851064bc1ea34` 的 Quality
`43806549649 / attempt1` 已 completed/failure。Windows failure，macOS 和
Ubuntu success。Windows 八项缺审现在有真实 I/O 失败分类与显示数字后缀，但
**具体根因仍为 UNKNOWN**。本任务未创建修复候选或补丁，`candidate/` 为空。

初始固定期限为 2026-11-16 05:03:58.119697–05:33:58.119697 UTC，
即上海时间 13:03:58.119697–13:33:58.119697。观察因三平台终态及日志全部取回，
于 05:28:34.537016 UTC 正常结束，观察进程实际 exit 0，未延长期限。
共追加 45 份快照、183 次 GET 回执，GET 失败 0、超时 0；各 GET 子进程均已
wait/reap。没有取消或重跑 GitHub run，没有跟进父的新分支 run。

所有快照均绑定固定源码、run、attempt 和三个指定 job；三个完整日志正文也均
包含上述完整源码 SHA。最终状态见 `snapshot-0045.json` 和 `observer-final.json`。

| 平台 / job | 终态 | app passed / failed / ignored | archive_tests passed / failed / ignored | app 秒数 |
| --- | --- | --- | --- | --- |
| Windows / 133098539513 | failure | 1320 / 8 / 2 | 4 / 8 / 0 | 1167.79 |
| macOS / 133098539273 | success | 1339 / 0 / 2 | 12 / 0 / 0 | 686.71 |
| Ubuntu / 133098539119 | success | 1340 / 0 / 2 | 12 / 0 / 0 | 767.51 |

macOS 普通 Rust 汇总 3220 passed / 0 failed / 48 ignored，rustdoc 14/0；
Ubuntu 普通 Rust 3213/0/49 ignored，rustdoc 14/0。Windows 仅执行到失败的 app：
已返回的 9 个 Rust 结果块合计 1480 passed / 8 failed / 6 ignored，不能当作完整
workspace 结果，rustdoc 未运行。各结果块保存在 `log-summary-r03.json`。

Python 实际结果：

| 平台 | Packaging | 工程 Python | 后续 archive helper |
| --- | --- | --- | --- |
| Windows | 57 run / 4 skipped / OK | 290 run / 34 skipped / OK | 未运行 |
| macOS | 57 run / 0 skipped / OK | 290 run / 1 skipped / OK | 30 run / OK |
| Ubuntu | 57 run / 0 skipped / OK | 290 run / 1 skipped / OK | 30 run / OK |

Windows 的八份新诊断完全一致地给出：

```text
stage=review-error kind=io reason=other-redacted display_os_error=Some(123)
busy=false operation_id_present=false pending_kind=None
session_present=true suspended=false directory_present=true
local_reading=false isolation_present=false close_hold=false
atomic_writes_started=0
```

其中 5 份有本地上传选择，计数分别为 1、1、1、1、2；3 份有远端解包选择，
计数均为 1。对应操作的控件在八份诊断中均 `visible=true`、
`disabled()=None`；另一操作的控件是 `Some(true)`。两控件的真实 bounds 均已
保留在 `windows-diagnostics.json`。不把 `None` 改写成一次独立原生控件验收。
已有审核准备通道报告 I/O 失败，没有得到完整不可变审核，夹具原子写入计数为 0。
这不能扩展为整个进程所有后端零写入或后续安全断言通过的证明。

`123` 仅为既有显示字符串的末尾数字，不是本任务保留的 typed io::Error，
不据此命名 errno、NTSTATUS、错误原因或失败 syscall。精确源码原件显示：

- `archive_tests.rs:106` 从 `(os error N)` 显示后缀解析数字；149 行起仅对有限的
  固定 I/O 文本分类，195 行已明确这一数字不是保留的 typed io::Error。
- `files.rs:750` 把普通 `SessionError` 变成 `FileFailure::Transport(error.to_string())`；
  `files.rs:2144` 再将其用于审核未完成状态。`SessionError::Io` 本来持有
  `std::io::Error`，见 `error.rs:3`。当前失败日志没有保留该 typed 值和阶段。
- `archive.rs:123` 调用 session 的 `plan_archive_transfer`，两种请求分别进入
  `archive_transfer.rs:259` 的本地 scan 或 277 行的 destination。
- Windows 的祖先链打开、逐层名称枚举、metadata/安全信息观察、源文件读取和
  目标不存在检查都可能经 `?` 进入这个通用 I/O 通道。参见
  `local_windows.rs:200`、224、336、383，`windows/mod.rs:108`、225，
  `windows/native.rs:42`、94、238、275、396。现有日志没有失败原生调用标签，
  也没有底层调用栈，不能从共同数字后缀唯一选中其中一条路径。

这八项真实失败的完整 stdout、panic 和最终失败列表，均保存在原始 Windows
日志及去 ANSI 副本。便于阅读的完整脱敏失败段为
`windows-failure-contexts-r03.redacted.log`；其第二个段是 Rust 末尾失败名单的
重复摘录，并非额外失败。八个唯一失败名及逐项状态见
`log-summary-r03.json` 和 `windows-diagnostics.json`。

具体缺口是：在发生失败的同一次 Windows 运行中，缺少受限的本地操作阶段、
错误来源类别与 typed 原始错误证据，例如祖先链打开、名称枚举、metadata/
安全观察、源读取或目标不存在检查的区分。下一步应在获授权的 session/platform
诊断范围内保留固定阶段枚举和 typed 错误信息，继续禁止输出路径、peer 文本或
secret；不能仅凭显示数字更改驱动路径、枚举方式、权限或安全检查。
本次只允许 app archive 候选，当前证据不足以支持该范围内的最小根因修复，
所以没有盲改或用测试适配掩盖底层问题。

原有十项有效测试、两个新增诊断测试以及不支持平台的条件测试，已逐函数保存
字节摘要。十项原测试中，本轮 Windows 两项通过、八项停在审核准备前置；
两个诊断测试均通过。八项后续安全断言没有因本任务被删除、跳过、重试、放宽或
加等待；也不能把它们未到达的正文签成通过。未伪造审核或替换后端。

正式 app archive 十个文件在初始、父提交更新后和最终三次核对均与精确原件
零漂移。父报告的 `bc9ebd6cf4f213b9330c04380f866ead9133fb0a` 门禁和 review
信息单列于 `parent-update-r01.json`，其门禁没有由本任务独立重跑，也不移签本次
固定 run。没有修改正式源码或 target；没有调用 Cargo、rustfmt、clean、commit、
push；没有新建用户 chat、worktree 或 agent。候选编译、专项、full gate、
非作者 review 和 Windows 修复验收均无可签结果，因为没有候选。

所有创建路径都在本任务根目录之下：

- `originals/NNNN-*.response.json`、`.stderr.log`：编号 GET 原件。
- `originals/*-logs.raw.log`：三平台原始日志，未去 ANSI 或改写字节。
- `originals/source/`：精确提交的十份 app archive 原件。
- `originals/context/`：八份只读代码上下文，不是候选。
- `report/`：45 份快照、全部 GET 回执、去 ANSI 日志、脱敏失败段、三轮结果
  摘要、源码基线、逐函数测试摘要、诊断记录、父更新、最终证据和本报告。
- `candidate/`：空，没有候选源文件或 patch。

完整逐文件路径、长度与 SHA-256 见 `created-files-manifest.json`；该清单自身的
摘要单列于 `created-files-manifest.sha256`。原始日志摘要为：

| 原件 | 字节数 | SHA-256 |
| --- | --- | --- |
| originals/0045-windows-logs.raw.log | 515046 | 6d9e42c2cfee556bf605dddd8882044dbfdb66bad40ac6aa5148826401849d4f |
| originals/0024-macos-logs.raw.log | 854609 | 23a7d3e3c11afed22dc4de33e07233d57f161d3627edd0c2be2a1c369863c85f |
| originals/0021-ubuntu-logs.raw.log | 874887 | 14d5d519d8a26c3c8bef4af148780c99c1e97d2240a0518b6bd5201888b14d13 |

建议父保留本次失败原件和 UNKNOWN 边界，接续受限底层阶段诊断；不要将此诊断
交付称为 Windows 修复，不跳过原八项，也不因本次失败取消或重跑固定 run。
本任务观察进程已经退出，现将全部副本交还父归档和清理。
