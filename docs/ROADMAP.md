# 当前开发路线 — 2026-11-17

本页只维护当前路线与接续顺序；完整产品仍在开发，FILE-05/M4 未整体完成。
先读[当前交接](HANDOFF.md)，再按[能力台账](product/CAPABILITIES.md)和各批精确记录接续。
[2026-11-16 路线原文归档](ROADMAP_HISTORY_2026-11-16.md)逐字节保留改写前的 1,311 行、98,769 字节。
历史“未推送／待评审”、旧计数和旧 HOLD 只描述各自时点；当前状态以最新授权和精确回执为准。

## 产品与工程约束

- 只管理远程 SSH，使用真实原生 GPUI Kit，覆盖 macOS、Windows、Linux；本地终端、RDP、串口不在范围。
- 保持 app/UI、core/领域存储、session/传输、AI 分层；阻塞文件、SSH/SFTP 和模型 I/O 离开 UI 线程。
- 保留 Frost「霜白」、Graphite「曜石」、Lavender「雾紫」及 System/Light/Dark；
  新主题只扩展 registry、Palette、组件形状与 chrome token，业务视图不按 theme ID 分支。
- AI 上下文发送与每项命令／文件动作分别显式审核，模型不能自主执行；host key 变化 fail closed。
- 凭据默认短暂驻留；显式持久化使用 OS 凭据库或认证加密 vault，普通配置与日志不保存凭据。
- 所有直接 registry 依赖要求仅用 `x.y`，Cargo.lock 保留 patch 解析；不 pin Rust 工具链、不设 `rust-version`，
  默认使用当前开发环境／CI 提供的受支持工具链，本机版本观察不转成产品 MSRV。
- 代码门禁、非作者复核、真实联调、原生桌面和发行安装分别验收；编译、模拟、headless 不代签 native。
- 共享 target 同时只允许一个构建任务；切换源码归属遵循[共享 target 合同](testing/SHARED_TARGET.md)。

## 当前交付快照

当前编辑器备份完整读回批的 30 session／5 app 新例、标准 OpenSSH 18 项已通过。
长状态详情移入／滚动 P2 已修复，两位新非作者追加复核均 APPROVE、P0–P3 全 0；
源码复核只签静态差分，功能复核以父级 prebuilt 独立执行 app/editor `88/0/0`。
三位自有 agent 已关闭。727 输入冻结后一次完整门禁 `0/reaped/701.469s`：
普通 Rust `3307/0/52 ignored`、app `1372/0/2`、14 rustdoc、Python293/2 skipped，
helper 另列 30；准确 app1374 库存与逐名执行相等，35 新例均 ok，输入前后未变。
正常 release build／包结构通过，未启动、未安装，原生仍待验收。
工程 `f810f674c57b2c25dac5b987d77bf38656e3af56` 已普通推送、三方一致，
206成员／727输入 Git 绑定与精确来源包通过。
其 Quality `43929740313` 的 Windows 测试 helper 漏新错误 variant 而编译失败，
filesystem70／archive41先通过，完整 gate未开始。
一行测试分支修订的 r5 其余726输入不变；限定 gate `0/reaped/26.866s`，
session343库存与执行相等、339/0/4、新30项ok，fmt／严格 Clippy／策略通过。
Banach新静态窄复核APPROVE、P0–P3全0，独立编译／执行0，四位自有agent均关闭；
修复后的精确 Windows CI 另验。242文件副本清理及目录恢复诊断
完整保留，不把 r4 宿主完整成绩改签 r5；
见[备份记录](testing/records/2026-11-17-editor-backup-readback.md)。

2026-11-17 上一功能批为远端 macOS 账户可见 TCP LISTEN 列表，设计见
[ADR 0152](adr/0152-darwin-tcp-listeners.md)。724 输入冻结后，一次完整宿主 gate
`0/reaped/794.954s`，普通 Rust `3272/0/51 ignored`、app `1367/0/2`、14 rustdoc；
293 项 Python／2 skipped，archive helper 另列 30。1369 app 独立库存和逐名执行
相等，26 新例全部 ok；两位非作者限定 APPROVE、P0–P3 全 0。
真实 Darwin SSH `9/0/0`、Linux Podman 恢复 `2/0/0`，自有运行资源已清理。
正常 macOS build／包结构通过，桌面未启动；原失败及最新锁屏事实保持，
见[本批记录](testing/records/2026-11-17-darwin-tcp-listeners.md)。
依赖 37 包／113 声明官方复查通过，manifest／lock 未改。
工程 `bd198a8ef2578ea9b96e0a603cd705bb2eadca13` 已普通推送、三方一致，
255 成员／724 输入精确 Git 绑定 `0/reaped/6.972s`、零 mismatch。
Quality `43916079699` 已三平台 completed/success。Windows／macOS／Ubuntu
普通 Rust `3259/0/49`、`3272/0/51`、`3265/0/52`，app `1356/0/2`、
`1367/0/2`、`1368/0/2`，各 14 rustdoc；Python 293 项分别 34／2／2 skipped，
macOS／Ubuntu 标准 OpenSSH success。31 成员
[精确 CI 终态](testing/records/2026-11-17-darwin-tcp-listeners-evidence/ci-completion/README.md)
及 724 Git 输入绑定通过，不代签当前编辑器工作差分。
精确正常包／结构检查通过；301 文件／88,633,128 字节自有副本已按 Git 原件回收，
两个保留正常包和共享 target 身份不变。

上一隧道工程 `6f28f5c` 的 `43901863960` 已三平台 completed/success，
普通 Rust Windows／macOS／Ubuntu 为 `3233/0/46`、`3246/0/48`、`3239/0/49`，
app 为 `1351/0/2`、`1362/0/2`、`1363/0/2`；各 14 rustdoc，Python 293 项，
分别 34／2／2 skipped。macOS／Ubuntu OpenSSH step success；
[25 成员终态](testing/records/2026-11-16-tunnel-configuration-reload-evidence/ci-completion/README.md)
与原 721 输入精确 Git 绑定通过，不移签为当前源码。

以下为此前 2026-11-16 的精确快照；历史终态保留在对应批次。

- 该批工程源码 `ee3ad7320e90f40c9d777c500543cc6e909d0484` 已普通推送、
  本地／tracking／远端三方一致，读回干净。25 成员／719 输入精确 Git 绑定通过。
  新 Quality `43880245383` 已 completed/success；Windows `133343901451`、
  macOS `133343901229`、Ubuntu `133343901650` 的完整 gate 均 success，
  macOS／Ubuntu 真实 OpenSSH 均 success，watch `0/reaped/1893.751s`。
  三平台普通 Rust 分别 `3212/0/46`、`3225/0/48`、`3218/0/49`，
  app 分别 `1330/0/2`、`1341/0/2`、`1342/0/2`，各 14 项 rustdoc；
  Python 均 293 run，Windows 34 skipped、另两平台 2 skipped。
  Windows 七类真实 tree 和 native Job barrier 实际终态通过，保持 exit7、原句柄与 ACK。
  [36 成员终态](testing/records/2026-11-16-windows-fixture-publication-evidence/ci-completion/README.md)
  重新派生并精确源码绑定通过 `0/reaped/6.699s`，Parfit 独立窄证据复核 APPROVE、P0–P3 全 0，
  已关闭；运行副本和 reviewer 原件清理有逐项 Git 绑定。
  CI app 只有运行声明数／唯一逐名结果，Python 只有完整 discovery 计数；不补写独立库存或单例回执。
- 原 `95813a76f0ec9b59fd043cd982e33cdc01dd69f8` 的 Quality `43873806902`
  已 completed/failure，未取消／重跑。Windows filesystem `70/0/0`、codec `41/0/0`、
  session `329/0/3 ignored`，332 名独立编译库存逐项相等；Zmodem 为 `43/0/2 ignored`。
  完整 gate 停在 Python 身份读取：290 run／1 failure／34 skipped，全量 Rust 未运行。
  macOS／Ubuntu 完整 gate 和真实 OpenSSH 均 success，普通 Rust 分别
  `3225/0/48 ignored`、`3218/0/49 ignored`，各 14 项 rustdoc；
  app 分别 `1341/0/2`、`1342/0/2`。
  CI app 只有运行声明数／唯一结果核对，宿主才有独立 `--list`，不混同。
  [21 成员终态补充](testing/records/2026-11-16-windows-zmodem-test-contract-evidence/ci-completion/README.md)
  保留原件、r2 诊断与 r3 派生，验证 `0/reaped/8.131s`、测试重放 0。
- 工程源码 `01f6f6bd8272303fe049bc7b52976afefdb44c37`、文档 `b7d6e92` 已推送，
  三方一致、读回干净。精确 Quality `43867112056` 已 completed/failure：
  Windows filesystem `70/0/0`、core archive `41/0/0`，session library `327/2/3 ignored`，
  332 个编译库存名与执行逐项相等；Zmodem 是其中 `41/2/2 ignored`，完整 gate 未运行。
  macOS／Ubuntu 完整 gate 与真实 OpenSSH 均 success，app 为 `1341/0/2`、`1342/0/2`。
  两平台 core archive 均 `41/0/0`；r2 只修正同行 stderr 导致的派生摘要误归属，原件保持。
- 此前文档提交 `2f29744a2615393bcc5bedb81cd2c425e7bc6372` 已读回本地、tracking、
  远端三方一致，读回时工作区干净。
- 前批源码 `f094de6` 的精确 Quality `43859165104 / attempt 1`，Windows job `133271138956`
  的 filesystem `70/0/0`、archive backend `29/0/1 ignored`，30 名库存／执行逐项相等，
  app `1330/0/2 ignored`，原四项解包／续传失败全部逐名通过。
  后续 core archive `39/1/0`，私有权限重编码断言失败，剩余 session gate 未运行；
  macOS `133271139049`、Ubuntu `133271138662` 的完整 gate 和真实 OpenSSH 均 success。
  整个 run 已终态 failure，原件全部收取；不签完整 Windows gate 或桌面。
- 本轮官方依赖查询实际 `0/reaped/9.586s`，37 包／113 声明全部通过，未改 manifest/lock；
  报告完成于 `2026-11-16T11:51:26Z`，见[依赖与文档维护记录](testing/records/2026-11-16-route-and-dependency-followup.md)；
  历史 UNKNOWN 原件保持。
- 后续 ZIP 修订统一私有权限元数据并提前 Windows codec／完整 session library 预检；
  宿主 scoped gate `0/reaped/51.622s`，342 passed／4 ignored，346 名逐项相等、
  719 输入零漂移；两项 DOS 负例均精确失败，恢复后重编译通过。
  新非作者限定批准、精确 Windows 41 项通过见[本批记录](testing/records/2026-11-16-portable-zip-metadata.md)。
- 当前只修订两项 Windows Zmodem 测试契约与 CI 库存名，生产改动 0、其余 43 块不变；
  宿主完整门禁 `0/reaped/723.521s`，普通 Rust `3225/0/48 ignored`、rustdoc `14/0/0`，
  app `1341/0/2 ignored` 且 1343 名逐项相等；Python discovery 290／1 skipped，
  archive helper 单列 30 项。Turing 静态 APPROVE、P0–P3 全 0，新断言随后已在精确 Windows CI 通过，
  见[测试契约记录](testing/records/2026-11-16-windows-zmodem-test-contract.md)。
- 接续仅修订 Python 工装的发布完成标记，保持身份、ACK、短预算及失败传播；
  其余 718 输入与 958 相等。限定 gate `0/reaped/15.369s`，Python 293 run／2 skipped、
  fmt／严格 Clippy／x.y 通过，两项受控负例精确失败；Boyle 限定 APPROVE、P0–P3 全 0，
  独立专项 72 run／70 passed／2 skipped、退出 0；
  25 成员原件已随当前工程推送；当前精确 Windows／macOS／Ubuntu 完整 gate 现已通过，
  见[发布屏障记录](testing/records/2026-11-16-windows-fixture-publication.md)。
- 当前正常 macOS arm64 包绑定 `ee3ad73` 与 719 冻结；release build
  `0/reaped/150.274s`，包结构／minos 15.0 检查通过；未启动、安装、签名或公证。
  [26 成员交付补充](testing/records/2026-11-16-windows-fixture-publication-evidence/delivery/README.md)
  保存源码 push、三方读回、包文件摘要、旧 CI 封存验证与 4 文件清理原件，不含二进制／活动 watch。
- 英文工具栏修订已随 `8c07d61` 推送并有正常包；Mac 锁屏使 GUI／截图均为 0，原生 P2 仍开放。
  包身份、首轮失败和限定几何结果见[工具栏记录](testing/records/2026-11-16-workbench-fit-and-file-diagnostics.md)。

## 接续顺序

0. 当前编辑器备份批 `f810f67` 已推送，精确 Windows CI 的测试 helper 编译错误
   已补一行明确分支，限定宿主门禁和新静态窄复核通过；封存并及时普通提交推送，
   跟进新精确 Windows CI，不重复未变宿主完整测试。
   Darwin TCP 功能的专项、独立复核、真实 SSH 与一次最终完整门禁均已通过；
   工程 `bd198a8` 已普通推送并三方读回，`43916079699` 精确三平台 CI 已全部成功。
   上一隧道源码的三平台 CI 已全部成功；不重复宿主全量或重放已成功 CI。
   两批原生桌面、完整产品及六目标发行仍待验。

1. `ee3ad73` 的 `43880245383` 三平台完整 gate、Windows 293 项 Python／七类真实 tree／Job barrier，
   以及 macOS／Ubuntu OpenSSH 均已收齐终态，证据窄复核批准、副本清理完成；
   后续继续下列产品／桌面／发行验收。
   旧 958 失败保持，不重放当前成功 CI 或未变 Rust 输入的宿主完整测试。
2. 源码质量门禁通过后继续处理产品验收缺口；已通过的四项 app 范围保持，
   不清空 UNKNOWN/quarantine、不重放不确定写入、不放宽审核、权限、完整字节和发布后重验。
3. 桌面可用后，以保留正常包的精确身份复核英文工具栏 P2，再补主题、尺寸、双语与完整键盘流程；
   当前保持锁屏阻塞，不把 headless 几何或旧包观察改签为新原生通过。
4. 后续功能批次按[阶段集中验证](testing/STRATEGY.md#阶段集中验证)：先集成、专项与新非作者复核，
   关闭已知发现后冻结输入，再做一次最终完整门禁；已完成批次按最新授权普通提交／推送并读回三方 SHA。

## FILE-05 / M4：高级传输仍开放

- 已实现[客户端 ZIP/tar.gz 归档](product/ARCHIVE_TRANSFERS.md)、有界 PAX、
  [两次审核的服务端打包](product/SERVER_ARCHIVES.md)、[PATH／版本与流式自测](product/ARCHIVE_TOOL_INSPECTION.md)，
  以及现有 SSH PTY 上的[审核式 Zmodem](product/ZMODEM.md)；源码实现与限定门禁不等于整阶段验收。
- 独立 peer 的直接 PTY strict `OO` 兼容保持 `HOLD/PARTIAL`：严格 `Finished` 必须实际收到 `OO`。
  缺握手警告、逐文件已验证保存、peer 退出 0、pipe relay 对照分别记录，均不能补签严格完成；
  继续独立 peer、真实 SSH PTY、取消／尾部恢复及同一 shell 新输入的可重复矩阵，见[收尾记录](testing/records/2026-11-15-bounded-zmodem-farewell.md)。
- Windows 来源／私有 staging／安全发布后端与 UI 已接入，前批后端预检和完整 app 通过；
  新 ZIP codec 41 项及 session 两项祖先替换断言已通过；原 958 完整 gate 的 Python 工装失败保持。
  发布标记修订已复核、封存、推送，当前 ee3 三平台完整 gate 已通过，原生桌面仍待验。
  以[根目录发布记录](testing/records/2026-11-16-windows-archive-root-publication.md)接续，不回退成“Windows 从未执行”。
- 既有 Linux arm64 本地后端／codec 专项只覆盖原冻结；不代签 Linux x64、GUI、独立 peer 或当前输入，
  见[本地后端记录](testing/records/2026-11-15-windows-local-backend.md)。更广远端 Linux 权限与旧 Python runtime 仍需覆盖。
- 用户自有 SSH 中继和审核式路径比较已有实现，真实 IP 延迟／丢包实验仅证明受控场景；
  用户中继长流程、跨平台和可重复收益仍待验，不把一次吞吐差异写成普遍加速，见[中继指南](product/SSH_RELAY.md)。

## 其余未完成产品范围

下表列当前缺口；已实现能力、历史复核和各自限定证据继续从对应指南／记录查询。

| 优先级 | 范围 | 当前边界与下一步 |
| --- | --- | --- |
| P1 | 工作区 | 分屏、逐叶标签、显式保存／恢复、独立窗口与窗口切换器已有实现；继续多 OS 窗口、外框、完整关闭／重连审核和 worker 回收原生矩阵，见[窗口指南](product/WORKSPACE_WINDOW_SWITCHER.md)。 |
| P1 | 外部 AI／MCP | API／受控 CLI Ask、逐步审核 Agent、对外 MCP 已实现；继续供应商账户、真实外部客户端和文件提案闭环、撤权／重连／取消；CLI 登录身份复用启用前仍需请求前隔离证明，见[能力计划](product/DESIGN_AND_AGENT_PLAN.md)。 |
| P2 | 日常文件与编辑器 | UTF-16 LE/BE、五种显式旧编码、审核保存／备份／合并／读回和键盘已有实现；继续更多编码边界、原生文件管理器拖放、完整键盘／辅助技术和其他平台，保持原字节与备份合同，见[文本格式](product/REMOTE_TEXT_FORMAT.md)及[文件键盘](product/FILE_BROWSER_KEYBOARD.md)。 |
| P2 | 监控与网络 | 按项能力、缓存、Linux 恢复、远端 macOS 只读资源及账户可见 TCP 监听已有实现；继续更广工具缺失后恢复、真实错误／取消、ICMP 与诊断完整矩阵和远端 Windows 适配；Darwin UDP／主动探测／终止及不支持指标保持未知，见[监听记录](testing/records/2026-11-17-darwin-tcp-listeners.md)与[资源记录](testing/records/2026-11-16-host-monitor.md)。 |
| P2 | 持久化隧道 | 命名规则、三方向转发、流量、审核／停止及运行快照显式恢复已有实现；继续完整重连／管理、owner 失效、真实远端和跨平台验收，见[隧道指南](product/NAMED_TUNNEL_RULE_MANAGEMENT.md)。 |
| P2 | 配置恢复／同步 | 审核恢复、回退与认证加密目录同步已有实现；跨机器挂载、断电耐久性、最终一致同步器适用性及约束仍待验，不把原子发布专项扩成这些承诺，见[同步指南](product/PROFILE_SYNC.md)和[原子发布记录](testing/records/2026-11-14-profile-sync-atomic-publish.md)。 |
| P3 | AI 知识复用 | 本地分类、显式 Ask、审核式批量管理与命令片段草稿已有实现；跨设备同步和更多复用待设计，不恢复旧 SSH 权限、不自动执行 runbook，见[知识指南](product/AI_KNOWLEDGE_LIBRARY.md)。 |

## 跨平台原生与发行矩阵

- macOS、Windows、Linux 的 x64／arm64 六目标按[发行合同](RELEASING.md)逐项核验，
  CI runner、编译或包结构成功不等于实际桌面；Linux Wayland／X11 与运行库约束单独记录。
- 三主题、明暗／系统、中文／英文、宽窗／最小及紧凑窗口、焦点／Tab／Shift+Tab、
  IME／CJK／RTL、VoiceOver／NVDA 等辅助技术和像素对比度均需原生证据；完整产品验收继续开放。
- 真实 SSH/SFTP、Podman 长流程、取消／重连／未知结果及多窗口不能由 loopback 或短验统包。
- 六目标包、主程序／MCP 架构和摘要、精确源码来源、签名／公证、权限、隔离安装、升级／回滚及重启均待完整发行验收；
  [自动更新](product/AUTOMATIC_UPDATES.md)已有实现，真实 release 载荷下载和安装目录替换仍需单独验收。
  标签发布授权不包含覆盖安装现有应用或上传遥测。

## 精确证据入口

计数只在绑定的源码／输入／平台范围内有效；历史记录中的原失败、HOLD、UNKNOWN 和签署边界保持。

| 范围 | 记录与边界 |
| --- | --- |
| 当前 Windows 发布／Quality | [根目录发布终态](testing/records/2026-11-16-windows-archive-root-publication.md#精确-ci-终态)、[ZIP 终态](testing/records/2026-11-16-portable-zip-metadata.md#精确-ci-终态补充)、[Zmodem 测试契约](testing/records/2026-11-16-windows-zmodem-test-contract.md)、[工装发布屏障](testing/records/2026-11-16-windows-fixture-publication.md)；原四项 app、新 codec 与 session 修订按各自精确源码通过，当前 ee3 三平台完整 gate 及 macOS／Ubuntu OpenSSH 已成功，桌面／发行仍开放。 |
| Kit 0.10 与三主题 | [指定源码 CI／限定原生](testing/records/2026-11-16-kit010-native-ci.md)；旧包观察不代签后续源码或英文工具栏 P2。 |
| 文件列表与命令框焦点 | [列表键盘](testing/records/2026-11-16-file-browser-keyboard.md)、[双向焦点补验](testing/records/2026-11-16-archive-diagnostics-and-native-focus.md)；双侧各一文件的 Frost 限定路径不等于完整 IME／辅助技术。 |
| 编辑器原生 | [键盘及 UTF-16／GBK](testing/records/2026-11-16-editor-keyboard-native.md)；Frost 仅限定片段，Graphite HOLD、Lavender NOT_RUN，其他平台仍开放。 |
| 路径与真实 IP 实验 | [SSH 路径比较](testing/records/2026-11-16-ssh-path-comparison.md)、[原生／netem](testing/records/2026-11-16-native-network.md)；受控样本不证明生产收益。 |
| 隧道恢复／审核式片段 | [owner 与恢复卡片](testing/records/2026-11-14-named-tunnel-owner-and-recovery-card.md)、[AI 片段保存](testing/records/2026-11-14-ai-reviewed-snippet-save.md)；源码批准不代签完整原生或最终源码全量门禁。 |

新需求记入 `docs/product/`，设计决定记入 `docs/adr/`，实际结果与原件记入 `docs/testing/`。
本页保持精简当前路线，历史批次不再整段追加；交接入口与精确 CI 记录同步维护。
