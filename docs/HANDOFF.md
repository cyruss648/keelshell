# KeelShell 当前交接 — 2026-11-05

本入口供后续 AI 会话接续；日期按 Asia/Shanghai。先读 [AGENTS.md](../AGENTS.md)，再按下列链接读取负责该范围的文档和证据。
最新用户要求优先于历史范围；源码、工程测试、受控原生、平台 CI、公开发布和安装分别验收。
完整旧稿见[历史快照](history/2026-11-05-handoff-before-theme-ci-readback.md)，其中“当前”、暂停及删除指令只属于当时，不构成新的操作授权。

## 已接受的产品与主题方向

- 真实 Rust / GPUI Kit 原生应用，目标 macOS、Windows、Linux；只管理远程 SSH。本地文件浏览仅服务于 SFTP，本地 AI 进程是推理入口；本地终端、远程桌面和串口不属于产品目标。
- 默认简体中文并维护英文、白底跨平台图标和专业工作区。真实会话、数据、操作与验收必须可追溯；演示、编译、受控 GPUI 或模拟均不代表原生通过。
- 用户已决定 **霜白 Frost、曜石 Graphite、雾紫 Lavender 三套全部内置并可继续扩展**，无需再要求选一套。主题家族 `theme_family` 与 System / Light / Dark 独立，工作区密度和面板布局另行设计。
- 原生主题注册目录集中稳定 ID、双语元数据、推荐外观、浅深 Palette、组件 token 和终端默认色；新增主题走 `ThemeDefinition` / `registry()`，业务视图不按主题 ID 分支。自定义文件导入、下载或插件机制尚未提供。
- 主题通过后台持久化成功后才应用，保留会话和草稿；旧配置默认 Frost，有效但未知的 ID 原样保留并回退显示。终端显式 ANSI/truecolor/OSC 颜色保持；切换主题不发送 AI 上下文。
- HTML 方向已用于设计探索，连接导航和工具区布局尚未全部迁入原生。增加主题数量后需另验选择器布局，不能外推当前三项结果。

主题细节见[使用与扩展](product/THEME_PREFERENCES.md)、[原生设计](design/NATIVE_THEMES.md)、[ADR 0095](adr/0095-extensible-native-theme-families.md)。
完整目标和差距由[能力清单](product/CAPABILITIES.md)、[远程要求](research/remote-ssh-requirements.md)、[路线图](ROADMAP.md)维护；归档/Zmodem、可验证的中继加速、跨设备同步等长期目标仍在，不因当前切片而删除。

## 当前已集成的源码

功能组合已提交推送为 `c36687126312191191236f1e9c8840abb7b4d808`，包括三主题、知识库管理、NET 命名规则管理及依赖升级。
当前已提交并推送的源码基线为 **`dc94e0e03782ba3d115e784c72823ee0f082b1b2`**；父任务保存了远端 main 同 SHA 的读回回执。
本次增量只改 SFTP 测试夹具、回归及文档：独立服务器使用独立主机密钥，同服务器重连保留身份；真实同一绑定端点反例验证碰撞机制，准备写入前建立清理 guard，并覆盖取消与展开。
生产隔离规则未改；功能提交之后的生产交付输入保持相同，Rust 测试文件确实变化，不能称整个 Rust 源树不变。

| 已集成范围 | 当前边界与负责文档 |
| --- | --- |
| AI-04 本地知识库：列表、字面搜索、单条审核编辑/删除、后台 StateStore 保存 | 来源身份保持；Conflict/Unknown 后显式 reload 并重新审核，不重放旧 candidate。知识上下文模型发送尚未完成。见[知识库指南](product/AI_KNOWLEDGE_LIBRARY.md)、[本轮记录](testing/records/2026-11-04-ai-knowledge-library.md)。 |
| NET-01：saved connection 绑定的 local/remote/dynamic executor，命名规则新增/编辑/删除及独立停止审核 | 运行 owner/流量属于原 live session；ephemeral quick connect 不能运行 named rule。真实 cancel 拒绝已有回归，真实远端超时只有源码复核；运行状态持久化、重启恢复、重连 owner 全矩阵及原生仍开放。见[NET 记录](testing/records/2026-11-04-net-01-rule-management.md)。 |

## 最新工程门禁、审查与 CI

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

上述源码 CI 已取得终态，并保留 macOS 外部容量限制。本次交接重写另经 Bohr 独立文档审查，P0/P1/P2=0；历史原稿逆向还原字节一致，相对链接检查通过。文档差异与提交绑定保存在 `work/theme-integration-20261104/handoff-refresh/independent-review/`；文档提交后的精确 CI 另行回读，不从源码或本机结果推断。

## 原生证据与 OPEN 边界

两份 macOS 包的证据必须分别引用，包内程序 SHA-256 如下：

| 程序身份 | 实际观察与保留状态 |
| --- | --- |
| 早期 `6d29c6ae68901147f3686da59624e5537f70e4ed214d1ea1ee3b1ceae2da4624` | 受控 loopback SSH/SFTP 中三主题切换、会话/远端列表/未发送草稿保持及重启恢复主题/语言。二进制已清理，原 manifest、AX、截图和退出回执保留；当前不能重算该二进制摘要。 |
| 最终 `6d92ada1ddeff800bac062599611949ab70899b77fc39aba407da277272898e3` | 包保留且重新核对摘要；只补验主题选择入口、中英文知识字段 AX 及内容保留。未重跑 SSH fixture，未验 NET 或知识库原生增删改。 |

最终包构建早于功能提交，manifest 无 commit/workflow 身份；**源码连续性不等于当前 HEAD 的原生执行**。
具体范围见[主题原生记录](testing/records/2026-11-04-native-themes.md)及[包身份与交付复核](testing/records/2026-11-04-theme-delivery-ci.md)。

以下仍为 **OPEN**，不能由本机门禁或上述限定观察关闭：

- Windows/Linux 桌面、完整三主题×明暗视觉矩阵、OS 外观通知、最小 OS 窗口几何、IME、VoiceOver；原生文件管理器拖放和其它既有原生缺口继续按[能力清单](product/CAPABILITIES.md)验收。
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
- 最后官方 freshness 为 **2026-11-04T11:45:03Z–11:45:19Z**：104 条声明/使用、34 个包、5 个成员通过。后续测试修复未新增依赖；这不是 11 月 5 日的新检查。见[依赖记录](testing/records/2026-11-04-dependencies.md)及[工程来源复核](research/2026-11-04-engineering-refresh.md)。
- 不引入私有 Git 依赖、开发者绝对路径、凭据或生产日志。凭据默认临时；显式保存使用 OS 凭据库或经认证、主密码加密的 vault；主密码及明文凭据不写 profile metadata。主机密钥变化必须拒绝连接。
- 功能提交前完成格式、严格 Clippy、测试、依赖策略、相关原生检查和新的非作者审查；失败证据与未验证范围必须保留。需求/设计/验收分别更新 `docs/product`、`docs/adr`、`docs/testing` 和 `docs/ROADMAP.md`，中英文 README 同步。
- 用 KeelShell 自身能力描述产品，仓库文本、文件名、注释及发行说明不得引入具名专有 SSH 产品比较。既有授权涵盖公开 GitHub、提交推送及标签触发多平台 Releases；不涵盖遥测上传或覆盖安装。

## 优先接续顺序

1. 核对最终交付读回及文档提交的精确 CI；托管 runner 恢复后补 macOS CI，保留两次未启动记录。区分本机通过、Windows/Linux 成功与 macOS CI 未执行，不以反复重跑或修改平台配置掩盖容量限制；其它功能可按下面顺序继续。
2. 按已接受的三主题设计继续连接导航/工具区原生布局迁移，保留会话、草稿、审核和焦点行为，并补完整主题视觉与最小窗口证据。
3. 推进知识上下文显式选择与模型发送、完整外部客户端流程和 CLI 登录复用；沿用原始会话/来源身份、敏感内容闸门和逐次审核。
4. 补 NET 原生与真实取消超时、重连 owner 全矩阵；运行状态持久化/恢复单独设计，不从已完成的命名规则元数据管理推断。
5. 逐项补三平台桌面、IME/VoiceOver、原生拖放、真实互操作与发布安装验证；按[路线图](ROADMAP.md)继续完整能力目标。

## 资源与证据接续

本轮所属原生 app、SSH/SFTP 夹具、HTML 服务及临时控制均已收尾；不用的 NET worktree/分支、独立缓存和旧包已清理。源码 CI 观察进程已退出并回收；后续文档 CI 与审查资源的最终状态见 `work/theme-integration-20261104/delivery-readback.json`，结束后及时关闭。
`dependencies-20261104` 和 `ai-04-knowledge-library` 受管理 worktree 的归档均被拒绝：**“Another active task still uses this worktree”**。
保留这两棵树、对应分支和现有修改，**不得用 shell 删除绕过保护**。后续清理先确认消费者与资源所有权，保留必要忽略材料，再用可恢复归档；不删除共享缓存或其它任务资源。

以下路径均相对仓库根目录，属于本地忽略证据，不是公开发行产物：

- `work/theme-integration-20261104/ci/sftp-fixture-identity/p2-closure/`：修后完整门禁、反例、输入清单和退出回执。
- `work/theme-integration-20261104/ci/sftp-fixture-identity/independent-review/`：原 HOLD；其 `p2-closure/final-binding/` 保存最终 APPROVE 与 `committed-review.json` 的精确提交绑定。
- `work/theme-integration-20261104/ci/`：原平台失败与后续 run/job 回读；`final-delivery-review/` 和 `worktree-cleanup.json` 位于 `work/theme-integration-20261104/`，保存包身份和资源清理复核。
- `work/theme-integration-20261104/handoff-refresh/`：本次旧稿原始 blob 字节、哈希、相对链接重定位与文档验证回执。

更多资料从[文档索引](README.md)进入；引用具体记录时核对其提交、包身份、时间及验证范围，历史“当前”段落不能覆盖本入口和后续真实读回。
