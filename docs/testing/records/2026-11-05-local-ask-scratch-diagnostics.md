# 本地 Ask 临时目录失败观测

日期：2026-11-05，Asia/Shanghai。基线
`77aa69c04ad0dd625a5d827552335878de993ee7`，独立分支
`feature/agent-scratch-diagnostics-20261105`。

## 当前范围

为本地 Ask 增加按请求隔离、有界、非阻塞的可选诊断，并使目录测试的
每个 readiness 等待点保留 case/kind 和该请求自己的阶段。
只记录静态操作和错误类别/数值，保留原错误优先级与期限。
本轮不包含另两棵工作副本中冻结的原生分屏和连接树候选。
实现已冻结；初轮专项、诊断失效反向控制及完整门禁已完成，新的非作者
源码及文档/证据组合复核均已批准限定范围。提交
`3d8476716ab80f0cec8b8c731a8cde0bcd7601a3` 已推送 feature，未合入 main，
提交绑定复核通过。Quality `41994734898 / attempt 1` 为 macOS success、
Linux ENOSPC、Windows preparation failure；原提交的 Windows 单 job
重跑已 success，attempt 2 整体仍 failure，Linux/macOS 继承首次结果。
原始 `current-ci/` 读回、重跑终态及后续六文件修订见
[容量记录](2026-11-05-ci-storage-capacity.md)。以下门禁/审查属于诊断提交，
不覆盖新 profile；旧 run `41967541069` 的根因仍 UNKNOWN。

设计见[ADR 0102](../../adr/0102-local-ask-scratch-diagnostics.md)。
本地证据根目录为 `work/agent-scratch-diagnostics-20261105/`。

## 历史失败与来源

Quality `41967541069 / attempt 1` 的 Linux job 失败，macOS/Windows 成功；
Linux 后续 OpenSSH 步骤 skipped。失败等待点返回 `Err(ScratchFailed)`，
历史唯一根因仍为 **UNKNOWN**。只读诊断确认 held-cwd 或 wait 两例均可能，
且 scratch 关闭失败能够覆盖此前请求结果。没有证据证明磁盘、inode、权限、
句柄耗尽或某个目录竞争是历史实际原因。

原始 run/jobs/日志、只读诊断和对应输入摘要复制到本轮 `baseline-ci/`，
导入摘要为 `baseline-ci/import.json`。复制不改变原始证据。

## 已执行

- 官方依赖检查从 `2026-11-05T07:21:53Z` 开始，于 `07:22:09Z` 完成，
  104 条声明/使用通过，
  进程退出 0并回收。报告为 `dependency-freshness.json`。
  当前没有增加或升级 registry 依赖，仍按 `x.y` 声明。
- 原生窗口接口本轮仍返回 Mac locked，未为诊断切片启动 GUI 或容器。
  这不是设备实际锁定的证明，也不影响该非 UI 诊断切片继续进行源码工作。
- 用户再次通知已解锁后，读取清单和重建 CUA 会话后的读取仍返回同一错误；
  本次没有启动新应用或夹具，未取得原生窗口证据。读回见
  `desktop-retry-readback.json`，冻结的分屏包和导航包仍按各自身份接续。

## 专项与反向控制

以下三个初轮执行均已读回退出 0，子进程及父执行会话已回收：

| 执行 | 实际结果与范围 |
| --- | --- |
| `focused-r1` | `keelshell-ai --lib local_agent`：77 passed / 1 ignored / 52 filtered out，含新增诊断单元测试。 |
| `directory-r1` | 原目录进程控制器通过。七份按请求隔离的回执包含双 CLI 的实际 cwd、held-cwd、取消，以及 Claude 启动 gate；均保留关闭前结果和成功清理结果。该 target 为 `harness=false`，不将其重复算入 ordinary Rust 数量。 |
| `observer-r1` | writer 在请求未完成时写入 0 次；输出写入失败不替换原成功或 8000ms Timeout。双 CLI 真实缺失 scratch 父目录得到 ScratchFailed / ScratchCreate / NotFound，无 child、无请求材料泄漏；65 项目录观察最多报告 64 项且不输出文件名。 |

初轮编译出现 `fetch_update` 弃用警告，随后改用环境编译器提供的 `try_update`。
上述三个成功早于该最后修订；下方完整门禁已覆盖修订后的源码。
第三方 `tempfile` 包装可能令 `raw_os_error` 为空；回归允许空值或实际数字，
没有从错误文本推断 errno。

`negative-control-r1` 临时让 `with_diagnostics` 丢弃 producer，
保持原 Ask 实现和同一集成测试。进程退出 101，失败点准确为
“the real failed operation retains its IO category”；此前原 ScratchFailed、
无 child 和无进度阶段断言已通过。该失败说明测试能够发现诊断缺失，
不说明历史 Linux 故障原因。writer 成功/Timeout 保留检查在本反例中仍通过。

控制脚本使用 `finally` 恢复源码；修前/突变/修后源码摘要和不变测试摘要、
原始失败日志及实际退出回执分别在 `negative-control-readback.json`、
`negative-control-r1-inputs.json`、`negative-control-r1.log` 和
`negative-control-r1-result.json`。所有进程已回收；原始失败保留。

## 最终本机门禁

`full-gate-r1` 执行 `python3 -B scripts/check.py`，实际退出 **0**；
子进程和父执行会话 `88224` 均已回收。环境为 macOS 上的 Rust/Cargo
1.99.1，不构成工具链 pin。格式、workspace/all-targets 严格 Clippy、
`x.y` 依赖策略、全仓测试及 readiness-error / observer-I/O / small-stack
额外进程控制器均通过。

| 分类 | 实际结果 |
| --- | --- |
| ordinary Rust | **2,126 passed / 23 ignored**；其中 app 868 passed / 2 ignored，已包含在总数内 |
| 新增诊断单元测试 | macOS 本机 **19 passed**，逐测试行号见读回；Linux 预计编译 18 项，Windows 14 项，尚需精确平台 CI 核对 |
| rustdoc | **14 passed**，含本次新增的一个示例和两个所有权约束 compile-fail 示例 |
| Python | **128 run / 1 skipped**，整体退出 0；不将 run 数写成通过数 |
| 工程输入 | **500 项**门禁前后完全相同；反向控制与最终门禁仅 `progress.rs` 不同，集成测试相同 |

原始日志、进程回执、逐行统计和最终输入为 `full-gate-r1.log`、
`full-gate-r1-result.json`、`full-gate-r1-readback.json`、
`full-gate-r1-inputs.json` 和 `post-gate-inputs.json`。
本轮未新增 UI、未构建原生包、未启动 Podman 容器，没有供应商 CLI 或真实模型调用。
本机成功不解释历史 Linux 失败。

## 独立审查

新非作者 Parfit 的 `independent-review/source-r1.md` 为
**APPROVE_SCOPED_SOURCE，开放 P0/P1/P2/P3 = 0**。
实际审阅 8 个变更 Rust 文件、ADR 设计及必要的原调用上下文，
核对请求所有权、共享预算、静态载荷、原错误与关闭优先级、期限、launcher
字节保持和平台条件编译；未修改源码，未重跑 Cargo 或原生测试。
所审生产/测试文件与最终门禁相同，绑定见 `input-hashes.json`。

复核认可上述真实创建/关闭/复制故障及反向控制的限定证据；
未逐分支注入 flush/元数据故障，也未将 sink 层并发测试写成两个完整 Ask
同时运行的端到端隔离验收。源码批准不包含精确提交绑定、远端 CI、
原生桌面、供应商或发行安装。

同一非作者随后完成 `independent-review/combined-r1.md`：
**APPROVE_SCOPED_COMBINED，开放 P0/P1/P2/P3 = 0**。
复核九份文档的本切片改动与必要上下文，独立重算全部 500 项工程输入和
原日志统计，核对专项、负控制、恢复和终态回执；没有重新运行测试。
当前文档摘要另存 `documentation-hashes.json`，原源码报告及其初读摘要保留。
组合批准仍不包含尚未发生的不可变提交绑定或远端执行。

## 待完成

1. 新修订完整门禁及 Nash 源码/组合复核已通过，文档 P3 已关闭；按
   [容量记录](2026-11-05-ci-storage-capacity.md)接续新提交绑定及精确 CI，
   两者仍 PENDING。原提交 Windows 重跑成功不替代新修订的平台验收。
2. 保留旧 Linux run 的 UNKNOWN；新 run 的 ENOSPC 不倒填历史根因。
   原生包按各自冻结输入及回执独立验收，范围见[交接](../../HANDOFF.md)，
   不覆盖上方当时的 Mac locked 失败，也不向本切片转移结论。
