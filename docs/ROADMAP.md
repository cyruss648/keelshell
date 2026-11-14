# 开发路线 — 2026-11-15

完整产品目标保持进行中。精确SHA、执行会话和Cargo归属从[当前交接](HANDOFF.md)接续；
[历史路线](history/2026-11-06-roadmap-before-editor-traceroute.md)保留上轮冻结状态。

## 2026-11-15 恢复 claim 生命周期与计划进度

本片源码实现提交为
`d180ece6fb4c91517532938f3ab2d9dfa30ede64`。在首次写入
`Unfinished` 恢复记录时先取得同一记录的 OS claim，再把记录公开给其它窗口；
持久恢复的只读计划在后台 worker 中持有 claim，完成后把 claim 交回审核状态。
取消审核、确认前置失败、worker 失败、挂起、导航撤销和旧审核替换均释放
claim。只读计划重新计算的已有字节数会同时回写持久记录和当前面板内存状态。
针对修复前复核发现的恢复列表旧快照竞态，本片增加单调 load generation 与
session 校验；同时加入真实持久 store 队列成功、初始化失败、取消和并发终态
回归，确保真实 queued worker 的终态与恢复记录一致。

本片 focused 结果为 core `8 passed / 0 failed`，app persisted-recovery
`8 passed / 0 failed`，app recovery `39 passed / 0 failed`，app parallel
`18 passed / 0 failed`。最终源码 SHA 的完整门禁已退出 `0`：普通 Rust
`2535 passed / 0 failed / 23 ignored`，doctest `14 passed / 0 failed`，
Python `131 run / 1 skipped`；app `1172/0/2`、core `644/0/0`、AI
`194/0/5`、session `448/0/16`、MCP `77/0/0`，依赖策略、fmt、workspace
严格 Clippy 和三组 AI controller 均通过。日志见
[恢复日志测试记录](testing/records/2026-11-14-transfer-recovery-journal.md)、
[ADR 0130](adr/0130-persisted-transfer-recovery-journal.md)和当前交接。

修复前独立严格只读 reviewer 固定在文档提交后的 exact SHA
`83db250d175845d084926c0742d2ab20620edc2f` 并返回
`REQUEST_CHANGES`，P0/P1/P2/P3 为 `0/0/2/0`；发现项和修复要求见
[修复前复核记录](testing/records/2026-11-15-transfer-recovery-review-before-fix.md)。
修复后的作者门禁已通过，当前状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_PENDING`；
下一步是固定文档提交后的 exact SHA，启动新的独立复核。仍需单独完成
macOS/Windows/Linux 原生桌面、真实客户 SSH/SFTP、Podman 长流程、IME/辅助技术、
签名、公证、安装升级和发布验收。公司内网不可用，在线依赖最新版本检查尚未完成；
本片只做本地 commit，不 push。

## 2026-11-14 重启后传输恢复日志

源码实现提交为 `52f861e3130749715ea20824efa1f1b1619f1e3b`。文件与目录传输现在
会把已开始但未完成、失败、取消或结果未知的任务写入独立的
`transfer-recovery.json`。日志有 schema version、记录数量、文档大小和路径
边界，使用 owner-only 文件、锁、临时文件、`sync_all`、原子替换和后台 worker
写入。记录不包含 SSH session、凭据、review token、worker、socket、命令或
秘密；绑定连接 ID 不匹配时在远端只读检查前拒绝。每条恢复记录由跨窗口、
跨进程的 OS 文件锁 claim 排他保护；队列/worker 启动失败、取消、挂起、导航
撤销和旧审核替换会释放 claim。无绑定记录必须先确认当前 SSH 主机，确认前
不会执行远端只读检查；计划刷新后的已有字节数会回写持久记录。

重启后只显示观察记录。用户必须检查当前 SSH 主机、重新完成只读目标审核并
显式确认新的续传；成功后或明确 Discard 后才删除记录。恢复日志的三主题双语
卡片、终态幂等性、旧格式主机确认、连接绑定和队列失败路径已有回归。

当前作者完整门禁通过：普通 Rust `2526 passed / 0 failed / 23 ignored`，
doctest `14 passed / 0 failed`，Python `131 run / 1 skipped`，依赖策略、
fmt、workspace 严格 Clippy 和三组 AI controller 均通过。完整证据见
[恢复日志测试记录](testing/records/2026-11-14-transfer-recovery-journal.md)
和 [ADR 0130](adr/0130-persisted-transfer-recovery-journal.md)。

该历史片在 2026-11-14 文档提交时的状态为
`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_PENDING`；后续 claim 生命周期修复和最终
独立复核状态见本路线顶部。macOS/Windows/Linux 原生桌面、真实客户 SSH/SFTP、
Podman 长流程、IME/辅助技术、签名、公证、安装升级和发布验收仍需单独完成。
公司内网不可用，在线依赖最新版本检查尚未完成；本片只做本地 commit，不 push。

## 2026-11-14 命名隧道 owner 与恢复卡片

本片在运行状态恢复基础上补齐会话替换边界和恢复候选的视觉层级。`90fbca7`
新增瞬时 `ReconnectSessionId`、`TunnelRuntimeOwner` 与
`TunnelRuntimeReviewToken`；恢复 plan 使用当前 owner 的 `confirm_for`，旧
SSH 会话替换后旧恢复/停止审核 fail closed，不自动重连、不自动重启、不重放
命令。`1c8df52` 将恢复候选重排为规则名称/详情、状态 callout、动作区和只读
详情，并补齐 Group/Status AX 语义、数量提示、三主题 token 和布局回归。

专用 target 的 focused 结果为 app `19 + 18`、core `18 + 10` 全部通过，core/app
严格 Clippy、fmt 和差异检查通过。测试记录见
[owner 与恢复卡片记录](testing/records/2026-11-14-named-tunnel-owner-and-recovery-card.md)，
设计见 [ADR 0129](adr/0129-named-tunnel-owner-and-recovery-card.md)。当前状态为
`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`；最终独立 reviewer 固定
`79c9090a7754b049f5a91fe3b71f96e15710d66b`，结论为 `APPROVE`，P0/P1/P2/P3
均为 `0`。EOF 空行与 provenance 表述问题已修正。原生三平台、真实 SSH/Podman、
完整辅助技术仍需独立证据。

## 2026-11-14 配置同步原子发布前置检查

本片提交 `540f9fc39e46d22bcdd810d6d23b3ca15132f8ca` 在共享 payload 发布前检查
目标路径，只接受不存在或普通文件；目录、符号链接及其他非普通文件在创建
staging 文件前被拒绝，保留目标和本地 StateStore。现有同目录临时文件、
`sync_all`、原子替换和 Unix 父目录同步行为保持不变。focused core 为
`7` 个单元测试加四个 integration suite 共 `28` 项通过，严格 Clippy、fmt 和
差异检查通过。边界与证据见
[配置同步原子发布记录](testing/records/2026-11-14-profile-sync-atomic-publish.md)；
断电、任意网络挂载和真实多机共享目录仍未验。

## 2026-11-14 AI 审核命令保存为片段草稿

本片基线为 exact HEAD
`b89445604b4ce44fc5a80ff26605f6e6b9039d57`，主体实现提交为
`9b831103aae5563a5450561988d668d2e77a19dd`，无障碍修复提交为
`50ca0ead2fead7b0e92679ee9d531604ae040772`，当前最终实现 SHA 为
`50ca0ead2fead7b0e92679ee9d531604ae040772`。实现位于独立 worktree
`/Users/hdh/.codex/worktrees/ai-command-snippet-save-20261114/keelshell`，
使用专用 target `work/ai-command-snippet-save-20261114/target`，全量门禁使用
`work/ai-command-snippet-save-20261114/full-target`。

Assistant suggestion 和 diagnostic step 只有在现有 `CommandProposal::review`
成功后才发出一次性 `AssistantEvent::SaveSnippet`；AssistantPanel 不直接写
StateStore。Workspace 在打开草稿和提交保存两个边界重新确认活动 SSH
tab/session/target、configuration gate 和 modal scope，失效时双语报错并丢弃
候选。SnippetEditor 保持 reviewed command body locked until explicit re-review，
名称/说明/标签可编辑，保存需要二次确认；失败或冲突保留完整草稿和错误，成功后
复用现有后台 persist、`AfterSave::SnippetSaved`、revision/conflict 检查。

保存结果是普通本地片段，不持久化 session、host、凭据、AI profile、review token
或秘密元数据；界面明确说明明文保存风险，保存/取消都不执行命令。Frost、
Graphite、Lavender、中文/英文、fixed footer/scroll 和 accessibility label
复用 registry、Palette 与组件 token；名称、说明、标签输入均有稳定
accessibility ID 与中英文 `aria-label`，业务代码不按 theme ID 分支。具体决策见
[ADR 0128](adr/0128-ai-reviewed-snippet-draft.md)，产品流程见
[AI 助手审核工作流](product/AI_ASSISTANT_WORKFLOW.md)。

作者 focused 回归通过：AI 片段 `6/6`、SnippetEditor `11/11`、Assistant review
`6/6`，core 包 `634/634`。完整 `python3 scripts/check.py` 实际退出 `0`：
Python `131 run / 1 skipped`，workspace `2499 passed / 0 failed / 23 ignored`，
其中 app `1153/0/2`、core `627/0/0`、AI `194/0/5`、session `448/0/16`、
MCP `77/0/0`；依赖策略、fmt、workspace 严格 Clippy、readiness/observer/
small-stack controller 均通过。脚本外单独 doctest 为 `14 passed / 0 failed`，
含 2 项 compile-fail。完整日志见
`work/ai-command-snippet-save-20261114/logs/full-gate-9b83110.log`，测试记录见
[2026-11-14 AI 片段保存](testing/records/2026-11-14-ai-reviewed-snippet-save.md)。

无障碍修复后的 focused 回归绑定最终实现 SHA
`50ca0ead2fead7b0e92679ee9d531604ae040772`：SnippetEditor `12/12`、AI 片段
`6/6`、Assistant review `6/6`；fmt、app tests 严格 Clippy 和基线到最终实现 SHA
的 diff check 均通过。第一次独立复核在 `db808e0b45a788f4744acf2fe98cde7bae7b2f0a`
发现 P2 无障碍标签问题，修复后第二次独立复核已固定最终实现 SHA 并返回
`APPROVE`，P0/P1/P2/P3 均为 `0`。原始复核报告见
[AI 片段保存独立复核原文](testing/records/2026-11-14-ai-reviewed-snippet-save-independent-review-raw.md)，
测试与未验证边界见
[AI 片段保存测试记录](testing/records/2026-11-14-ai-reviewed-snippet-save.md)。
完整 `scripts/check.py` 没有在无障碍修复提交上重复运行；已有完整脚本结果仍绑定
`9b831103aae5563a5450561988d668d2e77a19dd`，不能代签最终修复提交的 full gate。

## 2026-11-14 命名隧道运行状态持久化与显式恢复

本片固定基线为 exact HEAD
`fd4cc894aa876f23cde49698b693779e81027489`，主体实现提交为
`093f725037d5c037cfdd7b75a66f7448bc0732a`，最终实现 SHA 为
`7e6ecb3ea8c347f7f633ba0f205d936cd58d72ec`。命名规则在 worker 首次报告监听端点
后保存一个只含非敏感元数据的 `TunnelRuntimeSnapshot`；快照不是恢复指令，不含
密码、私钥原值、SSH session、socket、worker、命令或流量。

重启后的隧道面板只显示可能运行过的候选和只读详情。当前规则、connection ID、
路由/跳板、认证类别和认证目标指纹必须重新匹配，用户必须 Inspect、Review start、
检查完整 review token 并显式 Confirm and start fresh；没有自动重连、自动启动、
旧 listener/session/worker/traffic 复用或命令重放。规则缺失、连接或路由变化、
不支持版本、清理未确认、StateStore 冲突或未知保存结果均保留候选并 fail closed。
正常停止且清理确认后删除候选，Discard 也只能由用户显式删除观察记录。

实现分层为 core 快照/校验、app 面板审核和 Workspace 后台 StateStore 事件；
阻塞 I/O 不进入 GPUI 线程。恢复卡片使用现有 Frost、Graphite、Lavender 的
registry/Palette/token、双语文本和 fixed footer/scroll 习惯，不按 theme ID 分支。

作者完整门禁已在独立 target
`work/named-tunnel-runtime-20261114/target` 实际退出 `0`：
Python `131 run / 1 skipped`、workspace ordinary Rust
`2476 passed / 0 failed / 23 ignored`、app `1144 passed / 0 failed / 2 ignored`、
14 项 doctest（含 2 项 compile-fail）、fmt、workspace 严格 Clippy、依赖策略和三组
AI controller 均通过。focused core runtime 为 `8 passed / 0 failed`，
app `tunnel_rule_tests` 为 `19 passed / 0 failed / 1127 filtered`，app `tunnels`
为 `22 passed / 0 failed / 1124 filtered`。完整日志见
`work/named-tunnel-runtime-20261114/logs/full-gate-7e6ecb3-rerun.log`。

独立严格只读复核已经完成，原始报告已归档到
[项目内原始复核报告](testing/records/2026-11-14-named-tunnel-runtime-recovery-review-raw.md)，
项目内归档摘要见
[独立复核记录](testing/records/2026-11-14-named-tunnel-runtime-recovery-review.md)；
审查 exact SHA 为 `7e6ecb3ea8c347f7f633ba0f205d936cd58d72ec`，结论为
`APPROVE`，P0/P1/P2/P3 均为 `0`。独立 target 上 core `tunnel_runtime`
为 `8/8`，app `tunnel_rule_tests` 为 `19/19`，app `tunnels` 为 `22/22`，
app 测试清单为 `1146` 项；fmt、core/app 严格 Clippy 和 diff check 均 exit `0`。

本片当前状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。独立复核没有
扩大为原生 macOS/Windows/Linux、真实 SSH/Podman、IME、辅助技术、签名、公证和
安装发布验收；作者完整 `scripts/check.py` 的通过证据仍以作者独立 target 日志
为准，复核 agent 没有重复执行完整脚本。公司内网不可用，本片只做本地 commit，
不执行 push。证据采集完成后已清理临时独立 target、评审 worktree 和旧的命名隧道
恢复 worktree；作者日志和复核原文已保留。设计、产品和测试证据分别见
[ADR 0127](adr/0127-named-tunnel-runtime-recovery.md)、
[命名隧道规则管理](product/NAMED_TUNNEL_RULE_MANAGEMENT.md) 和
[测试记录](testing/records/2026-11-14-named-tunnel-runtime-recovery.md)。

## 2026-11-14 Unicode 编辑器复核修复

上一轮对本地提交 `b239d78691420a30d54c914c9ad7253023b0b681` 的独立复核返回
P1 `1`、P2 `2`、P3 `1`。本片已修复成功保存读回的 BOM-less U+FEFF 解释、
UTF-16 LE/BE 差异预览、测试记录 SHA/target 绑定和 session 编码错误文案；历史回执
保留在 [`previous-b239-review.md`](../work/unicode-codec-20261114/review/previous-b239-review.md)。

修复后的实现和测试源码提交为
`368a7f836f72ca06b34276e5d587a34497fc7f13`。完整 `python3 scripts/check.py` 已实际等待并退出 `0`，使用
`work/unicode-codec-20261114/full-target`，耗时约 `697s`：Python `131 run / 1 skipped`，
ordinary Rust `2462 passed / 0 failed / 23 ignored`，app `1138 passed / 2 ignored`，
14 项 doctest、依赖策略、fmt、workspace 严格 Clippy 和 readiness/observer/small-stack
controller 均通过。日志和 exit 回执见 [`unicode-full-gate.log`](../work/unicode-codec-20261114/unicode-full-gate.log)
与 [`unicode-full-gate.exit.json`](../work/unicode-codec-20261114/unicode-full-gate.exit.json)。
门禁结束后的外层 zsh wrapper 因使用只读变量名 `status` 返回 `1`，但门禁日志已到达
成功末段，exit 回执记录了这一 wrapper 问题；不能把 wrapper 退出码当作测试失败。

文档绑定与新的独立严格只读代码/功能复核提交为 `6add70cdd367c65f500b7fadf35c7a4fc296ec5a`，复核结论为
`APPROVE`，P0/P1/P2/P3 均为 `0`。独立 core Unicode、session `editor_reviewed_`、
app `editor_format` 和 app `merge` 分别为 `23`、`9`、`18`、`39` 项通过；targeted
Clippy、fmt、diff check、文档绑定检查和 BOM-less U+FEFF probe 均通过。完整回执见
[`final-review-6add70c.md`](../work/unicode-codec-20261114/review/final-review-6add70c.md)。
独立复核没有代签作者 full gate、原生桌面、真实 SSH、Podman 或发行验收。

当前状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。公司内网不可用，本片
只做本地 commit，不执行 push。原生桌面、跨平台窗口、IME、辅助技术、真实 SSH、
Podman 联调和发行验收继续开放。

## 2026-11-14 连接状态灯审查修复

首轮新独立复核在 `98f8f9d` 上发现一个 P2：恢复未连接、Starting 或空活动
面板可能使用成功色，原结论为 `REQUEST_CHANGES`。当前修复让标签/底部状态栏
共用当前生命周期与恢复状态的颜色映射，两条真实 painted quad 回归先复现失败，
修复后通过，覆盖三主题/明暗/中英文、Ready/Ended、缺失连接和有 sibling 标签的
空活动面板。app 清单已更新为 `1136` 项。

完整门禁已 exit `0`：ordinary `2442 passed / 23 ignored`，app 子集
`1134 passed / 2 ignored`、14 项 doctest、Python `131 run / 1 skipped`、
fmt、严格 Clippy 和三组 AI controller 通过。另一位新 agent 对冻结源码返回
`APPROVE`（P0/P1/P2/P3 均为 `0`），独立执行作者预构建二进制的 36 项重点
回归通过，未独立重编译。修复已本地提交为
`9363485ef19802c7b9b1292e2ba5b4adb57fd589`；父线程读取该提交的 519 个 Rust/Cargo
Git 对象，确认与复核冻结输入逐字节一致，二进制摘要和 `1136` 项清单也匹配。
状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_FROZEN_SOURCE_REVIEW_APPROVED /
COMMIT_BYTES_VERIFIED`，未推送；
详见[状态灯修复记录](testing/records/2026-11-14-connection-indicator-review-fix.md)。

## 2026-11-14 依赖升级与当前工具链验证

八个直接包已集成升级，含 GPUI Kit `0.9.1`、rmcp `3.10.0`、rust-embed
`8.14.0`、zip `9.5.1` 和 win32job `2.1.0`；33 个直接 registry 包、103 处
声明/使用、5 个 workspace member 的官方最新稳定版本检查通过，版本要求保持
`x.y`。Rust/Cargo 默认使用开发环境的 `1.100.0`，不把本地版本写成 MSRV。

升级后的完整工程门禁第二轮 exit `0`：Python `131 run / 1 skipped`，
workspace ordinary `2440 passed / 0 failed / 23 ignored`，app 子集
`1132 passed / 2 ignored`，14 项 doctest、fmt、严格 Clippy 和三组 AI controller
通过；新编译 app 清单 `1134` 项。首轮缓存编译 `E0514` 的失败回执保留，根因
未定。切片已本地提交为 `98f8f9d`，首轮独立复核返回一个工作台状态灯 P2，
依赖输入本身无待修发现，后续修复从本页顶部接续。临时实现 worktree 已归档，只做
本地提交、不推送。原生锁屏 `HOLD`、Windows/Linux、真实远端与发布安装边界保持开放。
详见[依赖升级记录](testing/records/2026-11-14-dependency-refresh.md)。

## 2026-11-14 主机监控面板视觉层级

本地提交 `aab1718` 在已提交的工作区 chrome 视觉切片上继续完善主机监控 rail：控制区
显示采样状态，资源/网络/端口/能力分区使用主题语义 token 和稳定的分组语义，
磁盘设备列表维持 160px 内层滚动视口。长英文能力状态会在详情列换行，状态
徽章保持单行；磁盘设备行保持紧凑连续布局，确保末条设备完整进入视口并可点击。

监控定向回归当前为 `21 passed / 0 failed`，磁盘末条滚动与能力状态双语可达性
分别有独立回归。设计决策见[ADR 0125](adr/0125-monitor-panel-visual-hierarchy.md)，
测试证据见[测试记录](testing/records/2026-11-14-monitor-panel-visual-polish.md)。
完整工程门禁已通过：Python `131 run / 1 skipped`，workspace ordinary
`2440 passed / 0 failed / 23 ignored`（app 子集 `1132 passed / 0 failed / 2 ignored`），
14 项 doctest（含 2 项 compile-fail）均通过。独立固定 SHA 复核返回 `APPROVE`，
P0/P1/P2/P3 均为 `0`，独立监控 `21`、工作台布局 `12` 项及 app 严格 Clippy、
fmt 和差异检查通过；其被中止的 app 全量命令未计为独立通过。
原生/真实远端联调仍待完成；公司内网暂不可用，本片只做本地提交，不执行 push。
本片 macOS arm64 精确包构建与静态检查通过；桌面专用通道确认锁屏，主题截图
来源/新鲜度未获核验，原生视觉和紧凑窗口保持 `HOLD`，详见
[原生记录](testing/records/2026-11-14-monitor-panel-native-smoke.md)。

## 2026-11-14 响应式工作台紧凑连接库收口

当前源码与测试补强已固定为
`a33e4c3b154867e4ef542cd9ef6a1193aee31c19`。上一片响应式工作台在 900×580
目标下保留 AI、导航和终端的设计层级；本片继续收口边界：

- AI 列在 720px 以下最低为 200px，在 720px 及以上最低为 240px，上限均为
  360px，900×580 仍保持 240px；
- 磁盘设备列表固定为 160px 高度，避免新增导航 header 后最后一行设备被视口
  裁掉；
- Rust 1.100.0 下的 workspace layout Clippy 诊断和未使用依赖已修复；
- 首次中文工作区的 AX label 与生产按钮语义保持一致。
- 480px 窄窗口下，快速连接表单在自身区域滚动，连接库列表和 Connect 动作保持
  可见和可操作；480×580 低高度窗口将快速连接区域收紧为 `120px` 并保留连接库
  约 `48px` 的正向列表视口；
- 480×580/480×900 均挂载真实保存连接，验证保存连接与快速连接的 Connect 动作在
  滚动后可达，快速连接滚动不会改变连接库 sibling bounds；
- 窄宽度工具栏使用图标优先布局，保留完整双语无障碍标签、tooltip、Lock 同步图标
  和紧凑批量操作栏；
- 连接库回归扩展为 12 条记录，覆盖末条滚动、全部动作 bounds 和实际编辑点击；
- 工具栏与批量栏覆盖中英文 Group 语义、AX label、按钮 bounds 和批量 Move；
- 三套主题、明暗模式、中英文和 480×580/480×900 高度矩阵覆盖连接管理器边界、
  文件夹、工具栏、列表视口、首条记录及其全部动作。
- 连接库旧帧 Edit 回调按当前 UUID 重新解析，连接删除或失效时失败关闭；
  `6d1f1a1` 加入两条 stale painted Edit 回归。
- session-tab reveal key 纳入导航 rail 宽度，修复 Connections `236px` 切换到
  Monitor `260px` 后关闭动作可能被旧布局裁切的问题；`a33e4c3` 加入对应
  workbench 回归。

定向回归包括 workbench layout `11/11`、主题设计 `4/4`、stale painted Edit
`2/2`、连接库相关过滤测试 `84/84`、session-tabs `6/6`、磁盘监控独立回归
`1/1`、紧凑快速连接 `1/1`、工具栏/批量栏 `1/1`、窄窗主题/语言/高度矩阵
`1/1`、首次中文启动 `1/1` 和 core workspace layout `12/12`。完整
`python3 scripts/check.py` 返回 `0`，Python 为 `131 run / 1 skipped`，app 为
`1127 passed / 2 ignored`（测试清单共 `1129` 项，新增测试已进入二进制），14 项 doctest、
严格 workspace Clippy、依赖策略和三组 AI controller 均通过。门禁与未覆盖边界
见[本片测试记录](testing/records/2026-11-14-responsive-workbench-follow-up.md)。

此前独立严格只读复核针对 `78091e684f95cba5faecc54571992142f5fcd766` 返回
P0 `0`、P1 `1`、P2 `1`、P3 `2`；P1/P2 已分别由 `6d1f1a1` 和 `a33e4c3`
修复，当前文档也已同步修正 P3 指出的旧证据。针对包含本片最终文档快照的新提交，
针对文档提交 `251a8ee0398f5927685ae36f2f486e2d6d77d71d` 的新的独立严格只读复核
已在独立 worktree 和独立 Cargo target 中完成，P0/P1/P2/P3 均为 `0`，范围内
merge recommendation 为 `APPROVE`。复核重新确认当前 `1129` 项 app 测试清单、
stale Edit 与 session-tab reveal 回归、三套主题 token 解耦、双语 Group/List
语义、Clippy、fmt 和差异检查；作者门禁的 app 全量 `1127 passed / 2 ignored`
作为独立证据之外的作者回执保留。独立复核没有重新完成宽 `library` 过滤集和 app
全量测试，未把中止命令计入通过。当前状态为
`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。480×580/480×900 的 headless
GPUI 几何和受控交互已覆盖；当前源码随后完成精确 macOS arm64 包结构检查和
启动级原生短验，但没有读到可访问窗口。Windows/Linux 原生桌面、IME、VoiceOver/
NVDA、真实 SSH/Podman、外部 AI provider、签名、公证和安装升级继续保持开放。
证据见[macOS 精确包短验](testing/records/2026-11-14-responsive-workbench-native-smoke.md)。

## 2026-11-13 响应式工作区视觉切片

KeelShell 工作区已收紧窄窗口的工作台层级。900×580 下，AI、新建工作区窗口、
凭据库和配置备份/恢复入口保留可见图标、完整双语 AX label 与 tooltip；AI 打开后
助手列按 34% 比例并限制在 240–360px，活动终端仍保留至少 200px 宽和 80px 高的
受控预算。连接导航与主机监控继续是同一 rail 中的互斥 panel，连接页使用 236px，
监控页保留 260px。

导航 rail 现在公开 `Role::Group` 和 `Workspace navigation` 语义，header 复用
`ChromeTokens::pane_header_height`，并随互斥 panel 显示 `Connection navigation`
或 `Host monitoring`。Frost「霜白」、Graphite「曜石」、Lavender「雾紫」共用同一
视图路径；业务布局没有 theme id 分支，新增主题仍只扩展 registry/Palette/token。
本片没有新增依赖，直接依赖版本继续遵循 `x.y`。

focused GPUI 回归已通过：工作区布局 `10 passed / 0 failed / 1113 filtered out`，
覆盖三套主题、明暗模式和中英文的关键入口 bounds/AX label、AI/终端预算、导航
rail role/header label、236/260px 宽度及 pane header token。focused app Clippy
使用 `--all-targets -- -D warnings` 通过；格式与差异检查通过。详细证据见
[ADR 0124](adr/0124-responsive-workbench-polish.md)、[产品说明](product/NATIVE_WORKBENCH.md)
和[测试记录](testing/records/2026-11-13-responsive-workbench-polish.md)。

源码提交后仍需独立形成 macOS/Windows/Linux 原生窗口、IME、VoiceOver/NVDA、
真实 SSH/Podman、像素对比度、安装包和签名验收；headless GPUI 结果不代签这些边界。

## 2026-11-08 工作区窗口切换器

工作区顶部新增主题化的“工作区 N”入口，持续显示当前原生窗口组数量；展开后列出
每个窗口在进程生命周期内稳定的序号、会话数和当前主机或“快速连接”空态。点击条目会激活对应真实
`AnyWindowHandle`，不会迁移 SSH、终端草稿、AI 审核或远程面板。新建窗口继续复用
既有配置闸门和八窗口上限。

定向 GPUI 回归已通过：窗口入口 `3/3`、全部窗口生命周期 `19/19`、modal/focus
`12/12`、工作区布局 `8/8`、主题 `9/9`；窗口列表覆盖中文/英文和 Frost「霜白」、Graphite「曜石」、
Lavender「雾紫」三套主题。当前修复还覆盖统一 modal scope、Dialog 语义、全屏输入
隔离、focus trap、Escape 关闭和工作区焦点恢复。详细设计见
[ADR 0123](adr/0123-workspace-window-switcher.md)，产品说明见
[工作区窗口切换器](product/WORKSPACE_WINDOW_SWITCHER.md)，测试证据见
[测试记录](testing/records/2026-11-08-workspace-window-switcher.md)。

源码提交 `373d4157ae816a29a74aa72cf490f688e3563457` 的完整工程门禁已通过：
Python `131 run / 1 skipped`，app `1,119 passed / 2 ignored`，workspace 其余 Rust
测试、14 项 doctest、严格 Clippy、依赖策略和 readiness/observer/small-stack 三组 AI
controller 均通过。独立只读复核、当前源码精确包、真实 macOS
多窗口、Windows/Linux 原生窗口、IME、VoiceOver、真实 SSH/Podman 和外部 AI provider
仍待后续收口。

独立严格只读复核以
`373d4157ae816a29a74aa72cf490f688e3563457` 为基线完成，P0/P1/P2/P3 均为 `0`，
范围内 merge recommendation 为 `APPROVE`。复核确认 1,121 项 app 测试清单及窗口
切换器 modal/focus 定向回归；当前源码精确包、真实 macOS 多窗口、Windows/Linux
原生窗口、IME、VoiceOver、真实 SSH/Podman 和外部 AI provider 仍待后续收口。

## 2026-11-08 本片交付状态

本片已经补齐连续无换行保存与
`merge-adopt → merge-save → late newer draft/readback` 组合回归；生产代码没有修改。
完整工程门禁第二轮 `python3 scripts/check.py` 退出 `0`：Python
`131 run / 1 skipped`，workspace Rust `2,437 passed / 23 ignored`，app
`1,115 passed / 2 ignored`，14 项 doctest、严格 Clippy、依赖策略和三个 AI
controller 均通过；`cargo fmt --all -- --check` 与 `git diff --check` 也退出 `0`。

第一轮完整门禁在 Python `131 run / 1 skipped` 后于格式阶段退出 `1`，原因是新增
断言未按 `cargo fmt` 展开；格式化后重新执行并通过。该失败事实和第二轮完整回执
已记录在[组合回归测试记录](testing/records/2026-11-08-editor-format-merge-combinations.md)。

源码实现已固定为
`9134cfaa771057a0f246ead47479be3706248415`。首次独立严格只读复核以该 SHA 为基线
返回 `REQUEST CHANGES`：P0/P1/P3 为 `0`，唯一 P2 是提交后的状态文案仍写成待提交；
本次文档修订已将固定 SHA、复核结果和剩余交付边界写实。后续复核确认该 P2 关闭后，
第二次独立复核已返回 `APPROVE`，P0/P1/P2/P3 均为 `0`。下一步生成绑定最终
文档提交 SHA 的精确包并执行有限 macOS smoke。Windows/Linux 原生、IME、VoiceOver、
真实 SSH/Podman 长流程和外部 AI provider 继续保持未验证边界。

## 2026-11-08 内联合并审核视口

内联三方合并审核已收口为固定 `220 px` 卡片：正文拥有独立滚动句柄，动作区固定为
两行，展开、采用和关闭动作保持在正文滚动之外。正文滚轮由带 `review_id` 的处理器
显式限幅并阻止外层文件工具区移动；冲突切换、方案选择和完整版本切换都会复位正文
滚动。旧 GPUI 帧 listener 在审核替换后失败关闭，不能改变新审核。

当前定向回归已通过：stale frame `1/1`、conflict merge `27/27`、review viewport
`12/12`、editor format `13/13`；格式和差异检查通过。随后按共享 target 规则失效并
重编五个项目包，完整 `python3 scripts/check.py` 退出 `0`：Python `131 run /
1 skipped`，workspace Rust 测试 `2,435 passed / 23 ignored`，app 为
`1,113 passed / 2 ignored`，14 项 doctest、依赖策略、严格 workspace Clippy 和
三个 AI controller 专项全部通过。首次视口回归的失败原因、首轮 Clippy 修复和完整
门禁证据已保存在[测试记录](testing/records/2026-11-08-inline-merge-review-viewport.md)；
设计见 [ADR 0121](adr/0121-inline-merge-review-viewport.md)。

本片源码提交、SHA 回读、exact package 和 macOS 主题/双语短验已完成；独立复核因
agent 未返回可审计报告而保持 `NON_AUTHOR_REVIEW_UNAVAILABLE`。连续无换行保存、
late newer draft/readback 组合、Windows/Linux 原生窗口、IME、VoiceOver、真实
SSH/Podman 和外部 AI provider 继续保持未验证边界。

## 2026-11-08 编辑器格式与合并迟到读回组合回归

新增两个真实 GPUI/SFTP 回归，分别覆盖无换行文件连续保存的 LF 策略保持，以及
合并采用后保存发布期间产生较新草稿时的完整读回和下一次保存基线重建：

- `editor_format_tests`：`14 passed / 0 failed / 1103 filtered out`；
- `conflict_merge_tests`：`29 passed / 0 failed / 1088 filtered out`；
- 两个新增用例精确运行均为 `1 passed / 0 failed / 1116 filtered out`；
- `rustfmt` 与 `git diff --check` 均退出 `0`。

本片没有修改生产代码；产品合同、设计决策和失败证据分别见
[远程文本格式](product/REMOTE_TEXT_FORMAT.md)、
[远程文本合并](product/TEXT_CONFLICT_MERGE.md)、
[ADR 0122](adr/0122-editor-format-and-late-readback-regressions.md) 和
[测试记录](testing/records/2026-11-08-editor-format-merge-combinations.md)。
首次接入时的闭包上下文编译错误和并行工作树覆盖测试文件的事实已保留，修复后
重新确认测试清单包含两个新增用例。完整工程门禁已经通过；独立复核、精确包和
原生验收仍待本片后续步骤。文档修订后的第二次独立复核已返回 `APPROVE`，
P0/P1/P2/P3 均为 `0`。

## 2026-11-08 快速操作中心

快速操作中心已完成 Workspace 生产布局和第一批动作接入：纯动作目录提供双语
标题、关键词、稳定排序、平台快捷键提示和会话上下文过滤；Workspace 复用既有
连接、会话、工具面板、批量命令、依赖工作流、AI 和设置入口。一次性快速连接、
主机监控面板路由和首末/唯一会话边界也已接入。模态根、搜索输入、结果列表和
选中状态共用三套主题的语义 token，新增主题只扩展 registry/Palette/token。

当前定向 GPUI 回归为 `16 passed / 0 failed / 1091 filtered out`，覆盖
`Command/Ctrl+K`、双语、Escape 焦点恢复、Enter 连接管理器路径、一次性快速连接、
主机监控路由、会话边界和 `900×580` 内容边界。随后完整
`python3 scripts/check.py` 已退出 `0`：依赖策略、Python `131 run / 1 skipped`、
格式、workspace 严格 Clippy、workspace tests、doctests 和 readiness/
observer/small-stack AI controller 均通过。workspace 测试中 app 单元为
`1105 passed / 2 ignored`，core、mcp、session 的单元与集成测试均通过；带有
Podman、OpenSSH、真实 Linux 采样等显式环境要求的可选测试继续保持 ignored。
源码提交
`c13724a9eab9df3d97fdb3ed1eaf25b5dcfabffa` 已推送，local、tracking 和 remote
SHA 已读回一致。新的独立严格只读复核已完成，P0/P1/P2/P3 均为 `0`，范围内建议为
`APPROVE`；
新建窗口、分屏、布局保存、原生多平台输入/辅助技术、真实 SSH/Podman 和外部
AI provider 保持后续验收项。当前只剩文档收口提交。设计见
[ADR 0120](adr/0120-command-palette.md)，实现说明见
[快速操作中心](product/COMMAND_PALETTE.md)，测试回执见
[快速操作中心记录](testing/records/2026-11-08-command-palette.md)。

## 2026-11-07 连接库选中层级与主动作

连接库视觉层级已完成一片源码候选并提交为
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9`。宽表格和窄卡片共用
selected-row 反馈：行公开 `aria-selected`，使用当前主题的 `surface_active`
背景和 `accent` 左边界，悬停使用 `surface_hover`。活动连接库中的 Connect
改为 GPUI Kit `primary`，编辑、移动、复制和回收站动作继续保持次级层级；回收站
仍不会暴露连接入口。视图继续只消费 registry/Palette token，没有主题 ID 分支，
未来新增主题只扩展注册定义。

定向连接库回归为 `27 passed / 0 failed`，selected-row 精确回归为
`1 passed / 0 failed`，应用严格 Clippy、格式和差异检查通过。完整
`python3 scripts/check.py` 退出 `0`，实际完成 Python `131 run / 1 skipped`、
app `1089 passed / 2 ignored`、workspace 其它测试、doctests、依赖策略和三组 AI
controller。首次新增测试曾把可访问标签误当可见文本而失败，已保留失败事实并
精确重跑通过；随后还收敛了 selectable role 与选中行悬停状态。记录见
[连接库视觉层级回执](testing/records/2026-11-07-connection-library-visual-hierarchy.md)，
设计决策见[ADR 0119](adr/0119-connection-library-visual-hierarchy.md)。

独立严格只读复核已以最终源码提交
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9` 为基线完成，P0/P1/P2/P3 均为
`0`，merge recommendation 为 `APPROVE`；复核实际执行 selected-row 精确回归
`1 passed / 0 failed / 1090 filtered out`。此前包不包含最终提交
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9`，不能代签本片真实
桌面颜色。macOS 当前源码包、Windows/Linux 原生窗口、完整键盘/IME/VoiceOver、
真实 SSH/Podman 长流程和外部 AI provider 继续保持独立验收边界。

## 2026-11-07 exact package smoke 与复核边界

当前主题选择器源码提交 `37b8eaf7d65fe31133c5e5058848341c50c8a820` 已重新构建
精确 macOS 开发包 `work/native-cua-20261107/package-r10/KeelShell.app`，
结构检查退出 0，manifest 绑定该 SHA。解锁桌面短验确认 Frost、Graphite、
Lavender 三套主题可以切换，System/Light/Dark 控件和 AX 文案可读，空连接库返回
正常；exact package 进程已清理。

本次仅覆盖主题弹窗和空工作区短流程，未覆盖真实 SSH、分屏、900×580、IME、
VoiceOver、签名/安装和 Windows/Linux。针对 exact source 的三次新独立复核尝试
均未在有界等待内返回正文，当前状态为 `NON_AUTHOR_REVIEW_UNAVAILABLE`；不能把
本片标成非作者批准。详细包和 smoke 证据见
[主题选择器记录](testing/records/2026-11-07-theme-picker-layout.md)。

## 2026-11-07 主题选择器可见性与交互回归

主题选择器已完成一片可验收的视觉和布局修复：主题卡片目录独立滚动，明暗模式
控制固定在弹窗可见区，三套内置主题 Frost「霜白」、Graphite「曜石」、
Lavender「雾紫」均可选择。预览和按钮继续由主题 registry 与语义 token 驱动，
以后新增主题只扩展注册定义，不增加业务视图分支。

主题模块定向回归为 9/9，磁盘监控主题联动为 1/1，格式、差异检查和应用严格
Clippy 通过。完整 `python3 scripts/check.py` 退出 0，Python 为 131 run / 1
skipped，完整 app 测试为 1088 passed / 2 ignored；其余 workspace tests、
doctests、依赖策略和三组 AI controller 也通过。详细证据见
[主题选择器记录](testing/records/2026-11-07-theme-picker-layout.md)。

本片仍不代签 macOS/Windows/Linux 原生窗口、900×580 accessibility frame、
IME、VoiceOver、安装包和真实外部 provider；这些范围继续按平台记录单独验收。

## 2026-11-07 Reef / reef-template 参考边界

状态：**文档切片已完成，可独立合并**。

新增 [ADR 0118](adr/0118-reef-reference-boundary.md)，记录本轮对指定 Reef
checkout（`75ccbf5d864fadb543ecfa62fc9871c7cff04153`）和 reef-template checkout
（`03bd71c096fdd941e389584c44bdcb40dfed0cdf`）的只读检查。证据覆盖：

- Reef 的 resolver 3、workspace package/依赖集中声明、`reef-compose` feature
  装配、`application` 的 owner/启动失败回滚/join/shutdown report，以及
  Core 的配置 loader 和 revision snapshot；
- reef-template 的 workspace 继承、单一 `crates/app`、公共/dev/release 配置、
  `build.rs`/入口、模板生成限制和工程同步文档；
- Reef checkout 检查时存在一处预先存在的 `Cargo.toml` 修改，模板 checkout
  干净；两者都没有被本轮修改或清理。

后续 KeelShell 只采用这些工程原则，继续保持 `keelshell-app`、
`keelshell-core`、`keelshell-session`、`keelshell-ai` 的桌面边界。服务端
runtime、私有 Git 依赖、内网 URL、HTTP/MQ/DB/telemetry 等 Reef 中间件、
模板 Liquid 和服务部署约定均不进入桌面产品。若未来需要真实 Reef 接入，先
创建新的 ADR，补齐公开依赖、版本 revision、配置路径、生命周期和跨平台验收。

## 2026-11-07 会话、连接库与 AI 助手界面增强

本片把连接库范围反馈、会话条键盘导航和 AI 审核阶段统一到可扩展的工作台表面：
连接库区分范围与空态，会话条显示数量并提供平台快捷键，AI 助手显示“捕获、提问、
预览、发送、审核回复、保存知识”六阶段。普通回复到达后自动显露审核区，长请求
仍在正文滚动区保留完整预览。

定向结果为连接库 26/26、AI 6/6、会话条 5/5、快捷键 1/1，长 MCP、Agent 编辑
问题和知识保存回归各 1/1，`cargo check`、格式和差异检查通过。首轮 AI 回复可达性
测试失败后补充响应锚点，精确重跑 2/2；Agent 编辑问题曾暴露旧布局滚动，最终在
render 后按实际几何修复。完整工程门禁 `python3 scripts/check.py` 已退出 0；
初始精确提交 `dd6c88b0710bb56ca785eeeb83ee0835a1b77ede` 已推送；随后窄栏修正
`58fd2d248302958675b0652b7bc15b8aee77b580` 也已推送，新的独立严格只读复核
返回 P0/P1/P2/P3 均为 0 并建议 `APPROVE`。当前源码的 `package-r7` 已按共享
target 规则重新构建、精确绑定并通过 macOS 静态结构检查；解锁后的短范围 smoke
确认六段工作流完整换行，AI 助手侧栏和 AX 元素可见。完整原生桌面证据仍待执行。设计见
[ADR 0117](adr/0117-session-navigation-and-assistant-workflow-surface.md)，
回执见[本轮记录](testing/records/2026-11-07-session-library-ai-ui-polish.md)。

## 2026-11-07 当前交付位置

当前候选已完成 pane 连接身份栏的源码、测试和文档收口准备。连接身份栏覆盖
真实 SSH 端点、协议/编码、生命周期状态、恢复占位和双语 AX；live IPv6 端点
已统一以括号包裹主机；未保存连接与配置缺失分别呈现，主题字体通过 registry
typography token 提供。三套内置主题 Frost、Graphite、Lavender 仍全部可选，
新增主题只扩展主题定义，不让业务视图按主题 ID 分支。

定向 GPUI 回归与完整工程门禁通过：Python 131/1 skip，app 1079/2 ignored，
core 197，session 434/16 ignored，workspace integration、MCP、SSH loopback、
transfer、doctest、readiness、observer、small-stack AI controller、严格
workspace Clippy、格式和差异检查均通过。首轮完整门禁曾因新增测试的
`needless_borrow` 失败，修复后的结果已重新执行并记录。

route/split 夹具在完整 app 并行负载下的首次 10 秒就绪等待曾早于生产 15 秒
SSH 操作期限结束；gateway、target 和跳板 SSH 会话当时仍存活。当前测试使用
有界 20 秒 UI 就绪等待，route server session 使用有界 120 秒上限，最终 app
单线程与并行重跑均为 1079 passed / 2 ignored。session 曾出现一次
`traceroute_hop_limit` 预期 hop-limit、实际 timeout 的负载抖动
（129 passed / 1 failed / 4 ignored），精确重跑、四线程重跑和最终 workspace
门禁通过，失败事实保留在测试记录中。

源码提交 `b2f875458e42cf374cb3d388d808e986a415c2cd` 已通过完整工程门禁并生成
精确来源绑定的 `package-r5`；旧 `package-r3` 继续承担详细 macOS 主题、双语、
横/纵分屏、合并、关闭面板、真实回环 SSH、连接中断和恢复场景人工观察。当前
`package-r5` 已完成结构检查和短范围主题/双语 smoke check。900×580 紧凑窗口因
无边框窗口 accessibility frame 异常保持未完成。下一步按交付顺序执行：固定
`b2f8754` 重新进行独立严格只读复核；本轮已启动两次 exact-tip 复核，但均未在
有界等待内形成可审计正文，状态保持 `NON_AUTHOR_REVIEW_UNAVAILABLE`。后续仍需
补一次有正文的复核，再更新合并建议。Windows/Linux 原生桌面、完整 IME/VoiceOver
和 Podman 真实联调仍保持开放。

上一片工作区视觉节奏收口已提交并推送，提交为
`e3226e7097b72f6927fbc38f05a94a365d8269d7`，本地、tracking 和远端 SHA 一致。
它包含主题 registry 的 chrome 几何 token、宽窗品牌与当前主机上下文、命令审核
目标、primary 执行动作和会话标签的主题化 hover/active 状态。

当前候选继续推进 pane 连接身份栏：所有终端 pane 在画布上方持续显示真实连接
主机、用户/端口、SSH/编码和连接状态，分屏时与会话标签和比例控制共享紧凑
header。实现只消费 Workspace 已有的连接/生命周期快照，不把传输状态塞进
TerminalView，并补齐中英文、三主题、最小窗口和焦点/AX 回归。设计和验证见
[ADR 0116](adr/0116-pane-connection-identity-bar.md) 与
[测试记录](testing/records/2026-11-07-pane-connection-identity-bar.md)。

当前身份栏和 IPv6 endpoint 修复的定向回归、完整 `python3 scripts/check.py`、
`package-r5` 结构检查及短范围 macOS smoke check 已通过；详细 SSH native 观察仍
归属于旧 `package-r3`，最新源码提交的独立非作者复核仍待完成。

旧固定提交的证据绑定问题已由 `9d02440` 及后续文档修订关闭；旧复核指出的
IPv6 live endpoint 问题已由 `b2f8754` 修复，当前包 provenance 已由 `package-r5`
精确 manifest 绑定。当前 exact-tip 复核仍是 `NON_AUTHOR_REVIEW_UNAVAILABLE`，
不能视为当前提交批准；后续必须固定 `b2f8754` 补一次有可审计正文的代码/功能
复核，再更新合并建议。

`e3226e7097b72f6927fbc38f05a94a365d8269d7` 已推送到
`feature/main-visual-integration-20261106`，本地、tracking 和远端 SHA 一致；
`main` 尚未推进。三组继续回归分别为 5/5、14/14 和 11/11 通过。独立非作者复核
仍因 agent 无报告、供应商 API-Key 403 及本地 agent 工具循环而不可用；真实 macOS
桌面通道仍报告锁屏，因此多窗口和编辑器原生操作保持 `NOT_RUN`。
文档提交后的首次完整门禁曾在 small-stack controller 收到 `EmptyReply` 退出 101；
精确重跑通过，第二次完整 `python3 scripts/check.py` 退出 0，保留首次失败证据。

## 当前集成

`feature/main-visual-integration-20261106` 已把 `main` 的最新功能基线与工作区
外壳视觉增量合并。集成后的 UI 使用 Frost、Graphite、Lavender 三套注册表主题和
语义 surface/chrome 层级，活动会话、连接导航、工具栏和状态栏共享同一套可扩展
组件 token。视觉来源的定向门禁、集成后的完整工程门禁和 design/workbench 定向回归
已经通过；独立非作者代码复核固定在源码提交 `26e11c5`，文档编号清理由 `ecdc8c1`
收口并已推送。开发包、原生桌面和跨平台运行仍需单独形成新的可交付证据。新主题
继续只扩展注册表、Palette 和组件形状，业务视图不增加主题 ID 分支。

复核发现监控文件系统容量条仍有硬编码颜色 `0x448eaa`。修复提交 `ef110d2` 增加
独立的 `Palette.resource`，六个主题变体均提供资源色，监控视图按当前主题读取该
token，并以轨道对比度测试防止回退到固定颜色。修后设计 4/4、磁盘相关 10/10、
独立主题回归 1/1、严格 Clippy 和完整 `scripts/check.py` 均通过；新的非作者只读
复核任务在有界等待内未返回，备用模型因 API-Key 权限返回 403；父级固定提交只读
核对未发现新的 P0/P1/P2/P3。当前采用
`LIMITED_PARENT_REVIEW / APPROVE_SCOPED_WITH_REVIEW_SERVICE_LIMITATION`，保留独立
子代理复核服务未形成报告的限制，不把它写成已获得非作者批准。

2026-11-07 的独立工作窗口与编辑器动作可达性候选已导入当前分支。新增实现通过
定向窗口、配置闸门、worker ownership、loopback SSH、编辑器视口和 11 项
临时目录根路径回归；完整 `scripts/check.py` 实际退出 0：workspace 2,397
ordinary / 23 ignored、11 rustdoc、Python 131 / 1 skipped，三组 AI controller
也通过。当前仍需新的严格只读独立复核和真实 macOS 多窗口/编辑器采用保存验收，
Windows/Linux 原生桌面验收继续单独保持开放。
两次新开的有界只读复核 agent 都未返回可审计报告，不能把当前候选标记为独立
批准；父级静态差分检查已修正新增的 worker stderr 输出，但不代替非作者复核。

## 最新交付

main已普通推送`fa7c05914e0eefc0f24ab32f8af690bdc50a63ed`，交付读回一致。
本批按项监控能力和审核式知识分类的536输入fresh完整门禁、开发包、
限定macOS A/B原件及正式文档均获各自非作者批准。
2313 ordinary/23 ignored、14 rustdoc、Python131/1 skip；28对原生AX/JPEG，
首轮HOLD/UNKNOWN保持，完整native finish开放。精确三平台CI均已成功；
唯一观察器exit2已实际wait/reaped；原Linux附件GET失败保留，新单次补充读回与
两平台各11项OpenSSH获独立`current-ci-r2`限定收口批准。三平台各204候选实际通过。
CI不代签桌面验收。见[本批记录](testing/records/2026-11-06-monitor-knowledge-categories.md)。

## 2026-11-07 工具栏远程上下文布局修复

工作区顶部工具栏已修复紧凑窗口中远程上下文逐字竖排的问题，并收口 1280px
断点：1280px 保留品牌区但采用紧凑动作标签，低于 1280px 隐藏品牌区；上下文
标题和当前主机值使用单行省略，紧凑布局保留 112px 最小宽度。布局回归覆盖
900/1280/1440 三档、中英文、三主题矩阵中的既有工作区场景，完整 app 为
1087 passed / 2 ignored，完整 `scripts/check.py` 退出 0。

源码提交为 `2d05525c00d21455420d25cce675bacdedf8a73a`。当前 macOS 包
`work/native-cua-20261107/package-r9/KeelShell.app` 已做结构检查和默认窗口短验；
紧凑尺寸因 GPUI accessibility frame 限制仍未形成原生通过证据。当前 exact tip
的独立只读复核已返回 P0/P1/P2/P3 均为 0、范围内 `APPROVE`；该复核没有代替
作者完整工程门禁或原生验收。Windows/Linux 原生桌面、完整输入/辅助技术矩阵、
当前包真实 SSH/Podman 长流程和外部 AI provider 仍开放。
测试证据见[工具栏上下文布局修复记录](testing/records/2026-11-07-toolbar-context-layout-repair.md)。

## 当前并行开发

| 优先级 | 用户功能 | 真实状态与下一步 |
| --- | --- | --- |
| P1 | [远程文本格式](product/REMOTE_TEXT_FORMAT.md) | r3三项连续编辑P2关闭；新增无换行连续保存 GPUI/SFTP 回归 `14/14`；父543输入门禁/包通过；native-r1限定批准macOS原生BOM/CRLF取消与确认，完整编辑器验收HOLD |
| P2 | [审核式有界traceroute](product/TRACEROUTE_DIAGNOSTICS.md) | stderr精确版本合同与最终门禁通过；同包真实SSH UI各审核确认UDPv4/v6一次，均一跳回复；原生ICMP/取消和完整平台另验 |
| P1 | 独立工作窗口 | 源码已导入当前分支；空白新窗、各窗独立会话/审核、安全关闭、共享 StateStore 协调、worker 回收与 loopback SSH 定向测试通过；完整工程门禁通过，独立复核和真实多窗口验收待完成 |
| P1 | 编辑器动作可达性 | inline editor 与三方合并正文独立滚动、底部动作固定、手工输入最小高度修复已导入；视口专项和本片 `merge-adopt` / `merge-save` / late newer 回归通过；完整工程门禁、独立复核和真实原生复验待完成 |

当前候选已装配本轮源码和文档输入，五个项目 crate 已按共享 target 规则失效重编，
42 个新增具体测试名均在当前库存中。修复 macOS 临时目录根路径夹具后，完整工程
门禁实际退出 0：2,397 ordinary / 23 ignored、11 rustdoc、Python 131 / 1 skip，
三组 AI controller 和 workspace strict Clippy 均通过。
新包构建与macOS包检查完成，全部成员和41项资产已读回；
Gauss的combined-r3/r4已批准限定源码及门禁/新包；
新runtime-r3原生797.330164/900秒结束、外层28510实际wait0/reaped，
38组AX/JPEG及完整字节原件获native-r1限定批准；
新增P2 NR1-P2-01与完整native HOLD保持，下一片修复。
源代码通过、实际工具、原生、CI和发布安装分别验收，见[组合记录](testing/records/2026-11-06-editor-traceroute-integration.md)。

## 后续产品缺口

| 优先级 | 范围 | 当前边界与下一步 |
| --- | --- | --- |
| P1 | 工作区 | 已有分屏、比例、逐叶标签、显式保存和单窗恢复；多OS窗口、外框及完整关闭/重连审核矩阵仍开放 |
| P1 | 平台与发行 | 继续Windows/Linux真实桌面、完整几何/IME/辅助技术、六目标签名/公证和隔离安装回退；三平台CI不等于桌面验收 |
| P1 | 外部AI集成 | API/受控CLI Ask、逐步审核Agent及对外MCP已有实现；供应商账户/完整外部客户端、CLI登录身份复用的请求前隔离证明仍待验 |
| P2 | 日常文件操作 | UTF-16 LE/BE 编辑、审核、成功保存读回和解码差异预览已实现；继续原生文件管理器拖放、更多编码边界、完整键盘/辅助技术及其他平台验收；现有字节与备份合同不扩大 |
| P2 | 监控与网络 | 按项能力与缓存已交付；继续df/ps缺失后恢复、ss可用、真实诊断错误/取消，以及远端macOS/Windows适配 |
| P2 | 持久化隧道 | 命名规则、三方向转发、流量、显式审核/停止及运行状态持久化与显式恢复已实现；完整重连/管理矩阵、跨平台和真实远端验收仍开放 |
| P2 | 配置恢复/同步 | 已有审核恢复、回退与认证加密目录同步；跨机器挂载、断电耐久性及最终一致同步器待验 |
| P3 | 高级传输 | 审核式归档/路径穿越防护、rz/sz实机、用户中继与可重复收益验收另行实施 |
| P3 | AI知识复用 | 本地分类、显式Ask和审核式批量管理已交付；跨设备同步及更多复用待设计，不恢复旧SSH权限或自动执行runbook |

2026-11-14 的更多文本编码切片已实现 UTF-16 LE/BE 的 BOM 识别、严格解码、保存、
审核、合并、成功读回和解码差异预览，并通过 core、session loopback 与 GPUI
定向回归。完整工程门禁已通过；最终修复提交后的独立复核和各平台原生验收仍未
完成。当前只做本地提交，尚未推送。

清理遵循资源所有权。已合并分支、无消费者自有容器/镜像及时清理；
受其他任务或primary保护的工作树保留，失败/UNKNOWN与不可变原件不删改。
产品能力从[能力清单](product/CAPABILITIES.md)及对应指南查询。
