# KeelShell 当前交接 — 2026-11-16

本文只维护当前状态和接续入口。历史批次原文完整保存于
[2026-11-16 交接归档](HANDOFF_HISTORY_2026-11-16.md)；历史“不 push”、
旧 HOLD、旧候选与首次快照不覆盖本页及更新的精确回执。完整产品仍进行中。

## 交付与工程规则

- 用户已确认远端恢复，完成的功能批次及时提交并推送，读取本地、tracking、
  远端 SHA；不强推。当前分支为 `feature/editor-unicode-codec-20261114`。
  最近已实际读回三方相等的源码提交是
  `8c07d61009dba14f989dd0c00a2a370dc7a2d37b`，读回时工作区干净。
  `origin/main` 另有独立状态文档，不能直接覆盖。
- 本批源码及证据已在 `878f13b` 推送原功能分支；临时
  `feature/archive-deadline-clock-20261116` 已删除，本地/tracking/远端均 absent。
  精确 Quality `43815115588 / attempt 1` 已终态 failure：
  Windows 预检及 macOS app 失败，Ubuntu success；不移签旧 run。
- 保持原生 GPUI Kit 的 UI/core/session/AI 分层，阻塞 I/O 离开 UI 线程。
  只管理远程 SSH；本地终端、RDP、串口不在范围。
- 三主题霜白、曜石、雾紫全部保留，通过 registry、Palette、组件形状和
  chrome token 扩展；业务代码不按 theme ID 分支。设计遵循
  [原生设计合同](design/HALLMARK_NATIVE.md)。
- AI 上下文发送和命令执行分别显式审核；模型不能自主执行。host key 变化
  fail closed，凭据不进入普通配置或日志。
- 按批次实现、专项验证、新非作者复核，再做一次最终完整门禁。
  不把编译、模拟、headless 或其他源码的结果写成当前原生验收。
- 依赖显式直接要求只用 x.y，保留 Cargo.lock 的 patch 解析版本。
  无 Rust toolchain pin、无 `rust-version`，使用开发环境/CI 的受支持工具链。
  本机最近实测 Rust/Cargo 为 1.100.0。
- 用户已授权提交推送、标签触发跨平台 Release、本地 Podman 联调；不含覆盖
  安装现有应用、遥测上传或跳过尚未完成的发行验证。

## 活动批次：Windows 私有归档早期预检

上一源码 `8c07d61009dba14f989dd0c00a2a370dc7a2d37b` 的 Quality
`43848390542 / attempt 1` 已终态 failure，三平台完整原日志已收取。
macOS/Ubuntu success，app 分别 `1341/0/2 ignored`、`1342/0/2 ignored`，
真实 OpenSSH 均成功。Windows preflight `70/0/0`，app `1326/4/2 ignored`：
解包确认后 mutation-uncertain，三项续传确认后 Failed/0/None，根因仍 UNKNOWN。
“解包隔离导致后续续传拒绝”仍只是待验证假设，不能由旧脱敏信息确认。

当前三文件增量仅修改测试和 CI：Windows 完整 gate 前先编译库存并核对两精确
归档名，再串行执行私有后端模块；既有 nested-tree 用例增加 build/publish 固定
阶段诊断，app 测试增加固定 quarantine 分类。没有生产、安全断言或隔离改动。
719 项冻结 `a86f82425764c4fcec2341c2bf47dc4e91be486039d60eb1a60c50256688606a`。
本机 scoped gate 实际 `0/reaped/29.208s`：fmt/x.y、严格 workspace Clippy、
app 归档 `13/0/0`、宿主发布 `1/0/0`，14 名逐项相等、输入零漂移。
Windows session cross-Clippy 实际 `101/reaped/6.879s`，依赖缺 Windows SDK
`windows.h`；Windows 类型检查未完成、实际执行 0，须由新精确 CI 验证。
新非作者三文件静态 APPROVE、P0–P3 全 0、36/36 静态核验通过，
独立 Rust/PowerShell/native 执行 0，已关闭，未签 Windows 通过。
见[本批记录](testing/records/2026-11-16-windows-archive-preflight.md)。

工具栏 1800 紧凑断点、五文件诊断与 44 成员证据已在 `8c07d61` 推送；
该批完整本机 gate `3224/0/48 ignored`、rustdoc `14/0`，非作者批准。
本轮原生 inventory 再次报告 Mac 锁屏，未启动应用、GUI/截图均 0；
英文顶栏原生 P2 保持开放，正常包仍保留供解锁后复核，不能签当前已验收。
此前三主题正常包的 19 图/21 AX、真实 SSH/SFTP 和两份独立结果已在
[111 成员证据](testing/records/2026-11-16-kit010-native-ci-evidence/README.md)
持久保存。原生范围、旧失败及清理记录见各批记录，不移签。

## 已交付：Windows 目录发布短期句柄与 Kit 0.10

基线为已推送的 `4d72e1e592bdd6194f6eadca3a6a760192b5fe82`。
其 Quality `43822369487 / attempt 1` 的 Windows job `133148170879`
预检实际 `63/4/0 / 1.03s`，67 名与预期逐项相等，三项新游标回归全部通过。
四项剩余失败均为 `reopen-directory-rename / PermissionDenied / raw code 5`，
完整 Windows gate `NOT_RUN`；macOS/Ubuntu 已终态 success，app 分别
`1339/0/2`、`1340/0/2`，两处 UI 同步回归和真实 OpenSSH 均通过。
不是完整 Windows 文件操作或桌面通过。

目录源的短期 DELETE lease 已改为保留句柄加空 NT 相对名，复用同步
`NtCreateFile`；长期 capability 仍只读/share-all。三项 Windows 回归保留句柄绑定、
私有 DACL、访问与分享约束，原 22 项测试正文保持。r2 专项 `0/reaped/2.890s`，
fmt/x.y、本机 platform `18/0/0` 与 Windows 全 target cross-Clippy 均过。
r2/r3 非作者静态批准各绑定原输入，独立执行 0；Windows 真实仍待新 CI。

同时升级 Kit 直接要求到 `0.10`，五包实际锁到 `0.10.0`；默认仍为 gpui-pre。
应用中英文统一同步布局方向，真实 Input 专项绿、删同步行重新编译红（abX ≠ aXb）、
精确恢复重新编译绿。最新 `dependencies-live-r3` 实际 `0/reaped/17.158s`，
37 包/113 声明全部通过；旧新版本差异和 UNKNOWN 回执保持原事实。

组合门禁 r1 在 AI 设置页栈溢出；缩小渲染函数后原精确用例通过，AI settings
实际 `79/0/2 ignored`。r4 的完整门禁 r2 又在文件删除审核页栈溢出，实际
`1/reaped/58.955s`。r5 将文件工具栏、编辑器及比较结果的构建移到不内联
`AnyElement` helper，保留原树层级、子节点顺序、处理器和审核条件；原精确失败
例重新编译实际 `1/0/0`。没有提高线程栈、skip 或改变安全断言。

r5 门禁 r3 在工作区隧道冲突恢复流程再次栈溢出，实际
`1/reaped/389.376s`；其中 344 项 Files 名称与库存一致且全过。r4 AI 和 r5 Files
非作者静态范围均 P0–P3 全 0、组合各 HOLD，已关闭，不移签原失败。
r6 将普通隧道 runtime 大构建从 rule review/editor 的 render 栈帧分离，
原精确失败例重新编译 `1/0/0`，相关 workspace 隧道测试 `19/0/0`。

当前最终候选 r6 为八个工程改动文件，719 项全输入 SHA-256：
`f0f64a6629b2441d63eef1e801189eaef69dd848aea0039eef4d09ff143b5164`。
`full-gate-r4` 实际 `0/reaped/687.799s`：Rust `3223/0/48 ignored`、
rustdoc `14/0`、Python `290 run / 289 passed / 1 skipped`；fmt、严格 workspace
Clippy、x.y 与三组 AI controller 均过。app `1340/0/2`，1,342 名与重新编译
库存完全一致，session 归档 40 名逐项匹配，719 项工程路径/字节零漂移。
新非作者隧道增量及 r6 组合静态批准，P0–P3 全 0；独立执行 0，已核对并绑定
父最终门禁。其余历史批准/HOLD 保持原范围，全部 agents 已关闭。
[237 份持久成员](testing/records/2026-11-16-windows-directory-rename-evidence/README.md)
已随源码提交推送，包含 232 份原件与 5 份明确标记的派生说明；current、index、
精确源码/证据 Git 树及清理后绑定均实际 0/reaped、零 mismatch。
精确 Quality `43834927464 / attempt 1`，源码 `3c331c1`，已收取三平台完整原日志；
Windows filesystem preflight 70 名逐项全过，完整 app 仍有四败，其他两平台
终态 success。原失败与未运行边界见上方活动批次，不签完整 Windows。

三处原候选目录已逐字节核对持久原件后回收：本批、前批游标及 Kit 作者，合计
5,200 文件/43,290,303 字节；另清理 5 份本轮自有 crash 原报告，故障线程与原 SHA
已保全；两处自有调试临时目录此前已 absent。当前只使用
交付及原生目录的必要原件已持久封存，正常包和重复原目录已在精确提交绑定后回收。
历史冻结、失败、作者与各窄范围评审均保持，见
[本批记录](testing/records/2026-11-16-windows-directory-rename.md)。

## 已交付：Windows 独立目录游标修复

基线 `80397acc98d5881cbd177d7259a54fd7f1556c4f`，最终四文件已在
`4d72e1e592bdd6194f6eadca3a6a760192b5fe82` 提交推送；Windows 生产改动为
保留句柄加空 NT 相对名重开独立游标，
校验 identity/attributes/security，并增加三项 Windows 回归。
app 的两处测试同步处理本机目录暂停超时和远端 macOS 合并审核缺审；
没有生产 app 补丁。作者及三名非作者均已关闭。
[本批记录](testing/records/2026-11-16-windows-directory-cursor.md)维护完整范围。

- 初始两文件冻结 719 项
  `67337a389845e537823adf5d10671d52b9fd3ec95df43ac1634161c1b5022749`；
  最终四文件冻结 719 项
  `f5ab8e7180b7e8fcd90610100f496e869468bdf0605f7b073c215ec30d4ebd83`。
- `scoped-gate-r1` 实际 0/reaped/3.213s：x.y、fmt、本机 platform `18/0/0`、
  Windows 全 target cross-Clippy `-D warnings` 通过；Windows 实际执行仍 0。
- `full-gate-r1` 已实际 exit 1/reaped/485.787s，app `1338/1/2 ignored`：
  普通目录暂停/取消回归等待条件 12s 超时，具体 await 未知。已保留失败，
  后续 target/rustdoc/controller 未运行，不能签完整通过。
- 新非作者仅批准 Windows 两文件静态范围，P0–P3 全 0，独立运行 0，已关闭。
  35 项静态检查中的检查器文档范围误报保持；不移签到新 app 修订。
- 父已将目录暂停测试的 55ms 时序条件换为真实 WRITE hold + session pause
  handoff，原安全断言保持；专项实际 `1/0/0`、0/reaped/16.627s，
  新非作者批准、开放 P0–P3 均 0、独立执行 0，已关闭。
  macOS 源码 `878f13b` 的 job `133125295686` 另失败于
  `stale_merge_frame_actions_cannot_mutate_a_replacement_review`，
  app `1338/1/2`。其候选已集成：真实线程让步并推进 GPUI 时钟，等待操作完成，
  focused `1/0/0`、完整模块 `35/0/0`，命令均 0/reaped。
  新非作者静态批准、P0–P3 全 0、独立执行 0，已关闭。
  自动重绘和部分按钮禁用意味着不能签四回调独立覆盖；原断言逐字节保持。
- 最终 `full-gate-r2` 实际 `0/reaped/727.352s`：普通 Rust `3222/0/48 ignored`、
  rustdoc `14/0`、Python `290 run / 289 passed / 1 skipped`；fmt、严格 workspace
  Clippy、x.y 和三组 AI controller 通过。app `1339/0/2`，1,341 名及 session
  归档 40 名逐项匹配；719 项工程输入路径和字节零漂移。
  首轮失败及三份窄范围批准各自保留，不扩大其签署范围。当前无活动 agent。
- 最新官方依赖检查实际 exit 0，37 包/113 声明全部核实，UNKNOWN 0，
  无 manifest/lock 变更。旧批 UNKNOWN 保持历史事实。
- 旧 Windows 编译库存 64 名加三新例形成预期 67 名，未当成实际执行。
  后续必须检查真实 Windows 预检及八项 app，未运行不算成功。
- 当前目录 `work/windows-directory-cursor-integration-20261116/`；
  [74 份持久原件](testing/records/2026-11-16-windows-directory-cursor-evidence/README.md)
  已随源码推送，index 与精确源码 Git 树绑定均零 mismatch。
  原作者候选目录 24 文件/457,582 字节已核对后回收。前批诊断 64 文件/4,244,910 字节已
  在精确归档及末次读回核对后回收，旧目录不能再依赖。

## 已交付：Windows 原生阶段诊断与 CI 预检

基线 `64ce892428ad67ea70c1377c21f6d361db36497d`，四个工程文件提供 test-only 的固定 native 操作标签，
原样保留 typed `io::Error`；正常依赖库不输出诊断。没有更改访问权、FFI 参数、
句柄生命周期、审核、枚举或发布语义。诊断已交付；下一片修复候选尚未集成。
见[本批记录](testing/records/2026-11-16-windows-native-diagnostics.md)。

- 新 host 诊断 `2/0/0`；Windows platform 全 target 交叉严格 Clippy 通过，
  Windows 实际执行 0。
- 719 项工程输入集合 SHA-256：
  `774b46416b9c1ef424d89d7d3cb49799563d1c00237d1e52bcd4598677dd8f29`。
  完整门禁实际 0/reaped/750.798s：Rust `3222/0/48 ignored`、
  rustdoc `14/0`、Python `290 run / 1 skip`；格式、严格 Clippy、x.y 和
  三组 controller 均通过。app 1,341 名、session 归档 40 名逐项相等，
  platform `18/0/0` 包含两新例，路径及字节零漂移。
- Windows CI 在原完整 gate 前串行执行 platform lib 预检；失败则 job 失败、
  完整 gate `NOT_RUN`；成功后执行原 gate。无重试、扩时或错误忽略。
  原作者 r1 强制继续条件未采用，原报告保留，父修订 r2 另记。
- 新非作者静态复核 APPROVE，P0–P3 全 0，22/22 静态检查通过；
  独立 Rust/UI 执行 0。补充件读回父完整门禁，原报告不改写，reviewer 已关闭。
- 当前实时依赖查询 37 包/113 声明：19 包核实最新，18 包请求失败 UNKNOWN，
  整体 exit 1；没有 manifest、lock 或 Rust 工具链变更。
- 新 Windows job `133125296042` 已 completed/failure，platform
  `48/16/0 / 0.50s`；完整 gate `NOT_RUN`。16 组失败均为
  `nt-create-file / InvalidFilename / raw_os_error=123 / NTSTATUS=0xc0000033`，
  紧随同一错误的 `open-directory-cursor` 阶段。
  对应代码 `open_child(parent, ".", true)`；已定位平台共同失败阶段，
  不能据此声称八项 app 归档已修复。新候选作者只写
  `work/windows-directory-cursor-candidate-20261116/`，不运行 Cargo。

## 已交付：归档固定截止时间测试

基线为上述 `5c23396`，源码提交为 `bc9ebd6`。只改 session manifest 的 dev
feature 及两个测试文件，生产归档实现不变。详细事实及原件编号见
[本批测试记录](testing/records/2026-11-16-archive-deadline.md)。

- 用受控 Tokio 时钟覆盖一次执行的一秒截止；真实 SSH/Python/SFTP 仍必须经过
  两个 barrier。两次五秒和一次两秒墙钟上限保持，未知写入双路径隔离不变。
- 精确候选 `1/0/0`、完整 target `40/0/0`；实际编译的分阶段续期变异被拒绝
  `0/1/0`，生产字节精确恢复后重新编译绿例 `1/0/0`。
- 718 项工程输入集合 SHA-256：
  `593459030133909451156982dbb949e53d872cccf6cfa30a18593b2411e66e21`。
  `full-gate-r1` 实际 0/reaped/785.431s：Rust `3220/0/48 ignored`、
  rustdoc `14/0`、Python `290 run / 1 skip`；fmt、严格 Clippy、x.y 和三组
  controller 通过，app 1,341 名及全部输入逐项相等。
- 新非作者源码与文档复核 APPROVE，P0–P3 均 0，独立测试/UI 重放 0，已关闭。
  签署时的门禁待定条件由相同输入的成功回执另行满足，原报告保持。
- [63 份持久原件](testing/records/2026-11-16-archive-deadline-evidence/README.md)
  已随源码提交推送，index/精确源码树及清理后绑定均零 mismatch。
  原作者和 reviewer 已关闭。
  两个旧目录的 83 文件/1,297,657 字节已回收，不能继续依赖它们。
- 本次实时依赖检查覆盖 113 声明/使用和 37 包，23 包核实最新，14 包元数据
  请求失败 UNKNOWN；整体退出 1。x.y 政策另行检查，无版本升级或 lock 变更。

## 精确远端 CI 与 Windows 待办

诊断源码 `76b8253acd9bfa62180563f482e851064bc1ea34` 已推送；其 Quality
`43806549649 / attempt 1` 已 completed/failure。三平台 job 分别为：

| 平台 | Job | 最新保存状态 |
| --- | --- | --- |
| Ubuntu | `133098539119` | success |
| macOS | `133098539273` | success |
| Windows | `133098539513` | failure |

Windows app 实际 `1320/8/2 ignored`，八项均在 review-error，显示后缀
`Some(123)`，原子写入开始 0；该后缀不证明 typed error、NTSTATUS 或具体 syscall，
根因仍 UNKNOWN。macOS app `1339/0/2`、Ubuntu app `1340/0/2`。
[640 份完整原件](testing/records/2026-11-16-windows-archive-ci-evidence/README.md)
已无损归档，18 份来源文件与源码 Git blob 一致；调查 agent 已关闭，
原目录核对归档后回收。

截止时间源码 `bc9ebd6` 的 Quality `43809777642 / attempt 1` 亦已
completed/failure：macOS `133108602697`、Ubuntu `133108603084` success，
Windows `133108603091` failure。三平台日志保留于本批原件；不移签旧计数。
不取消或重跑这些已终态 run 以掩盖失败。新诊断 push 触发的新 run 单独登记。

更早源码 `9d0f886acda4e48058004a28f85ef1fa3c510d4c` 的 run
`43801985537 / attempt 1` 已 completed/failure：Ubuntu success；
macOS 固定截止测试裸 Elapsed，target `39/1/0`；Windows app
`1318/8/2 ignored / 1131.09s`，同八项归档缺审，没有底层错误码。
原始及去 ANSI 日志保存在
[上一批证据](testing/records/2026-11-16-archive-diagnostics-and-native-focus-evidence/README.md)。

## 最近已交付与限定原生状态

诊断批次的 718 项冻结为
`e967a7619be613cca5c3fc103f4a6d3c8ad2ad746caacc32386cf169820dd1ff`。
其最终完整门禁 0/reaped/704.319s：Rust `3220/0/48 ignored`、rustdoc `14/0`、
Python `290 run / 1 skip`；app 1,341 名逐项一致。源码 `76b8253` 与后续
文档 `5c23396` 已推送且读回一致。两份非作者批准 P0–P3 均为 0，均已关闭。

真实 macOS Frost 浅色中文、自有 SSH/SFTP、两侧各一文件：从命令框开始，
反向 Shift+Tab 到 remote26/local32，正向 Tab 到 local29/remote35；
67 步完整草稿保持。焦点由原始 PNG 观察，167 份导航 AX 完整值逐字节核对。
初始 38 项列表的反向 100 步只有 remote，不签双列表完整路径；AX summary
只报告窗口根，AX setValue 建立的草稿不签 IME，marker absent 不签 raw PTY
零出站。原生包复用上批正常字节，不把暂存 manifest 的 HEAD 当新编译身份。
其他主题、尺寸、平台及 VoiceOver 继续开放。三根进程、两端口、私有目录和
76 个 birth 身份已回收确认。见
[诊断与焦点记录](testing/records/2026-11-16-archive-diagnostics-and-native-focus.md)。

其他近期能力及证据入口：

| 范围 | 当前入口 |
| --- | --- |
| 文件列表键盘、独立审核重命名/删除 | [产品](product/FILE_BROWSER_KEYBOARD.md)、[记录](testing/records/2026-11-16-file-browser-keyboard.md) |
| 编辑器快捷键与 UTF-16/GBK 原生保存 | [记录](testing/records/2026-11-16-editor-keyboard-native.md) |
| macOS 主机监控与 Linux 能力恢复 | [记录](testing/records/2026-11-16-host-monitor.md) |
| 服务端归档与单次显式审核 | [产品](product/SERVER_ARCHIVES.md) |
| Windows 本地后端、Linux arm64 专项 | [记录](testing/records/2026-11-15-windows-local-backend.md) |
| Zmodem strict OO 尚未完整通过 | [产品](product/ZMODEM.md)、[记录](testing/records/2026-11-15-bounded-zmodem-farewell.md) |
| 完整功能/阶段清单 | [能力台账](product/CAPABILITIES.md)、[路线](ROADMAP.md) |

## 接续顺序与资源归属

1. 精确 Quality `43834927464` 已终态失败，70 项预检及原八项归档已逐名核对；
   接续修复四项 app 失败和本轮实际观察的英文工具栏裁切。
   作者不运行 Cargo，共享 target 的构建由父串行集中执行。
2. 新修订专项、一次最终完整门禁和新非作者复核后及时推送当前原功能分支，
   读取三方 SHA 并跟进新精确 CI；旧失败及签署范围保持。
   不得放宽安全断言、原生权限或将 app 未运行范围改成成功。
3. 更新本批记录/路线、归档必要原件，完成 feature commit 和普通 push，
   实际读回本地、tracking 和远端 SHA。当前源码/归档提交身份分别绑定。
4. 及时关闭自有 agents，回收已持久化的重复临时文件；不清理其他任务的
   worktree、分支、受保护记录或仍需接续的 CI 原件。
5. 继续补跨平台实际桌面、主题/尺寸/键盘/IME/辅助技术、严格独立 peer、
   更广 Linux 权限矩阵及发行验证；完整产品、FILE-05/M4 仍开放。

唯一 Cargo target 为 `work/recovery-batch-integration-20261115/target`，
`CARGO_BUILD_JOBS=2`、`MACOSX_DEPLOYMENT_TARGET=15.0`。归属没有切换，不反复 clean。
换 checkout/源码副本前必须遵循[共享 target 合同](testing/SHARED_TARGET.md)。
最近正常 GUI 来自 `3c331c1` 的独立打包 app，SHA-256
`f78cdfd3446bea3a40e3b390c4aa29af1d480c2a45c452118fc81f480971b790`；
包内 MCP 为 `5166352519c48c15f2d02db8986c4d9301ccd8e0d055c736cf9471bd88ce584b`。
所测三主题/macOS 范围见新记录；不能用该包代签后续工作区修订。
本批与游标批次原目录已在持久归档和精确 Git 树绑定后回收，不能再依赖。
前批交付/CI、正常包、原生复核、Windows 库存及封存目录均已持久绑定后清理。
当前活动目录为 `work/workbench-fit-20261116/`，新非作者原件为
`work/workbench-fit-review-20261116/`（该 reviewer 已关闭）；
Windows-only 作者材料为 `work/windows-extraction-stage-candidate-20261116/`。
正常包仍保留给锁屏解除后的原生复核。
旧 `work/windows-native-phases-integration-20261116/` 已回收。
前批父命令回执目录 `work/archive-deadline-delivery-20261116/` 的 11 文件、
23,386 字节已在 `878f13b` 归档后回收。
旧 Windows 调查及 CI 作者两个目录共 646 文件/6,626,866 字节已逐项核对后回收。
旧 `work/archive-deadline-integration-20261116/`、
`work/archive-diagnostics-ci-20261116/` 也已回收，原件从持久证据读取。
旧 `work/windows-archive-fix-20261116/` 和 `work/command-focus-ci-20261116/`
已回收，不能继续依赖。尚无本批新建用户 chat 或额外 worktree。

## 文档维护

需求/能力记在 `docs/product/`，设计决定在 `docs/adr/`，测试事实与原件在
`docs/testing/`，未完成项在 `docs/ROADMAP.md`。此页仅维护当前状态与下一步，
不要再向入口逐批追加完整历史。历史事实以各批精确源码、冻结和原始回执为准，
更新状态不能覆盖或移签旧失败。中文和英文 README 共同维护；仓库用自身能力
描述产品。需要追溯旧环境、旧权限或旧活动目录时，先读对应记录，再查
[历史交接](HANDOFF_HISTORY_2026-11-16.md)，不要把历史路径当当前可用资源。
