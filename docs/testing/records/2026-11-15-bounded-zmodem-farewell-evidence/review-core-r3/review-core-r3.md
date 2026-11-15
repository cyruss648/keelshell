# KeelShell bounded farewell r3 独立严格只读差分复核

结论：**APPROVE，仅限本轮 core 源码与实际专项执行范围**。
该范围开放问题 **P0 0 / P1 0 / P2 0 / P3 0**；
`CORE-R2-01` 在 r3 关闭，`CORE-R1-01` 保持关闭。r2 HOLD、notice、失败日志
及全部原件均保留，没有覆盖。完整工程门禁、其他 reviewer 的范围、真实互操作、
原生平台与完整产品不在此批准内。

HEAD 固定为 `b66ec7e564e97c3cc8261b447288377ac1ffccc2`。
新冻结 `frozen-inputs-r3.json` 的 SHA-256 为
`e83c251520832cbb3a15f0c52c9ae23365729e0c4f192cf0edf2c537c0ba6434`，
包含 571 个工程输入和 12 个脚本。初始、独立执行后和结束核验均匹配全部 583 项。
r2→r3 恰好只有指定四文件的摘要变化：core codec、receiver、core tests、
session tests；`core/src/zmodem.rs` 及所有其他生产输入相对 r2 未变。

本轮是 r2→r3 差分复核，延续已阅读的 AGENTS、RTK、HANDOFF、SHARED_TARGET、
ADR 0137 及 `rust-best-practices` 第 1、2、3、4、5、7、8 章相关原则；
重新核对了当前 AGENTS、HANDOFF 当前批次与共享 target 约束。
reviewer 未写实现或测试，没有运行 Cargo、编译器、format、Clippy、Git mutation，
没有访问共享 target，没有创建 worktree 或用户 chat。写入仅在本 review-core-r3。

`CORE-R2-01` 的修复和验证构成闭环：

- [codec.rs:535](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/codec.rs:535)
  在识别第五个连续 CAN 时，只从 farewell 缓冲裁掉这五个已识别的取消字节，
  保留此前普通尾部。裁剪不根据调用分片长度推断协议所有权。
- [codec.rs:613](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/codec.rs:613)
  在 header 已完整解析且 CRC 验证成功之后，裁掉该候选对应的协议字节。
  Cancel/Abort/FileError 及触发拒绝的有效头不会混入随后交还的普通尾部；
  NeedMore、NotHeader、Corrupt 分支没有套用这一裁剪。
- [receiver.rs:137](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:137)
  对 Done 或 peer Cancelled 终态统一取出仍持有的 farewell 尾部，再按原顺序追加
  当前 feed 中尚未消费的后缀。peer cancellation 不再跳过先前保留的中文尾部。
- [receiver.rs:250](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:250)
  在本地 cancel 清理中取出仍持有的 farewell 尾部，覆盖输入失败或重试耗尽后
  已进入 Failed 的状态。`Option::take()` 转移所有权，已完成/取消终态提前空返回，
  防止重复交还。有效命令拒绝、失败与取消结果仍保持，不产生 Finished 或缺 OO
  警告作为替代。

父 `core-tail-error-red-r1` 的 receipt/log hash 相符，实际终态是
**0 passed / 7 failed / 0 ignored / 95 filtered out，exit 101**。
三个 peer cancel 用例覆盖有效 Cancel/Abort/FileError 头与五 CAN，分别使用整块、
逐字节和每个二段分片边界；三个错误用例覆盖 RemoteCommand、命令 invitation、
不允许的 File/Data/SenderInit 及 EOF，分别使用相同三种输入方式；第七项覆盖
retry 失败。父实际红日志已读取，reviewer 没有重编译或重跑旧失败二进制。

这七项在 r3 独立运行中全部通过，新增第八项输入上限失败回归也通过。错误路径
检查准确交还此前的 `个$ `，oversized feed 在 decoder 接收新数据前失败，不把
被拒绝的大块输入加入输出。取消、重复取消、后续 feed 和严格/缺失结果的排除
断言均存在。八项源码位置从
[zmodem.rs:2065](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/zmodem.rs:2065)
至末尾；逐名、行号和红绿关联见
[closure-and-coverage.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/closure-and-coverage.json)。

独立执行使用父构建的 `stable-tests/core-r4`，SHA-256
`8c51f02d4bf32d05d5757d310fe596c583cf7f986465ca958538098bff2cf75e`。
`core-r4-binary.json` 指向 r3 freeze；父库存摘要为
`5f10332d584c90afc174aadc8747666becd94f11275281a6bfe5600c00f745b6`。
父 `core-r4.log` 包含本主树实际重新编译记录，其 receipt/log hash 及 102 项
成功终态相符。reviewer 核验稳定副本前后摘要一致，没有读取原 target 产物。

| 本 reviewer 执行或静态核验 | 实际结果 |
| --- | --- |
| 稳定副本 `--list`，1 次 | exit 0；102 个唯一名字；与父库存字节相同，与源码测试名集合完全一致 |
| 稳定副本 `--test-threads=4`，1 次 | exit 0；102 passed / 0 failed / 0 ignored / 0 measured / 0 filtered out；测试时间 0.05s |
| r2 原 core 94 项 | 94/94 正文逐字节未改，全部包含在本次通过结果中 |
| HEAD 原 core 65 项 | 65/65 正文逐字节未改，没有删除、缺名或断言弱化 |
| 新 core 用例 | 8/8 存在并实际通过；包含七项先红后绿与一项输入上限回归 |
| r2→r3 core test 差分 | 只在末尾追加 212 行；原 test/helper 和属性没有改动 |
| session tests 差分 | 只插入两个新用例；去掉这两个块后 SHA-256 精确还原 r2；本 reviewer 未执行 session 测试 |
| 冻结 | 初始及结束均为 571+12，全部匹配，未冻结新增与遗漏均为 0 |
| reviewer 重编译、Cargo、format、Clippy、共享 target、Git mutation | 全部 0 |
| reviewer 真实 SSH/peer、原生桌面、完整工程门禁执行 | 全部 0 |

原 94 项的证明使用已保存的 r2 四文件 diff 和 HEAD blob 在内存中还原 r2，
逐文件校验还原后的 SHA-256 与 r2 freeze 完全相同，再比对 94 项测试正文、
属性和 helper 差分。没有将当前新源码误当 r2，没有通过切换 checkout 或写共享
target 获取旧版本。见
[source-reconstruction.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/source-reconstruction.json)、
[original-94-tests-preserved.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/original-94-tests-preserved.json)、
[core 差分](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/source-diff-r2-r3.patch)。

`CORE-R1-01` 的四字节 UTF-8 识别逻辑本轮未变。其六项中文尾部测试、三项
parity pad 测试、两项本地取消测试都包含在本次 102 项通过结果中。bounded tail
仍受 65,632 字节长度、96 字节 header candidate、65,536 字节单次 feed、
64 次累计 retry 及原 wire/packet/event 限制约束。严格 OO、单独缺失结果、
未审核 offer 与未确认保存的状态限制仍由原有未弱化测试覆盖。

本次 102 是唯一独立 Rust 测试计数。父运行、内部循环组合、上述覆盖分组及
静态检查不另行加到通过数；父提到的全批 55 项默认新增不是本 reviewer 独立
执行的另一套 55 项。session 新增两项仅作差分和断言审阅，不签署 session、
barrier、bridge 或 app 的执行结果。

现有 session/UI 审查的
[P2-003 notice](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-session-ui-r2/notice-P2-003.md)
将 post-pump cleanup 的真实输出交还问题标为 OPEN。它在本次 core 生产写集外；
本报告的 core 零问题计数不表示该项已关闭，也不代替该 reviewer 的桥接复核。
后续若任意冻结输入变化，应停止沿用当前全输入绑定，等新冻结再核对。

最终完整工程门禁没有由本 reviewer 执行，也没有在此报告中补签。父应以相同最终
freeze 的真实成功终态补齐 fmt、严格 Clippy、workspace 测试、依赖策略及其他
工程条件；若修复其他范围导致冻结变化，应重新绑定并复核差分。此前 gate-r2
停止/reap 的 exit -15 与 r2 HOLD 保持原事实。

真实 peer 的 native 8/8、Linux 7/8 等既有父方结果没有由本 reviewer 重跑。
严格 CRC32 Unicode 缺 Finished 的原 Linux 失败与全量互操作边界继续开放；
不能从 core 102/102 推导它们已通过。macOS/Windows/Linux 原生交互、Linux
本地 Rust 文件后端、Windows 安全发布、IME/辅助技术、安装签名发行和完整产品
均不在批准范围。本报告没有独立重编译，属于核对冻结和库存后独立执行父稳定
二进制的限定审查。

原始执行及冻结证据：
[core-r4-independent-execution.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/core-r4-independent-execution.json)、
[测试 stdout](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/core-r4-all102.stdout.log)、
[测试 stderr](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/core-r4-all102.stderr.log)、
[清单](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/core-r4-inventory.stdout.log)、
[初始冻结检查](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/freeze-initial.json)、
[结束冻结检查](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/freeze-final.json)。

交付前再次核验于 `2026-11-15T10:46:44.164718+00:00`，583/583 匹配，HEAD 和冻结文件摘要均未变；见 [freeze-post-report.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/freeze-post-report.json)。
