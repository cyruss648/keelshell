# KeelShell 当前交接 — 2026-11-17

完整产品、FILE-05/M4、三平台原生桌面与发行仍进行中。
本页维护当前入口；先读[路线](ROADMAP.md)，再查对应精确记录。
改写前本页 23,090 字节与路线 23,205 字节逐字节保存在
[交接历史](HANDOFF_HISTORY_2026-11-17.md)及[路线历史](ROADMAP_HISTORY_2026-11-17.md)，
SHA-256 分别为 `3b6e75e988ef096fd7ec4941241f9a03299c0f1e3933a3100ee5ac7af8496d74`、
`33f696a154f65b99556037809aab5a291aba2bd45ec9e10b44ed635d86d64e40`。
更早历史从对应归档及 Git 查阅，不再整段追加历史成绩。

## 当前实现与验证

Windows真实OpenSSH监控验收的本机工程已完成，基线文档提交`bfbea7b`：
隔离短命sshd、原进程调试接纳／终态和公开Rust路由用例分工实现，
见[ADR0157](adr/0157-windows-openssh-monitor-acceptance.md)与
[开发记录](testing/records/2026-11-17-windows-openssh-monitor.md)。
父级离线专项r2共39项通过，Rust目标已编译、逐名库存与严格Clippy通过；
本机负向控制实际以`WindowsRequired`失败退出101，不能误签Windows成功。
新非作者分别复核SSH／Rust与调试生命周期，均限定批准、P0–P3全0；
两位作者与两位reviewer已实际关闭。集成Python专项271 run／0失败／3跳过。
最终完整门禁r1已exit0/reaped/699.028665s，普通Rust3407/0/60 ignored、
14 rustdoc、Python493 run／3 skipped；app1398/0/2、session404/0/4。
app1400／session408／routing10／live1库存逐名相等，冻结765输入、aggregate
`71872297a9a7b067fd85723de4e5c9c10f908dcc6a7e63ab91eeed6cb902db3a`；
门禁前后与库存后无漂移。待普通提交推送及本批精确Windows执行；
本段不覆盖下述上一批成功范围。

Windows 远端只读监控已整合：独立平台身份、固定 PowerShell 5.1 JSON、严格解析、
按项缓存、CPU gauge／未知指标、工作集、端口、三主题双语与带年龄的 AI 缓存。
基线 `4209e272260aff6374e6216423ce8e0157bea2f4`；工程
`5d2df2fc5ac8566fe11182b7f4337076296dfa54` 已普通提交／推送，
本地／tracking／远端三方一致、读回干净。540 成员／758 输入的精确 Git 绑定通过，
`0/reaped/2.856026s`，不是仅以 HEAD 或编译缓存代签。
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

精确 Windows workflow `43972365007` 已 completed/success：
Server2025 AMD64／PowerShell5.1.26100.33648 的生产命令、独立 Win32 对照及认证路由 replay
步骤通过。原 artifact 14279316541 的 46,825 字节 ZIP 与 GitHub digest 相等；
34 原件复核五个 cmd→PowerShell 的原对象自然终态／exit0、fixture 终态、
4 自有双栈端点／2 排除连接、3 稳定接口和2固定卷，全部命令与当前源码逐字节相等。
Quality `43972364934` 三平台已全部 completed/success：
Windows／macOS／Ubuntu 普通 Rust 分别3394／3407／3400 passed，failed均0，
ignored为57／59／60；各14 rustdoc，Python主405 run，skipped分别37／2／2。
91成员精确CI封存与758 Git输入绑定通过，`0/reaped/2.423535s`；
逐项结果和边界见[精确CI终态](testing/records/2026-11-17-windows-read-only-monitor-evidence/ci-completion/README.md)。
Windows lab实际111 run＝108通过＋3跳过。新增非作者Hypatia仅复核API原件／replay，
102项离线复算通过，限定APPROVE、P0–P3全0；独立Windows执行／测试重跑均0，已实际关闭。
真实 Windows OpenSSH、原生桌面和发行继续单独验收；isolated-ssh 此次 skipped，
仅手动前置检查，明确 HOLD。最新原生检查仍为 Mac 锁屏且自动解锁失败，
GUI 操作／截图 0。API／replay 和包结构不扩大成这些验收。

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

1. Windows 本批已推送，本机完整门禁、两位非作者源码／功能复核、精确库存和Git绑定通过；
   API workflow与Quality43972364934三平台成功、原job／API原件封存通过，
   新非作者API回执复算批准且已关闭。维护从精确记录接续，不重复未变工程全门禁。
   下一批真实OpenSSH开发先从上面的ADR0157和开发记录继续，不能跳过最终新冻结与复核。
2. 按[路线](ROADMAP.md)继续真实 Windows SSH、独立 Zmodem peer、外部 AI／MCP、
   隧道与同步、原生矩阵及六目标发行；完整产品目标不标 complete。
3. 桌面可用后先复核英文工具栏 P2，再补三主题／尺寸／双语／焦点／IME／辅助技术。

唯一 Cargo target：`work/recovery-batch-integration-20261115/target`，
`CARGO_BUILD_JOBS=2`、`MACOSX_DEPLOYMENT_TARGET=15.0`。
归属为当前 checkout，连续使用不 clean；切换源码归属先依
[共享 target 合同](testing/SHARED_TARGET.md)失效本项目产物并核对编译库存。
当前正常包为 `work/windows-monitor-delivery-20261117/package-r2/KeelShell.app`，
manifest 绑定精确工程5d2df2f，五个应用文件与提交前包逐字节相等，
arm64／minos15.0 结构检查通过，未启动／安装。
本批CI／API原件从上述[精确CI封存](testing/records/2026-11-17-windows-read-only-monitor-evidence/ci-completion/README.md)读取，
推送、正常包、回收与非作者API报告从[交付封存](testing/records/2026-11-17-windows-read-only-monitor-evidence/delivery/README.md)读取。
`work/windows-monitor-finalize-20261117/`保留文档推送读回与最终自有副本清理回执。
CI／reviewer和交付父目录中的工作副本仅供Git绑定后回收，不作为维护入口；
该交付目录仅需保留上述当前正常包，唯一target继续保留。
六份专用副本已按原 SHA 回收，886,506,992 字节；再以精确 Git 原件核对回收
1,968 文件／106,004,384 字节及7自有目录，包括原集成／review／oracle／stage 和旧 CI 副本，
这些旧路径不可继续使用；9个辅助原件已纳入69成员交付封存。
本轮所有作者／reviewer 均已实际关闭，正常包六文件摘要及 target 目录身份前后不变。
target 和其他任务资源保留。
旧包与旧路径只描述历史，不把已回收路径当当前入口。
