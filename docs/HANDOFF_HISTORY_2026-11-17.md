# KeelShell 当前交接 — 2026-11-17

本页只维护当前工作和接续入口。完整产品、FILE-05/M4、三平台桌面与发行仍进行中。
先按[当前路线](ROADMAP.md)查缺口，再读对应精确记录；历史“不 push”、旧 HOLD 和
旧计数不覆盖当前授权。旧入口可从 `2f29744:docs/HANDOFF.md` 的 Git blob 读取；
更早原文见[交接历史](HANDOFF_HISTORY_2026-11-16.md)与[路线历史](ROADMAP_HISTORY_2026-11-16.md)。

当前新增Windows远端只读监控正在并行实现／集成，尚未验收：
父负责连接级平台探针、缓存及失效，三个agent分别负责collector／parser、
界面／行为测试和真实Windows工装。公共类型明确未知指标与CPU gauge；
ZIP锁文件更新9.6.1，清单保持9.6。设计见[ADR0156](adr/0156-windows-read-only-monitor.md)。
本轮官方依赖查询部分超时，完整新鲜度尚未通过；不移签旧门禁。
最新桌面可用性检查仍为Mac锁屏、自动解锁失败，GUI操作／截图0。
开发目录`work/windows-monitor-integration-20261117/`，沿用下述唯一target，
子agent不运行Cargo或触碰该target，父负责集成后集中门禁。

监控渲染栈修复已普通推送`bbd4eaccfa8bba2feb487ff43e0423567428bc82`，
本地／tracking／远端三方一致、读回干净。原UDP工程`1670329`已推送，其
Quality43945988599终态为failure，macOS完整gate／OpenSSH成功，
Ubuntu／Windows分别在两项既有嵌套监控回归中栈溢出。
[37成员终态](testing/records/2026-11-17-darwin-udp-sockets-evidence/ci-completion/README.md)
保存全部原job日志、已观察／未报告结果和派生诊断，729个精确Git输入绑定通过；
没有取消或重跑，watch已reap。

最终修复r2将全部分区移入私有AnyElement builder，主render只装配；
没有增加栈配置、改测试断言或依赖。731输入aggregate
`2ea283af87af7e0af2fa912ebc28f9360e0c5113cd40e1111502f8cdd6d8551e`。
限定gate为`0/reaped/354.196674s`，fmt、严格workspace/all-target Clippy、
x.y和显式2 MiB完整app1375/0/2通过，1377库存与逐名结果相等。
本片没有重跑完整workspace，不移签旧3325项。
Mill／Hubble最终r2分别静态和父级专用prebuilt限定批准，P0–P3全0，
后者独立78/0/0、独立编译0；原r1中止／漂移及全部诊断保持。
两位reviewer及规划agent Singer已关闭，2份专用副本／548,027,216字节已回收。
正常release构建及arm64/minos15.0包结构通过；192成员／731输入的精确Git绑定通过。
提交来源包五个应用文件与提交前包逐字节相等，只更新manifest来源。
新Quality43955379234已启动：Windows133608443190、macOS133608443592、
Ubuntu133608443616均运行中，尚无终态；未取消或重跑旧run，也未留下watcher。
按精确Git原件回收266文件／74,562,243字节、4个已无用途的自有目录；
当前正常包和共享target身份不变，首轮清理预检发现bytecode而安全停止的诊断保持。
本片没有原生桌面操作，最新依赖在线完整复查仍UNKNOWN。
接续见[栈修复记录](testing/records/2026-11-17-monitor-render-stack.md)及
[ADR0155](adr/0155-monitor-render-stack-boundaries.md)。

UDP与缓存分页的专项、真实Darwin SSH15项、原729输入宿主完整门禁和独立复核
保持各自通过范围，详见[UDP记录](testing/records/2026-11-17-darwin-udp-sockets.md)；
不能代签后续731输入或三平台桌面。
上一a1的Quality43934041540三平台成功与
[727输入终态](testing/records/2026-11-17-editor-backup-readback-evidence/ci-completion/README.md)
也只属于该精确源码。

上一批编辑器备份完整读回：30 session 新例和 5 GPUI 新例、标准 OpenSSH
18 项已通过；长状态详情移入／滚动 P2 已修复，两位独立追加复核均 APPROVE、
P0–P3 全 0。功能复核使用父级 prebuilt，独立 app/editor `88/0/0`；
源码复核仅签静态差分。作者和两位 reviewer 均已关闭。
r4 冻结 727 输入，aggregate `78d8a0c0000ce0afb661d8af0c6712a266cd44fa168b801d37c1672c39ddc4c2`，
一次集中完整门禁 `0/reaped/701.469s`：普通 Rust `3307/0/52 ignored`、
app `1372/0/2`、14 rustdoc、Python 293/2 skipped，helper 另列 30。
准确 app 的 1374 库存与逐名执行相等，35 新例全部 ok；工程输入前后不变。
正常 release build 和 arm64／minos 15.0 包结构通过；
未启动或安装，锁屏使原生仍待验收。工程
`f810f674c57b2c25dac5b987d77bf38656e3af56` 已普通推送、三方一致，
206 成员／727 输入精确 Git 绑定通过；提交来源正常包结构通过。
原 Quality `43929740313` 的 Windows session 预检因测试分类器漏新 variant
而编译失败，filesystem70／archive41通过，完整 gate 未开始。
该 Windows 测试 helper 只补一行穷尽分支；r5其余726输入不变，
aggregate `5c93cae03daaf370227820a815ad969623a0fb6bef1afb0d6e5362f09816528b`。
限定 gate `0/reaped/26.866s`，session343库存／执行相等、`339/0/4`，
新30项 ok，fmt／严格 Clippy／策略通过；Banach新静态窄复核APPROVE、P0–P3全0，
独立编译／执行0，已关闭，四位自有agent均关闭。
修复 `a1a97b3b2ee0f0ccf91ea53ca96623aa8867fe8d` 已普通推送、三方一致；
60成员／727输入精确 Git 绑定 `0/reaped/6.955532s`、零 mismatch。
新 Quality `43934041540` 的 Windows133533978752 已通过 session backend预检，
原编译错误在真实Windows消除；后续三平台完整质量门禁均已成功，见上方终态。
旧run已 completed/cancelled，Windows原failure保持，macOS／Ubuntu为cancelled，
不能补签终态成功。没有手动重跑旧run或活动watcher。
累计300文件／70,938,741字节副本已按Git原件回收；首轮目录删除诊断及恢复保留，
当前正常包六文件及恢复／后续清理的共享target目录身份核对通过。
最后交付原件见[最终补记](testing/records/2026-11-17-editor-backup-readback-evidence/delivery/final/README.md)。从
[本批记录](testing/records/2026-11-17-editor-backup-readback.md)接续，不重放未知写入。

上一功能批新增远端 macOS 账户可见的 TCP LISTEN 列表，使用已有 lsof，
独立于 Python 资源采集；UDP、主动探测、进程核对／终止保持不支持。
基线 `8d95d71cfa08181d5d44b689c108edcad7d995c4`，最终 r4 冻结 724 输入、
aggregate `baae8aaab5903ad60509c23341b91a23553fac47ea7d1b0ff9ac993031c2e6ef`。
完整 `full-gate-r1` 为 `0/reaped/794.954s`：普通 Rust `3272/0/51 ignored`、
app `1367/0/2 ignored`、14 rustdoc，Python discovery 293/2 skipped、helper 另列 30。
准确 gate app 的 1369 库存与逐名执行相等，26 新例全部 ok。
两位非作者分别限定源码／预编译功能 APPROVE，P0–P3 全 0；三位自有 agent 已关闭。
原缺帧 P2 已修复，初始负例和 mapped 显示失败保留。
真实 Darwin SSH `9/0/0`、Linux Podman 降级／恢复 `2/0/0`，自有服务、临时密钥、
容器、私有 network 与派生／临时基础镜像已清理。Linux 回执的 source-SHA 参数
是基线 HEAD，实际 working diff 另由冻结和 binary SHA 绑定，不能混同。
依赖官方复查 37 包／113 声明通过，本批 manifest／lock 未改。
正常 release build `0/reaped/151.594s`，arm64／minos 15.0 包结构检查通过；
没有启动或安装。最新 native 可用性观察仍为锁屏，GUI 操作／截图 0。
工程 `bd198a8ef2578ea9b96e0a603cd705bb2eadca13` 已普通提交／推送，
`2026-11-16T17:14:08Z` 本地／tracking／远端一致、工作区干净。
255 成员与 724 输入从该提交真实 Git blob 验证 `0/reaped/6.972s`、零 mismatch。
精确提交正常包位于 `work/darwin-tcp-listeners-20261117/package-r1/KeelShell.app`，
GUI `f94ecc51…`、MCP `bb415f3a…`，打包／结构检查通过。
已按精确 Git 原件回收 301 文件／88,633,128 字节；当前和上一正常包六文件摘要、
共享 target 目录身份前后相等。
Quality `43916079699` 的 Ubuntu `133470463980`、macOS `133470464468`、
Windows `133470464623` 已全部 completed/success；watch `0/reaped/100.277s`，
无活动 watcher。Windows／macOS／Ubuntu 普通 Rust 为 `3259/0/49`、
`3272/0/51`、`3265/0/52`，app 为 `1356/0/2`、`1367/0/2`、`1368/0/2`，
各 14 rustdoc；Python 293 项分别 34／2／2 skipped。
macOS／Ubuntu 标准 OpenSSH success。31 成员
[精确终态](testing/records/2026-11-17-darwin-tcp-listeners-evidence/ci-completion/README.md)
重算通过，724 Git 输入绑定通过；不代签当前编辑器工作差分。
新非作者 Locke 对原指定文档／封存独立重算并限定 APPROVE，P0–P3 全 0，
两个文档验证器 exit 0；已关闭，本批四位自有 agent 均已关闭。
[43 成员交付补充](testing/records/2026-11-17-darwin-tcp-listeners-evidence/delivery/README.md)
保存报告与实际交付／清理；该复核不签署父级后续补记或当前进行中的 CI。
按[本批记录](testing/records/2026-11-17-darwin-tcp-listeners.md)接续；
完整产品、原生和发行仍开放。

上一工程 `6f28f5c424752a221746fbfb621e319c20ff38b9` 的 Quality `43901863960`
已三平台 completed/success，watch 已 `0/reaped/1258.244s`，无活动 watcher。
Windows／macOS／Ubuntu 普通 Rust 为 `3233/0/46`、`3246/0/48`、`3239/0/49`，
app 为 `1351/0/2`、`1362/0/2`、`1363/0/2`，各 14 rustdoc；
Python 均 293 run，分别 34／2／2 skipped。macOS／Ubuntu OpenSSH success。
[25 成员终态](testing/records/2026-11-16-tunnel-configuration-reload-evidence/ci-completion/README.md)
从精确 Git blob 核对原 721 输入，`0/reaped/7.658s`、零 mismatch，不重放测试。
CI 无独立 app 库存，不把该批成功移签为当前 724 输入。

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
  上轮官方查询 37 包／113 声明通过；当前UDP批在线请求失败及有界超时，
  最新版本完整复查仍为UNKNOWN；manifest／lock未改，确定性x.y策略通过。
  [上一轮回执](testing/records/2026-11-16-route-and-dependency-followup.md)不代签本轮在线状态。
- 按批次实现、专项、新非作者复核，再集中完整门禁；源代码、CI、真实联调、原生桌面和发行分别验收。
- 用户已授权及时提交推送、标签触发跨平台 Release、本地 Podman 联调；不含覆盖安装或遥测。
  当前分支 `feature/editor-unicode-codec-20261114`，普通 push，不强推；读回本地／tracking／远端 SHA。
  `origin/main` 的独立状态文档不可直接覆盖。

## 此前 Windows 工装交付与精确 CI

该批工程 `ee3ad7320e90f40c9d777c500543cc6e909d0484` 已普通推送，三方一致、读回干净。
25 成员／719 输入的精确 Git 绑定 `0/reaped/6.641s`、零 mismatch。
新 Quality `43880245383` 已 completed/success；Windows `133343901451`、
macOS `133343901229`、Ubuntu `133343901650` 的完整 gate 均 success，
macOS／Ubuntu 真实 OpenSSH 也 success；watch `0/reaped/1893.751s`。

| 平台 | 普通 Rust passed / failed / ignored | app passed / failed / ignored | Python run / skipped | rustdoc passed |
| --- | --- | --- | --- | --- |
| Windows | `3212 / 0 / 46` | `1330 / 0 / 2` | `293 / 34` | 14 |
| macOS | `3225 / 0 / 48` | `1341 / 0 / 2` | `293 / 2` | 14 |
| Ubuntu | `3218 / 0 / 49` | `1342 / 0 / 2` | `293 / 2` | 14 |

各平台 92 个普通套件和全部 rustdoc 子运行的声明／唯一结果／摘要相等；
CI app 无独立 `--list`，Python 无逐名 `-v`，不能补写这些不存在的证据。
Windows codec／session 预检分别有独立 41／332 名库存，全部逐项相等。
七类真实 tree 保持原句柄、身份先于 ACK、exit7 与完整清理；
native nested Job barrier 保持前后两个 LIVE，拒绝后建子进程，最终两个原对象 TERMINAL，
controller `0`、active `0`、pipes drained。share=0 新例由完整 discovery 覆盖，
不另填单例逐名回执。[36 成员终态](testing/records/2026-11-16-windows-fixture-publication-evidence/ci-completion/README.md)
验证 `0/reaped/6.699s`、零 mismatch；Parfit 独立重算批准终态证据，P0–P3 全 0，
14 项原件哈希不变，没有 Cargo／测试／target 写入，已关闭。

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

## 本批工程收口：Windows 工装发布完成屏障

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
25 成员封存已随工程推送；后续当前精确三平台完整 CI 已通过，
原封存时 Windows 执行 0 的事实不重写。
另有[26 成员交付补充](testing/records/2026-11-16-windows-fixture-publication-evidence/delivery/README.md)
保存源码 push／三方读回、正常包摘要、清理与 CI 运行中快照；不含二进制或活动 watch。
详见[ADR 0150](adr/0150-fixture-publication-readiness.md)和
[测试记录](testing/records/2026-11-16-windows-fixture-publication.md)。

## 下一步与资源

1. 监控栈修复`bbd4eac`已普通推送，限定门禁、独立复核、精确Git绑定及正常来源包通过；
   收取运行中的Quality43955379234终态，保持原43945988599失败和全部诊断。
   保留依赖在线UNKNOWN和原生未验边界，不重复未变宿主测试。
   编辑器备份及Windows测试修复已推送；精确源码 `a1a97b3` 的
   Quality43934041540三平台完整门禁已成功，不重放未变的宿主测试或旧CI。
   Darwin TCP 工程 `bd198a8` 已普通推送、三方一致，`43916079699` 三平台终态已收齐，
   不重放未变的监听宿主 gate 或 CI。
   上批 `43901863960` 与更早 `43880245383` 的终态分别记录，不能代签当前源码。
2. Windows 源码完整质量门禁现已通过；继续真实桌面、协议 peer、远端平台及发行缺口，
   不把 runner 测试和正常包结构扩大成原生桌面／完整产品验收。
3. 桌面可用后复核英文工具栏 P2，再补主题／尺寸／双语／完整键盘／IME／辅助技术。
4. 继续路线中的外部 AI、隧道、配置同步、高级传输和发行矩阵；不把阶段测试写成完整产品完成。
5. 关闭自有 agent，核对持久原件后回收已无用途的副本；保留其他任务的工作树、分支和容器。

唯一 Cargo target 为 `work/recovery-batch-integration-20261115/target`，
`CARGO_BUILD_JOBS=2`、`MACOSX_DEPLOYMENT_TARGET=15.0`。同一 checkout 连续使用，归属未切换，
不反复 clean；换归属前遵循[共享 target 合同](testing/SHARED_TARGET.md)。
监控修复当前正常包与必要交付工装位于
`work/monitor-stack-delivery-20261117/package-r2/KeelShell.app`及其父目录；
旧`work/monitor-stack-20261117/`、`work/windows-monitor-planning-20261117/`、
`work/darwin-udp-ci-20261117/`与`work/darwin-udp-delivery-20261117/`已核对回收。
原件与Windows后续规划从本批Git封存读取，不能继续访问旧活动路径。
编辑器活动目录为 `work/editor-backup-verification-20261117/`；
编辑器原失败、r4正例及两位reviewer的原审／追加签署已移至docs主封存；
活动目录保留精确f810f67正常包和后续交付／Windows修复工装。
`work/darwin-tcp-listeners-20261117/` 是上一批保留正常包和交付回执。
本批与上一活动副本按封存／Git blob 核对后已回收 301 文件；只保留当前必要正常包、
交付工装与最后回执。上一隧道活动目录仅保留精确 `6f28f5c` 正常包；
原日志和解析器从 docs 封存读取，不再使用旧活动路径。
上一发布批活动目录为 `work/windows-fixture-publication-integration-20261116/`；
旧 publication、两个已完成 reviewer 目录及稳定测试副本在精确 Git 原件核对后
已回收 88 文件／13,465,970 字节，三个自有 agent 均已关闭。
旧 publication 路径不可再使用。ZIP 活动副本、Zmodem 准备候选与 Turing 目录
已在精确 Git 封存核对后回收，共 58 文件／1,040,297 字节；
保留必要交付和旧 CI 活动原件。Ramanujan、Boyle 均已收取并关闭；
新报告副本已在精确 Git 核对后回收 4 文件／66,095 字节。
纯文档 `ebefc90e850a4843475da01587b771ab1d244f11` 已推送、三方一致；
新非作者 Beauvoir 限定批准 13 项文档／两个补充，P0–P3 全 0，已关闭。
[17 成员后续补充](testing/records/2026-11-16-windows-fixture-publication-evidence/delivery/followup/README.md)
保存原报告和文档交付；旧 Zmodem 活动目录及本次报告副本在 Git 原件核对后
已回收 71 文件／9,100,899 字节，r3 解析器已复制到当前活动目录。
旧 Zmodem 活动路径不可再使用；共享 target 和正常包保持。
随后 Parfit 对当前精确 CI 限定 APPROVE、P0–P3 全 0，已关闭；
[8 成员复核补充](testing/records/2026-11-16-windows-fixture-publication-evidence/ci-completion/independent-review/README.md)
保存原报告和旧派生结果，本批副本再回收 98 文件／7,481,351 字节；
共 169 文件／16,582,250 字节，正常包六文件前后摘要与共享 target 目录身份不变。
活动目录现保留正常包及最后交付命令的工装／回执，r3 与其他解析器只从封存取用；
CI watcher 已实际 reap，没有活动 Cargo 或自有 reviewer。
没有新建用户 chat、worktree、分支或容器。

UDP当前正常包为 `work/darwin-udp-20261117/package-r1/KeelShell.app`，
绑定工程 `1670329` 与729输入；交付／CI工装已封存回收，
当前用上述监控修复交付目录中的必要helper接续。
本批199成员和上一CI27成员均从上述工程的Git blob逐项核对后，
回收235文件／70,913,865字节，含已被精确正常包替代的提交前包及下载手册；
共享target目录身份和当前正常包六文件摘要前后相等。
旧 `work/editor-backup-ci-20261117/` 已回收，主证据和旧CI终态从docs封存读取。

上一发布批的原生 inventory 明确报告 Mac 锁屏且自动解锁失败；GUI 操作／截图 0。
该批正常包原位于 `work/windows-fixture-publication-integration-20261116/package-r1/KeelShell.app`，
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
