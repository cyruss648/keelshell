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

最近源码 `f094de6ad2789d168f5a24bacb316c98b83608ac` 已推送。
随后文档 `2f29744a2615393bcc5bedb81cd2c425e7bc6372` 已实际读回三方一致，读回时干净。
后续源码候选的提交身份按本批记录与 Git readback 接续，不能将旧 CI 移签。

Quality `43859165104` 已 completed/failure，watch `1/reaped/1600.284s`，没有取消或重跑。
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

## 活动批次：ZIP 权限元数据与早期后端门禁

三个工程文件：core archive 编码器、core archive 集成测试、CI。
719 项冻结 SHA-256 为 `bf46afc32603f312f3ff9e7edb5e48ea9bb4c3073b109c072e2f6ded44a8f4fd`。
详见[ADR 0149](adr/0149-portable-zip-permission-metadata.md)和
[本批记录](testing/records/2026-11-16-portable-zip-metadata.md)。

ZIP 明确使用 Unix 权限元数据解释空间，文件继续 `0600`、目录继续 `0700`；
不放宽来源、ACL、审核、无覆盖发布或未知结果。新增回归检查文件／目录的原始头字段和完整 roundtrip。
Windows CI 先执行完整 core archive target，再执行完整 session library；均有精确库存 guard，
成功才进入原完整 gate，避免每次等完整 app 长流程后才发现后端错误。

宿主两项 DOS 受控负例均真实 Rust 101，新例 `Dos != Unix`，原例精确复现 `436 != 384`。
生产字节精确恢复后重新编译，`scoped-gate-r1` 实际 `0/reaped/51.622s`：
fmt、x.y、严格 workspace all-target Clippy；archive `41/0/0`、session library `288/0/4 ignored`、
app archive `13/0/0`。346 个唯一库存名与结果逐项相等，342 ok／4 ignored；719 输入零漂移。
本批没有重跑完整产品 gate，新的 Windows 执行须由后续精确 CI 取得。
新非作者 Noether 已限定 APPROVE，P0–P3 全 0；独立稳定预构建副本
`41/0/0 / exit 0 / reaped / 9.175s`，独立编译／Windows native／GUI 均 0，已关闭。

另有尚未集成的 Windows Zmodem 两例候选，保存在
`work/windows-zmodem-parent-rename-candidate-20261116/`。
当前实际 CI 尚未执行到这些用例，不声称已经失败或修复。
其独立静态报告 HOLD/P2=1：两个旧名字声称“替换后拒绝”，正文改为“OS 阻止替换”，
采用前必须准确改名并更新 CI guard；第三个空目录替换拒绝用例保持。
报告已随上述 CI 81 成员原件保存，reviewer 已关闭。

## 下一步与资源

1. 新非作者已完成且无开放发现；冻结不变后及时提交推送并读回三方 SHA。
2. 跟进新精确 CI 的 Windows codec／完整 session 预检与三平台终态；只按实际失败接续。
   若命中上述 Zmodem 两例，先保存原件，再审修订及准确命名，不机械改其他 rename。
3. 桌面可用后复核英文工具栏 P2，再补主题／尺寸／双语／完整键盘／IME／辅助技术。
4. 继续路线中的外部 AI、隧道、配置同步、高级传输和发行矩阵；不把阶段测试写成完整产品完成。
5. 关闭自有 agent，核对持久原件后回收已无用途的副本；保留其他任务的工作树、分支和容器。

唯一 Cargo target 为 `work/recovery-batch-integration-20261115/target`，
`CARGO_BUILD_JOBS=2`、`MACOSX_DEPLOYMENT_TARGET=15.0`。同一 checkout 连续使用，归属未切换，
不反复 clean；换归属前遵循[共享 target 合同](testing/SHARED_TARGET.md)。
当前活动目录为 `work/zip-unix-metadata-integration-20261116/`；旧 publication 目录已封存，
待提交／Git 绑定后回收。没有新建用户 chat、worktree、分支或容器。

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
