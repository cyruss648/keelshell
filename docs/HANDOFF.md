# KeelShell 当前交接 — 2026-11-16

本文只维护当前状态和接续入口。历史批次原文完整保存于
[2026-11-16 交接归档](HANDOFF_HISTORY_2026-11-16.md)；历史“不 push”、
旧 HOLD、旧候选与首次快照不覆盖本页及更新的精确回执。完整产品仍进行中。

## 交付与工程规则

- 用户已确认远端恢复，完成的功能批次及时提交并推送，读取本地、tracking、
  远端 SHA；不强推。当前分支为 `feature/editor-unicode-codec-20261114`。
  最近已实际读回三方相等的提交是
  `5c23396c3cfcc6dd021fb870a7007373c25475d1`。
  `origin/main` 另有独立状态文档，不能直接覆盖。
- 当前 HEAD 为 `64ce892428ad67ea70c1377c21f6d361db36497d`，已推送到
  `feature/archive-deadline-clock-20261116`，该分支本地/tracking/远端相等。
  原分支远端暂留 `5c23396`；两个旧 CI 均已终态，无须再为保留日志延后 push。
  本批验证及复核后同步原分支，并删除已被替代的临时分支。
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

## 活动批次：Windows 原生阶段诊断与 CI 预检

基线为上述 `64ce892`，四个工程文件提供 test-only 的固定 native 操作标签，
原样保留 typed `io::Error`；正常依赖库不输出诊断。没有更改访问权、FFI 参数、
句柄生命周期、审核、枚举或发布语义。Windows 根因仍 UNKNOWN。
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

1. 本批完整门禁及非作者复核已通过，归档必要原件；不因文档或摘要再跑全量。
2. 提交并推送原功能分支，读回精确新 CI；删除已被替代的临时本地/远端分支。
   根据真实 Windows 预检决定修复，不能把诊断签成修复。
3. 更新本批记录/路线、归档必要原件，完成 feature commit 和普通 push，
   实际读回本地、tracking 和远端 SHA。当前源码/归档提交身份分别绑定。
4. 及时关闭自有 agents，回收已持久化的重复临时文件；不清理其他任务的
   worktree、分支、受保护记录或仍需接续的 CI 原件。
5. 继续补跨平台实际桌面、主题/尺寸/键盘/IME/辅助技术、严格独立 peer、
   更广 Linux 权限矩阵及发行验证；完整产品、FILE-05/M4 仍开放。

唯一 Cargo target 为 `work/recovery-batch-integration-20261115/target`，
`CARGO_BUILD_JOBS=2`、`MACOSX_DEPLOYMENT_TARGET=15.0`。归属没有切换，不反复 clean。
换 checkout/源码副本前必须遵循[共享 target 合同](testing/SHARED_TARGET.md)。
正常 GUI 仍为上批摘要；门禁后 MCP 字节已变化，实测
`3e9270c84f3aad14f331b06cb3f15e77f2d2a40602dcd61e90dd39b5b939ed4b`。
不能再用 target 中 MCP 代签上一批程序；当前不签新正常包或新桌面验收。
当前批次目录为 `work/windows-native-phases-integration-20261116/`；
前批父命令回执目录 `work/archive-deadline-delivery-20261116/` 尚待归档回收。
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
