# r2 非作者严格只读复核：服务端工具版本与内存流式自测后端

**结论：SOURCE / 必要专项 APPROVE。没有剩余阻塞项。**
**合入建议：本范围可合入；父须在同一冻结输入上核验最终完整 gate 成功。**
截至本报告，父告知 `full-gate-r2` 已启动，终态 **PENDING**，本报告不代签其成功。
本 reviewer 使用父提供的预构建 Rust 稳定副本，**无独立重编译**。

| 严重度 | 当前开放数量 | 本次已关闭 |
| --- | ---: | ---: |
| P0 | 0 | 0 |
| P1 | 0 | 0 |
| P2 | 0 | 2 |
| P3 | 0 | 0 |

工作树：
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。
baseline 与复核时 HEAD 均为
`6e2b0a7ee3493ace52ba157049ab668ba464c4f0`。
最终候选为 `work/archive-validation-batch-20261115/frozen-inputs-r2.json`，
**607 项**工程输入，独立逐项重算零 mismatch，工程 SHA-256：

```text
523506564206cbeca415ae2bc9a82ed670cdb210705ba49f3173a84250bd4c17
```

摘要由实际文件 SHA 集合按
`json.dumps(files, sort_keys=True, separators=(",", ":"))` 重算一致。
清单文件自身 SHA-256 为
`12e7e0bf2032c3cb9534ccd685cd4013521d982c2bf5b5494579fc9574ce8280`。
绑定见 `freeze-r2-delta-binding.json`，最终读回见 `review-closure-r2.json`。

复核严格限于 session 的 `src/archive_validation.rs/.py`、
`src/archive_validation/tests.rs`、`src/archive_validation/test_archive_validation.py`、
`tests/archive_validation.rs`、`tests/openssh_interop.rs` 新增测试和 `src/lib.rs` 新声明。
已读 AGENTS、RTK、HANDOFF 当前首段、STRATEGY、SHARED_TARGET、
rust-best-practices；ADR 与产品指南用于核对契约。
未审签原生 UI 或其他功能。r1→r2 的 607 项清单只有以下四个输入改变：

| 文件 | r2 变化及结论 |
| --- | --- |
| `src/archive_validation.py` | 新增固定 supervisor、独立状态 pipe、所有路径先清组再 reap、reap 后有界非破坏性观察。源码复核及原红探针转绿，通过。 |
| `src/archive_validation/test_archive_validation.py` | 两类 POSIX 门限、新增关闭标准流的同组后代真实回归，调整组信号断言以区分 SIGKILL 与 signal 0；最终 25/25 通过。 |
| `src/archive_validation/tests.rs` | 本地 Python 桥接加 `cfg(unix)`，其余协议测试保持跨平台，通过。 |
| `src/archive_validation.rs` | 更新 supervisor 所有权的公开文档；命令构造、解析和 SSH API 行为代码未改。 |

两个 r1 P2 均已关闭，原始 notice、红色探针及失败日志/回执保留：

1. **同组后台后代遗漏 — CLOSED。** 当前
   `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation.py:199`
   创建 supervisor 作为新 session/group leader；工具实际退出状态经不传给 tool 的、
   最多 32 字节的单独 pipe 上报。第 310 行所有退出路径调用组清理，第 148 行
   `killpg(SIGKILL)` 先于第 154 行 wait；reap 后第 162 行仅 `killpg(0)`，
   不再发送破坏性信号。第 65–92 行的固定 supervisor 在启动工具前安装三秒
   watchdog，状态 reader 丢失也清理自有组。原 `probe_descendant_eof_r1.py`
   未修改，reviewer 独立重跑 **1/1 passed / exit 0**，实际返回时后代 PID
   已不存在。`python-descendant-eof-r2.log/.json` 绑定最终 r2，r1 原红保持。
   新生产回归还实际断言破坏性清组发生时 supervisor 尚未 reap。

2. **Windows 默认 gate 误跑本地 POSIX suite — CLOSED。** 当前
   `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation/tests.rs:323`
   有 `#[cfg(unix)]`；Python 第 35、193 行有类级 `skipUnless(posix)`。
   其余 Rust 协议测试、加密 SSH fixture 和公共 API 未受平台门限影响。
   关闭依据是最终源码的门限及 Unix 库存/执行；**Windows 原生执行为 0**。

功能与边界结论：

- 通过当前已认证 SSH 执行完整可审阅、固定引号封装的 stdlib helper；七项版本参数
  与三项内存样本确已实现。工具绝对路径进入 argv，工具分派不经 shell 拼接；
  子进程环境采用 allowlist，排除相对 PATH、工具选项与加载器/Python 环境钩子。
- tar 只接收固定唯一普通成员的内存 TAR，gzip 只接收固定压缩字节，
  Python 使用 BytesIO 做 ZIP/tar.gz 内容往返；没有用户归档参数、安装或网络行为。
  版本只是安全显示的观察值。版本与样本状态独立，zip/unzip/rz/sz 功能强制
  NotTested，不推导工具真实性、后续权限或 Zmodem 互通。
- 真实非阻塞 stdin/stdout/stderr 并发、单次/共享 combined 输出预算、
  deadline、提前关闭输入、非零退出、取消/信号和直接子进程回收均有实际专项。
  supervisor 修复补齐正常成功时同组后代的清理；清理不能确认时拒绝采用报告。
- 严格解析核对协议、七项数量/顺序、必填 nullable 字段、未知/重复字段、
  状态与路径/版本一致性、UTF-8 字节上限、控制/bidi/隐藏字符；
  截断、额外文档、非法 UTF-8、helper stderr、无成功退出均拒绝。
  最终预构建单元实际执行了这些用例。
- 加密 SSH fixture 实际覆盖选定连接、全量命令逐字节发送、独立新 channel、
  pre-cancel、晚 open、drop、exec ack/close deadline、晚有效回复拒收、
  malformed/无退出/exec 拒绝与 combined flood，并核验可保留的连接仍可用。
  这些受控 peer 不执行真实 shell，不冒充真实 OpenSSH。

预构建来源与库存已独立核验：

| 稳定副本 | 二进制 SHA-256 | 完整库存 / 本次实际执行 |
| --- | --- | --- |
| `stable-binaries/session-unit-r2` | `3db521334709761a4da7acce561573c45e14ab5767fdc4121544031d9ee9a075` | 205 / 选定 11 |
| `stable-binaries/session-ssh-r2` | `ea8d14c29eae9e112fe370ca7ebe4aa50c9bda3a2b0ffa6e3c1a579e6d50c892` | 11 / 11 |

独立执行两份 `--list`，结果分别与父库存原文字节完全相同，摘要分别为
`4b4c32f63db2f0de65477bc45781c2fb6d52f0ba9324b6f8caf41d1ff28944e0`、
`3a8187d3657ffeeecbf26a6a9de725dc1f1e55500261642e6922156bad437e31`。
当前范围全部具体用例名存在，两个二进制都含最终 r2 helper 的完整原文字节。
核验 `session-binary-binding-r2.json`、冻结与稳定副本一致；详见
`binary-inventory-verified-r2.json`。这增加了来源证据，**不等于 reviewer 独立编译**。
没有从共享 target 自行挑选、改写或重建产物。

| reviewer 最终专项 | 实际计数 | exit / 外层耗时与上限 | 原件 |
| --- | --- | --- | --- |
| Python 最终 suite，系统 3.9.6 | 25 run / 25 passed / 0 failed / 0 skipped | 0 / 2.519 秒 / 45 秒 | `python-author-suite-r2.log/.json` |
| 独立 Python 边界风险 | 11 run / 11 passed / 0 failed / 0 skipped | 0 / 1.148 秒 / 15 秒 | `python-independent-risks-r2b.log/.json` |
| 原 notice 探针未修改重跑 | 1 run / 1 passed / 0 failed / 0 skipped | 0 / 0.173 秒 / 8 秒 | `python-descendant-eof-r2.log/.json` |
| 预构建 Rust lib 选集 | 11 passed / 0 failed / 0 ignored / 194 filtered | 0 / 3.154 秒 / 60 秒 | `rust-unit-risk-r2.log/.json` |
| 预构建 Rust SSH 专项 | 11 passed / 0 failed / 0 ignored / 0 filtered | 0 / 10.073 秒 / 45 秒 | `rust-ssh-risk-r2.log/.json` |

Rust lib 桥接在上述执行中又运行同一 Python suite 25/25；它是重复执行，
不增加独立覆盖计数。205 是完整编译库存，不能写成 reviewer 跑了 205 项。
各次运行前后七项范围源码摘要均与最终 r2 一致，直接启动的命令均已 wait/reap。
无外层超时。运行环境固定 `PATH=/usr/bin:/bin`、独立 fixture HOME/cwd、
禁用字节码，实际 Python 为系统 3.9.6，没有 `os.waitid`。

保留的失败也明确区分：r1 原 notice 探针 **1 failed** 是已修复的生产缺陷。
第一次 r2 独立风险 suite 是 **10 passed / 1 failed**，原因是 reviewer 的
信号 fixture 仍向 tool 的 `getppid()` 发信号，而该 PID 已成为 supervisor。
只把 reviewer fixture 改为明确的 helper PID 后同 11 项全绿；详见
`signal-fixture-adjustment-r2.json`，原脚本和失败日志未覆盖，生产源码未因此改动。

主要命令均由本目录 `run_evidence.py` 加外层 deadline、隔离环境及原始日志回执：

```sh
/usr/bin/python3 -I -S -B /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation/test_archive_validation.py
/usr/bin/python3 -I -S -B /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/review-session/independent_python_risks_r2.py
/usr/bin/python3 -I -S -B /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/review-session/probe_descendant_eof_r1.py
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/stable-binaries/session-unit-r2 --list
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/stable-binaries/session-ssh-r2 --list
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/stable-binaries/session-unit-r2 archive_validation::tests:: --test-threads=1 --nocapture
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-validation-batch-20261115/stable-binaries/session-ssh-r2 --test-threads=4 --nocapture
```

各 JSON 回执保存完整外层 argv、环境、deadline、实际耗时、exit、wait 状态、
日志摘要与运行前后源码摘要。源码/库存核验是读操作，不计入行为测试项数。

未验边界与合入条件：

- reviewer **没有运行 Cargo、独立重编译、全仓 gate、真实 OpenSSH 或 Linux 容器**。
  父告知 r2 真实 OpenSSH 16/16、GUI/MCP 正常构建及 Linux helper/Python 通过；
  这些不计为本 reviewer 独立执行，也不替代父最终完整 gate 的终态。
- **最终 full-gate-r2 PENDING**。本范围 SOURCE/专项 APPROVE 可供父继续门禁；
  父应在相同 607 输入上取得成功终态并核验零漂移后完成合入/本地提交。
  无须等待 gate 终态才采用本报告的范围内结论。
- 原生 macOS/Windows/Linux 桌面、IME/辅助技术与安装发行仍未代签。
  Windows 平台门限由源码复核关闭，Windows 原生运行仍为 0。
- 恶意 executable 的沙箱/真实性、逃逸进程组、远端取消后实际停止、
  任意归档兼容性、用户数据/性能和 Zmodem peer 互通不由固定样本认证。
  OS 进程创建/文件系统元数据调用也不具硬实时保证。
- 所有 reviewer 写入均在授权 `review-session/`；无源码修改、格式化、提交、
  push、工作树清理或共享 target 更改。r1 gate 的 `exit -15 / 499.942s / reaped`
  为已读父回执，不改签为成功；原 notice、红色记录与初始报告保留。
