# KeelShell 当前交接 — 2026-11-07

先读[AGENTS](../AGENTS.md)、本页、[路线图](ROADMAP.md)及
[共享 Cargo 指南](testing/SHARED_TARGET.md)。时间使用 Asia/Shanghai，
原始回执 UTC 保持。上一阶段完整过程保存在
[原生收口前交接](history/2026-11-06-handoff-before-editor-native-closeout.md)。

## 2026-11-07 连接身份栏最终候选

本轮实现已完成源码与测试修复，当前候选包含 pane 连接身份栏、恢复状态语义
修正以及主题 typography token。源码仍保持 UI、core、session、AI 分层：

- 每个终端 pane 都显示会话标签、真实 SSH 用户/主机/端口、`SSH / UTF-8`
  和带文字的生命周期状态；
- 单 pane、横向/纵向分屏和恢复占位共用同一身份栏渲染路径；
- 未保存的一次性连接显示 `未保存连接 / Unsaved connection`，连接档案缺失
  显示 `配置已移除 / Profile unavailable`，两者使用不同语义色；
- Frost、Graphite、Lavender 仍全部可选，主题由 registry、Palette、组件形状、
  chrome token 和 typography token 组成，新增主题只需添加注册定义；
- pane 标签区域已收紧身份栏宽度，保留分屏标签与关闭动作的点击命中空间。

当前源码定向回归和完整工程门禁均通过：Python 131 passed/1 skipped，
`keelshell-app` 1078 passed/2 ignored，core 197 passed，session 130 passed/4
ignored；workspace integration、MCP、SSH loopback、transfer、doctest、readiness、
observer 和 small-stack AI controller 也通过。严格 workspace Clippy、格式和
差异检查通过。首轮完整门禁中新增测试曾触发 `needless_borrow`，已修复并保留
失败事实；当前还不能把该候选写成已获独立复核批准。

旧提交 `d9072ed` 的 Avicenna 只读报告发现 1 个 P1 与 3 个 P2；P1 已由
`1e63165` 修复，三个 P2 已在当前源码中关闭。该旧报告不替代当前提交的独立
复核，下一步必须固定最新提交重新审查并返回 P0/P1/P2/P3、路径/行号、真实计数、
限制和 merge recommendation。

原生收口仍需使用已解锁的 macOS 桌面实际观察单 pane、横/纵分屏、三主题、双语、
900×580、主题切换后的身份栏与会话标签、键盘焦点/AX 以及真实 SSH 连接中和断开
状态；Windows/Linux 桌面、IME/VoiceOver 完整矩阵和 Podman 真实联调仍未验证。
Headless GPUI 测试不能代替这些边界。

源码设计决策和测试证据见
[ADR 0116](adr/0116-pane-connection-identity-bar.md) 与
[测试记录](testing/records/2026-11-07-pane-connection-identity-bar.md)。

## 2026-11-07 连接身份栏继续推进（历史状态）

上一片视觉节奏收口已由 `e3226e7097b72f6927fbc38f05a94a365d8269d7` 提交并推送，
本地、tracking 和远端分支已读回一致。当前候选把每个终端 pane 的顶部统一成
会话标签、连接身份和 pane 比例动作组成的紧凑 header：

- 真实端点来自 Workspace 的远端快照，恢复标签从当前连接档案生成，缺失档案明确
  显示不可用，不生成示例主机；
- SSH / UTF-8 与状态 badge 分开显示，状态文字与语义色同时保留；
- 身份区具有双语 AX 标签，单 pane 与分屏都使用同一渲染路径；
- 继续使用 Frost、Graphite、Lavender 的 Palette、组件 token 和
  `ChromeTokens::pane_header_height`，没有主题 ID 业务分支。

源码、设计决策和测试记录分别见 `crates/keelshell-app/src/workspace/pane_view.rs`、
[ADR 0116](adr/0116-pane-connection-identity-bar.md) 和
[测试记录](testing/records/2026-11-07-pane-connection-identity-bar.md)。本段保留
提交前的历史状态；最终候选的门禁结果以上方记录为准。

该实现继续保持 UI、core、session、AI 分层，阻塞 I/O 不进入 UI 线程，连接身份栏
不改变 SSH 生命周期、命令审核或 AI 权限。真实 macOS/Windows/Linux 桌面、主题
切换截图、IME、键盘和 VoiceOver 仍不能用 headless GPUI 测试代签。

## 2026-11-07 继续推进回执

本次继续推进的视觉增量尚未提交时已完成定向回归和完整工程门禁。源码新增
`ChromeTokens` 并接入主题注册表，工作区宽窗增加品牌与远程工作台上下文，
命令栏增加审核目标徽章，执行按钮使用 primary 语义，会话标签补充
`surface_active`/`surface_hover`/按下层级。新增设计决策见
[ADR 0115](adr/0115-workbench-visual-rhythm-and-context.md)，测试回执见
[2026-11-07 视觉记录](testing/records/2026-11-07-workbench-visual-refinement.md)。
三套主题仍通过 registry、Palette、组件形状和 chrome token 扩展，业务视图不
按主题 ID 分支；900px 以下隐藏品牌区以保留紧凑窗口操作空间。

定向设计测试 4/4、工作区三主题/双明暗/双语/三尺寸布局 1/1、长标题会话
1/1、900×580 双语工具栏 1/1 通过。首次 app crate 全量运行出现既有 reconnect
分屏夹具 1 项失败（1074/1/2），精确重跑该用例 1/1 通过；随后完整
`python3 scripts/check.py` 退出 0，实际完成 Python 131/1 skip、app
1075/2 ignored、workspace 其余测试、严格 Clippy、rustdoc、依赖策略和三组
AI controller。该 linker 警告和首次 app 全量失败均保留，不伪装成无失败历史。

独立视觉/产品只读复核已返回：未发现 P0，确认当前主题注册表和本轮视觉增量
方向可维护；下一片优先建议为所有终端 pane 增加持续的连接身份栏，并随后处理
顶部工具栏分组、AI 面板窄窗比例和会话标签键盘/AX 语义。该报告不替代源码
独立非作者复核，后者仍在等待。

原生 CUA 通道仍需再次确认桌面解锁后才能观察当前工作树；在此之前 macOS
多窗口、主题切换、键盘/IME/VoiceOver 和当前视觉增量的实际截图继续保持
`NOT_RUN`，不以 GPUI headless 测试代签。

当前源码提交 `ac397c87f3fc49e1e1f940ecd99fab20566c7a23` 已于
2026-11-06 22:07:13 UTC 推送到
`origin/feature/main-visual-integration-20261106`，本地、tracking 和远端
分支均已读回同一 SHA；`main` 未改变。

在没有源码变更的前提下，继续执行了三组定向回归：`app_windows::tests`
5/5、`workspace::tests::windows` 14/14、`files::transfer_tests::review_viewport_tests`
11/11。应用使用隔离 `KEELSHELL_DATA_DIR` 完成编译并启动，随后停止并清理临时目录；
这只证明启动路径可执行，不替代桌面交互验收。

文档提交后的第一次完整 `python3 scripts/check.py` 在
`keelshell-ai --test local_agent_process -- --controller-small-stack` 处遇到
`EmptyReply` 并退出 101；精确重跑同一控制器通过，随后第二次完整门禁退出 0。
第二次结果保持 Python 131 run / 1 skipped、workspace 2,397 ordinary / 23 ignored、
11 rustdoc、严格 workspace Clippy、依赖策略和三组 AI controller 全通过。第一次
失败和精确复跑成功均保留为测试事实，不将第一次失败隐藏成从未发生。

本轮再次尝试非作者严格只读复核。默认 agent 在有界等待内没有报告；备用供应商
返回 API-Key 限制 403；本地 Codex agent 在工具调用循环中未到达审查正文。三次
尝试均没有修改工作树、提交、推送或清理任何项目资源，当前状态保持
`NON_AUTHOR_REVIEW_UNAVAILABLE`，不能写成独立批准。

CUA 桌面通道连续返回 macOS 仍锁定，未能观察多窗口、Quit/Install 聚合审核、
主题切换、`merge-adopt`、`merge-save`、late-save 或连续无换行保存；当前原生范围
保持 `NOT_RUN`。下一次桌面可用后，先运行这些受控场景，再更新本页和对应测试记录。

## 当前集成：主题与工作区外壳视觉

本分支 `feature/main-visual-integration-20261106` 已将 `main` 的最新监控、知识分类、
远端文本格式和 traceroute 能力，与视觉分支的三主题语义 token 和工作区外壳修整合并。
集成源码提交为 `26e11c5`，文档编号清理提交为 `ecdc8c1`，进度记录提交为
`2cca8cd`；继续工作前应按交付回执重新读取本地、tracking 和
`origin/feature/main-visual-integration-20261106` 的实际 HEAD，后续文档修订可能
继续推进分支 SHA。
视觉增量覆盖顶部工具栏、连接导航选择器、会话栏、底部工具栏和状态栏：统一使用
`chrome`、`surface`、`surface_hover`、`surface_active`、`toolbar`、`focus` 和
`accent` 语义层级，活动会话以边界与底部强调线表达当前目标，不改变会话实体、焦点、
审核、网络、持久化或权限语义。

集成后复核发现监控面板文件系统容量条仍使用硬编码 `0x448eaa`，导致资源条不随
主题明暗变体切换。修复提交 `ef110d2` 增加 `Palette.resource`，为三套主题的六个
变体定义资源色，容量条读取当前 `visual.resource`，并以设计测试校验资源色与进度
轨道的至少 3:1 对比度。修后定向设计 4/4、磁盘相关 10/10、独立磁盘主题回归 1/1、
严格 Clippy 和完整 `python3 scripts/check.py` 均退出 0。随后尝试启动新的非作者只读
复核，但默认复核任务未在有界等待内返回报告，备用模型因 API-Key 权限返回 403；
没有任何复核任务修改工作树。父级对固定提交 `052eebc` 的限定只读核对未发现新的
P0/P1/P2/P3，当前状态为
`LIMITED_PARENT_REVIEW / APPROVE_SCOPED_WITH_REVIEW_SERVICE_LIMITATION`，不把它写成
非作者子代理已批准。

霜白、曜石、雾紫仍由注册表、Palette 和组件形状扩展；业务视图不按主题 ID 分支，
新增主题只需要扩展主题注册表、语义 Palette 和组件形状。视觉来源切片的定向门禁、
焦点环实际 alpha 合成对比度检查和第二轮独立复核均已通过；集成后的
`python3 scripts/check.py` 已重新执行并退出 0，包含 Python 131 run / 1 skipped、
workspace tests、格式、workspace Clippy、依赖策略和三组 AI controller 门禁。
集成后的 design 4 项和 workbench layout 5 项也再次通过；独立集成复核固定在
`26e11c5`；`ecdc8c1` 解决 ADR 编号冲突，`2cca8cd` 记录集成门禁和当前
交付边界，后续文档提交只推进状态记录，不改变源码输入。
原生包、Windows/Linux 桌面、紧凑窗口、输入法和辅助技术仍保持各自未完成边界。
实现和视觉来源记录见 [工作区外壳 ADR](adr/0113-workbench-chrome-hierarchy.md)
与 [视觉测试记录](testing/records/2026-11-06-workbench-chrome-polish.md)。

## 接续片：独立工作窗口与编辑器动作可达性

2026-11-07 已从受保护的隔离源码副本导入两项下一片实现到当前集成分支：

- `AppWindows` 为同一 `StateStore` 路径提供进程内窗口注册、共享更新服务和
  `ConfigurationGate`。新窗口通过真实 `gpui_kit::open_window` 创建，从空工作区
  开始；每个 `Workspace` 独立持有 SSH、终端输入、草稿、AI 审核、远程面板和
  transport owner，窗口最多八个。
- 配置写入使用 `WriteLease` 绑定 snapshot revision、epoch 和 owner。兄弟窗口
  收到变更后只更新主题/语言并撤销旧配置审核，保留 live SSH 与本地草稿，显式
  reload 后才能继续配置操作。关闭窗口先审核精确会话/草稿/格式快照，再按 owner
  有界停止和 join worker；未知清理结果保留 owner，不自动重试。
- inline editor 与三方合并手工输入的布局修复已导入：正文和完整版本独立滚动，
  底部审核动作固定在可见动作区，900×580 下手工输入保留可达最小高度。

当前 app 测试清单为 1,075 项、2 项 ignored；配置闸门 4、窗口行为 12、真实
loopback SSH 窗口清理 1、worker 回收 3、编辑器视口 11，以及本轮 macOS
临时目录根路径回归 11 项均通过。第一次 workspace 重跑暴露两个并行传输夹具
直接使用 macOS `/var` symlink 临时路径的问题，已让这两个夹具使用
canonicalized temporary root；产品的静态 symlink ancestor fail-closed 规则保持不变。
随后完整 `python3 scripts/check.py` 实际退出 0，包含 Python 131 run / 1 skipped、
workspace 2,397 ordinary / 23 ignored、11 rustdoc、workspace strict Clippy、
依赖策略和三组 AI controller 门禁。独立非作者复核、开发包、真实 macOS 多窗口/
编辑器采用保存，以及 Windows/Linux 原生验收仍待完成。正式边界见
[ADR 0114](adr/0114-independent-workspace-windows.md) 和
[本轮记录](testing/records/2026-11-07-multiwindow-editor-layout.md)。
本轮两次新开的有界只读复核 agent 均未返回可审计报告，已停止且没有改动工作树；
因此不能把本批写成已获独立批准。父级只读差分检查发现并修正了新增的 worker
stderr 输出，未发现新的 P0/P1；该检查不替代非作者复核报告。

## 已交付基线

本批基线为已交付的 `fa7c05914e0eefc0f24ab32f8af690bdc50a63ed`，
包括按项监控能力与审核式知识分类。536 输入 fresh 完整门禁为
2313 ordinary / 23 ignored、14 rustdoc、Python 131 run / 1 skipped；
限定新包原生与正式文档已有各自非作者批准。
精确 main / Quality / push / attempt 1 的 run `42153758152` 三平台成功，
候选各 204 名通过。原观察器外层 91099 实际 wait、exit 2 / reaped；
原 Linux 附件 GET 失败保留，原期限内一次固定附件补取与两平台各 11 项 OpenSSH
由 `current-ci-r2` 限定批准，不重启或重跑旧 run。
见[该批记录](testing/records/2026-11-06-monitor-knowledge-categories.md)。

## 本批：文本格式与路径探测

整合工作树分支 `feature/editor-traceroute-20261106`，基线 fa7c059。
最终 r3 为 543 项工程输入、21 个源码路径，冻结文件 SHA256：
`1b5c7cf42d8a4d9ede5de6fcca29caf11da5daeaef59fc8e580ea4a1172590bd`。
原件入口 `work/next-batch-20261106/` 被 Git 忽略，不表示公开附件。

- UTF-8 BOM 与 LF/CRLF 保持：25 个新增用例实际通过，三项连续编辑 P2
  已在独立 r3 差分复审中关闭。备份 ACK、目标发布 ACK 与目标完整读回分别处理。
- 审核式有界 traceroute：17 个新增 Rust 用例实际通过；真实工具版本输出在 stderr，
  r3 修正精确准入合同，14 项 Python 与 10 个错误流负例通过。
  原 r2 工程通过但真实版本拒绝的失败保留。
- 最终完整门禁 `full-gate-r3` 外层 5546 实际 wait、exit 0 / reaped，
  575.972 秒：2355 ordinary / 23 ignored，含 app 1047 / 2 ignored；
  14 rustdoc，Python 131 run = 130 passed + 1 skipped。
  42 新增与基线 204 个具体名均实际通过，543 输入前后不变。
- 采用 `full-gate-r3-summary-r2.json` 与 `full-gate-r3-exec-readback-r2.json`；
  首次派生摘要误留旧日志信息的原件保持，修正未重跑门禁。
- 正常程序构建、macOS 打包和静态检查均 exit 0 / reaped；新包 6 成员、41 资产读回。
  app SHA256 为 `9e83d83af253ffe3c3aba29997450ea75f4c70a6dbe9311e61469228bd0158b9`，
  MCP 为 `e29f58d15f0c26c6634033f2eb0efed62b5b71dc37fcbfbab6bbbc9d28ed78a6`。
  Gauss 的 `combined-r3` 源码、`combined-r4` 门禁/新包增量复核均限定批准，
  开放 P0–P3 为 0；不签原生或未来 CI。
- 本批官方依赖检查 34 包 / 104 要求通过，manifest / lock 未改变；
  直接 registry 要求保持 `x.y`，不固定 Rust toolchain。

### 限定原生

runtime-r3 owner 为 `01e8d214a619471b8ad93786b6083afe`。
Curie `report-r3` 仅批准准备；父真实运行取得 38 组 AX/JPEG。
BOM/CRLF 完整审核取消后字节/stat/目录不变；重新审核确认一次后，
app 自己完整读回，独立观察目标 29 字节与唯一原备份 22 字节。
真实 SSH UI 中 UDP IPv4 / IPv6 各审核确认一次，均返回目的地址一跳回复。

inline 编辑器底部动作可达性待修；合并仅准备草稿和一次固定竞争写，
`merge-adopt` / `merge-save`、late-save 和连续零换行保存保持 NOT_RUN。
不能用源码/夹具观察替代这些原生结论。Gauss 的 `native-r1` 已批准观察子集与
基础设施收口，建议允许当前 543 输入批次限定交付；新增 P2 `NR1-P2-01`
保持 OPEN，完整原生继续 HOLD。报告 SHA256：
`9ce082e4be171262753f750ae4f931f37ca8be61cdeb71b985bc3960c3e07f70`。

外层 28510 由父实际 wait、exit 0 / reaped，797.330164 / 900 秒，没有重启或延期。
app exit 0 / reaped，sshd -9 / reaped，容器 exit 0；
该 owner 的 container / network / image / private 已清理，共享 base / VM / 第三方 target
保留。原 `cleanup.json` 的阶段性 false 与后续 `editor-cleanup.json` 的 true 分别保留。
旧 runtime-r2 因 0700 目录先 chown 后写 seed 失败，app 未启动，外层 49450
实际 wait、exit 1 / reaped。Curie 于 05:13:12 UTC 另编号完成旧 image/private
补充清理，7 条命令实际 wait/reaped、0.298/30 秒，精确身份与 absent 已读回；
旧 HOLD / false、失败和 12 项受保护原件不变，不回填成功。

完整来源与失败见[组合记录](testing/records/2026-11-06-editor-traceroute-integration.md)，
原生范围与原件见[原生记录](testing/records/2026-11-06-editor-traceroute-native.md)。

## 并行下一片与 Cargo 归属

2026-11-06 04:38:48 UTC 父已将共享 Cargo 槽释放给 **Dewey 的 multiwindow 隔离源码**；
原件 `work/next-batch-20261106/cargo-slot-release-r1.json`。
父只消费不可变 `package-macos-r1`，不得用当前 target 字节重新签旧包。
其他角色不能并行 Cargo / clean。归属切换先只使五个项目包失效，保留第三方缓存。

| 切片 | 当前范围与状态 |
| --- | --- |
| 独立工作窗口 | 源码已导入当前分支并通过定向窗口、worker 和 loopback SSH 测试；workspace 全量门禁通过，独立复核和真实多窗口验收待完成 |
| 编辑器动作可达性 | 源码布局修复已导入并通过 11 项视口专项；`merge-adopt` / `merge-save`、late-save 和连续无换行保存的真实原生复验待完成 |

作者隔离副本没有 `.git`，不修改父正式源码、文档或 Git。
已合并分支及时清理；受管建树已两次超时，旧 worktree 的其他任务/primary 保护保持，
不用 shell 绕过。失败和不可变原件保留。

## 接续顺序

提交与 CI 身份不能在同一提交正文中自引用。执行下列步骤前先读取实时 Git 和
`work/next-batch-20261106/delivery-r1.json`、`delivery-exec-readback-r1.json`
及 `current-ci-preparation/` 的实际回执；已有实际完成记录的动作不重放。

1. 原生非作者报告已收取；正式文档、链接及差分由非作者作最终限定复核。
2. 提交本批既有 543 输入，普通快进推送 main，读回本地/跟踪/远端 SHA。
3. 用已封存 `current-ci-preparation/launch_exact.py --sha <新的精确 main SHA>`
   启动一次新观察器；首次构造开始原 5400 秒时钟，不能重启、延时或重跑。
   248 候选的平台预期为 macOS 248 / Linux 247 / Windows 246。
   新 SHA / run / exec / deadline 尚未绑定，不把旧 fa7 CI 当本批结果。
4. 当前两项源码候选完成新的非作者审后，统一 fresh 完整门禁，构建新包及真实原生验收。

## 持续约束与开放边界

仅管理远程 SSH；本地终端、RDP、串口不在范围。UI / core / session / AI 分层，
阻塞 I/O 不进 UI 线程。霜白、曜石、雾紫全部保留，通过注册表和语义 token 扩展，
遵循[原生设计合同](design/HALLMARK_NATIVE.md)。
AI 上下文与命令各自显式审核，模型不能自主执行；主机密钥变化 fail closed，
凭据不进普通配置或日志。

Windows/Linux 真实桌面、完整主题/尺寸/键盘/IME/辅助技术、多窗口的完整原生矩阵、
供应商账户/外部 MCP、签名安装仍开放。用户已授权自主开发、提交推送 main、
标签触发 Release 和本地 Podman 联调；不含覆盖安装或遥测。产品总目标保持进行中。
