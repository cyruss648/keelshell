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

当前工程 `ee3ad7320e90f40c9d777c500543cc6e909d0484` 已普通推送，三方一致、读回干净。
25 成员／719 输入的精确 Git 绑定 `0/reaped/6.641s`、零 mismatch。
新 Quality `43880245383` 运行中：Windows `133343901451` 的三项预检 step success，
完整 gate 进行中；macOS `133343901229`、Ubuntu `133343901650` 也在完整 gate。
新具体库存、293 项 Python、exclusive file 与 Job barrier 结果仍待终态原日志。

前一源码 `95813a76f0ec9b59fd043cd982e33cdc01dd69f8` 的 Quality `43873806902`
已 completed/failure，watch `1/reaped/1576.244s`，未取消／重跑。
Windows filesystem `70/0/0`、codec `41/0/0`、session `329/0/3 ignored`；
332 名编译库存与执行相等，Zmodem 45 名 `43/0/2 ignored`，两新例与空目录替换例均 ok。
完整 gate 停在 Python 身份记录读取错误：290 run／1 failure／34 skipped，全量 Rust 未运行。
macOS／Ubuntu 完整 gate 与真实 OpenSSH 均 success，普通 Rust 分别 `3225/0/48`、`3218/0/49`，
各 14 项 rustdoc，app 分别 `1341/0/2`、`1342/0/2`。
CI app 只核对运行声明数与逐名结果，独立 `--list` 库存来自宿主，不能混同。
[21 成员终态](testing/records/2026-11-16-windows-zmodem-test-contract-evidence/ci-completion/README.md)
验证 `0/reaped/8.131s`；r3 正确处理提前到达的 Doc-tests stderr，原解析诊断保留，无测试重放。

更早的 ZIP 权限缺陷、原四项 app 失败和各自精确源码／库存见
[ZIP 终态](testing/records/2026-11-16-portable-zip-metadata.md#精确-ci-终态补充)、
[根发布终态](testing/records/2026-11-16-windows-archive-root-publication.md#精确-ci-终态)。
原失败、UNKNOWN 与封存时点保持，不追溯修改成后续通过。

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
25 成员封存已随工程推送；新候选 Windows 的运行签署仍待当前精确 CI。
另有[26 成员交付补充](testing/records/2026-11-16-windows-fixture-publication-evidence/delivery/README.md)
保存源码 push／三方读回、正常包摘要、清理与 CI 运行中快照；不含二进制或活动 watch。
详见[ADR 0150](adr/0150-fixture-publication-readiness.md)和
[测试记录](testing/records/2026-11-16-windows-fixture-publication.md)。

## 下一步与资源

1. 原 `43873806902` 三平台终态已收齐，工装片已复核、封存、推送和三方读回；
   收取当前 `43880245383`，不重复未变 Rust 输入的宿主完整测试。
2. 跟进当前精确 CI 的 293 项 Python 与 Windows 原七类 tree／实际 exclusive 文件反例，
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
新报告副本已在精确 Git 核对后回收 4 文件／66,095 字节。
没有新建用户 chat、worktree、分支或容器。

本次再次原生 inventory 仍明确报告 Mac 锁屏且自动解锁失败；GUI 操作／截图 0。
新正常包为 `work/windows-fixture-publication-integration-20261116/package-r1/KeelShell.app`，
绑定 `ee3ad73` 与当前冻结。release build `0/reaped/150.274s`，
打包及本机结构／arm64／minos 15.0 检查通过；没有启动、安装、签名或公证。
app SHA-256 `be1029bda1fe6afc8882d11cbf9a0be789ae32faa587028c3e7fccf2d77f494a`，
MCP `5166352519c48c15f2d02db8986c4d9301ccd8e0d055c736cf9471bd88ce584b`。
旧 `work/workbench-fit-20261116/package-r1/` 的 8c 包保留作工具栏历史证据，
不代签当前源码或当前 GUI；桌面可用后优先用新包补验。

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
