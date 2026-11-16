# Windows 归档后端预检与固定阶段诊断

## 结论与范围

本批基线为 `6f0eb4c3827026807b38cb53d6f1681a64b3847c`，仅修改 CI 和两个测试文件，
不改变生产传输、权限、审核或隔离行为。目的为提前取得 Windows 私有归档后端的
实际执行结果；此前应用测试先失败，完整门禁没有机会运行后续 session 测试。
本机专项与严格 workspace Clippy 已通过；Windows 交叉检查受缺失 SDK 阻塞，
实际 Windows 执行及四项应用失败的修复仍开放。

## 上一批精确 CI 的终态

Quality `43848390542 / attempt 1` 绑定源码
`8c07d61009dba14f989dd0c00a2a370dc7a2d37b`，最终为 failure。
完整三平台原日志及终态查询已收取，watch 命令 `1/reaped/1301.503s`，
不是取消或超时。

| 平台 | Job | 结论 | 应用测试 |
| --- | --- | --- | --- |
| macOS | `133234328891` | success | `1341 passed / 0 failed / 2 ignored` |
| Ubuntu | `133234329484` | success | `1342 passed / 0 failed / 2 ignored` |
| Windows | `133234329379` | failure | `1326 passed / 4 failed / 2 ignored` |

macOS、Ubuntu 的真实 OpenSSH 联调均成功，分别 32.543s、15.739s。
Windows 文件系统预检 `70/0/0 / 0.80s`；应用执行 605.04s，四项失败为：

1. tar.gz 解包确认后进入 `mutation-uncertain`，保留 isolation 和 close hold，
   未发布目标。不是此次 fixture 启动失败。
2. 显式文件续传取得 `ResumeFile` 审核后以 `Failed / 0 / None` 结束。
3. held READ 隔离测试在进入真实 READ hold 前以相同末态结束。
4. retained-window 测试在出现进度前以相同末态结束。

后三项原日志仅有 `execution-error / other-redacted`，具体原因仍 UNKNOWN。
“前一未知结果导致后续 local quarantine 冲突”仅是待验证假设；本批新增分类
不能回溯证明旧日志的因果。旧失败不改写为修复成功，其他源码的成功不移签。

## 诊断与 CI 行为

- Windows CI 在 platform 预检之后、完整 Rust 门禁之前串行执行 session 私有归档
  模块。先编译 `--list`，强制检查既有 nested-tree 和 empty-tree 两个精确名字，
  防止错误 filter 的零例假绿；然后用相同 filter、单线程实际执行。
  任一阶段失败即 job 失败，完整门禁为 NOT_RUN。
- 正确模块 filter 为 `sftp::archive_transfer::local::tests::`，Windows 通过
  `cfg_attr` 选择 `local_windows.rs`。源码静态库存仍为 29 名，其中 1 个既有
  ignored；本机同 filter 只执行 Unix 后端的 1 名，不能代签 Windows。
- 既有 Windows nested-tree 用例仅在 `build_stage`、`publish_stage` 的错误路径
  增加固定阶段、`SessionError` 变体、I/O kind 和 OS 数值。`inspect_err` 原样
  传播 typed error，不格式化动态 payload、路径、peer 或凭据；其余 fixture、
  lease、成员、DACL、identity、断言和清理字节保持。
- app 的测试诊断将一条已知固定 quarantine 拒绝信息分类为
  `queued-mutation-quarantined`，并扩充既有脱敏用例。没有新增测试名、
  清空 quarantine、重试或降低安全断言。

## 输入与实际验证

719 项工程输入冻结 SHA-256：
`a86f82425764c4fcec2341c2bf47dc4e91be486039d60eb1a60c50256688606a`，
使用排序路径、NUL、SHA 和 LF 聚合。三文件快照、原 diff、命令、回执及评审材料
见[本批持久证据](2026-11-16-windows-archive-preflight-evidence/README.md)。
共享 target 仍属同一 checkout，无切换或 clean。

- `windows-cross-clippy-r1`：实际 `101/reaped/6.879s`。`aws-lc-sys` 的 Windows
  C 依赖编译因 `windows.h` 缺失失败，未完成 session 的 Windows 类型检查，
  Windows 实际执行 0。保留原失败，不以宿主 Clippy 代签。
- `scoped-gate-r1`：实际 `0/reaped/29.208s`。格式、x.y 政策、严格 workspace
  all-target Clippy 成功；app 归档模块 `13/0/0`、宿主 session 发布模块 `1/0/0`。
  两组重新编译库存与 14 个实际执行名字逐项相等，719 项输入零漂移。
  app 链接器报告既有 compact-unwind 大小警告，未造成命令失败。
- 本批没有重新执行整个产品门禁；上一批完整本机和精确三平台结果分别保留，
  本批只修改测试/CI，不将旧完整结果标为本批完整结果。
- `dependencies-live-r1`：实际 `1/reaped/206.783s`，覆盖 113 个声明/使用、
  37 包；22 包核实通过，15 包 registry 请求失败，保持 UNKNOWN。
  没有 manifest、lock 或工具链修改；旧批全部核实的结果保持其原时点。

新非作者 `Leibniz` 批准三文件静态范围，P0=0/P1=0/P2=0/P3=0，
36/36 项静态核验通过，并独立核对父 14 名库存、执行与失败回执。
其 Rust/PowerShell/native 执行均 0，未签 Windows 类型检查或 native 通过，
已关闭。首轮检查器口径偏差原件保留，不作为源码缺陷或改写成首轮全过；
文档维护不在该三文件代码评审签署范围。

## 原生边界与接续

本轮再次调用原生工具 inventory，仍返回 Mac 锁屏且自动解锁失败。
未启动应用、GUI 操作 0、截图 0；未保存无关浏览器资料。
上一批正常包继续保留供手动解锁后的英文工具栏验收，原生 P2 保持开放。

后续以本批推送触发的真实 Windows 预检结果继续定位，不能把诊断交付当作修复。
若 build/publish 之外失败，以原 typed error 和实际失败断言继续调查。
Windows/Linux 桌面、完整输入/辅助技术矩阵、供应商模型和发行安装仍分别验收；
完整产品、FILE-05/M4 未完成。
