# 定时工作流与逐目标参数组合 — 2026-10-06

这是在精确 `c15892c099a70ae8e1ccd89f24175fcf5c346dbf` 上的新独立作者组合，
不是原生桌面验收或新的非作者结论。旧有限定时作者 FINAL-v2 与新的非作者
候选保持冻结；这次完整逐字核验旧非作者 7,450 份封存正文、336,471,352 字节
及完整文件集合，核验 15 路径三方材料后只应用 14 个 clean/new 路径，
路线图保留该基线无关状态并手工合入定时条目。现有自定义参数、传输、磁盘、
核心导出及工作区最终认证检查保留，不使用旧候选全树覆盖。

旧限定审查见[独立记录](2026-10-06-scheduled-workflows-independent.md)。
那份候选没有用户自定义参数能力，不能借其通过结论关闭本轮组合。
根后继 Linux/Windows CI 的四项测试隔离修复另有记录；这棵 c158 独立树不
继承后继源码或 CI 结果，合入根时需要三方保留后继修改和新的完整门禁。

## 实际反例与窄修复

重复计划首轮收到完整成功回执后，传输 handle 已释放且 ledger 仍 Active。
旧输出 view 只检查 handle 或整个计划 complete，导致等待第二次触发时
已有正文隐藏，无法查看最近一次输出。实际 production GPUI 控件、人工
确认、自有 TCP/SSH peer 的首轮结果复现此问题：原正式用例 exit 101，
0 passed/1 failed，4.48 秒用例正文、47.873204 秒外层。
627 个完整工程输入正文及前后 map 相等，原测试与完整失败日志保留。

窄修复只让存在已存任务 receipt 时继续展示输出；下次实际 claim 仍清除
旧 progress/detail，保持仅最近一次完整有界输出。原反例测试正文保持精确
相同，修复后实际通过。另将中英文失效提示补齐参数与定时配置的可能变化；
没有扩展重连、恢复、重试、持久化或 AI/MCP 授权。

## 新组合行为

四项生产 GPUI/TCP 测试实际 4 passed/0 failed，正文 66.06 秒、外层
74.796899 秒；627 输入前后相等、每一份正文独立保存。没有 mock 时间或
手工 tick。两个真实 60 秒等待由自有 Tokio wall-clock timer 与 GPUI
生产调度器驱动，外层单次测试预算 1,200 秒。

- 通过实际打开、选目标、同步参数、键盘焦点 Textarea 填值、时间输入、
  完整审核及人工确认启用。`{{release}}` 的值含中文、apostrophe 与换行，
  混合保留 `{{endpoint}}` 元数据。count=2、间隔 60 秒；两次精确 exec
  wire bytes 匹配独立字面引用向量，两个回执均明确成功。首轮之后及第二轮
  之后，实际输入 Entity 仍附着原目标且 raw 值相同。实际隐藏/重开保留
  panel 和原完整审核。900×580、中文/英文与 System/Light/Dark 六组合中
  最近一次输出可滚动到达，固定操作区保持。
- 首轮完整成功后隐藏面板，用被捕获的实际 Textarea `set_value` 静默改值，
  不触发 InputEvent。原完整 review 仍保留，但重新读取 snapshot 已不等。
  等待第二个原真实 deadline 后失效，只有首轮一个 wire；恢复原值、重开
  面板及实际再等待不能复活已结束授权。
- 原认证 SSH 被显示端点 metadata 不变的另一独立已认证连接替换时，以及同连接的 metadata 改变时，
  生产路径均停止定时放行并清除旧目标值，两个 peer 使用独立动态回环 TCP 端口，不能据此声称同一物理 host:port
  的真实重连已验收；两个 peer 都没有收到命令。没有
  手工 maintain/tick 来模拟 scheduler；生产目标维护可在到期前先停止授权。
- 各例检查零 PTY 写入，实际私有 state 文件和持久 state 模型不含参数、
  展开命令或输出；command histories、pending audits 和已存 batch audits
  均为空。SSH peers 只记录命令并生成受控回执，不运行 OS shell。

## 门禁与来源

首个严格 Clippy 的新测试 helper 上下文类型错误 exit 101 保留，627 完整
输入与原日志前后相等；仅测试 helper 修正后严格 Clippy actual 0，
32.997598 秒。格式检查及 x.y 门禁各 actual 0。输出反例及新组合也各保留
完整当次输入正文，不能把这四项通过当作尚未运行的整仓门禁。

第一轮完整组合 scripts/check.py actual 0/669.956798 秒，1434 普通、8 doc、
6 Python、16 ignored（原摘要 14 的计数错误保留并更正）；默认与 2 MiB 控制器各 626 顺序事件、38 socket 事件、
future 5176 字节。628 输入前后相等，完整正文与日志保存。此后作者只读核对
发现两个测试在外观操作之后才计算剩余等待 floor，慢机可能假失败；仅将
计时改为确认前依据原 spec 的第二绝对 UTC 期限和真实 monotonic 起点，
包含所有隐藏/重开/布局耗时并保留生产允许的两秒 drift 边界。旧原测试、
原 101、原同字节 PASS 和第一轮完整通过均保留，生产调度/传输/核心未改。

最终新计时测试版本完整 scripts/check.py actual 0/554.162505 秒：1434 普通、
8 doc、6 Python、格式、x.y 和严格 Clippy。16 ignored 均不算通过：安装的
供应商 CLI 2、需单独自有 native probe 的应用测试 2、真实 Linux 样本 1、
OpenSSH 11。额外 2 MiB 是独立 custom controller main，没有第二份 libtest
ignored 计数。两控制器各 626 事件 sequence 0..625、38 socket、future 5176 字节，
都有 controller/end。628 完整输入 13,914,145 字节前后相等；输入 map SHA-256
`b387bfee4cc84e019a989e6951a46dd6b7894b5c502eafe3516bc96fe997e1fa`；
完整日志 386,124 字节、SHA-256
`e7bb4e9744521984d6164e046c602248169755721c4caee3048539ef4475cf66`。

后继受控 OpenSSH 11 项 actual PASS/31.61 秒，外层 actual 0/31.685549 秒，
628 完整输入仍相等。harness 累积 62 份 kernel birth identities 已停止，
ancestry_unverified 为空；它保留“两个观察之间完全脱离的后代不能追溯证明”
的原 census 边界。作者另行读取六个 principal PID 都为 ESRCH，原监听端口
ECONNREFUSED，私有运行目录已移除，空的所属父目录也已移除；没有删除
日志/receipt。这属于现有 transport interoperability，不是原生桌面定时验收。

静默 `set_value` 的锁定依赖来源另行读回：gpui-base 0.7.0 的原 crate 包
SHA 与 Cargo.lock 一致，选定源码与原包逐字相同，set_value 明确关闭 Change
事件。首次读取器错误假设 extracted registry 有 .cargo-checksum.json，
FileNotFoundError 与原因保留；原 inline preimage 未单独保存。具名修正读取器
改为核对原 crate checksum，不把原失败继承为成功。没有修改依赖或工程输入。

成功后只更新本测试记录和路线图中的结果文字；其它 Rust/Cargo/scripts/
文件集合与最终门禁一致，doc-only delta 单独记录。新的作者候选仍需冻结后
非作者组合增量审查和根三方整合门禁，作者不自行宣布新组合独立通过。

APFS CoW 复制来自已静止的自有旧作者 target，复制前 cargo lock 无 holder；
新 target 是独立目录且没有 shared writable target 或 symlink。根 target
没有读取或写入。本次不是 clean rebuild。每次 runner 使用私有 TMPDIR、
独立进程组、有限等待、原进程退出与组消失检查；所有日志、完整源码、
launch/ownership/result receipt 和失败保留在 ignored 私有工作材料中。

初始三方材料读取器曾假设新文件存在于 theirs 目录而发生 FileNotFoundError；
发生在源 mutation 之前，旧完整 seal 已核对。原因与 tool output 保留，
原 inline 读取器没有单独完整文件 preimage；此限制不隐藏为产品失败。
后续具名读取器重新完整通过后才应用 14 个路径。

没有运行 native application、客户 SSH、Podman、供应商 CLI、真实模型、
安装、提交、推送或标签。三平台桌面、真实远端 shell 执行、系统休眠/时区
变化、跨平台最小窗口/辅助技术、后台持久调度与任务级持久审计继续 OPEN。
