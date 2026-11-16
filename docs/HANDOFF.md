# KeelShell 当前交接 — 2026-11-16

本页只维护当前工作和接续入口。完整产品、FILE-05/M4、三平台桌面与发行仍进行中。
先按[当前路线](ROADMAP.md)查缺口，再读对应精确记录；历史“不 push”、旧 HOLD 和
旧计数不覆盖当前授权。旧入口可从 `2f29744:docs/HANDOFF.md` 的 Git blob 读取；
更早原文见[交接历史](HANDOFF_HISTORY_2026-11-16.md)与[路线历史](ROADMAP_HISTORY_2026-11-16.md)。

## 不变的实现与交付约束

- 真实原生 GPUI Kit，覆盖 macOS、Windows、Linux；只管理远程 SSH。
  app/UI、core/领域存储、session/传输、AI 分层，阻塞 I/O 离开 UI 线程。
- 三主题 Frost「霜白」、Graphite「曜石」、Lavender「雾紫」全部保留；
  新主题通过 registry、Palette、组件形状和 chrome token 扩展，业务代码不按主题 ID 分支。
  设计遵循[原生合同](design/HALLMARK_NATIVE.md)。
- AI 上下文发送与命令／文件动作分别显式审核，模型不能自主执行。
  host key 变化 fail closed；凭据默认短暂驻留，显式保存只能用 OS 凭据库或认证加密 vault。
- UNKNOWN、quarantine、pending mutation 与 close hold 保持；不自动重放不确定写入。
- 直接 registry 依赖仅用 `x.y`，保留 Cargo.lock 的 patch 解析；不 pin 工具链、不设 `rust-version`。
  本机现场默认 stable Rust/Cargo 为 1.100.0，不将本机版本转成产品 MSRV。
  本轮官方查询 37 包／113 声明全部通过；[实际回执](testing/records/2026-11-16-route-and-dependency-followup.md)保留旧 UNKNOWN。
- 按批次实现、专项、新非作者复核，再集中完整门禁；源代码、CI、真实联调、原生桌面和发行分别验收。
- 用户已授权及时提交推送、标签触发跨平台 Release、本地 Podman 联调；不含覆盖安装或遥测。
  当前分支 `feature/editor-unicode-codec-20261114`，普通 push，不强推；读回本地／tracking／远端 SHA。
  `origin/main` 的独立状态文档不可直接覆盖。

## 最近实际交付与精确 CI

当前源码 `95813a76f0ec9b59fd043cd982e33cdc01dd69f8` 已普通推送，三方一致、读回干净。
93 成员／719 工程输入的精确 Git 绑定 `0/reaped/7.116s`、零 mismatch。
新 Quality `43873806902` 正在运行。Windows `133321520130` 已 failure：
filesystem `70/0/0`、codec `41/0/0`、session `329/0/3 ignored`；
332 名库存与执行相等，Zmodem 45 名为 `43/0/2 ignored`，两新例与空目录替换例均 ok。
完整 gate 停在 Python discovery 的身份记录读取错误，290 run／1 failure／34 skipped，
全量 Rust 未运行；macOS `133321519808`、Ubuntu `133321520197` 仍在完整 gate。

最近源码 `01f6f6bd8272303fe049bc7b52976afefdb44c37` 和文档
`b7d6e92a937b3ad994c13c37e19db0bd0d059c8f` 已推送，三方一致、读回干净。
绑定该源码的 Quality `43867112056` 已 completed/failure；
watch `1/reaped/1580.816s`，原日志已全部收取，没有取消或重跑。
Windows `133298403995`：filesystem `70/0/0`、core archive `41/0/0`，
session library `327/2/3 ignored`，332 名编译库存与执行逐项相等；
其中 Zmodem 45 名为 `41/2/2 ignored`，完整 gate 未运行。
macOS `133298403996`、Ubuntu `133298403683` 的完整 gate 与真实 OpenSSH 均 success，
app 分别 `1341/0/2 ignored`、`1342/0/2 ignored`。两平台 core archive 均 `41/0/0`，
新 ZIP 用例逐名 ok；r2 修正同行 stderr 导致的摘要误归属，原日志及错误 r1 保持，
未重跑测试、未清理 target。详见[精确终态](testing/records/2026-11-16-windows-zmodem-test-contract.md)。

前批源码 `f094de6ad2789d168f5a24bacb316c98b83608ac` 的 Quality `43859165104`
已 completed/failure，watch `1/reaped/1600.284s`，没有取消或重跑。
三平台完整原日志全部收取：

| 平台 | Job | 实际结果 |
| --- | --- | --- |
| Windows | `133271138956` | filesystem `70/0/0`，archive backend `29/0/1 ignored`、30 名库存／执行逐项相等；app `1330/0/2 ignored`，原四项解包／续传全部逐名通过。后续 core archive `39/1/0`，剩余 session 与 target 未运行。 |
| macOS | `133271139049` | 完整 gate、真实 OpenSSH success，app `1341/0/2 ignored`。 |
| Ubuntu | `133271138662` | 完整 gate、真实 OpenSSH success，app `1342/0/2 ignored`。 |

Windows 新失败是危险来源权限重编码：读回 `0664`，安全断言要求 `0600`。
旧四项 app 失败已在此精确源码关闭，整个 Windows gate 与桌面仍未通过。
[终态记录](testing/records/2026-11-16-windows-archive-root-publication.md#精确-ci-终态)和
[81 成员原件](testing/records/2026-11-16-windows-archive-root-publication-evidence/ci-completion/README.md)
独立于原封存时点；验证实际 `0/reaped/6.835s`，成员及 719 工程输入零 mismatch。

## 活动批次：Windows 工装发布完成屏障

ZIP 私有权限与 Zmodem 两新断言已在上述精确 Windows runner 通过；
原 958 宿主完整门禁为普通 Rust `3225/0/48 ignored`、rustdoc `14/0/0`、
app `1341/0/2 ignored`，1343 名逐项相等。Turing 限定静态批准，
Ramanujan 补查 93 成员／719 Git 输入、r2 摘要及验收记录也限定 APPROVE、P0–P3 全 0；
两位均已关闭，原静态检索诊断与未验边界保持，详见
[Zmodem 记录](testing/records/2026-11-16-windows-zmodem-test-contract.md)。

接续只修改 `scripts/test_windows_native_crash.py`，其余 718 工程输入不变。
普通 tree 与 native Job barrier helper 都先等待写者关闭及 rename 后的发布标记，
再读取并校验身份；发布后 PermissionError 仍失败关闭。原始句柄、ACK、预算与清理断言保持。
719 冻结为 `1a516c0280487e62157ad3ee470206ef91c12a08501f12214ef7348c688cfc3a`。
专项 72 run／2 skipped；集中限定 gate `0/reaped/15.369s`，Python 主 discovery
293 run／2 skipped，fmt、严格 Clippy、x.y 和 diff 全过；两项受控负例精确失败，
工程输入零漂移。Rust 源码／manifest／lock 未变，本片不重复原完整 Rust 和三组 controller，
不将原 958 成绩移签为新 719 输入全量通过。
新非作者 Boyle 限定 APPROVE、P0–P3 全 0，独立专项 `72 run/70 passed/2 skipped/exit 0`，
unittest 6.134s／外层 6.203s，实际 waited；辅助检索诊断原文保留，已关闭。
候选 Windows 实际执行 0。
详见[ADR 0150](adr/0150-fixture-publication-readiness.md)和
[测试记录](testing/records/2026-11-16-windows-fixture-publication.md)。

## 下一步与资源

1. 收齐原 `43873806902` 三平台终态；新工装片完成非作者复核并封存后普通提交推送，
   读回三方 SHA。工程 push 不取消尚有价值的原 run。
2. 跟进下一精确 CI 的 293 项 Python 与 Windows 原七类 tree／实际 exclusive 文件反例，
   以及完整三平台 gate／OpenSSH；已过的 codec、session 与两新断言按原精确输入保留。
3. 桌面可用后复核英文工具栏 P2，再补主题／尺寸／双语／完整键盘／IME／辅助技术。
4. 继续路线中的外部 AI、隧道、配置同步、高级传输和发行矩阵；不把阶段测试写成完整产品完成。
5. 关闭自有 agent，核对持久原件后回收已无用途的副本；保留其他任务的工作树、分支和容器。

唯一 Cargo target 为 `work/recovery-batch-integration-20261115/target`，
`CARGO_BUILD_JOBS=2`、`MACOSX_DEPLOYMENT_TARGET=15.0`。同一 checkout 连续使用，归属未切换，
不反复 clean；换归属前遵循[共享 target 合同](testing/SHARED_TARGET.md)。
当前活动目录为 `work/windows-fixture-publication-integration-20261116/`；
旧 publication、两个已完成 reviewer 目录及稳定测试副本在精确 Git 原件核对后
已回收 88 文件／13,465,970 字节，三个自有 agent 均已关闭。
旧 publication 路径不可再使用。ZIP 活动副本、Zmodem 准备候选与 Turing 目录
已在精确 Git 封存核对后回收，共 58 文件／1,040,297 字节；
保留必要交付和旧 CI 活动原件。Ramanujan、Boyle 均已收取并关闭；
新报告副本在精确 Git 封存核对后回收。
没有新建用户 chat、worktree、分支或容器。

最近原生 inventory 仍明确报告 Mac 锁屏且自动解锁失败；GUI 操作／截图 0。
保留 `work/workbench-fit-20261116/package-r1/KeelShell.app` 供工具栏复核：
app SHA-256 `3b127956723c187943236537c9c49814e99866834a9d01a2468d85cf751dd19f`，
MCP `5166352519c48c15f2d02db8986c4d9301ccd8e0d055c736cf9471bd88ce584b`。
该包绑定旧 8c 工程输入，不代签当前源码；没有覆盖安装或启动未隔离应用。

## 文档入口

需求／能力维护在 `docs/product/`，设计在 `docs/adr/`，测试／原件在 `docs/testing/`，
剩余范围在[路线](ROADMAP.md)。近期相关记录：
[Kit 0.10 与限定原生](testing/records/2026-11-16-kit010-native-ci.md)、
[文件列表键盘](testing/records/2026-11-16-file-browser-keyboard.md)、
[双向焦点补验](testing/records/2026-11-16-archive-diagnostics-and-native-focus.md)、
[编辑器 UTF-16／GBK](testing/records/2026-11-16-editor-keyboard-native.md)、
[监控与恢复](testing/records/2026-11-16-host-monitor.md)、
[严格 peer 边界](testing/records/2026-11-15-bounded-zmodem-farewell.md)。
当前入口不再整段追加历史；精确 SHA、失败、UNKNOWN 与原签署范围以各批记录为准。
