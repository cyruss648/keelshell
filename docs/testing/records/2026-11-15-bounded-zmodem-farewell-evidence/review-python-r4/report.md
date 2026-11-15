**APPROVE — 独立只读源码复核，仅限本批两个 Python 脚本。**

开放问题：P0 **0**、P1 **0**、P2 **0**、P3 **0**。未发现需要发出 notice 的问题。
本结论不代表 full-gate-r4 通过，也不代表原生或真实 Linux fixture 验收。

审查树为 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。
基线与审查前后 HEAD 均为 `b66ec7e564e97c3cc8261b447288377ac1ffccc2`；
本次审查工作树相对该 HEAD 的未提交差分，不是审查另一个提交。
已读取 AGENTS.md、其引用的 RTK.md、docs/HANDOFF.md、作者说明、原红/绿回执和原 gate-r3 失败。
实质源码审查范围仅为 `scripts/run_linux_file_interop.py` 与 `scripts/test_linux_file_interop.py`；
只读追踪 `scripts/openssh_interop.py` 以核实异常来源与身份清理合同。

冻结文件为 `work/pty-farewell-20261115/frozen-inputs-r4.json`，
SHA-256 `9133ce6141be9498034641d25fe064b60c0904fb4ced9f8d73f52a76e2e4c58c`。
审查前、测试后分别核对全部 **571 + 12 = 583** 项，均为 **0 mismatch**，
冻结文件本身字节未变。两个范围内文件与作者 `.frozen` 副本一致，
两个基线 Git 对象与作者 `.before` 副本一致。

| 输入 | 审查的 SHA-256 |
| --- | --- |
| scripts/run_linux_file_interop.py | a58176da6d9fb3ae2f902dd5ee7bfcdb4289ae1ee082ab7138b566550312916e |
| scripts/test_linux_file_interop.py | 68a62eb77d67d47af1a4be92bbaa1c3a6fba8be794f4bf92e30971e46a00f57c |
| scripts/openssh_interop.py，仅作只读依赖追踪 | 78a9074e567d7ef128096ffd5dc40c5f458ed9a6db2cb635671d01da47b25a5d |

**逐行审查结果**

- [run_linux_file_interop.py:192](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_file_interop.py:192)：catch 的范围仅覆盖 capture，类型仅为 `ownership.InteropFailure`。与当前 helper 核对后，其 `TimeoutExpired` 直接 cause 来自有界 `ps` 探测；两个完整字符串分别来自枚举前和读取身份期间的 deadline 检查。只有识别到这些预算错误且 `monotonic() >= deadline` 时才继续进入原 timeout 路径。距离绝对 deadline 尚有剩余时仍原样抛出，未把 3 秒 inventory probe 上限混同为较长的命令期限。
- [run_linux_file_interop.py:197](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_file_interop.py:197)：identity 错误没有上述 cause/完整信息，permission 和 `CalledProcessError` 不属于被 catch 的类型。因此即使越过命令期限，非预算错误仍保留；修复没有采用 blanket catch、空 inventory 或重试。
- [run_linux_file_interop.py:205](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_file_interop.py:205)：成功 capture 后立即重查同一个绝对 deadline；等于和超过边界均置 `timed_out=True` 并退出循环，即便期间子进程已经退出 0。deadline 未重新起算，未调整 sleep 或 0.8 秒真实回归期限。单调时钟使 catch 的预算分支随后必然进入到期分支。
- [run_linux_file_interop.py:212](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_file_interop.py:212)：其他异常仍由原外层捕获路径标记 `interrupted=True` 并原样抛出；新增预算到期分支不误记 interruption。
- [run_linux_file_interop.py:215](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_file_interop.py:215)：finally 的 AST 与基线完全相同。它仍把累计的身份字典交给 `stop_owned`，独立给出 `now + 7` 清理预算，保存 step/progress；正常 timeout 若清理未验证，仍抛出 cleanup `FixtureFailure`。发生其他原始异常时，原本的异常与 `owned_processes_stopped=False` 记录策略保持。cleanup 抛出的异常不在新增 catch 内。
- [run_linux_file_interop.py:232](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_file_interop.py:232)：清理完成后仍由原 `check` 合同决定命令失败；原非零退出码和日志保存不变，`check=False` 的既有行为未被重新定义。`register`、`capture`、`finish` 的 AST 及 ownership helper 的完整字节均与基线相同，未改变身份准入或信号发送边界。
- [test_linux_file_interop.py:126](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/test_linux_file_interop.py:126)：7 项新增测试共 17 个受控场景，实际断言状态、异常对象 identity、固定 deadline、finally 执行一次、身份字典和落盘 receipt。精确边界、到期后成功 capture、提前预算错误、到期后非预算错误、cleanup 未验证和期限前成功均有覆盖，没有删除原测试。
- [test_linux_file_interop.py:277](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/test_linux_file_interop.py:277)：原真实 child/grandchild 回归仍为 `timeout=0.8`，只增强了未 interruption、已记录 grandchild 身份以及完整落盘 step 相等的断言；仍要求清理已验证、grandchild absent 或 zombie。

新增受控场景的计数为：probe timeout 的等于/超过边界 2；内部 deadline 信息 2；
成功 capture 的等于/超过边界 2；期限前错误 6；到期后非预算错误 3；
清理未验证 1；期限前成功 1，合计 **17**。

**独立执行**

实际环境为 macOS、Python `3.15.2`。两次测试子进程均设置
`TMPDIR=/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/tmp`
及 `PYTHONDONTWRITEBYTECODE=1`，外层预算各为 45 秒，均已实际 wait/reap，
无外层超时、无 supervisor error、前后源码摘要相同。
临时测试数据使用用户指定的本批 tmp，持久审查产物均在本 review 目录。

| 执行 | 测试结果 | exit | 外层 elapsed |
| --- | --- | --- | --- |
| 固定基线实现 + 最终 CommandDeadlineTests | 7 run；预期红，2 failures / 5 errors | 1，预期 | 0.325184 秒 |
| 最终 test_linux_file_interop.py 全文件 | 17 passed / 0 failed / 0 errors / 0 skipped | 0 | 1.393398 秒 |
| 限定两文件 `git diff --check` | 通过 | 0 | 见 diff-check.json |

绿色命令为
`python3.15 -B -m unittest discover -s scripts -p test_linux_file_interop.py -v`，
执行路径、版本、日志摘要和精确命令见 `independent-file-suite.json`。
基线红测直接从固定 Git 对象在内存中加载旧 fixture，运行最终冻结的 7 个测试，
没有替换工作树源码；driver 为 `run_independent.py`。

共执行 **2 个测试子进程、24 次 named-test 执行**：
最终候选的唯一测试数为 **17**，其中包含新增 7 项和原真实进程回归 1 项，
不能把其受控场景数或旧实现重跑数叠加为新的通过数。
另有 **1 次**限定差分检查。未重跑作者的全 Python 148 项或全仓 gate。

作者原 `red-boundary.log` 的 `7 run / 4 failures / 3 errors` 原样保留；
本次独立红测采用最终调整过 assert/subTest 嵌套的测试，故为
`7 run / 2 failures / 5 errors`，不以此改写原红结果。
原 full-gate-r3 的 `exit 1 / 7.410s` 与原日志摘要核对一致：
其 `ps` 命令仅剩约 `0.003871541` 秒，`TimeoutExpired` 包装为 `InteropFailure`
从 `capture` 逃逸，原真实进程回归因此报错；原始
`141 run / 1 error / 1 skipped` 不被后续绿色结果覆盖。
作者的 `148 run / 147 passed / 1 skipped` 只作为作者证据，不计为本审查独立执行。

**源码批准条件，均已满足**

1. 批准仅绑定上述 r4 冻结及两文件的指定 SHA-256；583 项前后核对零 drift。
2. inventory 预算错误必须已经到绝对 deadline 才进入 timeout；非预算原始异常继续失败。
3. cleanup 身份可信边界、7 秒预算、0.8 秒真实回归以及原失败记录保持。
4. 独立全文件测试 17/17 通过；基线交叉红测能实际检出修复前问题。

**父任务的集成条件与限制**

full-gate-r4 由父任务负责。本报告没有执行、等待或签署它的成功终态；
后续集成/提交须由父任务取得完整门禁成功回执，并再次绑定同一冻结输入。
若任一范围内文件变化，本批准不自动转移到新字节。

本审查没有运行 Cargo、修改 target、修改工程源码/文档/Git、启动 Podman、
创建 worktree 或 chat，也没有清理其他任务资源。
没有执行真实 Linux 容器/SSH/lrzsz/ACL fixture，没有原生桌面验收；
真实进程清理回归仅在当前 macOS 主机执行，不签 Linux 或 Windows 行为。
没有运行 Python 3.8 或其他旧 runtime。对接近真实 `ps` deadline 的确定性复现
来自受控边界测试；没有通过增加负载去强制本机 `ps` 再次抖动。

审查材料入口：`inputs-before.json`、`inputs-after.json`、`reviewed.patch`、
`static-invariants.json`、`independent-baseline-red.{json,log}`、
`independent-file-suite.{json,log}`、`diff-check.{json,log}`、
`preserved-author-evidence.json`、`handoff.json`。
任务结果已完成，可交接父任务收取并关闭本 review agent。
