# 开发交接 — 2026-10-03

本仓库已整体迁移到用户指定的项目目录。迁移保留 `.git`、所有已跟踪/未跟踪文件、ignored 构建目录和未提交改动。用户已于 2026-10-03 授权公开 GitHub 仓库、推送和标签发布；当前 remote 为 `https://github.com/cyruss648/keelshell.git`。发布与验证状态见 [发布记录](testing/records/2026-10-03-release.md)。

## 2026-10-04 继续交接（覆盖下方历史状态）

本节记录当前工作区相对于下方历史交接内容的最新状态。后续实现和验证应以本节、`docs/ROADMAP.md`、能力清单和对应测试记录为准；历史章节保留用于追溯，不代表当前未完成项已经关闭。

- 批量任务现在支持受限的逐目标元数据模板：`{{name}}`、`{{host}}`、`{{port}}`、`{{user}}` 和 `{{endpoint}}`。模板只读取已保存路由或一次性会话的非敏感元数据，在本地展开；未知变量和不支持的上下文会在审核前拒绝。审核面板逐目标展示最终命令，确认前不会发起 SSH 请求，确认后按目标绑定执行。审计摘要的命令摘要同时覆盖源文本与逐目标绑定，但仍不保存命令正文、输出、地址或凭据。实现见 `crates/keelshell-core/src/batch_template.rs`、`crates/keelshell-app/src/batch_commands.rs`，设计与证据见 [ADR 0027](adr/0027-reviewed-per-target-batch-templates.md) 和 [测试记录](testing/records/2026-10-04-reviewed-batch-templates.md)。
- 失败的文件或目录传输现在会在同一活动 SSH 会话中保留一个显式恢复提议。点击“检查并续传”只会创建新的只读校验计划，之后仍需用户审阅并确认；它不会自动重放、自动重连、跨会话复用或绕过现有内容校验。只有实际开始过的失败传输可产生提议；列表/规划操作本身不会产生新的提议，且只有失败卡片在非忙碌、无待审核时才能使用既有提议。只读计划不会改写旧失败卡的终态，状态变化后的旧点击会被再次拒绝。实现见 `crates/keelshell-app/src/files.rs`，设计与证据见 [ADR 0026](adr/0026-explicit-transfer-recovery-proposal.md) 和 [测试记录](testing/records/2026-10-04-transfer-recovery.md)。
- AI transport 现在按配置快照显式区分 Chat Completions 与 Responses。设置页可切换协议，已知 `/chat/completions` 与 `/responses` 后缀会同步替换并清除旧临时密钥；Responses 预览使用 `instructions`/`input`，回复只接受 `output_text`，不会启用 tools 或自动操作。协议契约、回环 HTTP 和 GPUI 回归见 [ADR 0029](adr/0029-ai-responses-transport.md) 与 [测试记录](testing/records/2026-10-04-ai-responses.md)。
- 文件工作层新增有界本地目录快照 worker、SFTP 远程元数据快照和纯核心比较引擎。文件面板现在可以显式发起只读“比较目录”，显示一致、变化、仅本地、仅远端和待确认统计，并列出前 100 条相对路径结果；缺失字段标为 `Uncertain`，超过深度/条目/路径边界直接失败。内容哈希、同步计划和差异应用仍未接入。设计与证据见 [ADR 0028](adr/0028-bounded-directory-comparison.md) 和 [测试记录](testing/records/2026-10-04-directory-compare.md)。
- 本轮新增切片在本机完成 `cargo fmt --all -- --check`、严格 Clippy 和完整 workspace 门禁：应用 267 项、核心 69 项、会话库 64 项、批量集成 14 项、SSH loopback 95 项，OpenSSH 外部互操作 6 项因未提供 `KEELSHELL_OPENSSH_*` 环境而忽略，doctest 全部通过。GPUI 还新增中英文 accessibility label 回归，记录见 [可访问名称记录](testing/records/2026-10-04-accessibility-labels.md)。GitHub [Quality 37176372172](https://github.com/cyruss648/keelshell/actions/runs/37176372172) 已在 macOS 26、Ubuntu 24.04、Windows 2025 成功，包含本轮 AI 与目录快照比较改动；流水线证明构建、测试与打包路径，不等同于 Windows/Linux 原生桌面交互验收。
- 仍未关闭的产品差距包括自动传输恢复与并行调度、任务依赖/编排/定时、交互 shell 可编程补全、目录比较的内容哈希/同步计划/差异应用、更丰富的网络协议诊断、Anthropic/Agent 等更多 AI 工作流，以及 Windows/Linux 原生窗口验收、签名/公证和已安装目录更新验收。

## 当前产品要求（优先于旧文档）

1. 名称 KeelShell，Rust + GPUI Kit，跨平台。
2. 仅远程 SSH 管理，不做本地终端、RDP 或串口。目标覆盖连接组织、认证、会话、文件、监控、隧道及进阶远程运维；不能将部分实现视作完成。
3. UI 采用紧凑的远程运维工作区，要求现代、美观、交互清晰：浅色工具栏、左主机监控、中央深色终端、下方命令/文件工具区、独立连接管理器。应用图标必须白底，外部透明，适配各平台。
4. 多语言，默认简体中文，当前支持中文/英文。切换不能丢草稿、会话或在途任务。
5. AI 配置与功能交互参考 DBX。官网与本机设置已查看，详见 `research/dbx-ai-reference.md`。
6. 依赖尽量最新实用版本，所有直接 registry 版本必须 `x.y`；维护锁文件、设计文档、单元/集成测试与原生验收记录。
7. 已只读评估 Reef、reef-template，见 research；参考工程与生命周期模式，不引入私有后端框架。

## 已有本地提交

- `b5ed0ce`：工作区与设计研究基线。
- `4bc6084`：SSH、SFTP、监控、隧道、AI 审阅与第一版原生界面；这是用户纠正范围之前的检查点。
- `6e79fe4`：远程专用/双语/现代工作区、DBX式命名AI配置与取消、白底三平台图标及本地打包；181项测试和Mac受控流程通过。
- `3cd322f`：从剪贴板导入受限 OpenSSH 配置，并加入有界、零持久化的 keyboard-interactive 传输响应；本地整仓门禁通过。
- `3bb67c7`：同步中英文 README 的 OpenSSH 导入能力说明和审阅边界。
- `51c0eac`：OpenSSH 剪贴板导入候选审阅、确认后保存、来源定位、`key=value` 解析及重复/未知指令警告。

截至本交接版本，提交 `51c0eac` 的 GitHub Quality `37155464824` 已在 macOS、Ubuntu、Windows 全部通过；此前 `3cd322f` 的 Quality `37133899213` 与 `3bb67c7` 的 Quality `37134477105` 也均已通过。流水线验证的是代码、测试和打包路径；Windows/Linux 桌面交互仍不等同于本机原生窗口验收。

本阶段完成用户纠正范围后的远程专用工作区、命名 AI 配置与白底图标实现，按本地 Git 保存检查点。不能用早期提交的测试记录代表当前代码，最近验收如下。

## 当前实现与证据

2026-10-04 的 OpenSSH 导入增量已把剪贴板解析改为候选审阅流程：解析支持常见空格和 `key=value` 指令写法，候选连接、跳板、认证类型、来源行与 Include 来源会在确认弹窗中展示；未知/语义敏感/重复指令及 Host 块内 Include 会保留为可定位警告。确认后才写入连接库，取消保持原状态。核心 52 项单元测试、7 项 OpenSSH 集成测试，以及应用侧确认/取消 GPUI 回归已通过；完整整仓门禁与本次提交的多平台 Quality 需以本轮提交后的记录为准。

最新主线继续增加了六个可验证的远程工作流切片：连接库支持 JSON 剪贴板导入/导出、收藏和删除；每个 SSH 标签拥有仅驻留内存的有界命令历史；SFTP 提供单 worker FIFO 队列、分块进度与边界取消并已接入文件面板；Linux 监控面板可按需读取监听 TCP/UDP 端口，并能从 TCP 行显式发起由远程主机执行的固定 `nc -z` 连接探测；SSH 初次连接对瞬态传输失败使用可取消的有界退避重试；终端提供限定在活动 SSH 标签滚动区内的搜索覆盖层、上一项/下一项定位和匹配高亮。对应实现与记录分别见 `testing/records/2026-10-03-connection-library.md`、`testing/records/2026-10-03-command-history.md`、`testing/records/2026-10-03-files-transfer-queue.md`、`testing/records/2026-10-03-transfer-queue.md`、`testing/records/2026-10-03-socket-diagnostics.md`、`testing/records/2026-10-03-tcp-service-probe.md`、`testing/records/2026-10-03-connect-retry.md` 和 `testing/records/2026-10-03-terminal-search.md`。

- 删除本地 PTY 产品后端和依赖。`events.rs` 承载 SSH/界面共享事件；`remote_only.rs` 扫描产品边界，使用 Cargo 运行时目录以支持移动后的构建缓存。
- `workspace.rs` 管生命周期与保存，`workspace/view.rs` 管布局，`workspace/modals.rs` 管连接/认证弹窗。`design.rs` 统一白色/浅灰/蓝色控件主题。
- 默认空会话、中文；新标签打开 SSH 管理器。分屏显式记录两个 EntityId，焦点切换不交换左右位置。命令从首次输入起绑定会话，切换标签不能发往另一目标，清空后才重新绑定。
- 窄窗口打开 AI 时隐藏左监控栏；空状态不显示空监控列；文件区高度随窗口调整；连接表支持横向滚动。
- 早期门禁 `work/integration-gate-12.log` 的 181 项测试仅证明当时版本。后续功能的测试和失败修复分别记录于 `testing/records/`；最新代码需以对应提交的本地门禁与 GitHub Quality 结果为准。
- 原生 macOS 已走通：创建/保存中文 SSH 配置、指纹核对/信任、密码登录、SFTP 列表/读取/审核保存、SSH 分屏、AI 配置发现/测试/保存、精确请求预览/手动发送、建议入命令栏/手动发送、中文英文切换保留状态。
- 夹具只绑定回环地址，SSH terminal 只回显而不执行系统命令；SFTP 仅临时目录；AI 仅本机 HTTP 模拟服务。不能据此宣称真实 Linux 指标、生产 SSH 互通或商业模型服务已验收。

最新连接组织与动态代理增量见 [集成记录](testing/records/2026-10-03-library-socks-integration.md)：持久化嵌套目录树、空目录导入、标签编辑、连接移动、可恢复回收站及最近 50 次成功连接；旧分组保留名称并自动迁移。目录同时改名/移动使用 `update_folder` 一次校验最终树。最近记录排队等待当前保存完成，并在入队及保存时检查目的地快照；编辑目的地清除旧成功记录。异步连接通过 `runtime_bridge` 执行，完成后按当前可见弹窗恢复焦点。

连接管理器同时提供 **导入 SSH 配置**：从剪贴板读取受限 OpenSSH 子集，只接受精确 Host、HostName、Port、User、IdentityFile、ProxyJump 及调用方显式提供的 Include 内容；通配/条件/ProxyCommand 等语义会进入警告报告，解析和路由错误保持连接库不变。实现、设计和测试边界见 [OpenSSH 导入记录](testing/records/2026-10-03-openssh-import.md) 与 [ADR 0023](adr/0023-openssh-config-import.md)。

动态 SOCKS5 仅绑定回环，IPv4/IPv6/域名经 SSH CONNECT；正常停止保留 SSH，异常通道清理到期可断开共享 SSH，并以同 socket shutdown 兜底。界面提供实际代理 URI、复制、逐行停止与汇总状态；不包含 UDP/BIND、代理认证或保存规则。详见 [专项记录](testing/records/2026-10-03-dynamic-socks.md)。

## AI 模块

`keelshell-core/src/ai_profiles.rs` 与 Settings 实现命名配置目录、默认项、供应商/协议/认证引用、高级参数元数据及旧配置迁移。配置 JSON 严格校验，API Key 不序列化。

`keelshell-ai/src/discovery.rs` 实现可取消的异步模型发现、固定无终端上下文的连接测试、已审核 payload 发送。模型地址按完整 Chat Completions 或 Responses endpoint 同源推导；没有自动跨地址尝试、重定向或重试。响应大小、模型/地址长度、超时和错误分类均受限。

`ai_settings.rs` / `ai_settings/view.rs` 独立配置页支持多配置 CRUD、默认项、预设、端点、模型、临时遮罩密钥、发现、测试和取消。保存采用 revision 快照，不覆盖保存期间的新编辑。助手选择临时配置不改变已保存默认项。

当前请求后端支持 Chat Completions 与 Responses 两种显式协议；Anthropic、自定义请求头、代理、Token/推理参数尚未接通，请求验证明确拒绝，不能静默丢弃这些设置。Responses 请求使用 `instructions`、`input` 和 `output_text`，切换协议会安全替换已知 URL 后缀并清除旧密钥。AI 密钥默认驻留内存，现支持显式加密保存、每次启动后主密码解锁、清除临时密钥及解除关联。后台保存只更新草稿引用，Apply 成功才更新工作区与助手；目的地/认证/预设变化会清除旧密钥和引用。设计见 `adr/0009-ai-encrypted-credentials.md`，专项见 `testing/records/2026-10-03-ai-encrypted-credentials.md` 与本轮 AI transport 测试记录。

SSH 密码与私钥口令已接入显式保存、每次主密码解锁和解除关联流程。凭据库 schema 2 认证完整 manifest，保存时检查经过认证的文件快照；加密 payload 绑定连接目的地，配置中只保存不透明引用。解除关联不会删除加密条目；vault 与 state 两次写入不是一个事务，失败可能留下孤立密文。后台已开始的保存可在关闭弹窗后完成，但不会自动连接。设计及边界见 `adr/0006-authenticated-credential-vault.md`，验收见 `testing/records/2026-10-03-vault-search-integration.md`。

## 凭据维护与目录传输增量

主工具栏新增凭据库维护。打开前等待保存/连接完成并排除其他草稿；模态存在时冻结本进程配置写入。条目显示关联配置名称，回收站和所有 AI 存储引用均受保护；磁盘引用每次重新加载。`load_existing` 防止文件删除竞态误创建空库。主密码轮换分阶段重加密并保留旧快照的并发检查；忙碌关闭等待后台结果并把最终消息传回工作区。参见 `adr/0008-credential-vault-maintenance.md` 与对应专项记录。

文件面板新增“上传目录”，选择远程目录后“下载选中项”。后台只读扫描后审核源、完整目标、数量与字节，确认后重新扫描并在同一 SSH 连接执行。空目录保留，目标必须是新目录；32 层、10,000 项、16 GiB、30 秒扫描/15 分钟执行限制，拒绝静态符号链接及跨平台危险名称。失败/取消可能保留部分目录，不自动递归删除。长确认条采用受限高度滚动正文与固定按钮，防止完整路径挤出确认/取消。参见 `testing/records/2026-10-03-recursive-transfer.md`。

最终本机整合通过 344 项单元/集成测试、2 项文档测试和 47 项打包回归。原生 macOS 已核验 AI 重启锁定/错误与正确解锁、手动测试、凭据主密码轮换与未关联条目清理，以及递归目录上传/下载哈希一致、中英文审核及取消不创建目标。程序、两项回环服务已退出，SSH 临时文件树已清理。详细构建哈希和证据见 [集成验收](testing/records/2026-10-03-credentials-tree-integration.md)。

shell/exec/SFTP 通道在打开前即由独立任务持有，覆盖迟到确认和结果交接取消；SFTP 的高层关闭拥有独立取消控制，避免写入背压阻塞关闭。正常清理保留共享 SSH，异常超时允许有界断开同一 TCP；CLOSE 入队不代表远端确认。独立复审及 80 项 session 测试见 [专项记录](testing/records/2026-10-03-session-channel-ownership.md)。

随后 Quality 在 Windows 目录取消/FIFO 测试发现栈溢出。四处传输缓冲改为直接堆分配，保持原有分块大小与取消语义；新增两项 Future 尺寸回归，取消/FIFO 在显式 2 MiB 线程运行。修复后的本机整仓门禁通过 346 项单元/集成测试与 2 项文档测试，session 为 82 项。代码提交 `9c88c58` 的 Quality 运行 `37092321464` 在 macOS、Ubuntu、Windows 全部通过，Windows 取消/FIFO 及尺寸回归成功。原始失败、本机未复现边界及重跑证据见 [小栈回归](testing/records/2026-10-03-transfer-stack.md)。上述 GUI 哈希属于内存布局修复之前的构建。

## 命令片段与本地建议增量

命令工具区现在无需 SSH 会话即可管理显式保存的片段，支持新建、编辑、搜索与二次确认删除。领域 CRUD 保留 UUID，以候选快照验证，存储继续使用现有 revision/锁/原子写入。编辑器保留名称、说明、CSV 标签及多行命令原文，保存中立即冻结输入；失败保留草稿。历史与未执行命令不进入配置文件。

命令栏已改为多行 Textarea，解决单行控件删除换行的问题。最多 8 项本地建议来自当前 SSH 历史和片段；后台单 worker 合并输入，迟到结果验证目标、输入 revision、原文和来源 generation。点击候选再次检查来源与目标，只填入不执行。程序填入显式推进 revision，因为 set_value 不产生 Change。建议的键盘交互捕获 Input action 而不是原始按键；IME 组合态交还输入组件，上下键将所选行滚动至可见范围。详见 [设计](adr/0010-command-snippets-and-local-suggestions.md) 与 [集成验收](testing/records/2026-10-03-command-snippets.md)。

该增量本机整仓门禁通过 385 项单元/集成测试、2 项文档测试、格式、严格 Clippy 和依赖策略。最终 macOS 构建验证了 Enter 仅填入多行片段、明确执行后回显和会话历史显示；程序、夹具正常退出，临时根目录与监听已清理。二进制哈希和失败回归记录保存在上述验收记录；Windows/Linux GUI 未据此验收。

领域提交 `2ea9b1c` 与整合提交 `02fc8fc` 已推送。整合提交的 Quality 运行 `37094089051` 在 macOS、Ubuntu、Windows 全部通过；Rust 测试总数分别为 387、387、382（含文档测试），各有 47 项打包回归，新增 10 项工作区 GPUI 回归均通过。按平台条件编译的数量差异已在集成记录列明，尚未创建发布标签。

## SSH 跳板路线与首页增量

已保存 SSH 配置可关联最多四个跳板，逐跳独立指纹、密码/密钥/agent认证，最终目标独占终端和最近记录。每次尝试持有专属父链，取消或关闭目标释放链，迟到网络/凭据/信任回调按请求token隔离。主机信任与版本2加密凭据绑定规范化路线；首跳兼容旧直接信任，版本1凭据仅可直连。修改上游目的地或认证会使下游凭据引用和最近记录失效；删除、恢复、导入遵守完整依赖图。设计见 [ADR 0011](adr/0011-owned-ssh-jump-routes.md)。

跳板选择器支持搜索、分页与拒选原因。连接编辑、认证、指纹弹窗采用固定按钮和滚动正文，普通认证为紧凑布局；进度卡不抢占其他编辑器焦点。最终整仓门禁通过439项单元/集成与2项文档测试，47项打包回归通过，新增8项真实SSH工作区测试涵盖取消A后启动B及目标Save→Unlock交接。macOS原生完成逐跳认证/指纹、SFTP与终端、英文路线、取消/重连及链清理；最终构建哈希和证明边界见 [集成验收](testing/records/2026-10-03-jump-integration.md)。

中英文README按成熟开源项目首页结构重新整理，展示真实截图、能力、首次连接、AI审阅、源码运行与贡献入口。仓库公开内容和提交说明持续进行名称扫描，研究来源与链接检查见 [首页记录](testing/records/2026-10-03-readme.md)。

## 传输暂停、续传与 OpenSSH 互通增量

普通文件和目录传输支持后台 ACK 确认的暂停、继续、取消。显式续传先只读扫描、审核，再在同一 SSH 连接复核完整源 SHA-256 和目标全部前缀；目录先检查整棵树，通过后补齐缺失项和空目录，拒绝额外目标、链接和冲突。暂停期间保留 FIFO 位置且不消耗活动预算；重连或重启后重新建立计划。完整内容保证只适用于显式续传，取消仍可能留下部分输出，SFTP v3 不提供文件系统事务。

文件面板按 worker、传输状态、视图和协议夹具拆分。新增 8 项真实 GPUI+SSH/SFTP 回归覆盖审核、取消、迟到消息、语言、目标和窄面板；本机整仓门禁 469 项单元/集成与 2 项文档测试通过，47 项打包回归通过。真实 OpenSSH 4 项测试另行执行，发现并修复 subsystem 确认前合法 WindowAdjusted 被误判为关闭的问题。新增 macOS/Linux CI 互通步骤与独立进程/临时密钥清理回执；最终 macOS 构建完成双向文件与目录续传、只读审核、暂停/语言切换/继续及内容哈希比对；应用、测试服务、临时密钥和远端树已清理。首次 Quality 的 macOS/Ubuntu 普通测试和 OpenSSH 互通通过；Windows 揭示目录下载续传查询不完整 verbatim 盘符路径的错误，已改为检查完整绝对祖先并补真实文件系统回归；原失败流水线保留，修复提交 `7f95c46` 的 Quality `37099262632` 三平台全部通过：macOS/Ubuntu 各 472 项普通测试、2 项文档测试和独立 4 项 OpenSSH；Windows 465 项普通与 2 项文档测试，47 项打包检查按平台记录跳过。设计见 [ADR 0012](adr/0012-resumable-sftp-transfers.md)，当前构建、原生验收与失败证据见 [集成记录](testing/records/2026-10-03-sftp-resume.md)。

## 每跳上游代理增量

已保存配置支持 SOCKS5 或 HTTP CONNECT，代理可以分别置于本机到首跳、前一跳 SSH 到后一跳之间；匿名与用户名/密码认证、远端域名解析、严格协议边界和无直连回退已接入。配置只保存端点及用户名，代理密码仅本次使用，并与 SSH 保存/解锁流程分开。代理配置参与规范化路线身份；无代理保持版本 1 的旧键，含代理使用版本 2，单跳代理也不继承旧直连指纹或旧凭据。

连接编辑器增加可折叠代理配置；认证、指纹和路线预览显示每跳协议及端点。取消、失败、保存后路线变更及迟到网络/凭据回调继续受请求身份和资源所有权检查。固定操作按钮、可滚动正文与内容最小高度回归覆盖中英文和 480/760/1280 宽度。

本机最终整仓门禁通过 523 项普通测试与 2 项文档测试，独立 OpenSSH 4 项及打包回归 47 项全部通过。独立复审未发现剩余阻断；macOS 最终构建验证 SOCKS5 → SSH 跳板 → HTTP CONNECT → SSH 目标、逐跳认证/指纹、错误密码、取消后重新连接、英文认证、目标终端回显和 SFTP 中文读取。退出后两类代理连接、专属父链、夹具进程、监听与临时根全部清理。最终二进制哈希、初始失败与证明边界见[验收记录](testing/records/2026-10-03-upstream-proxies.md)，设计见 [ADR 0013](adr/0013-upstream-ssh-proxies.md)。此项不覆盖 HTTPS/PAC/企业代理认证或代理密码保存，也不代替 Windows/Linux 桌面验收。

## 已建立会话重连增量

默认手动、可选有界自动重连已接入保存配置。typed 连接/shell 终态区分正常退出、显式关闭、传输丢失和保活超时；Ready 以 PTY/shell 确认为准。原标签位置替换为新 EntityId，按帧排空旧输出后移交有界历史，旧输入/解析器模式不继承。完整 SSH/代理路线重新建立，认证和信任提示需显式继续，预算按整条路线计数并在稳定 30 秒后重置。

旧命令原文保留并要求重新审核；旧 AI 请求/上下文撤销。文件、监控、隧道面板保留最多一份上一会话快照，远端能力已停用；真实取消/写入回执保留，不自动恢复任务。旧快照有未保存草稿时，同意绑定面板及文本，完成前再次复核，迟到结果不能丢弃新编辑。设计和最终验证进度见 [ADR 0014](adr/0014-remote-session-reconnection.md) 与[验收记录](testing/records/2026-10-03-reconnection.md)。

上一增量提交 `6ac8f90` 的 Quality `37100961437` 三平台全部通过：macOS/Ubuntu 各 523 项普通与 2 项文档测试、47 项打包检查和独立 4 项 OpenSSH；Windows 516 项普通与 2 项文档测试、46 项打包通过及 1 项 Unix 权限跳过。本次重连增量不能引用该提交结果代替自己的门禁。

## 远端命令与路径补全增量

命令栏显式远端补全已接入。Core 分析光标词并安全引用；Session 固定 PATH 探针与 SFTP 只读扫描；App 按会话、原文、光标、目录和请求身份复核候选，只有显式接受才局部替换，Enter 不执行。每会话补全目录与交互终端 cwd 独立，可编辑或显式取 Files/SFTP 起点；普通输入不发查询。取消、IME、Undo、新实体隔离及中英文小窗口布局均有真实 GPUI 回归。

本机整仓 642 项普通与 2 项文档测试、严格 Clippy、格式和依赖策略通过；47 项打包、独立 5 项 OpenSSH 通过。macOS 原生完成多行中文词替换、引号/空格名称、目录继续输入、单步撤销、仅光标失效与语言切换，最终包另做冒烟；所有应用/夹具、监听和临时树已清理。设计见 [ADR 0015](adr/0015-remote-command-completion.md)，精确构建哈希、原失败和平台边界见[验收记录](testing/records/2026-10-03-remote-completion.md)。交互 alias/function 与可编程参数补全仍未实现。补全提交 `8cd89fa132f7695400ca50b3297f533d5efcb8ee` 的 Quality `37105632462` 已在 macOS 26、Ubuntu 24.04、Windows 2025 全部通过；macOS/Linux 独立 OpenSSH 成功，Windows 按配置跳过。后续变量片段和批量 exec 增量如下，不能沿用该提交的通过状态。

## 参数化片段与批量 exec 增量（已通过本地与远端 Quality）

片段增加显式 `parameterized` 开关，旧配置缺字段默认关闭，旧双花括号正文不解析。Core 纯编译推导最多 32 个参数，完整词/赋值值/长选项值按 POSIX 字面引用；值不会保存到片段。编辑器显示语法和变量列表，新参数弹窗提供未填写/显式空值、只读完整预览和固定按钮。确认前重读输入，工作区最终复核原文/revision/目标 EntityId/完整片段快照；拒绝后保留值。展开命令默认不记会话历史，仍需单独点击执行。

批量任务对当前已认证会话显式选择 1–32 个目标，审核共同命令、并发、超时和失败策略后用独立 SSH exec 执行。命令不注入 PTY、不继承终端 cwd、不自动重试或重连；后端持有旧连接，实体变更不会把任务导向新会话。每行显示有界 stdout/stderr 和明确退出/拒绝/未开始/未知结果；隐藏面板保留当前批次，取消不证明远端进程终止。

已确认模板 20 项领域/存储与 4 项真实 `/bin/sh`、14 项真实 TCP 批量协议、编辑器 9 项和参数组件 6 项 GPUI，以及当次 6 项工作区专项通过。实际小视口回归包含 32 参数滚动和中英文固定按钮。曾发现预览 state 的只读状态被控件渲染默认值覆盖，现视图显式只读并以真实输入不变验证；原失败保留。最终整仓门禁、最终包哈希与 macOS 原生、六项 OpenSSH 和新提交 CI 结果须由集成负责人追加到[验收记录](testing/records/2026-10-03-parameterized-snippets-batch-exec.md)。本节尚不代表完整集成通过。设计见 [ADR 0016](adr/0016-parameterized-snippets-and-batch-exec.md)。

## 图标与打包

`assets/icons/source.png` 是用户选定白底版本；`SOURCE.md` 记录来源。macOS ICNS、Windows 九尺寸 ICO、Linux hicolor PNG 共35个产物经过尺寸、alpha、容器目录、像素、哈希与重复生成检查。macOS 原生 iconutil 解码通过。

`packaging/package.py` 已用真实本机二进制生成 .app 并启动；主程序接入 app identity、Linux desktop/app_id 和 X11 内嵌图标，Windows build.rs 接入资源。六平台原生构建与发布流水线已配置，实际运行状态见发布记录；Windows/Linux 桌面显示仍未验收，未签名、公证或安装。

## 接续顺序

### 2026-10-04 一次性 SSH 快速连接

无活动会话时已加入可操作的快速连接表单：主机、端口、用户名、Agent/私钥或密码认证均在表单中完成，连接库保留在表单下方。点击“连接”构造不写入 `AppState` 的直连路线，复用现有认证和指纹核验流程；取消或成功连接都不会创建连接库条目或最近记录，成功的临时会话也不创建重连绑定。点击“保存为连接”只打开标准连接编辑器，仍需再次点击保存才写入配置。Core 路线、GPUI 交互和无持久化回归见 [ADR 0024](adr/0024-one-time-quick-connect.md) 与 [验收记录](testing/records/2026-10-04-quick-connect.md)。

该切片的本地测试和 GitHub Quality `37157850288` 证明一次性路线和存储边界，不代表真实生产主机或 Windows/Linux 原生窗口已验收。高级跳板、代理、重连设置继续使用持久化编辑器。

1. 最新 .app 已验证文件编辑器高度修复和访达白底图标显示，详见 testing/records/2026-10-03-remote-ai-icons.md。所有测试夹具和配置位于 ignored work，仅用于受控验收。
2. 本阶段测试记录和实现已本地提交，测试App与两个夹具均已退出，SSH临时目录已清理。最新用户授权已覆盖原“仅本地提交”限制。
3. SSH 凭据库 UI 与终端搜索已接入并补齐焦点隔离测试。连接目录/回收/最近与动态 SOCKS5 已补齐本轮切片；凭据维护、AI 加密密钥和有界递归传输已接入；命令片段与本地建议已补齐；已保存跳板路线已补齐；传输暂停/继续及显式内容校验续传已补齐；每跳上游代理已补齐；已建立会话重连和显式远端 PATH/字面路径补全已接入；变量化片段和基础批量操作审核已接入；远程文件面板新增审核式 POSIX 权限修改，使用独立 SFTP `SETSTAT`，执行前复核目标 mode/类型及父目录、执行后读回确认，仅允许 0000–7777 八进制 mode 并拒绝符号链接；公开起点提交 `790bf1c` 的本地专项已通过，Quality 运行 `37119306924` 在 macOS 26、Ubuntu 24.04、Windows 2025 全部通过，其中 macOS/Linux 的独立 OpenSSH 互通也通过。当前仍未完成全部远程 SSH 能力目标。

## 待补的完整性

连接批量组织与永久回收清理 UI、凭据备份/恢复与跨文件事务、外部 ProxyCommand、远端进程的断线恢复、交互 shell 可编程补全、逐目标模板参数/任务依赖/持久化批量审计、传输自动恢复/并行、ACL/所有权/文件差异、TCP 协议级服务健康、更多监控、同步、打包和 Windows/Linux 原生验收均需继续追踪。键盘交互认证的 UI 提示收集已接入：密码或私钥认证可显式切换为 server-driven MFA，挑战逐批显示在双语模态中，答案只经一次性有界通道传给当前路由，不写入凭据库；外部 MFA 服务与跨平台原生窗口仍未验收。当前 TCP 探测只证明远程主机完成握手，不代表协议或应用已就绪。终端搜索目前只搜索活动标签的本地滚动区，不读取远端文件或重新执行命令。

工具栏已接入“关于/更新”面板：它显示内置变更日志和项目主页，用户点击后在后台检查固定 GitHub Release，下载当前平台资产并验证同一发布的 SHA-256。已校验包保存在唯一临时目录，可从面板查看；显式点击“自动安装并重启”后，由同二进制的隐藏 helper 再次校验 `package-manifest.json`，仅替换清单文件，文件占用有界重试并在失败时回滚。开发构建或非标准安装会退回手动安装；签名、公证、安装权限和三平台原生验收仍未完成。设计与验证见 `adr/0021-about-and-update-panel.md` 和 `testing/records/2026-10-03-update-panel.md`。

## 迁移核验

本阶段功能提交 `2ee0693c02f1ceca4796d5ccc500ff48a487bd6a` 与验收记录提交 `184d214d38480d21e05ab5638d3cc3e741e7a1e1` 已推送到公开仓库；功能提交的三平台验证见 [Quality 37125780395](https://github.com/cyruss648/keelshell/actions/runs/37125780395)，验收记录提交的三平台验证见 [Quality 37127192429](https://github.com/cyruss648/keelshell/actions/runs/37127192429)。旧产品别名已从可达 Git 历史清理，详细证据见 `testing/records/2026-10-03-final-feature-slice.md`。

目录通过同一文件系统 rename 移动，82 个源码/配置文件 SHA256 前后一致；迁移时 Git HEAD 与 porcelain status 前后一致，旧目录不存在。`git fsck --full` 初次发现 Finder 的 `.git/refs/.DS_Store`，已可恢复地移至 ignored `work/relocation-quarantine/refs/.DS_Store`，复检通过。迁移后的历史清理和远端更新见上方最终验收记录。

测试日志与本应用的安全空状态截图复制到 ignored `work/relocation-evidence/`。第三方应用中含用户连接树的截图留在原任务私有工作区，不进入仓库。原始失败记录保留，不能删除以掩盖失败。
