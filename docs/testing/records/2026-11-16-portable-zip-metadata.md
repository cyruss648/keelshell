# ZIP 权限元数据与 Windows 早期后端门禁

## 范围

工程基线为 `f094de6ad2789d168f5a24bacb316c98b83608ac`，本批编辑前 HEAD 为
文档提交 `2f29744a2615393bcc5bedb81cd2c425e7bc6372`，已推送、三方一致且干净。
三个工程文件是 core archive 编码器、其集成测试和 CI。决策见
[ADR 0149](../../adr/0149-portable-zip-permission-metadata.md)。
没有 manifest、lock、工具链或 Windows 文件系统权限变化。

## 新的真实失败

Quality `43859165104` 已终态 failure，watch 实际 `1/reaped/1600.284s`，
没有取消或重跑。三平台完整原日志已收取并封存。
Windows 文件系统预检 `70/0/0`、archive 后端 `29/0/1 ignored`，
30 名编译库存和执行结果逐项相等；完整 app `1330/0/2 ignored`，
此前四项解包／续传失败在该精确源码全部逐名通过。

随后 core archive target 实际 `39 passed / 1 failed / 0 ignored`，
`reencoding_discards_dangerous_incoming_permissions_and_ownership` 的原安全断言
读回 436（`0664`），要求 384（`0600`）。完整 Windows gate 因此失败，
后续 session 库及 target 尚未运行。
macOS、Ubuntu 的完整 gate 与真实 OpenSSH 均 success；不能移签为当前修订通过。
完整事实见[前批终态](2026-11-16-windows-archive-root-publication.md#精确-ci-终态)。

尚未集成的 Windows Zmodem 候选没有取得新的实际失败证据。
其独立静态评审还发现两处名称与正文契约不符的 P2，保留 HOLD。
后续若采用，必须改为准确名称并更新 CI 库存映射；本批没有提前修改这两例。

## 实现与宿主验证

ZIP 编码显式使用 Unix 元数据 system，继续丢弃来源权限，文件为 `0600`、
目录为 `0700`。新用例检查 reader 和原始 central-directory 的 system、
类型／mode 与完整 roundtrip；既有安全断言没有放宽。
Windows 先执行完整 archive codec target，再执行完整 session library，
均有具体库存名 guard。原完整 gate 及其失败传播规则保持。

719 项工程冻结 SHA-256：
`bf46afc32603f312f3ff9e7edb5e48ea9bb4c3073b109c072e2f6ded44a8f4fd`。
共享 target 归属没有切换，没有不必要的 clean。

- 首次宿主 archive 绿例：`0/reaped/15.113s`，`41/0/0`。
- 受控 DOS 元数据负例：两项精确 Rust 命令均退出 101，各 `0/1/0`；
  新例得到 `Dos != Unix`，原安全例精确复现 `436 != 384`。
  控制器退出 0 表示预期失败和 `finally` 精确恢复均完成，不表示负例通过。
- 恢复后 `scoped-gate-r1`：`0/reaped/51.622s`。fmt、x.y 政策、
  严格 workspace all-target Clippy 通过；重新编译 core 后 archive `41/0/0`、
  宿主 session library `288/0/4 ignored`、app archive `13/0/0`。
- 346 个唯一编译库存名与执行结果逐项相等：342 ok、4 ignored；
  全部 719 项工程输入零漂移。没有把专项重复累加为不同测试。

本批没有重跑完整产品 gate；采用阶段专项与后续精确三平台 CI 接续，
避免每个 Windows 条件实现的小修订都重复宿主完整应用长流程。
当前宿主不执行 Windows 后端，新的 PowerShell／Windows 结果仍待推送后 CI。
新非作者 Noether 限定 APPROVE，P0=0/P1=0/P2=0/P3=0；
35 次只读静态命令的诊断与补救按原报告保留。
其独立核对恢复后编译的稳定副本，41 个唯一库存名与 source-r2 逐项相等，
独立实际执行 `41/0/0 / exit 0 / reaped / 9.175s`。
程序运行前后 SHA-256 均为
`f317ce23f6ff0117b89f6efdeeae9b62a30fcec1e08826e2ed84eeabf12887b3`；
独立编译、Cargo、Windows native、GUI 均为 0，已关闭。
父 342 项结果没有计入这 41 项独立专项，也不将其累加为不同测试。

原静态报告、最终补充件、独立 list／执行日志、冻结、两项负例与宿主门禁原件
由[本批证据](2026-11-16-portable-zip-metadata-evidence/README.md)维护。

本轮官方依赖查询已核实 37 包／113 声明全部通过，未改 manifest/lock；
现场工具链、默认 stable 和旧 UNKNOWN 原件见
[依赖记录](2026-11-16-route-and-dependency-followup.md)。
英文工具栏原生 P2 仍受 Mac 锁屏阻塞；GUI 操作／截图为 0。
完整产品、FILE-05/M4、三平台桌面、IME／辅助技术和发行未完成。

## 实际交付与后续进展

源码与原件已在 `01f6f6bd8272303fe049bc7b52976afefdb44c37` 提交推送；
本地、tracking、远端 SHA 实际一致，读回时工作区干净。
32 成员和 719 输入的精确 Git 绑定验证 `0/reaped/6.950s`、零 mismatch，
没有重放测试。交付原件在[补充回执](2026-11-16-portable-zip-metadata-evidence/delivery/manifest.json)。

新 Quality `43867112056` 绑定该源码。Windows job `133298403995` 的
filesystem／archive codec 两个预检 step 已 success，当前正在执行完整
session library；这证明当前 Windows codec 编译／执行通过，但确切
41 名库存与计数仍待完整原日志。完整 Windows gate 尚未运行。
macOS `133298403996`、Ubuntu `133298403683` 执行完整门禁，新终态待收取。
原封存的 Windows NOT_RUN 是提交前时点，保持不改写。

旧 publication、两个已完成 reviewer 目录及稳定测试副本，逐字节核对精确提交
中的归档／摘要后回收 88 文件、13,465,970 字节；三位自有 agent 均已关闭。
当前 CI 原件、主 target、正常 GUI 包和待用 Zmodem 候选保留。
没有新建或删除用户 chat、其他 worktree、分支、容器。
