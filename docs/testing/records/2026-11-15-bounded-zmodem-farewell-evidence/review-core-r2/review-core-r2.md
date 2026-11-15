# KeelShell bounded farewell r2 独立严格只读复核

结论：**HOLD**。开放问题为 **P0 0 / P1 0 / P2 1 / P3 0**。
原 `CORE-R1-01` 在本次 r2 输入中关闭；新增 `CORE-R2-01` 阻止本批 core
尾部保真范围的批准。专项已经实际执行至终态，94/94 通过；这个结果不能覆盖
缺少回归的取消和错误路径。父已确认收到新 notice，负责后续红回归与修复；
r2 原件保持，本报告不扩展到尚未冻结的 r3。

审查基线为 HEAD `b66ec7e564e97c3cc8261b447288377ac1ffccc2`；
审查对象是以下四个文件相对 HEAD 的工作树差分：

- `crates/keelshell-core/src/zmodem.rs`
- `crates/keelshell-core/src/zmodem/codec.rs`
- `crates/keelshell-core/src/zmodem/receiver.rs`
- `crates/keelshell-core/tests/zmodem.rs`

已读取 AGENTS、RTK、HANDOFF 的当前批次与持续约束、SHARED_TARGET、ADR 0137
及原 CORE-R1-01 notice；应用 `rust-best-practices` 第 1、2、3、4、5、7、8 章
与本次相关的所有权、预算、状态、错误、断言和文档原则。用户禁止 Cargo 与
格式化的要求优先于 skill 中的通用执行建议。

**CORE-R2-01 / P2 — peer cancellation 或失败状态丢失已保留的普通尾部。**
主要定位是
[receiver.rs:253](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:253)。
`cancel()` 仅在当前状态仍为 Finishing 时提取 farewell tail，然而 peer cancel
和输入错误会先改变状态。

在有效初始 ZFIN 后，先成功 `feed("个$ ".as_bytes())`，再输入有效
Cancel/Abort/FileError 头或五个 CAN：
[receiver.rs:122](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:122)
及
[receiver.rs:343](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:343)
将状态改为 Cancelled。
[receiver.rs:137](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:137)
只为 Done 提取尾部，随后本地 `cancel()` 又在
[receiver.rs:247](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:247)
空返回，因此先前的 `e4b8aa2420` 永远没有 Terminal 事件。需要排除已经验证的
取消协议字节，并将此前普通输出交还一次。

另一个同根路径是在成功保留该尾部后输入有效 ZCOMMAND。
`header()` 返回 RemoteCommand，
[receiver.rs:98](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:98)
使状态进入 Failed。按公开错误合同调用 `cancel()` 时，Finishing 条件已不成立，
仍只得到 Send(CANCEL) 与 Cancelled，丢失此前安全接收的普通尾部。后续
retry/limit 失败也有相同状态所有权问题。这里不要求重放失败输入、放宽命令拒绝，
或产生 Finished/FarewellMissing。

这是确定性的源码和状态追踪，**没有新编译或执行上述失败 Rust 探针**。现有
peer/error 测试没有前置普通尾部；新增本地取消测试只覆盖仍处于 Finishing
和已经严格 Done 的两条路径。影响为 shell 提示符或诊断输出在取消/失败收尾中
静默丢失。[即时 notice](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/notice-new-01.md)、
[逐行证据](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/notice-new-01-evidence.json) 已在发现时落盘。父需要集中增加整块、
逐字节及分片的 peer cancel 与 Failed-cleanup 回归，检查重复取消和后续输入
不重复尾部，再绑定新冻结供独立复核。

原 **CORE-R1-01 已在 r2 关闭**：
[codec.rs:470](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/codec.rs:470)
至候选起点判断只回看最多四字节，保护有效或合法未完整 UTF-8 序列中的 `0xaa`，
没有禁用通用 parity header 解析。六项原始红回归已在本次独立 94 项运行中全部
通过，覆盖真实 OO 与独立缺失结果两条路径的整块、逐字节及每个二段分片边界。
三个 parity 用例也通过，覆盖单个、两个、混合高低位和八个 pad；两个新增本地
取消用例验证尾部交还及严格完成后不重复。父此前六项真实红回归的 exit 101、
失败字节与日志摘要也已核验，原件未改变。见
[closure-and-coverage.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/closure-and-coverage.json)。

本次检查还确认以下限制在所审路径中成立；取消/失败尾部缺口仍由上面的 P2
单独阻断：

- 只有完整 CRC 有效的初始 ZFIN 才进入等待；未审核 offer、接收中、未确认保存、
  错误和取消均不能转换成缺失 farewell。
- `FarewellMissing` 与严格 `Finished` 分别表示不同终态；前者保持
  `is_finished() == false`，后续调用幂等，后续 feed 只交还新字节。core 的
  rustdoc 明确由调用者证明精确 lease、最新回复写完、grace 或有序 clean exit。
- farewell 保留长度上限为 65,632 字节，header candidate 上限 96 字节，
  单次 feed 上限 65,536 字节，累计 retry 上限 64；原 wire/packet/event
  限制继续存在。只有验证过的重复 ZFIN 在成功解析时裁剪，未完整 lookalike
  和 unmatched O 在通常收尾路径保留。相关预算、严格结果和尾部专项均包含
  在已执行的 94 项中，不另行叠加计数。

独立执行是直接运行父的稳定副本
`work/pty-farewell-20261115/stable-tests/core-r3`，
SHA-256
`40586dcf5a757695a86c1cf1a50ca9ae8426cda277b27499836c02e65c97970c`。
`core-r3-binary.json` 指向本次 r2 freeze，清单摘要为
`4107f95f28226d66845d3cd04d3a008f370c31c1b2eb4a900a7e0c8311204a7a`。
父 `core-r3.log` 中的实际重编译记录、94 项终态和 receipt/log hash 相符；
未读取其 shared target，也没有独立重编译。

| 复核执行或核验 | 实际结果 |
| --- | --- |
| 稳定副本 `--list`，1 次 | exit 0；94 个唯一测试名，与源码及父清单逐字节/逐名一致 |
| 稳定副本 `--test-threads=4`，1 次 | exit 0；94 passed / 0 failed / 0 ignored / 0 measured / 0 filtered out；0.05s 测试时间 |
| 原 HEAD 65 项测试 | 65/65 测试正文逐字节一致；无删除、改写或缺名 |
| 整个测试文件差分 | 729 行新增、0 行删除；原 helper 与断言未被替换 |
| 初始及结束完整冻结检查 | 每次 571 工程输入 + 12 脚本；全部 583/583 匹配，未冻结新增/遗漏均为 0 |
| reviewer Cargo、编译、format、Clippy、git mutation、共享 target 操作 | 全部 0 |
| reviewer 真实 peer、SSH、原生桌面、完整工程门禁执行 | 全部 0 |

首次 reviewer 清单校验脚本误把 baseline helper 也作为测试名，触发脚本断言；
`--list` 本身返回 0，尚未启动测试。该错误已保留在
[inventory-harness-attempt-01.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/inventory-harness-attempt-01.json)；
修正为只读取 65 个原测试名后，测试仅实际运行一次，共 94 个。这个脚本错误
不计作 Rust 失败，也未通过重复专项放大通过数。

完整执行证据为
[core-r3-independent-execution.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/core-r3-independent-execution.json)、
[运行原始 stdout](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/core-r3-all94.stdout.log)、
[运行原始 stderr](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/core-r3-all94.stderr.log)、
[清单 stdout](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/core-r3-inventory.stdout.log)、
[原 65 项比对](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/original-65-tests.json)、
[测试差分保护](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/test-diff-preservation.json)和
[四文件源码差分](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/source-diff-r2.patch)。

冻结为 `work/pty-farewell-20261115/frozen-inputs-r2.json`，SHA-256
`8efdd6ebf0253323d169abd4e87a05bef98011e17c3dfa63beed52dac5eecd1a`。
结束核验时间为 `2026-11-15T10:35:18.224203+00:00`，HEAD 未变，冻结项无 drift；
见 [freeze-initial.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/freeze-initial.json)、
[freeze-final.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/freeze-final.json)。报告只对该次快照成立；父后续修改源码
不属于本报告，须等新 r3 freeze 与稳定二进制另审。

父最新确认完整 `full-gate-r2` 已停止且实际 reap，exit -15，218.112s；
本轮没有完整门禁通过结论。父提供的 peer 范围是 native-current-r2 8/8 exit 0，
Linux linux-peer-r2 7/8，原严格 CRC32 Unicode 缺 Finished 断言仍失败；
interactive 同 shell strict_OO 1 项、reference 及清理通过。这些为父方信息，
本 reviewer 未独立执行，不把它们合并到上述 94 项，也不借源码审查补签互操作。

session/bridge/GPUI 的修复与测试、transport 往返屏障及实际 EOF 顺序不属于本轮
四文件源码签署范围。macOS/Windows/Linux 原生交互、Linux 本地 Rust 文件后端、
Windows 安全发布、IME/辅助技术、安装、签名、发行与完整产品均未验收。
新 P2 关闭后，工程门禁仍应由父绑定同一最终 freeze 的真实成功终态补齐。

即时父消息工具返回 native ancestor 禁止，当前没有可用的原生协作消息工具；
已在 commentary 当即报告并保留
[parent-notification-status.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/parent-notification-status.json)。
父随后明确确认已收到 CORE-R2-01，要求完成 r2 HOLD 报告并等待 r3；本报告据此收口。
本 reviewer 的所有写入均在此 review-core-r2 目录，没有修改源码、测试、原 notice、
历史失败、冻结、二进制、共享 target，没有创建 worktree 或用户 chat。

交付时检测到 r2 输入已变化，已停止对变化源码作任何新结论，等待父提供 r3 冻结。此前 r2 快照的 HOLD、P2 notice 与 94 项实际运行事实保留。变化清单见 [freeze-post-report.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r2/freeze-post-report.json)。
