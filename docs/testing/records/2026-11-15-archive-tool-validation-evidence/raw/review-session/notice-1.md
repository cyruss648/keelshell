# P2 — 正常退出路径遗漏仍在同一进程组的后台后代

状态：OPEN / REQUEST_CHANGES。发现于冻结 r1：
`d68bec67c2cd4313383736733096f5c4ae68ccc6d786793609d31cb34aedc210`，
baseline `6e2b0a7ee3493ace52ba157049ab668ba464c4f0`。

位置：

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation.py:211`：stdout/stderr EOF 后 `child.wait()` 已经回收 leader。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation.py:227`：只有 `child.returncode is None` 才执行组级清理，正常退出时直接跳过。

触发：PATH 中一个合法可执行包装器 fork 普通后代，后代关闭继承的三个标准流但仍留在 helper 新建的同一 session/process group；leader 输出版本并正常退出。后代无需 setsid、无需另建进程组，也无需访问文件或网络。

影响：helper 立即返回 `passed / 0`，仍属其创建组的后代继续运行，不再受一秒命令或六秒共享工作预算管理。重复诊断可以积累后台进程；“不认证恶意可执行文件 / 不能控制逃逸进程组”并不能解释普通、未逃逸组的进程遗漏。现有 `test_exited_leader_with_descendant_holding_pipes_is_timed_out_and_group_killed` 只覆盖后代保留输出管道的情况，覆盖不到本路径。

独立实际证据：

```text
python3 -B work/archive-validation-batch-20261115/review-session/run_evidence.py \
  --name python-descendant-eof-r1 --seconds 8 -- /usr/bin/python3 -I -S -B \
  /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/review-session/probe_descendant_eof_r1.py

helper_state='passed', direct_exit=0, stderr=''
leader=31496, descendant=31497, descendant_group=31496
same_group=True, descendant_still_exists=True
Ran 1 test in 0.039s
FAILED (failures=1)
reviewer_cleanup: descendant absent
```

原件：

- 本目录 `probe_descendant_eof_r1.py`
- 本目录 `python-descendant-eof-r1.log`
- 本目录 `python-descendant-eof-r1.json`（exit 1，外层 8 秒上限未触发，直接子进程已 wait）

fixture 后代自身仅休眠 2.5 秒；reviewer 在 `finally` 中确认仍属自己创建的组后 SIGKILL，并实际观察到 PID 消失。没有读取真实用户文件、运行 Cargo、修改源码或共享 target。

修复方向：正常结束也应完成所有权内的进程组收尾，在 leader 的 PID/PGID 所有权仍受保证时处理组内后代，然后确认/回收直接子进程。不要在已经 reap leader、可能出现 PID/PGID 复用后无条件 killpg。请添加“后代关闭标准流、leader 正常退出”的真实有界回归，并保留本失败。

## r2 关闭记录 — CLOSED

2026-11-15，基于最终 r2 的 607 项工程输入，SHA-256
`523506564206cbeca415ae2bc9a82ed670cdb210705ba49f3173a84250bd4c17`，
本 P2 已关闭。上文为 r1 发现时的原始事实；原红色日志与回执均保留。

修复实现已独立读回：固定 Python supervisor 持有新 session/group 的 leader；
工具退出经独立、最多 32 字节且不传给 tool 的状态 pipe 上报。根 helper
在所有退出路径先 `killpg(SIGKILL)`、再 wait supervisor；reap 后只做
`killpg(0)` 的有界非破坏性确认。状态 reader 丢失及三秒 watchdog 也进入
supervisor 自有组的清理。没有新增 waitid 或 libc ABI 依赖。

原始 `probe_descendant_eof_r1.py` 未作修改，reviewer 独立重跑：
`python-descendant-eof-r2.log/.json`，**1 run / 1 passed / 0 failed，exit 0**。
实际 `passed / 0` 返回时，后代 PID 已不存在。运行前后七项范围摘要全部等于
最终 r2；`freeze-r2-delta-binding.json` 另核对全部 607 项零 mismatch。
新生产回归也已在 reviewer 的 Python 3.9.6 suite 中通过，25/25。

附加独立风险专项修订运行 `python-independent-risks-r2b.log/.json` 为 11/11。
前一次 r2 专项有一项 reviewer fixture 仍向 `getppid()` 发信号，实际目标已变为
supervisor；已把 fixture 明确指向 helper PID，解释和前次失败保存在
`signal-fixture-adjustment-r2.json` 与 `python-independent-risks-r2.log`，未修改生产源码。
