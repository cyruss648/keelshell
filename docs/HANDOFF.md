# KeelShell 当前交接 — 2026-11-17

完整产品、FILE-05/M4、三平台原生桌面与发行仍进行中。
本页维护当前入口；先读[路线](ROADMAP.md)，再查对应精确记录。
改写前本页 23,090 字节与路线 23,205 字节逐字节保存在
[交接历史](HANDOFF_HISTORY_2026-11-17.md)及[路线历史](ROADMAP_HISTORY_2026-11-17.md)，
SHA-256 分别为 `3b6e75e988ef096fd7ec4941241f9a03299c0f1e3933a3100ee5ac7af8496d74`、
`33f696a154f65b99556037809aab5a291aba2bd45ec9e10b44ed635d86d64e40`。
更早历史从对应归档及 Git 查阅，不再整段追加历史成绩。

## 当前实现与验证

Windows 远端只读监控已整合：独立平台身份、固定 PowerShell 5.1 JSON、严格解析、
按项缓存、CPU gauge／未知指标、工作集、端口、三主题双语与带年龄的 AI 缓存。
基线 `4209e272260aff6374e6216423ce8e0157bea2f4`，工程随本批记录提交，精确远端交付另在本批记录补记。
设计见 [ADR0156](adr/0156-windows-read-only-monitor.md)，使用见
[Windows 监控](product/WINDOWS_MONITORING.md)，实际结果见
[本批记录](testing/records/2026-11-17-windows-read-only-monitor.md)。

两处独立发现已修正：SSH channel-open 后发送前再次核对 generation；
Windows 资源对照独立枚举接口／固定卷并拒绝稳定资源漏报，变化不确定则 HOLD。
源审 r3 限定 APPROVE、P0–P3 全 0；功能／工装非作者最终也限定 APPROVE、P0–P3 全 0。
真实 Darwin SSH 15 项、Linux 恢复 2 项已通过并清理自有运行资源。
首次完整门禁 r1/r2 在新版 Clippy 的测试 helper 告警停止，原失败保留；
三处 helper 等价改用 as_chunks 后，最终 r3 冻结 758 输入，aggregate
`6759f672ae1efb83f6a400145109ee9bdc75ed192e2b7647813a252a1eded210`，
最终完整门禁 `0/reaped/813.820404s`：普通 Rust 3407/0/59 ignored、14 rustdoc，
app1398/0/2、session404/0/4、Windows routing9/0/1；Python405 run/2 skipped，
包装与内嵌 helper 单列。93 普通套件及 rustdoc 声明／逐名／摘要相等，
app1400、session408、routing10 的精确二进制独立库存也逐名相等，758 输入前后不变。
正常 release build 和 arm64／minos15.0 包结构已通过，未启动／安装。

Windows API、真实 cmd／PowerShell 终态、capture replay、真实 Windows OpenSSH、
原生桌面和发行均单独验收。新 Windows workflow push 后自动运行 direct API／replay，
isolated-ssh 仅手动前置检查，明确 HOLD。本次最新原生检查仍为 Mac 锁屏且自动解锁失败，
GUI 操作／截图 0。不要将受控 SSH、头部测试或包结构扩大为这些验收。

## 精确已交付基线

监控栈修复工程 `bbd4eaccfa8bba2feb487ff43e0423567428bc82` 已推送；
Quality `43955379234` 三平台 completed/success，37 成员终态封存与 731 Git 输入绑定通过。
Windows／macOS／Ubuntu 普通 Rust 分别为 3312／3325／3318 passed，failed 均 0，
ignored 为 56／58／59；app 为 1364／1375／1376 passed、各 2 ignored；各 14 rustdoc。
Python 主 discovery 各 293 run，skipped 为 34／2／2；包装与 helper 独立计数。
[精确 CI](testing/records/2026-11-17-monitor-render-stack-evidence/ci-completion/README.md)
只签该工程；原 UDP `43945988599` 栈溢出失败及更早各批边界保持。
已完成编辑器、UDP、隧道、文件发布等阶段从[本日历史](HANDOFF_HISTORY_2026-11-17.md)
和各自测试记录读取，不重放未变宿主测试或旧 CI。

## 不变合同与交付方式

- 真实原生 GPUI Kit，只管理远端 SSH；app／core／session／ai 分层，阻塞 I/O 离开 UI 线程。
- Frost「霜白」、Graphite「曜石」、Lavender「雾紫」和 System／Light／Dark 全保留；
  新主题扩展 registry、Palette、形状和 chrome token，业务代码不按主题 ID 分支。
  设计遵循[原生合同](design/HALLMARK_NATIVE.md)。
- AI 上下文发送与命令／文件动作分别显式审核，不能自主执行；host key 变化 fail closed。
  凭据默认短暂驻留，显式保存只用 OS store 或认证加密 vault，不写明文或主密码。
- 保留 UNKNOWN、quarantine、pending mutation 与 close hold，不自动重放未知写入。
- 直接 registry 依赖只用 x.y，Cargo.lock 保留 patch；不 pin toolchain、不设 rust-version。
  本机已观察 Rust/Cargo1.100.0；zip 锁更新9.6.1、声明9.6。三轮官方复查未全部成功，
  最新完整新鲜度仍 UNKNOWN，不拼接部分成功或用旧回执代签。
- 用户已授权及时普通提交／推送和 tag Releases、本地 Podman 联调；不含覆盖安装或遥测。
  当前分支 `feature/editor-unicode-codec-20261114`；不强推，读回三方 SHA。
  `origin/main` 独立状态文档不直接覆盖。
- 按批次并行实现、专项、新非作者复核，关闭发现后冻结，一次最终完整门禁；
  源码、CI、真实联调、原生、发行分开。及时关闭自有 agent、核对原件后回收自有副本。

## 当前资源与下一步

1. Windows 本批最终完整门禁、两位非作者复核和精确库存均通过；
   封存原失败／原件后普通提交推送，收取精确 Quality 与 Windows API workflow 终态。
2. 按[路线](ROADMAP.md)继续真实 Windows SSH、独立 Zmodem peer、外部 AI／MCP、
   隧道与同步、原生矩阵及六目标发行；完整产品目标不标 complete。
3. 桌面可用后先复核英文工具栏 P2，再补三主题／尺寸／双语／焦点／IME／辅助技术。

唯一 Cargo target：`work/recovery-batch-integration-20261115/target`，
`CARGO_BUILD_JOBS=2`、`MACOSX_DEPLOYMENT_TARGET=15.0`。
归属为当前 checkout，连续使用不 clean；切换源码归属先依
[共享 target 合同](testing/SHARED_TARGET.md)失效本项目产物并核对编译库存。
本批活动目录 `work/windows-monitor-integration-20261117/`，正常包 `package-r1/KeelShell.app`；
六份 reviewer／interop 专用副本已按原 SHA 回收，886,506,992 字节；
本轮所有作者／reviewer 均已实际关闭。target 和其他任务资源保留。
旧包与旧路径只描述历史，不把已回收路径当当前入口。
