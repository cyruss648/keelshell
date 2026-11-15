**APPROVE — 最终绑定 r5，仅批准本批两个 Python 文件的源码差分。**

开放问题：P0 **0**、P1 **0**、P2 **0**、P3 **0**。
审查基线及本次读回 HEAD 均为 `b66ec7e564e97c3cc8261b447288377ac1ffccc2`。
范围仍为 `scripts/run_linux_file_interop.py` 与 `scripts/test_linux_file_interop.py`。

最终冻结为 `work/pty-farewell-20261115/frozen-inputs-r5.json`，
SHA-256 **`d8d27989768ae1827697458ee52d366d407ca8543a9eebd41baf6d8f283a2b2e`**，
**571 + 12 = 583** 项逐一与当前文件核对，**0 mismatch**。
完整绑定见 [final-r5.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-python-r4/final-r5.json)。

r4→r5 没有路径新增或删除，仅
`crates/keelshell-app/src/ssh_bridge_tests.rs` 的摘要变化：
`8825c13cea74f348259c57b19179e59d0366147da751c12bc84ea453679e4655`
→ `47771ef2e6d6e14a7afe3698281ee5e9334e4a230578e6dce003e6437d178c01`。
它属于本审查范围外的 Rust 测试文件；本报告只核对该差异的身份，不批准其断言内容。
其余 **570** 项工程输入及全部 **12** 个脚本摘要逐字节相同，生产输入未变。

因此按用户指示复用 r4 的独立测试证据，本次 r5 绑定新增测试执行为 **0**：

| 独立执行证据 | 结果 | exit | 外层 elapsed |
| --- | --- | --- | --- |
| 最终 Python 全文件 | **17 passed / 0 failed / 0 errors / 0 skipped** | 0 | 1.393398 秒 |
| 固定基线实现交叉运行最终新增测试 | 7 run，预期红，2 failures / 5 errors | 1，预期 | 0.325184 秒 |
| 两文件限定 `git diff --check` | 通过 | 0 | 见原回执 |

总执行计数保持 **2 个测试子进程、24 次 named-test 执行、最终候选唯一 17 项**。
新增 7 项/17 个受控场景与原真实 child/grandchild 用例已包含于 17 项内，不重复相加。
两个测试子进程各有 45 秒外层预算，均已 wait/reap，无外层超时；
测试使用本批 tmp 和 `PYTHONDONTWRITEBYTECODE=1`，已观察到的自有进程均无存活身份。

原 [r4 完整报告](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-python-r4/report.md)
以及原交接、日志、冻结和检查记录全部保留，没有覆盖。
原报告 SHA-256 仍为
`9993b3d35b996f6ce6b9b0ffb4adafdccf95228a4123e8df55c54c154c5f08c5`；
本次对原清单内 **16** 件产物复核为 **0 mismatch**。
作者原红/绿、gate-r3 原始失败和本次独立预期红的计数保持各自含义。

**源码批准条件，已满足**

1. 批准仅适用于 r5 中两个 Python 文件的既有审查摘要；当前文件、作者 frozen 副本和独立绿色执行摘要一致。
2. 原报告关于预算异常识别、绝对 deadline 复查、原始非预算异常保持和 finally 清理边界的结论沿用；0.8 秒真实回归期限及独立 7 秒清理预算未变。
3. r5 的 583 项当前字节已匹配冻结，r4 证据未被覆盖；范围内字节若后续变化，需重新判断批准的适用性。

父任务仍须取得完整工程门禁成功终态，并将其回执绑定最终 r5。
本报告不签署 full-gate-r4 的终态，不批准范围外 Rust 测试变化，
不补签原生桌面、真实 Linux fixture、Linux/Windows 进程清理运行或旧 Python runtime。
未重跑全 Python 148 项或全仓 gate，未运行 Cargo/Podman，
未修改源码、target、Git 或创建 worktree/chat。

最终交接入口为本文件与 `final-r5.json`；父任务收取后可关闭本 review agent。
