# KeelShell 当前交接 — 2026-11-05

本入口供后续 AI 会话接续；日期按 Asia/Shanghai。先读 [AGENTS.md](../AGENTS.md)，再按下列链接读取负责该范围的文档和证据。
最新用户要求优先于历史范围；源码、工程测试、受控原生、平台 CI、公开发布和安装分别验收。
完整旧稿见[历史快照](history/2026-11-05-handoff-before-theme-ci-readback.md)，其中“当前”、暂停及删除指令只属于当时，不构成新的操作授权。

## 本轮文件工作区与渲染栈 — 源码与三平台 CI 通过

当前切片为浏览优先的文件工作区，基线 `3a1473e241263f0b531af16e4e1dd8fd46fc65b0`。
默认收起文件操作和传输设置，保留真实列表、常驻批量入口与队列入口；
两行导航、紧凑审核路径和隐藏输入焦点已实现。三主题仍共用现有目录与语义 token。
需求、方案和每轮失败/通过分别见[文件指南](product/FILES_WORKSPACE.md)、
[ADR 0097](adr/0097-file-workbench-disclosures.md)、
[验证记录](testing/records/2026-11-05-native-file-workbench.md)。

最终 `full-gate-r3` 已退出 0并回收：**2,036 ordinary Rust passed / 23 ignored**，
含 app 848 / 2 ignored；10 rustdoc（一项仅编译），Python 运行 128 项 / 1 skipped，
格式、严格 Clippy、依赖策略和额外进程控制器通过。**490 个输入**
在最终门禁、原生构建和捕获后检查中相同。
最终包程序 SHA-256 为 `132881a279779de29f813ecad5c30276bfa5b3387d66b962304ebbd7ce80a668`，
已验霜白浅色、曜石深色、雾紫浅深中文与雾紫深色英文，
草稿保持、900×900 英文审核/取消、双向真实 SFTP 内容摘要和完成队列 0→1→2。
前一布局包的三主题完整浅深画面单独保留，不能补齐最终包矩阵。
最终包最小高度缩放未成功。后续同包原生方向对照已确认：上传完成后，
同一点 `down` 在顶部无位移，`up` 使文件行和列移动，再次 `down` 返回。
保存的原始截图证实可逆的纵向及横向位移；接收事件字段和历史唯一原因仍 UNKNOWN，
不能据此认定物理触控板方向正确或关闭多轴交互问题。
独立生产 Workspace 的真实上传后像素/行滚轮在 900×900、900×580 均通过，
已成为永久回归；没有修改生产滚动实现，受控事件不代表 OS 滚动验收。
手工冲突焦点的初审 P2 已由 reviewer 撤回为 WITHDRAWN_UNPROVEN；
测试在旧 guard 下本来就通过，不能称为复现后的修复。

基线的精确 Quality **41891939556** 已读回 failure：
Linux/Windows 测试栈溢出；macOS runner ID 0、未执行。
渲染构造已分成保持调用边界的辅助函数，测得 `render_workspace` 入口分配
**1,101,856 → 62,208 字节**，原两项场景在请求 2 MiB 栈时通过。
1 MiB 的其它渲染帧故障与 512 KiB 的控件转换故障分别保留，
没有提高 CI 栈预算；历史平台失败的确切故障指令/唯一根因仍未知。
非作者最终组合复核为 **APPROVE_SCOPED_CHANGE，P0/P1/P2=0**，
Hallmark 对上述捕获范围给出 ship；精确提交/CI 读回见本轮记录及
`work/native-file-workbench-20261105/delivery-readback.json`。

功能提交 **`5d676b97683af3e5060c448fe9f5f61d81bf38b9`** 已合入并推送 main。
其精确 Quality **41907439739** 已读回 **success**：
macOS、Linux、Windows 均通过，app 分别为 **848 / 848 / 839 passed**，
各 2 ignored。三平台原始日志均记录此前两项失败场景和两个新增上传后滚轮测试通过；
macOS/Linux 的真实 OpenSSH 互操作步骤通过，Windows 按工作流不运行该步骤。
这些结果关闭本提交的源码 CI 缺口，不关闭 Windows/Linux 桌面、
完整原生矩阵或历史失败的唯一根因。原始日志及摘要位于
`work/native-file-workbench-20261105/current-ci/final-readback.json`。
后续只补验收文档；490 个构建/门禁输入与该功能提交保持一致。

以下原生导航、主题与管理记录属于已经集成的前轮证据；不能覆盖上方新候选状态。

## 已接受的产品与主题方向

- 真实 Rust / GPUI Kit 原生应用，目标 macOS、Windows、Linux；只管理远程 SSH。本地文件浏览仅服务于 SFTP，本地 AI 进程是推理入口；本地终端、远程桌面和串口不属于产品目标。
- 默认简体中文并维护英文、白底跨平台图标和专业工作区。真实会话、数据、操作与验收必须可追溯；演示、编译、受控 GPUI 或模拟均不代表原生通过。
- 用户已决定 **霜白 Frost、曜石 Graphite、雾紫 Lavender 三套全部内置并可继续扩展**，无需再要求选一套。主题家族 `theme_family` 与 System / Light / Dark 独立，工作区密度和面板布局另行设计。
- 原生主题注册目录集中稳定 ID、双语元数据、推荐外观、浅深 Palette、组件 token 和终端默认色；新增主题走 `ThemeDefinition` / `registry()`，业务视图不按主题 ID 分支。自定义文件导入、下载或插件机制尚未提供。
- 主题通过后台持久化成功后才应用，保留会话和草稿；旧配置默认 Frost，有效但未知的 ID 原样保留并回退显示。终端显式 ANSI/truecolor/OSC 颜色保持；切换主题不发送 AI 上下文。
- HTML 方向已用于设计探索；连接/主机导航、独立会话栏、紧凑补全与浏览优先文件工具区已迁入原生。增加主题数量后需另验选择器布局，不能外推当前三项结果。

主题细节见[使用与扩展](product/THEME_PREFERENCES.md)、[原生设计](design/NATIVE_THEMES.md)、[ADR 0095](adr/0095-extensible-native-theme-families.md)。
完整目标和差距由[能力清单](product/CAPABILITIES.md)、[远程要求](research/remote-ssh-requirements.md)、[路线图](ROADMAP.md)维护；归档/Zmodem、可验证的中继加速、跨设备同步等长期目标仍在，不因当前切片而删除。

## 前轮已集成的源码

本入口随 **原生工作区导航与会话层级** 功能提交维护，基线为
`4f937281850464c7d52611e7d4df8bbc46f7d5c8`。本次功能提交的精确 SHA、推送及 CI
读回保存在 `work/native-workbench-20261105/delivery-readback.json`；提交后的外部结果
不能从本机门禁推断。

- 全局操作与会话标签分层，左侧连接按实际文件夹排列，支持搜索、新建、管理及已连接会话切换；
  主机信息复用原监控实体。点击重查 UUID，修改目标/跳板/信任绑定后不复用旧 SSH。
- 导航/AI 在窄窗口互相让出空间，保留实体和草稿；独立补全目录、准确审核目标、
  64px 多行命令和工具页选择继续保持。侧栏及工具页提供 AX toggle 状态。
- 弹窗期间隐藏的导航不接收返回焦点；后续按键落在惰性工作区，不回退到终端。
  本轮不增加自动执行或 AI 发送。

见[工作区指南](product/NATIVE_WORKBENCH.md)、[ADR 0096](adr/0096-native-workbench-navigation.md)
及[本轮记录](testing/records/2026-11-05-native-workbench.md)。

以下是本次基线之前的主题与管理功能来源：

功能组合已提交推送为 `c36687126312191191236f1e9c8840abb7b4d808`，包括三主题、知识库管理、NET 命名规则管理及依赖升级。
SFTP 修复源码基线为 **`dc94e0e03782ba3d115e784c72823ee0f082b1b2`**；父任务保存了远端 main 同 SHA 的读回回执。
该增量只改 SFTP 测试夹具、回归及文档：独立服务器使用独立主机密钥，同服务器重连保留身份；真实同一绑定端点反例验证碰撞机制，准备写入前建立清理 guard，并覆盖取消与展开。
生产隔离规则未改；功能提交之后的生产交付输入保持相同，Rust 测试文件确实变化，不能称整个 Rust 源树不变。

| 已集成范围 | 当前边界与负责文档 |
| --- | --- |
| AI-04 本地知识库：列表、字面搜索、单条审核编辑/删除、后台 StateStore 保存 | 来源身份保持；Conflict/Unknown 后显式 reload 并重新审核，不重放旧 candidate。知识上下文模型发送尚未完成。见[知识库指南](product/AI_KNOWLEDGE_LIBRARY.md)、[本轮记录](testing/records/2026-11-04-ai-knowledge-library.md)。 |
| NET-01：saved connection 绑定的 local/remote/dynamic executor，命名规则新增/编辑/删除及独立停止审核 | 运行 owner/流量属于原 live session；ephemeral quick connect 不能运行 named rule。真实 cancel 拒绝已有回归，真实远端超时只有源码复核；运行状态持久化、重启恢复、重连 owner 全矩阵及原生仍开放。见[NET 记录](testing/records/2026-11-04-net-01-rule-management.md)。 |

## 前轮工程门禁、审查与 CI

原生工作区修后完整 `scripts/check.py`（`full-gate-r2`）退出 0、进程已回收：
普通 Rust **2,026 passed / 23 ignored**（含 app 838 / 2 ignored）、rustdoc 10 passed
（一项仅编译）、Python 128 / 1 skipped，格式、严格 Clippy、依赖策略及额外进程控制器通过。
485 个源码/脚本/打包输入在最终门禁、最终原生构建与收尾核对中相同。
非作者 Einstein 初审两项 P2 由相同测试的失败/通过对照修复，源码再审 P0/P1/P2=0；
最终候选连同文档和按包区分的原生证据为 APPROVE，P0/P1/P2=0，
Hallmark 对实际捕获范围为 ship。复核范围见本轮记录，不能把读取执行日志说成 reviewer 重跑测试。

工作区基线 `4f937281` 的 Quality **41873553476** 已读回终态 failure：
Linux/Windows 成功，macOS 因未获托管 runner 而取消，runner ID 0、步骤为空；
没有执行 macOS CI。此为基线结果，本次功能提交的精确 CI 另查交付回执。

下面保留之前 SFTP 修复的独立门禁与失败历史：

`dc94e0e` 对应修后完整 `scripts/check.py` 实际退出 0，545 个非 Markdown 输入在门禁前后相同。

| 检查 | 已保存的实际结果 |
| --- | --- |
| 普通 Rust | 2,014 passed / 23 ignored；其中 app 826 passed / 2 ignored，不能重复累加 |
| rustdoc | 10 passed，其中 1 项仅编译 |
| Python | 128 项 / 1 skipped；整体退出 0 |
| 工程策略 | 格式、workspace/all-targets 严格 Clippy、x.y 策略及额外 readiness / observer I/O / small-stack 控制器通过 |
| 独立审查 | Hypatia 最终源码/门禁复核 APPROVE，P0/P1/P2=0；随后精确提交绑定核对全部 545 个非 Markdown blob 与门禁输入一致 |

首候选虽有 2,012 passed / 23 ignored（app 824 / 2 ignored），初审仍为 **HOLD / P2**：准备写入早于清理 guard。
修后门禁与最终审查关闭该 P2；原 HOLD、首候选门禁、旧密钥及禁用 Drop 反例原样保留。目录句柄为零不代表普通文件句柄计数。
reviewer 复核已有控制日志与源码，没有重新运行 Cargo；临时突变过程及缺少逐次突变源码摘要的边界见[SFTP 记录](testing/records/2026-11-04-sftp-fixture-identity.md)。
清理仅在仍能安全检查自有连接时成立，不能检查时保留隔离和错误，不承诺任意失败都回收全局状态。

- 精确 Quality **41863354940**：Linux/Windows **success**，app 分别 826/817 passed（各 2 ignored），三个新增回归、原 MCP/更新器失败项实际通过，Linux OpenSSH 通过。macOS 首次 job `126691000390` 及限定重跑 `126711796757` 均 **cancelled**，runner ID 为 0、步骤为空，annotation 明确说明未分配到托管 runner 及 ARM 容量紧张。两次尝试整体均为 failure；macOS 远端 CI **未执行**，需待 runner 可用后补验。第二次尝试的 Windows/Linux 继承第一次成功，没有重新执行。
- 初期 API 报 queued，但后读 Linux 日志显示已执行；API 观测不能当作没有执行的证明。同期 Actions 服务事件与精确 job 的结果分开记录，见[SFTP 记录](testing/records/2026-11-04-sftp-fixture-identity.md)。
- 上一提交 `7782979351291abfc95966b286f824fca683deb6` 的 Quality **41848787327**：macOS/Windows 成功；Linux 在 UTF-8 种子写入收到 `MutationQuarantined`，未到 OpenSSH 步骤，整个 run 失败。准确隔离 owner/原因 **UNKNOWN**，32 组容量假设仍未排除；身份碰撞反例不能证明历史 CI 的唯一原因。
- 更早 Windows 原生 Job 准备失败与更新器超时的原始原因仍 **UNKNOWN**；后续控制通过不倒填旧原因。原失败和后续成功分别见[Job 记录](testing/records/2026-11-04-native-barrier-diagnostics.md)、[更新器记录](testing/records/2026-11-04-updater-transport-fixture.md)、[Unix 分片记录](testing/records/2026-11-04-unix-fixture-fragment-readiness.md)及[提交回读](testing/records/2026-11-04-theme-delivery-ci.md)。

上述 SFTP 源码 CI 已取得终态，并保留 macOS 外部容量限制。先前交接重写经 Bohr 独立文档审查，P0/P1/P2=0；历史原稿逆向还原字节一致，相对链接检查通过。其文档差异与提交绑定保存在 `work/theme-integration-20261104/handoff-refresh/independent-review/`；后续 `4f937281` 的精确 CI 已在上方读回。

## 原生证据与 OPEN 边界

最新工作区证据按包分别验收：

| 程序 SHA-256 | 实际范围 |
| --- | --- |
| `cb002f6028158209c11c6121f9be8925e8cc7b04dfa7a088f87fcca556d44b1f` | 首包三主题浅深 1440×900；雾紫深色中英文 900×580、AI/导航互换、两条 SSH 会话切换及草稿保持。早于两项 P2 修复。 |
| `ac938e3196a77c66343acc92fe203c8cca7213f547686cd4e1eb1a7e1d98ed5a` | 修后包直接捕获雾紫深色英文 900×900 和霜白浅色中文 1440×900、工具 AX 状态；另从导航搜索打开弹窗，在弹窗内缩至 900×900，返回后按键不进入终端或修改草稿/查询，含可见搜索按键正向对照。 |

最终包的构建输入与本轮最终门禁相同；包构建早于提交，其 manifest 不提供提交身份，
提交绑定另由输入清单核对。18–21 原生捕获读到了错误旧应用，已明确标为无效；
27–30 未成功缩放，不能用文件名替代内容。详细失败、正确截图与实际尺寸见本轮记录。
最终包的 900×580、完整主题×语言×尺寸矩阵仍未在实机重跑。

之前两份主题 macOS 包的证据也必须分别引用，包内程序 SHA-256 如下：

| 程序身份 | 实际观察与保留状态 |
| --- | --- |
| 早期 `6d29c6ae68901147f3686da59624e5537f70e4ed214d1ea1ee3b1ceae2da4624` | 受控 loopback SSH/SFTP 中三主题切换、会话/远端列表/未发送草稿保持及重启恢复主题/语言。二进制已清理，原 manifest、AX、截图和退出回执保留；当前不能重算该二进制摘要。 |
| 最终 `6d92ada1ddeff800bac062599611949ab70899b77fc39aba407da277272898e3` | 包保留且重新核对摘要；只补验主题选择入口、中英文知识字段 AX 及内容保留。未重跑 SSH fixture，未验 NET 或知识库原生增删改。 |

上述历史主题最终包构建早于功能提交，manifest 无 commit/workflow 身份；**源码连续性不等于当前 HEAD 的原生执行**。
具体范围见[主题原生记录](testing/records/2026-11-04-native-themes.md)及[包身份与交付复核](testing/records/2026-11-04-theme-delivery-ci.md)。

以下仍为 **OPEN**，不能由本机门禁或上述限定观察关闭：

- Windows/Linux 桌面、最终候选完整主题×语言×窗口矩阵、OS 外观通知、最终候选最小 OS 窗口几何、IME、VoiceOver；原生文件管理器拖放和其它既有原生缺口继续按[能力清单](product/CAPABILITIES.md)验收。首包三主题浅深默认尺寸画面不能关闭这些范围。
- NET 原生操作、真实远端取消超时；知识库原生修改及完整知识上下文发送；更广泛的远程环境兼容性和完整 AI/MCP 流程。当前受控 OpenSSH CI 不代表全部部署环境。
- 六目标 Release、签名/公证、真实下载、安装/更新/回滚及完整产品目标；本轮没有创建发行标签或覆盖安装。

## AI 与对外 MCP 的固定方向

**MCP 是 KeelShell 向外部智能体提供能力的服务端**；应用内 API / 本地 Claude Code、Codex CLI 是独立推理入口，不增加通用第三方 MCP 客户端。
外部 stdio 伴随程序通过认证桌面 IPC 使用用户授权的会话与工具；八项工具只提供限定读取、待审命令/现有文件替换提案或状态查询，客户端不能自行批准。
关闭、结束或重连原 SSH 会话使旧授权失效；已发出的远端操作仍可能结果未知，不自动重发写操作。AI 输出不能绕过人工审核或自主执行。
上下文捕获、保存和发送均须显式选择；敏感正文不进入模型或知识记录，远端输出是不能提升权限的不可信数据。知识来源是历史事实记录，不保证外部事实仍成立。

完整外部 Codex 业务尚未通过；历史配对调用没有覆盖后续拒绝、原文件再读和撤权，历史七工具证据也不关闭供应商第八工具。
同时活跃的应用内 Agent/MCP 撤权、三平台桌面和真实 provider 分别验收；CLI 当前使用明确 API 身份，订阅登录复用未实现。
见[MCP 接入](product/EXTERNAL_MCP.md)、[授权生命周期](product/MCP_SESSION_LIFETIME.md)、[审核式 Agent](product/REVIEWED_AGENT.md)、[方向复核](testing/records/2026-10-07-mcp-direction-review.md)及[CLI 登录研究](research/local-cli-owned-authentication.md)。

## 工程、依赖与授权规则

- `keelshell-app` 为 UI，`keelshell-core` 为领域/存储，`keelshell-session` 为传输，`keelshell-ai` 为 AI；阻塞 I/O 离开 UI 线程。公共 API 写 rustdoc，使用 typed errors，测试外避免 panic，行为测试使用隔离临时数据与有界等待。
- 直接 registry 要求统一 `x.y`，应用 `Cargo.lock` 保留 patch；**不 pin Rust 工具链**，使用开发环境或 CI 支持的版本。历史 Rust 1.99.1 是执行环境记录，不是项目限制。
- 每轮开发检查官方最新稳定、非撤回直接依赖并保存新证据：`python3 scripts/check_dependencies.py --json-report <新报告.json>`，或标准门禁前执行 `python3 scripts/check.py --fresh-dependencies`；离线标准检查继续可用。
- 最新官方 freshness 开始于 **2026-11-04T19:01:59Z**（上海 11 月 5 日）：104 条声明/使用通过，报告为 `work/native-file-workbench-20261105/dependency-freshness.json`。本轮没有新增或升级依赖；前轮首次报告目录缺失失败亦保留。早前升级和工程来源见[依赖记录](testing/records/2026-11-04-dependencies.md)及[工程来源复核](research/2026-11-04-engineering-refresh.md)。
- 不引入私有 Git 依赖、开发者绝对路径、凭据或生产日志。凭据默认临时；显式保存使用 OS 凭据库或经认证、主密码加密的 vault；主密码及明文凭据不写 profile metadata。主机密钥变化必须拒绝连接。
- 功能提交前完成格式、严格 Clippy、测试、依赖策略、相关原生检查和新的非作者审查；失败证据与未验证范围必须保留。需求/设计/验收分别更新 `docs/product`、`docs/adr`、`docs/testing` 和 `docs/ROADMAP.md`，中英文 README 同步。
- 用 KeelShell 自身能力描述产品，仓库文本、文件名、注释及发行说明不得引入具名专有 SSH 产品比较。既有授权涵盖公开 GitHub、提交推送及标签触发多平台 Releases；不涵盖遥测上传或覆盖安装。

## 优先接续顺序

1. 在同包原生可逆滚动证据上，继续核对接收事件方向及多轴行为，补最终候选主题×语言×窗口矩阵、最小 OS 高度、导航键盘树操作及无障碍证据。浏览优先文件区、连接/主机导航及会话栏已实现，不要重复重做；受控滚轮测试不代替 OS 验收。
2. 推进知识上下文显式选择与模型发送、完整外部客户端流程和 CLI 登录复用；沿用原始会话/来源身份、敏感内容闸门和逐次审核。
3. 补 NET 原生与真实取消超时、重连 owner 全矩阵；运行状态持久化/恢复单独设计，不从已完成的命名规则元数据管理推断。
4. 逐项补三平台桌面、IME/VoiceOver、原生拖放、真实互操作与发布安装验证；按[路线图](ROADMAP.md)继续完整能力目标。文件工作区提交的三平台源码 CI 已通过，保留前次失败，不重复将已闭合源码检查列为待办。

## 资源与证据接续

文件工作区的最终应用和 SSH/SFTP 夹具退出 0、已回收；关闭后的清单确认没有
KeelShell 进程。低栈诊断副本、临时目录和独立滚轮源码副本已按归属清理，
输入、原日志、失败历史和必要截图保留在 `work/native-file-workbench-20261105/`。
保留的最终开发包与旧包清理、提交和 CI 的实际状态由该目录的交付回执维护。

工作区本轮的两次 SSH/SFTP 夹具均正常退出并回收；最终包先后两次运行也已退出回收，
最后一次通过系统关闭按钮结束。一次 Cmd-Q 尝试未退出，保留 AX，不声称该快捷键验证通过。
旧捕获辅助函数误拉起的首包实例已按本轮进程身份清理。只保留最终开发包和必要证据，
资源与提交收尾见 `work/native-workbench-20261105/delivery-readback.json`。
之前主题轮所属 HTML 服务、NET worktree/分支、独立缓存等已清理，其回执仍在原目录。
`dependencies-20261104` 和 `ai-04-knowledge-library` 受管理 worktree 的归档均被拒绝：**“Another active task still uses this worktree”**。
保留这两棵树、对应分支和现有修改，**不得用 shell 删除绕过保护**。后续清理先确认消费者与资源所有权，保留必要忽略材料，再用可恢复归档；不删除共享缓存或其它任务资源。
本轮导航作者工作副本的归档也被管理器拒绝：**“The task's primary checkout cannot be removed.”**
作者已关闭，文件和必要证据已导入；保留该树，不用 shell 绕过保护。

以下路径均相对仓库根目录，属于本地忽略证据，不是公开发行产物：

- `work/native-file-workbench-20261105/`：本轮各次失败/通过检查、490 输入绑定、
  `native/` 中按包分开的原始 JPEG/AX 和真实双向传输读回、
  `independent-review/` 中撤回的初审风险与最终复核、
  `previous-ci/triage/` 中精确平台失败和各栈预算的独立诊断。
- `work/native-workbench-20261105/`：每轮失败/通过门禁、输入清单、最新依赖检查；
  `native/` 保存两包身份、原始 JPEG/AX、无效捕获说明和进程退出回执，
  `independent-review/` 保存本轮 HOLD、修后复核与最终交付绑定。
- `work/theme-integration-20261104/ci/sftp-fixture-identity/p2-closure/`：修后完整门禁、反例、输入清单和退出回执。
- `work/theme-integration-20261104/ci/sftp-fixture-identity/independent-review/`：原 HOLD；其 `p2-closure/final-binding/` 保存最终 APPROVE 与 `committed-review.json` 的精确提交绑定。
- `work/theme-integration-20261104/ci/`：原平台失败与后续 run/job 回读；`final-delivery-review/` 和 `worktree-cleanup.json` 位于 `work/theme-integration-20261104/`，保存包身份和资源清理复核。
- `work/theme-integration-20261104/handoff-refresh/`：本次旧稿原始 blob 字节、哈希、相对链接重定位与文档验证回执。

更多资料从[文档索引](README.md)进入；引用具体记录时核对其提交、包身份、时间及验证范围，历史“当前”段落不能覆盖本入口和后续真实读回。
