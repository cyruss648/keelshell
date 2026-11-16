# 当前开发路线 — 2026-11-17

完整产品继续开发，FILE-05/M4、原生桌面和六目标发行仍开放。
先读[当前交接](HANDOFF.md)，按[能力台账](product/CAPABILITIES.md)和各批精确记录接续。
改写前原文逐字节保存在[本日路线历史](ROADMAP_HISTORY_2026-11-17.md)；
历史状态不覆盖当前授权或精确源码回执。

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

## 当前批次与接续顺序

1. Windows 远端只读监控：固定 PowerShell 采集、独立身份、资源／进程／端口、按项缓存、
   三主题双语与未知值语义已整合。真实 Darwin SSH 15 项、Linux 恢复 2 项及专项通过；
   独立审查发现的 admission 后撤权与对照漏报均已修复。
   最终 758 输入 r3 的完整门禁已通过，普通 Rust3407/0/59 ignored、app1398/0/2；以[本批记录](testing/records/2026-11-17-windows-read-only-monitor.md)为准，
   两位独立非作者均限定批准，精确库存与冻结通过；普通提交／推送后收取 Quality 和 Windows API workflow 终态。
2. 监控栈修复 `bbd4eaccfa8bba2feb487ff43e0423567428bc82` 的 Quality `43955379234`
   已三平台成功，37 成员封存和 731 输入 Git 绑定通过；原 UDP 栈溢出失败保持。
   [栈修复记录](testing/records/2026-11-17-monitor-render-stack.md)与
   [精确 CI](testing/records/2026-11-17-monitor-render-stack-evidence/ci-completion/README.md)
   不代签当前 Windows 差分。
3. 本次最新原生可用性观察仍为 Mac 锁屏且自动解锁失败，GUI 操作／截图 0。
   桌面可用后，使用当前正常包复核英文工具栏 P2，再覆盖三主题、尺寸、双语与完整键盘流程。
4. 已完成批次及时普通提交／推送并读回 local／tracking／remote SHA；
   新功能按[阶段集中验证](testing/STRATEGY.md#阶段集中验证)集成、专项、新非作者复核和最终完整门禁。
   保留 UNKNOWN／quarantine 和原失败，不重放不确定动作；回收仅限本任务自有且已封存资源。

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
| P2 | 监控与网络 | 按项能力、缓存、Linux 恢复、远端 macOS 只读资源及账户可见 TCP/UDP端口与缓存分页已有实现；UDP专项、真实SSH、最终门禁及两位独立复核通过，原生另验。Windows 只读采集与界面本批已整合，完整 gate／独立复核通过，真实 Windows API 验证接续中；继续真实 Windows SSH、更多工具缺失后恢复、真实错误／取消和 ICMP 诊断矩阵；Darwin主动探测／终止及不支持指标保持未知，见[UDP记录](testing/records/2026-11-17-darwin-udp-sockets.md)、[监听记录](testing/records/2026-11-17-darwin-tcp-listeners.md)与[资源记录](testing/records/2026-11-16-host-monitor.md)。 |
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
