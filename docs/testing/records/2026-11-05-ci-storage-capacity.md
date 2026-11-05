# CI 容量修订与证据边界

日期：2026-11-05，Asia/Shanghai。HEAD 为
`3d8476716ab80f0cec8b8c731a8cde0bcd7601a3`，已推送
`feature/agent-scratch-diagnostics-20261105`，**未合入 main**。
本记录覆盖其后的六文件未提交容量修订，设计见
[ADR 0103](../../adr/0103-ci-storage-capacity.md)。
以下证据路径相对 `work/agent-scratch-diagnostics-20261105/`，
属于本地忽略材料；原始失败保持，不是公开发行产物。

## 已提交诊断与 attempt 1

诊断提交的本机完整门禁退出 0并回收：2,126 ordinary Rust / 23 ignored、
14 rustdoc、Python 128 run / 1 skipped；500 个工程输入一致。
非作者源码、文档/证据及提交绑定复核已通过限定范围，开放 P0/P1/P2/P3=0。
详见[诊断记录](2026-11-05-local-ask-scratch-diagnostics.md)及
`delivery-readback.json`；这些批准不覆盖后续容量修订。

精确 Quality **41994734898 / attempt 1** 为 **completed / failure**。
已读原始 `current-ci/api/run-00046.json`、
`current-ci/api/jobs-page1-00047.json` 和三平台日志：

| 平台 / job | 实际结果 |
| --- | --- |
| macOS / `127135066252` | success；ordinary Rust 2,126 passed / 23 ignored，含 app 868 / 2；14 rustdoc。Scripts Python 128 run / 1 skipped，OpenSSH 步骤通过。 |
| Linux / `127135065960` | failure；已运行部分 ordinary Rust 153 passed / 5 ignored，18 项诊断单元测试通过；目录控制器收到 ENOSPC 后停止，未完成全仓或 OpenSSH 验证。 |
| Windows / `127135066120` | failure；Scripts Python 128 run / 1 failure / 1 skipped，`PreparationTreeTests.test_real_parent_success_cannot_leave_detached_descendant_running` 失败，尚未进入 Cargo。 |

Linux 原日志 `current-ci/logs/linux-127135065960-00021.raw.log`：

- **第 744 行**：16:32:47 报告 runner 剩余 **70 MB**。
- **第 957 行**：16:33:01，`selected-cancel / Codex` 的
  `OwnedExecutableWrite` 返回 `StorageFull / raw_os_error=28`；
  关闭前结果为 `ScratchFailed`，scratch 关闭成功。该行另记自有夹具
  可执行文件 48,787,960 字节，不是新 profile 的测量。
- 第 959–964 行：原 readiness 断言失败，Cargo 退出 101、门禁步骤退出 1。

这确定了**本次写入的 ENOSPC**，没有证明磁盘占用的唯一来源。
旧 Quality **41967541069 / attempt 1** 没有这些操作级事实，其
`ScratchFailed` 根因继续为 **UNKNOWN**，不得倒填。

Windows 原日志 `current-ci/logs/windows-127135066120-00014.raw.log`
第 407–428 行记录 `bound_pids=None`、未创建 ACK、`owned_preparation_failed`
及 controller exit 125。关闭后 record/published 存在、活动进程为零、
管道排空不能证明准备阶段成功；具体失败操作与原因仍 UNKNOWN。
限定分析保留在 `current-ci/windows-preparation-triage/`。
attempt 1 的观察器执行会话 `39239` 已退出 0并回收，55 个观察子进程均已回收，
见 `current-ci/exec-readback.json`；观察成功不表示 CI 成功。

## 六文件修订

| 文件 | 变化 |
| --- | --- |
| `crates/keelshell-ai/tests/local_agent_directory_process.rs` | 串行场景结束后显式 `close`；共享夹具确认唯一 owner 后关闭。 |
| `crates/keelshell-ai/tests/support/local_agent_fixture_lifetime.rs` | 新增 helper，在下一夹具创建时断言前一 root 已不存在。 |
| `Cargo.toml` | dev 的非 workspace 依赖 `debug=0`，workspace `debug=1`；test 继承，保留既有优化与 release 配置。 |
| `.github/actions/setup-build/action.yml` | 缓存键增加 `dependency-debug-0` 分代。 |
| `scripts/check.py` | 各命令前后记录数值容量；失败后也观察，输出错误不能替换原结果。 |
| `scripts/test_check_storage.py` | 三项专项覆盖真实数值、错误正文脱敏及输出失败时原错误保持。 |

既有断言、请求/控制器期限及四线程测试入口不变；不减少生产校验或复制。
下方记录新 profile 本机终态和具体程序字节；远端容量修复仍待新提交 CI 验证。

## 本机终态与输入绑定

先行寿命对照 **exit 101 → exit 0**，两次执行均已回收：
before 日志第 16–17 行失败于前一 root 未释放，after 第 66 行为 20 cases passed。
`ci-capacity/fixture-lifetime-readback.json` 绑定同一 helper
`59fcf5e528512e0e7fa0f1d730b2646ab9178b296ca81afb0de986fc9cc7d069`；
两次仅目录控制器源码不同，均早于 profile 修改。原日志、输入及退出回执
`fixture-lifetime-{before,after}*` 保持。新 freshness 于 17:01:46–17:02:04
执行，34 个 registry package 的 104 条声明/使用通过，exit 0并回收；
见 `ci-capacity/dependency-freshness.json`、`ci-capacity/dependencies-r1-result.json`。

新 profile `ci-capacity/full-gate-r1` 于 **17:09:35–17:25:49** 完成，
`scripts/check.py` PID **69232 exit 0 / reaped**，
结束时间戳 `1793870749.4265962`；外层执行会话 **77447 exit 0 / reaped**
由父 agent 通知确认。原始结果为 `ci-capacity/full-gate-r1-result.json`。
文档作者重新读取全部日志并统计，没有重跑门禁。

| 分类 | 实际结果与计数边界 |
| --- | --- |
| ordinary Rust | **2,126 passed / 0 failed / 23 ignored**；76 条汇总与逐项状态行相符。app **868 / 2 ignored** 是其中子集，不另加。 |
| rustdoc | **14 passed / 0 failed**；5 条汇总，含 2 个 compile-fail 与 1 个 compile-only，均不重复相加。 |
| Scripts Python | **131 run / 130 passed / 1 skipped**，0 failure/error；三项新增容量测试已计入。此前三项专项及父会话 `73042` exit 0 另留历史，不加到总数。 |
| 非 libtest 控制器 | 目录 20 cases；额外 readiness-error 10 cases、observer I/O 与 small-stack 均通过，单列，不加到 ordinary Rust。 |
| 工程检查 | 格式、workspace/all-targets 严格 Clippy、`x.y` 策略及原七条门禁命令通过。 |
| 输入绑定 | **506 / 506 完全一致**，无新增、删除或摘要差异；六个源码/配置文件同时匹配 Nash 冻结摘要。 |

逐汇总行、逐项交叉计数及执行回执新增于 `ci-capacity/full-gate-r1-readback.json`；
门禁前清单 `ci-capacity/full-gate-r1-inputs.json` 与新增
`ci-capacity/post-gate-inputs-r1.json` 逐字节摘要比对。计数只取全仓命令，
不重复累加后续控制器、旧门禁、平台继承结果或 reviewer 专项。

## 本机程序大小与容量时点

日志中的执行文件路径已用于只读检查构建后实际文件；路径不复制到仓库文档。
完整文件名、SHA-256、测量时点及证据行保存在上述新读回。

| `local_agent_directory_process` 调用 | 字节数与证据 |
| --- | --- |
| 全仓调用，程序后缀 `ed07fe7c88a483b8` | **10,078,320 字节**；门禁日志第 1271 行绑定程序，1281–1333 的七条 outcome 均记录该值，构建后文件大小相同。 |
| 后续 observer 专项，程序后缀 `dcde3f90198cff19` | **9,921,664 字节**；第 4633 行绑定另一程序，数值来自构建后实际文件，不冒充该调用的运行期大小回执。 |

两种 Cargo 调用生成不同程序，分别记录；不与旧 Linux 的 48,787,960 字节
直接作 profile 收益比较。14 条 before/after 观测完整保留：首条可用容量
**1,015,457,787,904 字节**，末条 **1,007,332,446,208 字节**；
workspace/temporary 本次数值相同，不相加。它们是宿主机文件系统时点值，
包含共享构建及其它活动，不是 target 大小、空间节省量、峰值或远端容量验收。

## 原提交 Windows 重跑终态

原 SHA `3d847671…` 的 Quality **41994734898 / attempt 2** 为
**completed / failure**。终态原始来源是
`current-ci-windows-r2/api/run-00066.json`、
`current-ci-windows-r2/api/jobs-page1-00067.json` 和平台日志。

| 平台 / attempt 2 列表中的 job | 起止时间（Asia/Shanghai） | 判定 |
| --- | --- | --- |
| Windows / `127145848735` | **16:53:53–17:24:45**，首次为 16:28:33–16:29:39 | 新执行，**success**。 |
| Linux / `127145850589` | **16:28:33–16:33:05**，与首次相同 | 继承 attempt 1 **failure**。 |
| macOS / `127145911550` | **16:28:34–16:47:09**，与首次相同 | 继承 attempt 1 **success**。 |

API 三条均标 `run_attempt=2` 且有新 job ID；Linux/macOS 的
`started_at`、`completed_at`、全部步骤及原始日志字节均与 attempt 1 相同，
据此判定继承，不按新 ID 计作重跑。首次原日志与失败保持。

Windows 新原日志 `current-ci-windows-r2/logs/windows-127145848735-00068.raw.log`
独立重算为 **2,061 ordinary Rust passed / 18 ignored**，含 app
**859 / 2 ignored**；**14 rustdoc passed**（含 2 compile-fail、1 compile-only）。
Scripts Python **128 run / 127 passed / 1 skipped**；
Packaging Python **57 run / 53 passed / 4 skipped**，分开统计。
目录 15 cases 和额外进程控制器单列，OpenSSH 按工作流条件 skipped。
这些结果只属于原诊断提交，不包含新容量修订。

`current-ci-windows-r2/observer-final.json` 记录 71 个观察子进程均已回收，
包括两次非零 GET 失败，原失败材料保留；外层会话 **32235 exit 0 / reaped**
由父 agent 通知确认，不改写观察器无法自证自身退出的原字段。
新增 `ci-capacity/windows-r2-final-readback.json` 保存两轮完整步骤/日志
比较、精确计数与回收补充。重跑成功不解释首次 Windows preparation 失败原因。

## 审查与范围

Nash 的 `ci-capacity/independent-review/source-r1.md` 为
**APPROVE_SCOPED_SOURCE，开放 P0/P1/P2/P3=0**；
六文件摘要与本次门禁及当前文件相同。reviewer 未重跑 Cargo、完整 Python
suite 或原生；其两项只读 Python 测试和内存检查不混入上述门禁数量。
文档预审 `ci-capacity/independent-review/docs-r1-input-summary.json` 的
**P3 CAP-DOC-R1-001** 已由 Nash 在
`ci-capacity/independent-review/combined-r1.md` 中确认关闭；最终组合结论为
**APPROVE_SCOPED_COMBINED，开放 P0/P1/P2/P3=0**。该报告绑定六文件源码、
506 项工程输入及十文档冻结摘要；本次只据报告回写审查状态，未扩大原生或 CI 范围。
**新提交绑定及精确 CI 仍 PENDING**，本记录形成时修订尚未提交推送。
组合复核所用十文档冻结摘要、链接检查及只读核对结果保留在
`ci-capacity/documentation-final-readback-r1.json`；状态回写差异及最终文档摘要
另存 `ci-capacity/review-status-update-r1.json` 和同名 `.diff`。

导航与分屏按各自冻结输入及回执独立验收，不向本容量切片转移原生结论。
旧 `desktop-retry-readback.json` 的失败保持；其它切片的新证据只在
[HANDOFF](../../HANDOFF.md)简述。旧 Linux run 根因、首次 Windows 准备失败、
容量修订远端结果和发布安装边界分别保留。
