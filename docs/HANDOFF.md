# KeelShell 当前交接 — 2026-11-16

## 当前批次：编辑器键盘与原生文本流程（集中门禁与限定复核通过）

基线 `92c8ddaab84d19a1044a5803d73a0495d0b89354`。按用户要求集中实现后统一
集成、专项修复、非作者复核和最终完整门禁，只做本地提交。独立作者在受管
worktree 实现限定焦点的保存审核/查找快捷键，不运行 Cargo；父负责真实 macOS
文件与编码操作、集成和验证。唯一 target 归属没有切换。
限定焦点的 Cmd/Ctrl+S 只展开完整保存审核，Cmd/Ctrl+F 展开草稿并聚焦唯一查找框；
重复按键或 Enter 不确认，终端和其他输入保留原路由。另修 UTF-16 审核把编码末字节
误当末尾 LF/CR 的显示问题，精确出站字节与备份/保存合同不变。

最终冻结 715 项工程输入，SHA-256
`a17d4a12b9187755a59af96f99afa5f0998498b0de1a980de0bb9c13f2916c3c`。
完整门禁 r2 实际 `exit 0 / reaped / 698.707s`：普通 Rust `3168/0/48 ignored`、
rustdoc `14/0`、工程 Python `284 run / 1 skipped`、helper `25/25`，格式、严格
Clippy、x.y 政策和三组 controller 通过。11 个新增默认 Rust 名逐项通过，门禁后
完整路径及字节零漂移。r1 的历史 SHA fixture 失败保留；只修测试为真实读取 HEAD，
生产来源/库存 guard 不变。三份新非作者报告分别批准源码、Python 窄差分及限定
原生证据，累计一个 P2 已关闭，各范围开放 P0–P3 为 0，四个 agent 均已释放。
Rust 复核使用核对后的父预构建程序，没有独立重新编译；原报告待门禁条件由最终
成功回执另行绑定，不改写原报告。

真实 macOS Frost 接续操作完成 UTF-16 LE 与 GBK 两份审核保存：正文/Enter、
Cmd+F 焦点、Cmd+S 展开、重复不确认、取消不写、各一次确认后的完整目标及唯一
原始备份读回。早期 r1–r7 失败保留，不能由最终接续成功反推完整脚本成功。
部分键盘投递仍 HOLD，Graphite 未到达新增快捷键，Lavender 本批原生 NOT_RUN；
保存结果提示 PNG 可见但 AX 缺文本，UTF-16 补发保存无单列事件回执。
12 个受管根进程已结束，181 条 birth 身份无匹配、八次结束端口关闭、四个私有
目录 absent；作者受管 worktree 已归档且路径和 Git 记录 absent。没有新建用户
chat 或容器。正常 arm64 GUI/MCP 构建及暂存包结构检查通过，没有安装或发行。

从[本批记录](testing/records/2026-11-16-editor-keyboard-native.md)和
[持久证据](testing/records/2026-11-16-editor-keyboard-native-evidence/README.md)接续；
本地源码提交与自有临时目录清理待 Git 绑定后完成。后续只改文档不重复运行
工程输入未变的门禁。Rust/Cargo 实测 `1.100.0`、默认 stable，无 toolchain pin、
rust-version 或新增依赖；官方最新版本查询超时并 reap，不签全部依赖最新。
三主题共用 registry/Palette/token；完整产品、三平台桌面、IME/辅助技术与发行保持开放。

## 当前批次：macOS 资源监控与 Linux 能力恢复（集中验证收口）

基线为本地 `539c172389b4450570026f023ff89d50df90f910`。两个独立作者并行实现
Darwin 内核采集/严格解析、Linux 真实能力恢复实验，父负责模型、原生 UI 与集成。
唯一 Cargo target 继续属于父 checkout，没有切换来源；不重复作者全仓编译。
实际 session monitor `81/0`、app monitor `70/0`、Mac disposable SSH `6/6`、
Linux 恢复 `2/2 opt-in` 及正常 arm64 GUI/MCP 构建通过。三份主要非作者、测试
lint 与最后三文件测试兼容窄审共五份报告均批准，六项 P2 全部关闭。

首轮完整门禁在测试 lint 失败，r2 在旧磁盘脚本 guard 失败；随后集中 session
包专项定位另一处旧正文比较与外部测试缺 inner cfg。只修测试的局部 lint、
精确正文/包装断言及首行 cfg，生产代码和安全/数值/超时断言保持。
最后格式、remote-only `5/0/0`、session monitor `81/0/0` 通过；原失败、
UNKNOWN 和未完成回执原样保留，见[本批记录](testing/records/2026-11-16-host-monitor.md)。
最终 r6 为 713 项工程输入，SHA-256
`e0b6ae274c4fad8d1246df5cba5c21b779b8afa6341385818a99b0a2264effd3`；
相对 r2 仅三个测试文件变化，其余 710 项不变。最终完整门禁 r3 实际
`exit 0 / reaped / 680.172s`：普通 Rust `3157/0/48 ignored`、rustdoc `14/0`、
工程 Python `284 run / 1 skipped`、helper `25/25`，格式、严格 Clippy、x.y
政策和三组 controller 通过。52 个新默认 Rust 名逐项通过，713 项完整路径、
字节和摘要零漂移；五份原报告的 PENDING 条件另由最终成功回执绑定，不改写原件。

设计见 [ADR 0143](adr/0143-darwin-resource-monitoring.md)。两个作者受管 worktree、
匹配分支及本批七个作者/复核 agent 均已清理。
自有容器/网络/镜像和私有数据已回收，完整 ID/name 再读回 absent，正常程序
摘要、共享 base 与唯一 target 保持。旧工具 helper UNKNOWN 的恢复依据为原 CLI 子进程
实际 reap/exit 0 和新 owner/full-ID/tag，不能从后来 absent 推断旧创建已完成。
本地源码提交 `c5ad5dca621207644e39aa28f21f7f20a3116cdf`。
1035 份原件已无损封存到[持久证据](testing/records/2026-11-16-host-monitor-evidence/README.md)；
713 项工程输入及所有原件已通过 Git index/blob、精确源码提交和清理后重新绑定，
零 mismatch。随后回收本批 1053 个临时文件、逻辑 `898,623,036` 字节；
正常程序、唯一 target、共享 base 和其他任务资源保留。后续只补文档回执，
不重跑工程输入未变的完整门禁。
Rust/Cargo 实测 `1.100.0`、默认 stable，无 toolchain pin；无新增依赖。
本批官方最新版本检查 45 秒超时并 reap，不签全部依赖最新。
只本地提交、不 push；三主题继续共用 registry/Palette/token。
完整产品、三平台桌面、IME/辅助技术与签名发行仍开放。

## 当前批次：原生流程与真实 IP 网络实验，集中门禁与限定复核通过

基线 `5a6b30a2b9cca6d89aea551409738145608ac689`。继续按并行实现、集中集成验证
修复、非作者复核、本地提交推进；不 push。父在独立状态目录、暂存 macOS arm64
包和 disposable localhost SSH 服务上实际检查窗口、三主题及路径比较。
隔离作者负责自有 Linux network namespace 的真实 IP 延迟/丢包实验。
唯一 Cargo target 归属仍保持父 checkout，作者不运行 Cargo。
从[本批记录](testing/records/2026-11-16-native-network.md)和
[持久证据](testing/records/2026-11-16-native-network-evidence/README.md)接续；
本批工作目录在完成 Git blob 核验后已回收，不作为后续唯一证据入口。
真实 macOS 流程发现并修复待完成跳板被周期清理、审核正文缺 AX 语义两项问题；
19 项相关回归与非作者预构建独立执行通过。正常 GUI/MCP 构建及暂存包检查通过。
修后原生实际核对三处公钥 pin，目标认证停留后完成两条 SSH/SFTP 连接，
显式审核并完成 18 个比较样本；三主题切换、中英文结果和请求 900×580 的
窄窗口已留截图与 AX 原件。初始自动流程 `exit 1` 保留，后续滚动接续单列；
AX 屏外存在不算同屏可见。会话已停止，两个端口关闭，18 条 birth 身份无活匹配。

首轮全门禁 `exit 0 / reaped / 705.126s`：Rust `3105/0/40 ignored`、
rustdoc `14/0`、工程 Python `225 run / 1 skipped`、helper Python `25/25`。
网络非作者发现超时创建可能被提前签 absent 的 P2，已补真实子进程红/绿回归，
创建请求未知时保留 `UNKNOWN/CleanupUnconfirmed`。Python 专项 `24/24`，
全套重跑 `227 run / 1 skipped` 通过。最终真实 netem r7 五次 opt-in 通过，
三场景各 18 样本，受损侧实际 skb drops 分别为 7 与 8，控制侧均为 0；
9 个自有资源、164 个客户端及私有目录清理确认。r5、失败及中断原件保持。
非作者补充发现返回非零/信号退出仍误记完成的残余，另两项真实子进程回归先红，
修后专项 `26/26`。旧 `full-gate-r2` 已停止并实际 reap，
`exit -15 / 404.37s`，3 条 birth 身份无活匹配，自有中断临时目录已回收。
最终 r3 冻结 699 项，SHA-256
`f435b9fa78f9a40bc2962fa3043b983cf5818e1ab4e34db5deae5960dfc50e21`；
相对 r1/r2 仍仅 runner 与其 Python 测试变化，其余 697 项不变。
最终 `full-gate-r3` 实际 `exit 0 / reaped / 701.952s`：Rust
`3105/0/40 ignored`、rustdoc `14/0`、工程 Python `229 run / 1 skipped`、
helper Python `25/25`，格式、严格 Clippy、x.y 策略与三组 controller 通过，
两个新默认 Rust 名逐项通过，最终完整路径和摘要零漂移。
app 非作者 r2 已批准七文件、开放 P0–P3 为 0，并已关闭；
其签署时全门禁待定保持，父已绑定最终 gate 与不变 app 输入；网络非作者 r3 也已批准、
唯一 P2 及其残余全部关闭，开放 P0–P3 为 0，独立 Python `26/26`，
已关闭 reviewer。两报告原签署与预构建/非独立重编译限制保持。
本地源码提交 `6699148b0fd9b5455c69a2ae6b9187ecb1cc3633`，
[提交绑定](testing/records/2026-11-16-native-network-evidence/commit-input-binding.json)
核对 699 项工程输入、1422 份原件与当前文件零 mismatch；
清理后再次绑定通过。删除本批 1435 个临时文件、逻辑 `781,265,332` 字节，
[最终清理](testing/records/2026-11-16-native-network-evidence/cleanup-final.json)
记录正常程序、唯一 target、共享 base image 及其他任务资源保留。
作者受管 worktree、匹配分支与本批三个 agent 均已清理；没有新建用户 chat。
后续仅补文档回执，不重跑相同工程输入的门禁。
在线依赖检查 45 秒超时并 reap，无新增依赖，不签全部最新。
完整产品总目标保持 active，三平台完整桌面、IME/VoiceOver、签名发行和 M4 仍开放。

## 当前批次：审核式 SSH 路径比较，集中门禁通过

基线 `162d883460a98cd03056731fd8d4be76a41d5c15`。继续按独立功能并行实现、
同批集成、集中验证修复、新非作者复核、本地提交的节奏推进。传输与真实实验
在两个受管 worktree 开发，父负责原生监控入口及双连接授权。作者不运行 Cargo；
唯一 target 仍为 `work/recovery-batch-integration-20261115/target`，归属未切换。
设计见 [ADR 0141](adr/0141-reviewed-ssh-path-comparison.md)，
记录见 [本批记录](testing/records/2026-11-16-ssh-path-comparison.md)。
最终 r3 为 669 项工程输入，集合 SHA-256
`066d7835e75d09895ae7a97f5b83863904a8dcbb0419bb1d8e865f89b7a047e0`。
完整 `full-gate-r1` 实际 `exit 0 / reaped / 698.757s`：普通 Rust `3103/0/40 ignored`、
rustdoc `14/0`、工程 Python `203 run / 1 skipped`、helper Python `25/25`，
fmt、严格 Clippy、x.y 策略与三组 controller 通过。26 个新默认 Rust 名逐项通过，
门禁后输入零漂移；原编译、app 测试与伪 DuplicateAck fixture 失败保持原件。
最终真实 OpenSSH r2 五次 opt-in 测试通过，三场景各 18 个样本，正常 GUI/MCP
arm64 Mach-O 构建通过。两轮 197 条 birth identity、16 次端口及私有目录均回收。
两个新非作者 r3 分别批准 app 与传输/runner 范围，累计一个 P1、四个 P2、一个 P3
全部关闭，各范围开放 P0–P3 为 0；独立预构建执行 `11 + 15`，没有独立重编译。
原签署时门禁/正常构建条件均已由父在相同冻结满足，原报告不改写；
两名 reviewer 已实际关闭。两个作者 worktree 和三个自有探查容器均再次确认 absent。
原件从[持久证据](testing/records/2026-11-16-ssh-path-comparison-evidence/README.md)接续；
源码已本地提交 `030835a2e53ebaab86f582b3041c0354ca78c5d0`，
[提交绑定](testing/records/2026-11-16-ssh-path-comparison-evidence/commit-input-binding.json)
核对 669 项工程输入与 258 份原件的精确 Git blob/当前文件，零 mismatch。
随后回收本批重复目录与 13 个测试副本，共 `698,350,238` 字节；
[最终清理](testing/records/2026-11-16-ssh-path-comparison-evidence/cleanup-final.json)
确认目录 absent、正常程序摘要保持、唯一 target 与其他任务资源保留。
后续只补文档与回执，不重跑工程输入未变的门禁。
新增依赖为零；本轮官方最新版本检查 45 秒超时，已终止并 reap，未签署全部最新。
只本地提交，不 push。三平台原生、签名发行、IP 丢包网络实验及完整 M4 保持开放。

## 当前批次：显式远程文本旧编码，工程门禁通过

基线 `2cfe0deb732db78f43a9b76d9e2c7e5c214a1f24`。core 与 session
两个并行作者已完成并经逐文件 SHA256 导入，作者已关闭，受管 worktree 已归档，
实际路径和 Git worktree 记录均 absent。父继续集成原生界面与真实联调。
当前实现 GBK、GB18030、Big5、Shift_JIS、Windows-1252，默认 Unicode 不扩大；
读取意图与打开格式分离，保存拒绝任何有损映射。
设计见 [ADR 0140](adr/0140-explicit-legacy-editor-encodings.md)，
接续见 [本批记录](testing/records/2026-11-15-legacy-editor-encodings.md)。

core 文本 `63/63`（新增 40）、session 专项 `36/36`（新增 30、复用 6）、
app 新专项 `14/14` 与真实 disposable localhost OpenSSH `17/17` 已通过。
两名新非作者范围复核均无开放 P0–P3；独立执行使用父预构建副本，没有独立重编译。
`full-gate-r1` 实际退出 `1 / reaped / 513.085s`，服务器归档为 `39/1`。
已用有界诊断复现：祖先目录在 stat/open 之间因旁支创建/删除而变更链接数和时间，
完整 stat 比较误拒绝。现只对祖先比较设备/inode/mode/uid/gid/flags，所选树完整
快照及新建对象 ACL 合同保持。六项新真实文件系统回归先取得 `31 run / 2 errors /
1 skipped` 的红证据，修后 `31 run / 1 skipped` 通过；core/session 归档 `14+40`
项通过。非作者 `report-r3` 已批准这两文件窄差分、零开放 P0–P3，并独立运行
Python 红/绿对照；原报告和 610 项不变证据保留。最终全仓 `full-gate-r2` 已实际
`exit 0 / reaped / 696.329s`：普通 Rust `3077/0/37 ignored`、rustdoc `14/0`、
工程 Python `187 run / 1 skipped`，格式、严格 Clippy、x.y 策略和三组 controller
通过。全部 84 个新默认 Rust 名实际通过，6 个复用 fixture 不算新增行为。
此前 app 失败、本次失败和诊断原件均保留。最终 r4 冻结为 612 项、集合 SHA256
`e8029ecb7eea9a28e76d4c47f8d4d280afa57de4356049f2d83ab39020749ce6`；
相对 r3 仅归档 helper 和其测试变化，其他 610 项不变。
286 份原件已无损封存到[持久证据](testing/records/2026-11-15-legacy-editor-encodings-evidence/README.md)，
612 项工程输入及全部原件已通过 Git index/blob 逐项核验。本地源码提交为
`eb6869d7f78684de5a978e12ba333d48200d31ec`；
[提交绑定](testing/records/2026-11-15-legacy-editor-encodings-evidence/commit-input-binding.json)
已核对精确 Git blob 与当前文件，零 mismatch。没有 push。
最终真实 disposable localhost OpenSSH r2 `17/17`，91 个跟踪身份及私有目录已回收，
父另查目录 absent、原端口拒绝连接。正常 GUI/MCP arm64 Mach-O 构建通过；
不是本批原生桌面、键盘、IME 或辅助技术验收。两名 reviewer 报告已收取且已关闭，
四个实现/复核 agent 已释放，两个作者 worktree 和匹配分支 absent。提交绑定后，
本批重复目录及六个测试副本共 `420,086,551` 字节已回收；
[清理回执](testing/records/2026-11-15-legacy-editor-encodings-evidence/cleanup-final.json)
保留唯一有效 target、正常程序和其他任务资源。本批没有新建用户 chat 或容器。
后续仅补文档与回执，不重跑工程输入未变的门禁。

唯一 Cargo target 保持 `work/recovery-batch-integration-20261115/target`，归属未切换。
Rust/Cargo 实测 `1.100.0`、默认 stable，无 toolchain pin 或新 MSRV。
新增 `encoding_rs = "0.8"`；依保存的官方 metadata 升级 zip 至 `9.6.0`、
rand 至 `0.10.4`。两轮在线超时原件保留；后续 37 包官方响应采集窗口为
`2026-11-15T15:58:55Z`–`15:59:07Z`，同一快照重算 112 项声明全部通过，
不是第二次联网查询。不相关传递包版本保留，依赖图已由非作者独立核对。
仍仅本地提交，不 push。
三主题通过 registry/Palette/token 保持，三平台原生、strict peer `HOLD/PARTIAL`、
用户中继、性能收益、FILE-05/M4 与完整产品总目标继续开放。

## 当前批次：服务端工具版本与流式自测，工程门禁与限定复核通过

基线为本地 `6e2b0a7ee3493ace52ba157049ab668ba464c4f0`。新增独立审核的
工具版本与固定内存归档样本检查；PATH 观察继续保留，不授权工具执行或传输。
后端在受管 worktree 并行开发，父负责原生界面、集成及集中验证。
设计见 [ADR 0139](adr/0139-reviewed-archive-tool-validation.md)，使用入口见
[工具指南](product/ARCHIVE_TOOL_INSPECTION.md)。
本地源码提交为 `df363691fa4ee238bdd1db826f83c916c7d18194`；
[提交绑定](testing/records/2026-11-15-archive-tool-validation-evidence/commit-input-binding.json)
已逐项核对其 Git blob、最终冻结和当前文件：607 项工程输入及 156 份原件零 mismatch。
后续仅补文档与回执，不重跑输入未变的 Rust 门禁；没有 push。

最终 r2 共 607 项 Rust/Cargo/Python 输入，集合 SHA-256
`523506564206cbeca415ae2bc9a82ed670cdb210705ba49f3173a84250bd4c17`。
完整门禁实际 `exit 0 / 674.299s / reaped`：普通 Rust `2987/0/36 ignored`、
rustdoc `14/0`、工程 Python `187 run / 1 skipped`、helper Python `25/25`，
格式、严格 Clippy、x.y 策略和三组 controller 全通过。29 项新增默认 Rust 逐名
通过并包含在总数中，门禁前后完整路径及摘要零漂移。

真实 disposable localhost OpenSSH `16/16`、Linux arm64 helper/Python `25/25`、
正常 arm64 GUI/MCP 构建通过。两个新非作者范围均 APPROVE、零开放 P0–P3，
两个 P2 已关闭；Rust 复核为核对来源和库存后的父预构建独立执行，没有独立重编译。
原报告签署时门禁 PENDING，父已在相同冻结核验成功终态，满足其工程条件。
旧失败、notice 和中止门禁均保留，不能改签成功。详见
[本批记录](testing/records/2026-11-15-archive-tool-validation.md)与
[持久证据入口](testing/records/2026-11-15-archive-tool-validation-evidence/README.md)。

156 份原件已无损导出并与 607 项工程输入一起从 Git index/blob 读回；
随后回收本批临时目录和稳定测试副本 `325,398,545` 字节。四个自有容器按完整 ID
再次确认 absent，作者 worktree 已受管归档且路径 absent，四个 agent 已释放。
唯一有效 Cargo target、其他任务 worktree/容器、共享 image/VM 与原始失败保留，
清理回执从持久入口读取。本批未新建用户 chat。

唯一 Cargo target 仍为 `work/recovery-batch-integration-20261115/target`，
归属没有切换；作者 worktree 不运行 Cargo。继续只本地提交，不 push。
Rust/Cargo 实测 1.100.0，默认 stable，无项目 toolchain pin；本批在线最新依赖
检查 60 秒超时，已停止并实际 reap，不能签全部最新。按阶段集中集成、验证、
修复和非作者复核，减少重复全仓重跑。本批原生三平台执行为 0，不以构建、
容器或 headless 签署桌面验收。完整输入/辅助技术、严格 Zmodem peer
`HOLD/PARTIAL`、用户中继、性能收益、FILE-05/M4 与完整产品目标保持开放。

## 当前批次：Windows 文件后端与 Linux 专项，工程门禁通过

基线为本地 `a092dd5712c0a92ee4af18970fb0500fb230b760`，当前
`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_APPROVED / WINDOWS_NATIVE_NOT_RUN`。
本地源码提交为 `51f7cb7cd6eca78c930e25bbd2644de839ce3d84`；
[提交绑定](testing/records/2026-11-15-windows-local-backend-evidence/commit-input-binding.json)
逐项核对该提交 Git blob、最终冻结与当前文件：602 项工程输入和 1301 份原件
零 mismatch。后续仅补文档回执，不重跑输入未变的 Rust 门禁。
仍只本地提交，不 push。唯一父 target 为
`work/recovery-batch-integration-20261115/target`，源码归属没有切换。
实际运行、原件和清理从
[本批记录](testing/records/2026-11-15-windows-local-backend.md)和
[证据入口](testing/records/2026-11-15-windows-local-backend-evidence/README.md)接续。

1301 份原件已无损导出；全部 602 项工程输入及原件经 Git index/blob 读回一致。
三个 reviewer 均已关闭，六个自有容器再次按完整 ID 确认 absent；
批次目录、SDK/registry payload、稳定测试副本等 `2,609,954,825` 字节临时
内容已回收。唯一有效 target、其他任务资源及原始失败保持。清理见
[最终回执](testing/records/2026-11-15-windows-local-backend-evidence/cleanup-final.json)。

Windows 平台安全接口、Zmodem 来源/保存、归档来源/私有树发布、UI capability
及条件测试已接入。`keelshell-platform` 仅在 Windows 模块允许有说明的 ABI；
旧 crate 继续 forbid unsafe。实现及限制见
[ADR 0138](adr/0138-windows-handle-local-files.md)。最终 r2 为 `588 + 14`
工程输入，SHA-256
`1fb9b4b343d25634fed4bc6cf9ec15d12ac7049562fd83f977960d9d0b78e1c9`。
完整门禁 `full-gate-r2` 实际 `exit 0 / 632.963s / reaped`：
普通 Rust `2958/0/35 ignored`、rustdoc `14/0`、Python `187 run / 1 skipped`，
格式、严格 all-target Clippy、x.y 策略和三组 AI controller 全通过。
全部 23 项新增默认 Rust 逐名通过，工程摘要与完整路径集合零漂移。

Windows platform+session strict cross Clippy r3、四个测试 target 的 AMD64
PE32+ 链接通过，143 个选定新描述符字节匹配；不是 Windows --list 或运行。
正常 macOS GUI/MCP 构建通过，不签新窗口验收。Windows native 实际运行 0。
平台 r3 SOURCE_CONTRACT APPROVE、Linux runner r2 及最终运行证据 r3 APPROVE，
各范围零开放 P0–P3。session r2 最终差分也 APPROVE：两项 P2 已关闭，
三个范围共关闭六项 P2。Unicode reservation 与归档 worker 结果交接均有真实
红/绿回归；六项预构建专项由非作者独立执行通过，保留非独立重编译限制。
原 gate r1 因新发现实际停止/reap，exit -15；原 HOLD/notice/失败全部保留。

最终 Linux 是验证专用快照 `f945c4c780010067aee664bf7ccbf6caace4eea5` 的
588 文件 Git 归档，使用 Rust/Cargo 1.98.1、原生 arm64、UID 1000、
network none、独立离线 target/Cargo home：`252/0/0 / 59.797s`。
六个 ELF/逐名库存、实际结果及容器/进程回收已独立核验。
共享 worker 3 项只在 Linux 库存中，不计入其选集；由宿主门禁另行通过。
旧基线 251、旧 r1 的 252 和漂移 notice 不改签当前输入。

本机 Rust/Cargo 1.100.0、默认 stable，无项目 toolchain pin；两轮在线全量
最新依赖检查均超时，不能签全部最新。SDK 的清单摘要/签名限制保留。
后续优先真实 Windows 文件系统与原生流程、三平台完整桌面/IME/辅助技术、
direct PTY strict peer `HOLD/PARTIAL` 和发行。完整产品与 FILE-05/M4 仍 active。

## 2026-11-15 有界 Zmodem 收尾：集中门禁与非作者复核通过

本批基线为本地 `b66ec7e564e97c3cc8261b447288377ac1ffccc2`，当前状态为
`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_APPROVED / DIRECT_PTY_ACCEPTANCE_OPEN`。
本地源码提交为 `590cdfb654c41d615f01bc135375ffc02d525461`；
[提交绑定](testing/records/2026-11-15-bounded-zmodem-farewell-evidence/commit-input-binding.json)
从 Git blob 和当前文件两侧核对 `583` 个工程输入及 `708` 份原件，零 mismatch。
最终 r5 冻结 `571 + 12`，SHA-256
`d8d27989768ae1827697458ee52d366d407ca8543a9eebd41baf6d8f283a2b2e`。
完整 `full-gate-r5` 已实际 wait/reap，`exit 0 / 646.751s`：
普通 Rust `2935/0/35 ignored`、rustdoc `14/0`、Python `148 run / 1 skipped`，
格式、严格 workspace all-targets Clippy、`x.y` 策略和三组 AI controller 通过。
新增默认 Rust `59` 项逐名通过且已包含在总数，另有 Python `7` 和 opt-in peer
`1`；门禁后摘要及完整工程路径集合零漂移，完整 app 库存 `1238`。

三名新非作者在各自范围均 APPROVE、零开放 P0–P3，已关闭六项 P2。
core `102/102`、session `25/25`、真实 barrier `4/4`、app Zmodem `30/30` 与
final bridge `15/15` 的重复与同源复用明确单列；Rust reviewer 使用核对摘要和
完整库存的父预构建副本，没有独立重编译。父在相同最终冻结取得成功门禁，满足
原报告的工程条件，不改写此前 HOLD、失败、drift 或签署时条件。
设计见 [ADR 0137](adr/0137-bounded-zmodem-farewell.md)，接续从
[本批记录](testing/records/2026-11-15-bounded-zmodem-farewell.md)与
[持久证据](testing/records/2026-11-15-bounded-zmodem-farewell-evidence/README.md)读取，
原临时 `work/pty-farewell-20261115` 已删除。

`EndedWithoutFarewell` 明确区分缺握手与严格 `Finished`，只在精确 lease/revision
回复经真实认证连接往返确认、等待到期或有序 clean exit 后收尾；已知 completion
禁止计时抢先。普通尾部跨取消/失败保留，worker 有界投递到实际 UI receiver；
持续背压或断开显式失败，保留 primary error。逐文件回执、未知隔离和精确输入
交接保持，旧输入不重放，七项原 strict peer 断言没有放宽。

macOS 原版 peer `native-current-r3` `8/8`，其 `231` 个 core/session/root 输入
与最终 r5 相同；正常 GUI/MCP arm64 Mach-O r4 构建通过，生产输入到 r5 未变。
Linux r3 `6/8` 的两项严格 Finished 失败保持；interactive 实测
`strict_OO=0 / missing_optional_OO=1 / same_shell_continued=true`。
Linux r4 停在公开包安装连接失败，未进入协议。严格互操作仍为 `HOLD/PARTIAL`；
Windows 安全后端、Linux 本地 Rust 后端、旧 Python、真实三平台桌面、IME/
辅助技术及发行保持开放，FILE-05/M4 和完整产品总目标未结束。

708 份原件已无损校验，随后回收本批重复目录、稳定测试副本与临时 peer，
本次 `626,097,502` 字节、连同先前过期副本合计 `1,547,625,534` 字节。
所有实现及复核 agent 已关闭，四轮 owned 容器、进程、临时密钥有清理回执。
本批没有新建用户 chat 或实现 worktree；保留唯一有效 target
`work/recovery-batch-integration-20261115/target` 和其他任务资源，没有切换源码归属。
两轮在线最新依赖检查超时，不能签全部最新；本批不新增依赖。
Rust/Cargo 实测 `1.100.0`，无 toolchain pin 或新 MSRV。仅本地提交，不 push。

## 当前批次：审核式 PTY Zmodem 候选，工程门禁与源码复核通过

基线为 `721bad2157da8d3a0bc11a1eab5ac59b6616aa9c`。
本地源码提交为 `71450f07638aec657e7412ec330592618a5350e1`；
[提交绑定](testing/records/2026-11-15-reviewed-pty-zmodem-evidence/commit-input-binding.json)
逐项核对 `570 + 12` 输入的冻结、Git blob 与当前文件，零 mismatch。
当前为 `AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_APPROVED / DIRECT_PTY_ACCEPTANCE_OPEN`。
core 协议、session 文件审核、安全发布、单一 PTY 写入通道和原生审核已集成；
仍只本地提交，不 push，完整产品与 FILE-05/M4 保持开放。
设计见 [ADR 0136](adr/0136-reviewed-pty-zmodem.md)及[使用限制](product/ZMODEM.md)。
最终输入为[持久冻结](testing/records/2026-11-15-reviewed-pty-zmodem-evidence/frozen-inputs-r3.json)，`570 + 12`，
SHA-256 `14b353ecbf84d9684b20dd7bafa0ffc090644b452fe199e4d32f1c2745e8e882`。

作者实际通过 core `65/65`、session lib `27/27`、session integration `18/18`、
app Zmodem `19/19`、完整 bridge `6/6`；app 两组有重叠，不叠加计数。
两名新非作者已经关闭四项 P2 与一项 P3，当前均 APPROVE、零开放 P0–P3，
并绑定 r3 的 `582/582` 输入；其执行使用父预构建稳定副本，没有独立重编译。
原始失败、notice 与限定结论均保留；源码批准不等于互操作或完整产品验收。
所有实现与复核 agent 均已关闭、会话已归档。三个实现 worktree 已受管归档，
core/Linux 的早期活动任务保护拒绝保持原件，后续已成功且路径实际不存在。
没有绕过保护或清理其他任务 WIP。

完整 gate r1 主动停止/reap `exit -15`，r2 `exit 1` 于 remote_only 架构 guard：
新 test-only 文件缺少首行 inner cfg 标记。补齐标记后 guard `5/5`、fmt 通过；
r2→r3 仅该属性与空行，所有生产输入相同。最终 `full-gate-r3` 已实际取回
`exit 0 / 613.125s / reaped`：普通 Rust `2876/0/34 ignored`、rustdoc `14/0`、
Python `141 run / 1 skipped`，fmt、严格 workspace all-targets Clippy、`x.y`
策略及三组 AI controller 全部通过。新增 `129` 项逐名存在并在最终 gate 通过，
已经包含于总数；门禁后 `570 + 12` 输入零 drift。原 reviewer 签署时门禁仍在
运行，父已核验相同冻结的成功终态，满足其工程条件，未改写原报告。
唯一 Cargo target 一直属于本主树。
正常 GUI/MCP arm64 Mach-O 已实际构建通过，9.632s；没有新原生桌面交互验收。

直接 PTY 继续 `HOLD/PARTIAL`：本机最终生产输入 r6 `6/7`，Debian r2/r3
`5/7`、`4/7`；部分完整 FileSaved 后收到 ZFIN、peer 退出 0，却缺最终 OO。
不以旧 r5 `7/7` 或 pipe 对照补签 Finished。最终 Linux relay 完整周期
`exit 0 / 95.217s`，包含真实 archive/ACL `17/17`、独立 reference、应用 relay
`7/7` 和清理；它不签直接 PTY、Linux Rust 本地文件后端或 Linux 桌面。
所有自有 fixture 容器、进程和临时密钥已按身份核验回收，其他任务资源保留。
Windows Zmodem 文件选择显式停用，Python 3.8 runtime 不可用。

继续从[本批记录](testing/records/2026-11-15-reviewed-pty-zmodem.md)和
[持久证据](testing/records/2026-11-15-reviewed-pty-zmodem-evidence/README.md)
的回执、逐名库存、原始 review 与保留失败接续。源码、工程、单文件保存、
批次握手及三平台原生分别验收；没有新的模型/用户权限自动授予或远端写入重放。
747 份原件已无损导出并校验，随后回收本批重复运行目录、稳定测试副本与临时
peer，共 654,860,244 字节；[最终清理](testing/records/2026-11-15-reviewed-pty-zmodem-evidence/cleanup-final.json)
保留唯一有效 target、其他任务 WIP、受保护 worktree 和容器。
收口时另修正三份原始 HTTP headers 的 Git 换行归一化；受影响原件使用局部
binary 属性保存，不修剪原报告或上游文字，最终 Git blob 与持久清单逐项核对。
后续为证据与文档提交，工程输入不变，历史检查失败另存回执。
下一步优先定位两版真实 peer 在直接 PTY 下缺最终 OO 的原因，再以新冻结和
明确的直接 PTY 结果验收；relay 对照不替代该项，Windows 安全后端和原生矩阵单列。

先读[AGENTS](../AGENTS.md)、本页、[路线图](ROADMAP.md)及
[共享 Cargo 指南](testing/SHARED_TARGET.md)。时间使用 Asia/Shanghai，
原始回执 UTC 保持。上一阶段完整过程保存在
[原生收口前交接](history/2026-11-06-handoff-before-editor-native-closeout.md)。

## 2026-11-15 服务器归档：最终门禁与非作者复核通过

基线为本地 `9112e232b6f0e022d299cba9e8498088c77b3d3f`，当前状态为
`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。本批实际实现服务器多选 ZIP/tar.gz 打包；
先审核只读扫描命令，再审核完整成员、摘要、写入命令和标准输入，确认后才在
服务器压缩并发布新的同级归档。完成后可另行下载或解包，不自动串联操作。
不是工具 PATH 观察的替代界面。

本地源码提交为 `2726b6466e68508037ff81b1daf01fcf2c91b6e5`；
[提交绑定](testing/records/2026-11-15-server-archive-evidence/commit-input-binding.json)
逐项核验最终 `553 + 10` 输入、Git blob 和当前文件，零 mismatch。

core/Python 与 session 由两个独立实现 agent 在受管 worktree 并行开发，
父负责原生 UI、集成和唯一 Cargo target
`work/recovery-batch-integration-20261115/target`。集成后集中验证，没有切换
该 target 的源码归属。实现 agent 已关闭、两个实现 worktree 及本地分支均已回收。

最终 r5 冻结包含 `553` 个 Rust/Cargo/Python 工程输入及 `10` 个门禁脚本，
完整 gate r4 实际退出 `0`、`603.804s`：普通 Rust `2747/0/27 ignored`、
rustdoc `14/0`、Python `131 run / 1 skipped`；fmt、严格 workspace all-targets
Clippy、`x.y` 策略和三组 AI controller 通过。新增默认用例 `72` 项逐名通过，
已经包含在普通总数中。最终 app 库存 `1208`，不只核对数量。

core 非作者发现的继承 ACL P2 已由真实 macOS 红回归确认、fd 权限修复、
独立真实探针及差分复核关闭。session/UI 收口发现的 Unix-only 测试声明条件
P2 已修正方法、导入和字段的 cfg，r4→r5 仅两个测试文件变化；两个 reviewer
最终均 APPROVE、零开放 P0–P3，已关闭。所有原始 notice、HOLD、失败及被终止门禁
保留。独立执行使用
核对摘要和清单的父预构建稳定副本，未独立重编译，不能代签原生平台。

真实 localhost OpenSSH r2 `15/15`、正常 GUI/MCP arm64 Mach-O 构建通过；
它们与最终 r5 的生产输入相同。自有 SSH 进程和临时目录已回收，本批未建容器。
本次两轮在线依赖复查均超时，无最新版本报告；当前实际 Rust/Cargo `1.100.0`，
无 toolchain pin 或新 MSRV。完整运行、评审、源码绑定与清理证据持久保存在
[本批证据](testing/records/2026-11-15-server-archive-evidence/README.md)，
提交绑定及清理从
[集中验证记录](testing/records/2026-11-15-server-archive-integration.md)接续。
设计见 [ADR 0135](adr/0135-reviewed-server-archive-packing.md)。
248 份原件已逐字或无损压缩保存并校验，随后删除本批重复运行目录与稳定测试
二进制，回收约 332 MB；[最终清理](testing/records/2026-11-15-server-archive-evidence/cleanup-final.json)
记录所有实现/复核 agent 已关闭、两个实现 checkout/分支不存在、自有 SSH 回收，
保留唯一有效 target、其他任务 WIP 和受保护 worktree。

只本地提交、不推送；产品总目标和 FILE-05/M4 继续开放。远端 Linux 文件系统
权限与 Python 3.8 运行、Windows 安全解包、Zmodem、中继、三平台原生交互、
IME/完整辅助技术及发行验收尚未完成，不能由本机 gate 或 headless 结果补签。

## 2026-11-15 已完成批次：服务端工具观察与 PAX 兼容

在本地已提交的 `e97705c3d773ea6919d76441685c5064e641ca86` 上继续开发。
本批状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`，
仅代表本批工程与源码复核范围。
本地源码提交为 `0f4719d72b83238ecaa7bb244c5ba913a7448224`；
[提交绑定](testing/records/2026-11-15-archive-tools-pax-evidence/commit-input-binding.json)
逐项核对 538 个 Rust/Cargo 输入及 10 个脚本的 Git blob、冻结和当前文件，
零 mismatch。后续绑定提交仅更新文档与回执，不重跑无关 Rust 门禁。
两个独立实现 agent 已交付 SSH 工具检测和有界 PAX 解码；主线程集成原生
审核卡片、会话绑定及实际 GPUI 回归。实现 agent 不执行 Cargo，主线程独占
同一主树的有效 target，集成后集中完整验证、修复和新的非作者复核。

工具观察需要审核完整固定命令，只有当前真实 SSH 连接可确认；取消、连接
替换及迟到回复不能产生新授权。PAX 仅接纳严格普通元数据子集，原始/有效
路径都继续检查，Python Unicode `?` 备用路径等限制明确保留。
设计见 [ADR 0133](adr/0133-reviewed-server-archive-tool-inspection.md)、
[ADR 0134](adr/0134-bounded-pax-archive-metadata.md)。
本次在线依赖检查 60 秒超时，没有版本报告。仍只本地提交，不 push。

最终 r4 完整门禁退出 `0`，耗时 `647.902s`：普通 Rust `2675/0/26 ignored`、
rustdoc `14/0`、Python `131 run / 1 skipped`；fmt、严格 workspace all-targets
Clippy、`x.y` 依赖策略和三组 AI controller 全部通过。538 个 Rust/Cargo 输入、
10 个 gate 脚本在门禁后仍与最终冻结一致；app 清单 1201 项，74 个本批新增
默认用例逐项存在且通过，不叠加专项或 reviewer 的重复执行。

PAX 首轮非作者复核发现完整 TAR size 可能被底层 base-256 解析截断的 P2，
两项实际 Rust 红回归已确认。修复对所有允许头完整解析原始 size，再进入预算和
PAX 覆盖；6 项新增边界回归纳入 40 项 PAX 测试。新的差分复核关闭 P2，
工具/UI 与 PAX reviewer 最终均 APPROVE、零 P0–P3。两者使用核对摘要及清单
的父预构建 binary，分别独立执行 `6 + 16` 和 `40 + 40`，没有独立重编译。
原始报告签署时尚无最终 gate 回执；父随后核对相同最终输入的 exit `0`，
满足其工程条件，未改写原报告或此前失败证据。

r3 真实 localhost OpenSSH `14/14`，正常 GUI/MCP arm64 Mach-O 构建通过；
r3→r4 仅两处等价测试转义变化，生产输入一致。实现及复核 agent 已关闭，
两个实现 worktree 已受管归档，OpenSSH 自有进程与临时目录已回收；本批未创建
容器。命令、冻结、失败、复核和边界见
[集中验证记录](testing/records/2026-11-15-archive-tools-pax-integration.md)及
[持久证据](testing/records/2026-11-15-archive-tools-pax-evidence/README.md)。
127 份原件已逐字或无损压缩保存并验证，随后删除本批重复工作日志与 tmp；
[最终清理回执](testing/records/2026-11-15-archive-tools-pax-evidence/cleanup-final.json)
确认两个实现 checkout/本地分支均不存在、四个 agent 已关闭后不再可查。
保留当前有效 target、其他任务 WIP 和受保护 review worktree。
服务端实际打包、版本/功能验证、Zmodem、中继、Windows 安全解包、三平台原生
交互、IME/完整辅助技术和发行验收仍开放，FILE-05/M4 与产品总目标继续进行。

## 2026-11-15 已完成批次：审核式客户端归档传输

本批在 FILE-05 下实现本地多选打包上传和远端归档下载后安全解包，
格式为 ZIP、tar.gz。core 编解码、session 传输、app 原生面板按独立写集
并行开发，集成后统一执行完整门禁和新的非作者复核。审核包含每个成员及准确
目标，确认时重新核验源；已有目标保留，未知写入和未验证 staging 保留隔离。
Frost、Graphite、Lavender 继续共用 registry、Palette 和组件 token。

当前状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。
首轮复核的两项 P2 已修复：归档成功或已知终态恢复之前的隔离目标，未知结果
仍保留关闭保护；目录 builder 只在 Unix 分支可变，修正 Windows lint 原因。
真实 GPUI 生命周期回归先红后绿，最终作者和独立执行均 `9/9`；独立 reviewer
确认 P0–P3 均为 `0`。reviewer 使用已核对 SHA/1195 项清单的作者预构建二进制，
没有独立重编译。其原始报告保留最终 gate 尚在运行时的条件性 HOLD；父随后核验
相同冻结的最终 gate exit `0`，满足该工程条件，没有改写报告。

最终完整 gate `588.762s`：普通 Rust `2601/0/25 ignored`、rustdoc `14/0`、
Python `131 run / 1 skipped`，fmt、workspace all-targets strict Clippy、`x.y`
依赖策略和三组 AI controller 全部通过。532 个 Rust/Cargo 输入和 10 个脚本
在门禁、复核及正常程序构建后均匹配 r3 冻结。当前源码的真实 localhost
OpenSSH `13/13`，临时服务与目录已回收；GUI/MCP 正常 arm64 Mach-O 构建通过。
本地源码提交为 `9b7b96442f422908b89ab3a2769b1c932f22d787`；
[提交绑定](testing/records/2026-11-15-reviewed-archive-transfer-evidence/commit-input-binding.json)
已从 Git blob 和当前文件两侧逐项核对 532 个输入及 10 个脚本，零 mismatch。

Windows 安全解包后端尚待实现，界面明确停用解包，打包上传保留；Windows
strict gate 和 Linux 运行没有在本机执行。在线最新依赖检查 75s 超时，不能
宣称本批所有版本最新；Podman 仅只读确认 VM stopped，没有创建容器。
服务器端打包/能力探测、Zmodem、中继、归档原生操作、跨平台、IME/辅助技术
和发行验收仍开放，FILE-05/M4 没有整体完成。
设计与限制见 [ADR 0132](adr/0132-reviewed-client-archive-transfer.md)、
[归档指南](product/ARCHIVE_TRANSFERS.md)与
[集中验证记录](testing/records/2026-11-15-reviewed-archive-transfer.md)。
两个实现 agent 和 reviewer 已关闭，已合入 core worktree 已受管归档。
当前仅本地提交，不 push；其他任务 WIP 和受保护 worktree 保留。

## 2026-11-15 已完成批次：恢复记录批量整理

本批新增恢复记录多选、按当前列表顺序建立待审核队列、逐条进入既有独立
审核，以及需要明确确认的批量丢弃。队列没有连接或执行权限；每条审核仍
取得独占 claim，重新确认当前 SSH 主机/保存连接，并完成
`Inspect → Review → Confirm`。批量丢弃在 claim 内核对确认过的记录快照，
记录变化、Busy 和失败保留未处理项，未知结果重新读取持久存储。

采用阶段集中验证：实现 agent 只做受影响包编译和风险专项，主线程统一
集成、全仓门禁、真实协议联调与新的独立复核。固定 `5cdbc96` 的独立复核发现
worker 创建失败时当前记录不能回到待审核队首的 P2，现已修复：只有 worker
成功创建后才消费 active/session。新增单条/两条真实 GPUI 故障回归先复现失败，
再验证回队、claim 释放、新审核后逐条显式续传成功及后续项不自动执行。
当前状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。
最终完整门禁退出 `0`，耗时 `608.170s`：普通 Rust `2533/0/23 ignored`、
rustdoc `14/0`、Python `131 run / 1 skipped`；fmt、整仓严格 Clippy、依赖策略和
三组 AI controller 通过。最终 app 清单 `1186` 项，10 个批量回归均实际通过。
新的非作者复核独立重编译清单、通过传输队列 `29/29` 及相关门禁，P0–P3 均为
`0`，原 P2 关闭。冻结的 524 个 Rust/Cargo 输入及 10 个 gate 脚本前后相同，
复核使用另一 target 的相同冻结候选，已与本地修复提交
`e94f31e49897ac981f4c3d20d22bc39f0cfba3bb` 逐项绑定源码字节。
修复前源码另有独立本机 OpenSSH `11/11` 和 macOS 可见窗口隔离启动短验；
本次状态修复不把旧正常程序或原生短验改签为最终源码。

Podman 引擎已成功读回，但 Docker Hub 拉取超时，容器联调未开始；自有资源
已核验不存在，VM 已停止。官方依赖在线检查因请求无响应/HTTPS `403`
未能完成；本轮不代签最新版本。原生恢复控件交互、Windows/Linux、IME、
完整辅助技术及发行安装仍开放。

本轮按原始日志校正历史计数：依赖刷新基线实际为普通 `2523` 加 rustdoc `14`，
合计 `2537`；此前“普通 2537 / session 448”包含重复分类，session 普通应为
`434`。下方保留历史记录，本批以集中验证记录中的逐 suite 汇总为准。设计见
[ADR 0131](adr/0131-persisted-transfer-recovery-batch-review.md)，实时证据和
边界见[集中验证记录](testing/records/2026-11-15-transfer-recovery-batch-integration.md)。
公司内网不可用，当前仍只做本地 commit，不 push。

本批实现及两轮复核 agent 均已关闭。独立 reviewer target、旧依赖刷新 target
和测试临时目录已清理，保留当前主树一个有效编译缓存。复核 worktree 的两次
受管归档均因 `Another active task still uses this worktree.` 被保护拒绝，
重新附加主开发树后仍相同；保留这份检出及其三文件候选，不绕过保护删除。
源码绑定、持久日志与清理回执统一位于本批证据目录。

## 2026-11-15 当前交接校正

当前维护 checkout 为
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`，
分支 `feature/editor-unicode-codec-20261114`，本批已提交的依赖基线为
`59c59a8`（`chore(deps): refresh nix direct requirement`）；传输恢复实现基线为
`e2b00af3cb8bcae67d54cbf69cf04e8baab2b7cd`。公司内网不可用，当前只允许本地
commit，不执行 push。

用户在 2026-11-15 要求缩短开发周期，本批开始采用
[阶段集中验证](testing/STRATEGY.md#阶段集中验证)：开发中保留必要编译和风险专项，
子任务集成后集中跑完整 gate、真实联调及新的独立复核，修复先做受影响回归。
不再为每个小改动重复执行全仓 gate 或创建多轮文档专属 reviewer；最终提交仍需
完整成功门禁和非作者复核。正在开发的切片保持未完成状态，不沿用上一片通过结果。

本片修复了恢复列表在旧异步 load 与 Discard/计划进度 mutation 交错时的 stale
snapshot 风险。成功 mutation 会推进 `recovery_mutation_generation` 并启动新的
完整 store reload；旧 callback 同时校验 session、load generation 和 mutation
generation，因此不能覆盖新快照。两个真实 GPUI window/context 交错回归覆盖了
Discard 和 persisted plan progress。Frost、Graphite、Lavender 三个主题继续由
registry/Palette/token 提供，业务代码不按主题 ID 分支，后续主题可继续独立注册。

作者完整门禁和新的独立严格只读复核均已通过，当前状态为：

`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`

作者 full gate 的 Python 为 `131 run / 1 skipped`，普通 Rust 为
`2537 passed / 0 failed / 23 ignored`，workspace doctest 为
`14 passed / 0 failed`；app `1174/0/2`、core `644/0/0`、AI `194/0/5`、
session `448/0/16`、MCP `77/0/0`。依赖策略、fmt、workspace 严格 Clippy、
readiness/observer/small-stack controller 均通过。已知
`__eh_frame section too large` linker warning 没有改变任何退出码。

作者 full gate 证据见
[最终测试记录](testing/records/2026-11-15-transfer-recovery-review-final.md)、
[full gate 日志](../work/transfer-recovery-refresh-20261115/logs/full-gate-screen.log)
和[exit 回执](../work/transfer-recovery-refresh-20261115/logs/full-gate-screen.exit)。
独立 reviewer 审查 exact SHA `e2b00af3cb8bcae67d54cbf69cf04e8baab2b7cd`，
结论为 `APPROVE`，P0/P1/P2/P3 为 `0/0/0/0`；原始报告见
[最终独立复核原文](testing/records/2026-11-15-transfer-recovery-review-final-raw.md)。
reviewer worktree、独立 target、日志和临时报告已清理。

当前开发机 active/default Rust/Cargo 为 `1.100.0`。仓库没有
`rust-toolchain.toml`、`.cargo/config*` 或 `rust-version`，Cargo 继续跟随开发机或
CI runner 的 active toolchain；项目没有把本机版本误写成 MSRV。直接依赖继续使用
`x.y`，Cargo.lock 保留 patch 解析结果。2026-11-15 已完成官方依赖复查并将
`keelshell-ai` 的 Unix 目标直接 `nix` requirement 更新为 `0.32`；由于
`process-wrap 10.0.1` 仍要求 `nix ^0.31.1`，锁文件同时保留 `0.31.3` 与
`0.32.0`。证据见[依赖复查记录](testing/records/2026-11-15-dependency-refresh.md)。
原生三平台、真实 SSH/SFTP、Podman、IME/辅助技术、签名公证、安装
升级和发布验收仍需单独完成。不要把 headless GPUI、loopback 或作者门禁写成
这些边界的通过。

## 2026-11-14 重启后传输恢复日志

本片源码实现提交为 exact SHA
`52f861e3130749715ea20824efa1f1b1619f1e3b`，当前 checkout 在本次文档提交前的
HEAD 为该提交。实现新增独立的 `transfer-recovery.json` 日志，最多 32 条
记录、文档最多 256 KiB、单路径最多 4096 字节；记录仅保存方向、路径、
文件/目录、进度、审核已有字节、时间、状态和可选 `connection_id`。它不保存
SSH session、SFTP handle、socket、worker、review token、凭据、私钥、命令、
秘密、代理认证或流量。

真实传输开始前写入 `Unfinished`，进度、失败、取消、未知结果、队列初始化
失败和 SFTP 入队失败更新终态。恢复前必须在当前 SSH 会话完成
`Inspect → Review → Confirm`；保存连接绑定不匹配在任何远端只读检查前拒绝。
旧格式或快速连接没有绑定时，界面要求用户确认当前 SSH 主机。成功的显式
恢复或用户明确 `Discard` 才删除记录；Discard 只删除本地观察记录。

存储使用 owner-only 文件、同目录锁、临时文件、`sync_all`、原子替换和 Unix
父目录同步，所有阻塞文件系统操作在后台 worker。每条记录还有跨窗口/跨进程
排他的 OS 文件锁 claim，带 claim 的更新与删除校验所有权；队列或 worker
失败、取消、挂起、导航撤销和旧审核替换会释放 claim。无绑定记录必须先确认
当前 SSH 主机，确认前不会执行远端只读检查；只读计划刷新后的已有字节数会回写
持久记录。三主题、双语、终态幂等性、旧格式主机确认、连接绑定和显式丢弃均有
回归。设计、能力和测试证据分别见
[ADR 0130](adr/0130-persisted-transfer-recovery-journal.md)、
[能力清单](product/CAPABILITIES.md)和
[测试记录](testing/records/2026-11-14-transfer-recovery-journal.md)。

作者完整门禁使用：

```text
CARGO_TARGET_DIR=work/transfer-recovery-fix-20261114/full-target \
python3 scripts/check.py
```

日志为
`work/transfer-recovery-fix-20261114/logs/full-gate-52f861e.log`，doctest 日志为
`work/transfer-recovery-fix-20261114/logs/doctest-52f861e.log`，实际结果为
Python `131 run / 1 skipped`、普通 Rust `2526 passed / 0 failed / 23 ignored`、
workspace doctest `14 passed / 0 failed`；其中 app `1164/0/2`、core `643/0/0`、
AI `194/0/5`、session `448/0/16`、MCP `77/0/0`。依赖策略、fmt、workspace
严格 Clippy、readiness/observer/small-stack controller 均通过。
app 链接阶段的 `__eh_frame section too large` 是已知 warning，没有改变退出码。

当前开发机 active/default Rust/Cargo 仍为 `1.100.0`。仓库没有
`rust-toolchain.toml`、`.cargo/config*` 或 `rust-version`，因此 Cargo 默认
跟随开发机或 CI runner 的 active toolchain；不会把本机版本误写成项目 MSRV。
直接依赖仍遵守 `x.y`，Cargo.lock 保留 patch 解析结果。在线依赖最新版本检查
因公司内网不可用尚未完成。

该历史片在文档提交时的状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_PENDING`；
后续 claim 生命周期修复和最终独立复核状态见本页顶部。

## 2026-11-14 AI 审核命令保存为片段草稿

本片基线是 exact HEAD
`b89445604b4ce44fc5a80ff26605f6e6b9039d57`，主体实现提交为
`9b831103aae5563a5450561988d668d2e77a19dd`，无障碍修复提交为
`50ca0ead2fead7b0e92679ee9d531604ae040772`，当前最终实现 SHA 为
`50ca0ead2fead7b0e92679ee9d531604ae040772`。源码位于独立 worktree
`/Users/hdh/.codex/worktrees/ai-command-snippet-save-20261114/keelshell`，
分支为 `feature/ai-command-snippet-save-20261114`。父工作树
`/Users/hdh/work/develop/projects/keelshell` 保留其现有未提交改动，本片没有触碰
或清理；当前任务父 checkout
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell` 保持 clean。
旧的
`/Users/hdh/.codex/worktrees/ai-command-snippets-20261113` 保留其原有未提交 WIP，
本片没有触碰或清理它。

### 已实现

- `AssistantPanel` 为 suggestion 与 diagnostic step 复用 `CommandProposal::review`，
  只在审核成功后发出带 command、target、session identity、一次性 review ID 的
  `AssistantEvent::SaveSnippet`；事件处理不直接写 StateStore。
- `Workspace` 在事件打开草稿和编辑器提交保存时重新确认活动 SSH tab、terminal/
  session、target、configuration gate 和 modal scope。迟到、切换、恢复、关闭、
  退休或被替换的目标双语报错并 fail closed。
- `SnippetEditor::new_reviewed_draft` 把已审核正文锁定；名称、描述、标签可编辑。
  第一次保存点击只进入确认态，第二次才发出保存事件；编辑命令必须取消并重新
  审核。冲突和失败保留完整草稿，保存成功才关闭编辑器。
- 保存使用普通 `Snippet`，不写 session、host、凭据、AI profile、review token
  或秘密元数据；界面警告片段以明文保存在本机，保存和取消不执行命令。
- Frost、Graphite、Lavender 通过 registry/Palette/token 共享 UI 代码，双语、
  fixed footer、滚动和 accessibility label 已覆盖。名称、说明、标签输入也有
  稳定 accessibility ID 与中英文 `aria-label`，业务代码没有 theme ID 分支。

### 作者验证

作者专用 target：
`work/ai-command-snippet-save-20261114/target`。

完整门禁 target：
`work/ai-command-snippet-save-20261114/full-target`。

`python3 scripts/check.py` 已实际等待并退出 `0`。Python 为 `131 run / 1 skipped`；
workspace 测试为 `2499 passed / 0 failed / 23 ignored`，其中 app `1153/0/2`、
core `627/0/0`、AI `194/0/5`、session `448/0/16`、MCP `77/0/0`；依赖策略、
fmt、workspace 严格 Clippy 和 readiness/observer/small-stack controller 均通过。
单独 `cargo test --workspace --doc` 为 `14 passed / 0 failed`，含 2 项 compile-fail。

专项回归为 AI 片段 `6/6`、SnippetEditor `11/11`、Assistant review `6/6`，
core 包全测试 `634/634`。日志保存在
`work/ai-command-snippet-save-20261114/logs/`，主门禁日志为
`full-gate-9b83110.log`，测试记录为
[AI 片段保存测试记录](testing/records/2026-11-14-ai-reviewed-snippet-save.md)。
app 链接阶段的 `__eh_frame section too large` 是已知 warning，没有改变 exit code。

无障碍修复后的 focused 回归绑定最终实现 SHA
`50ca0ead2fead7b0e92679ee9d531604ae040772`：SnippetEditor `12/12`、AI 片段
`6/6`、Assistant review `6/6`；`cargo fmt --all -- --check`、app tests
严格 Clippy 和基线到最终实现 SHA 的 `git diff --check` 均通过。该修复没有重新
运行完整 `scripts/check.py`；此前完整脚本的通过结果仍绑定主体实现提交
`9b831103aae5563a5450561988d668d2e77a19dd`，最终提交的 focused 证据见测试记录。

### 独立复核与当前状态

第一次独立严格只读复核固定在 `db808e0b45a788f4744acf2fe98cde7bae7b2f0a`，
发现一个 P2：SnippetEditor 的名称、说明、标签缺少显式无障碍名称。该问题已在
`50ca0ead2fead7b0e92679ee9d531604ae040772` 修复，并增加 GPUI 中英文可访问性
名称回归。第一次原始回执已归档到
[修复前独立复核原文](testing/records/2026-11-14-ai-reviewed-snippet-save-independent-review-before-fix-raw.md)。

第二次独立严格只读复核固定在最终实现 SHA
`50ca0ead2fead7b0e92679ee9d531604ae040772`，使用独立源码 worktree
`/Users/hdh/.codex/worktrees/d362/keelshell`、target
`work/review-final-50ca0ea/target` 和日志目录
`work/review-final-50ca0ea/logs/`。原始报告已归档到
[AI 片段保存独立复核原文](testing/records/2026-11-14-ai-reviewed-snippet-save-independent-review-raw.md)。
结论为 `APPROVE`，P0/P1/P2/P3 均为 `0`。独立实际通过 SnippetEditor `12/12`、
AI 片段 `6/6`、Assistant review `6/6`、core `634/634`、core doctest `7/7`，
并通过 fmt、app tests 严格 Clippy 和 diff check。对应独立日志已归档到
[独立复核日志目录](testing/records/2026-11-14-ai-reviewed-snippet-save-independent-review-logs/)。

当前状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。独立复核没有把
macOS/Windows/Linux 原生窗口、真实 SSH、Podman、外部 AI provider、IME、
VoiceOver/NVDA 或其他完整辅助技术矩阵、跨进程 StateStore、签名、公证、安装升级
和发布验收写成通过。公司内网不可用，本片只允许本地 commit，不 push。复核原始
报告、独立日志和作者日志保留在项目中；临时复核 worktree、target 和分支已清理。

不要把 headless GPUI、loopback fixture 或 full gate 写成 macOS/Windows/Linux
原生、真实 SSH、Podman、外部 AI provider、IME、辅助技术、签名、公证、安装升级
或发布验收。公司内网不可用，本片只允许本地 commit，不 push。

## 2026-11-14 命名隧道运行状态持久化与显式恢复

本片产品实现基线是 exact HEAD
`fd4cc894aa876f23cde49698b693779e81027489`，最终实现 SHA 为
`7e6ecb3ea8c347f7f633ba0f205d936cd58d72ec`。主体实现提交为
`093f725037d5c037cfdd7b75a66f7448bc0732a`，最终提交补上私钥认证目标绑定。
从基线到最终实现共有 13 个源码/测试文件：

- `crates/keelshell-core/src/tunnel_runtime.rs`
- `crates/keelshell-core/tests/tunnel_runtime.rs`
- `crates/keelshell-core/src/model.rs`
- `crates/keelshell-core/src/connection_library.rs`
- `crates/keelshell-core/src/error.rs`
- `crates/keelshell-core/src/lib.rs`
- `crates/keelshell-app/src/tunnels.rs`
- `crates/keelshell-app/src/tunnels/rules.rs`
- `crates/keelshell-app/src/tunnels/management_tests.rs`
- `crates/keelshell-app/src/tunnels/workspace_tests.rs`
- `crates/keelshell-app/src/tunnels/workspace_review_tests.rs`
- `crates/keelshell-app/src/workspace.rs`
- `crates/keelshell-app/src/workspace/view.rs`

运行快照只保存规则、连接路由/认证目标指纹、实际监听端点、开始时间和
`PossibleRunning`/`CleanupUnconfirmed` 状态。它不保存密码、私钥原值、私钥路径
原值、vault credential ID 原值、SSH session、socket、worker handle、代理凭据、
命令或流量。加载候选不会连接、启动、复用旧运行时或重放命令。

重启后的面板只读显示候选；用户必须 Inspect、Review start、重新匹配当前规则和
连接绑定、通过 review token 并显式 Confirm and start fresh。规则/连接/路由/认证
目标变化、版本不支持、清理未确认、StateStore 冲突和未知保存结果都 fail closed
并保留候选。正常确认清理或用户明确 Discard 才删除候选。Workspace 负责在后台
executor 中处理 StateStore 事件，GPUI 面板不做阻塞 I/O。

作者工程门禁已在当前 worktree 的独立 target
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/named-tunnel-runtime-20261114/target`
实际完成并退出 `0`。命令为：

```text
CARGO_TARGET_DIR=work/named-tunnel-runtime-20261114/target python3 scripts/check.py
```

实际结果：Python `131 run / 1 skipped`；workspace ordinary Rust
`2476 passed / 0 failed / 23 ignored`；app `1144 passed / 0 failed / 2 ignored`；
14 项 doctest（含 2 项 compile-fail）；依赖策略、fmt、workspace 严格 Clippy、
readiness/observer/small-stack controller 全部通过。focused core
`tunnel_runtime` 为 `8 passed / 0 failed`，app `tunnel_rule_tests` 为
`19 passed / 0 failed / 1127 filtered`，app `tunnels` 为
`22 passed / 0 failed / 1124 filtered`。完整门禁脚本日志在
`work/named-tunnel-runtime-20261114/logs/full-gate-7e6ecb3-rerun.log`，
专项日志也保留在同一目录。
app 链接阶段的 `__eh_frame section too large` 是已知 warning，未导致门禁失败。

独立严格只读代码与功能复核已经完成，固定同一 exact SHA，使用独立源码 worktree
和独立 Cargo target。原始复核报告已归档到
[项目内原始复核报告](testing/records/2026-11-14-named-tunnel-runtime-recovery-review-raw.md)，
项目内复核摘要见
[独立复核记录](testing/records/2026-11-14-named-tunnel-runtime-recovery-review.md)；
复核结论为 `APPROVE`，P0/P1/P2/P3 均为 `0`。独立复核实际完成：

- `keelshell-core` `tunnel_runtime`：`8 passed / 0 failed / 0 ignored`；
- `keelshell-app` `tunnel_rule_tests`：`19 passed / 0 failed / 1127 filtered`；
- `keelshell-app` `tunnels`：`22 passed / 0 failed / 1124 filtered`；
- app 测试清单：`1146 tests listed`；
- `cargo fmt --all -- --check`、core/app 严格 Clippy 和基线到实现 SHA 的
  `git diff --check`：全部 exit `0`。

当前状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。复核没有把原生
macOS/Windows/Linux、真实客户 SSH、Podman、IME、辅助技术、签名、公证、安装升级
和发布验收写成通过；作者完整 `scripts/check.py` 的结果由作者日志提供，独立复核
没有重复执行该完整脚本。app 测试链接阶段的 `__eh_frame section too large`
仍是已知 warning，没有改变任何 exit code。公司内网不可用，本片只允许本地
commit，不执行 push。证据采集完成后已清理临时独立 Cargo target、评审
worktree 和旧的命名隧道恢复 worktree；作者门禁日志与复核原文已保留在项目内。

详细设计、产品边界和测试证据见 [ADR 0127](adr/0127-named-tunnel-runtime-recovery.md)、
[命名隧道规则管理](product/NAMED_TUNNEL_RULE_MANAGEMENT.md) 和
[测试记录](testing/records/2026-11-14-named-tunnel-runtime-recovery.md)。

## 2026-11-14 Unicode 编辑器复核修复

上一轮对 `b239d78691420a30d54c914c9ad7253023b0b681` 的独立严格只读复核返回
`REQUEST_CHANGES`：P0 `0`、P1 `1`、P2 `2`、P3 `1`。历史回执已复制到当前
工作证据目录 [`previous-b239-review.md`](../work/unicode-codec-20261114/review/previous-b239-review.md)，
原始内容不要删除。

本轮实现与修复源码、测试绑定在提交
`368a7f836f72ca06b34276e5d587a34497fc7f13`，文档绑定和独立复核固定在最终提交
`6add70cdd367c65f500b7fadf35c7a4fc296ec5a`，完成四项修复：

- 成功保存后的完整读回继续使用审核时的 `TextFormat`，只把实际读回得到的换行
  策略合并回原编码/BOM 外壳，避免 BOM-less UTF-8 正文首个 U+FEFF 被重新解释
  为 BOM；
- UTF-16 LE/BE 差异预览先解码两侧正文，再调用逻辑文本差异 API；
- 测试记录改为绑定实际 target、完整门禁计数和当前 `source_head`，不再保留旧
  工作树的 SHA 或路径；
- session 错误文案改为准确说明当前入口只比较编码。

完整 `python3 scripts/check.py` 已在实现提交上实际等待并退出 `0`。它使用当前
checkout 的 `work/unicode-codec-20261114/full-target`，耗时约 `697s`：Python
`131 run / 1 skipped`，ordinary Rust `2462 passed / 0 failed / 23 ignored`，app
`1138 passed / 2 ignored`，14 项 doctest、依赖策略、fmt、workspace 严格 Clippy
和 readiness/observer/small-stack controller 均通过。日志和手工生成的 exit 回执
保存在 [`unicode-full-gate.log`](../work/unicode-codec-20261114/unicode-full-gate.log)
和 [`unicode-full-gate.exit.json`](../work/unicode-codec-20261114/unicode-full-gate.exit.json)。
app 链接阶段有已知 `__eh_frame section too large` warning，但门禁命令退出 `0`。
门禁结束后的外层 zsh wrapper 曾复用只读变量名 `status`，因此 wrapper 返回 `1`；
该问题发生在门禁成功末段之后，不能作为测试失败，已记录在 exit 回执中。
新增 app 回归为 `editor_format_successful_save_preserves_bomless_leading_feff` 与
`editor_format_utf16_diff_preview_uses_decoded_bodies`。

新的独立严格只读代码与功能复核已固定到最终提交 `6add70cdd367c65f500b7fadf35c7a4fc296ec5a`，使用父工作树之外的
独立源码、Cargo target 和日志目录，结论为 `APPROVE`：P0 `0`、P1 `0`、P2 `0`、
P3 `0`。复核回执保存在
[`final-review-6add70c.md`](../work/unicode-codec-20261114/review/final-review-6add70c.md)。
独立执行的 core Unicode 为 `23 passed / 0 failed / 189 filtered`，session
`editor_reviewed_` 为 `9 passed / 0 failed / 145 filtered`，app `editor_format`
为 `18 passed / 0 failed / 1122 filtered`，app `merge` 为 `39 passed / 0 failed /
1101 filtered`，targeted Clippy、fmt、diff check、文档绑定检查和 BOM-less U+FEFF
probe 均通过。独立复核没有代签作者完整门禁、原生桌面或真实远端边界。

当前状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。公司内网不可用，本片
只允许本地 commit，不执行 push。macOS 当前桌面、Windows/Linux 原生窗口、IME、
辅助技术、真实客户 SSH、Podman 联调、签名、公证、安装升级仍不由本片门禁代签。

## 2026-11-14 连接状态灯审查修复与完整门禁

对依赖提交 `98f8f9d0558098061563fcb9c24f500551fb361a`、窗口契约和工作台
chrome 的首轮新独立复核返回 `REQUEST_CHANGES`，P0/P1/P3 为 `0`，唯一 P2
是恢复未连接、Starting 和空活动面板可能误画成功色。依赖输入本身无待修发现。

当前源码把标签和底部状态栏统一按当前终端与恢复状态选择主题语义色，仅当前
`is_open()` 使用 `success`；缺失连接为 `danger`，其他未连接状态为 `warning`，
没有活动目标为 `muted`。两条新回归实际检查 painted quad 的背景，覆盖三套主题、
明暗、中英文、生命周期、恢复占位及真实 tab/空面板点击。修复前两项均复现失败，
修复后 `2 passed / 0 failed / 1134 filtered`，app 新清单为 `1136` 项。

源码与 Cargo 输入已通过 `indicator-fix-inputs-r1.json` 冻结；修复后的完整
`scripts/check.py` 已实际 wait 并 exit `0`：ordinary
`2442 passed / 0 failed / 23 ignored`，app 子集 `1134 passed / 2 ignored`，
14 项 doctest（含 2 compile-fail）、Python `131 run / 1 skipped`、fmt、
workspace 严格 Clippy 和三组 AI controller 通过。

另一位新 agent 对相同冻结输入返回 `APPROVE`，P0/P1/P2/P3 均为 `0`；
独立执行作者预构建二进制的 36 项重点回归通过，未独立重编译或代签作者完整
门禁。5 个显式输入与补充 519 个 Rust/Cargo 输入在审查期间不变。该复核发生于
本地修复提交前；父线程随后将源码、测试和文档固定在本地提交
`9363485ef19802c7b9b1292e2ba5b4adb57fd589`，逐一读取该提交的 519 个 Rust/Cargo
Git 对象，与复核冻结摘要全部一致。当前二进制摘要与独立执行复制件也相同，
实际 `--list` 仍为 `1136` 项，两个具体新增名称均存在。
当前状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_FROZEN_SOURCE_REVIEW_APPROVED /
COMMIT_BYTES_VERIFIED`；独立报告保留其复核时点与范围，提交绑定原件为
`work/dependency-refresh-20261114/indicator-fix-commit-binding-r1.json`。

依赖实现 worktree 已归档，两位审查 agent 已关闭，复核临时二进制与 runtime
已清理；只保留当前构建缓存和审计回执。修复完整记录见
[状态灯修复](testing/records/2026-11-14-connection-indicator-review-fix.md)；
公司内网不可用期间只本地提交，原生锁屏 `HOLD` 和跨平台边界继续保留。

## 2026-11-14 依赖官方升级与 Rust 1.100.0 完整门禁

在 `4340b19e0099a00e460281502fd5cebd7c753ab9` 的父工作树集成八个直接包升级：
GPUI Kit `0.9.1`、rust-embed `8.14.0`、zip `9.5.1`、rmcp `3.10.0`、
toml `1.2.1+spec-1.1.0`、win32job `2.1.0`、uuid `1.29.2`、flate2 `1.1.11`。
官方在线复查在 UTC `2026-11-14T00:58:01Z` 完成，`33 packages /
103 declarations and uses / 5 workspace members` 全部通过。所有直接 registry
依赖要求仍使用 `x.y`，Cargo.lock 保存解析版本；没有修改 Rust 源码。

本机默认工具链再次读回为 Rust/Cargo `1.100.0`。仓库继续没有工具链 pin 或
`rust-version`；后者表示最低支持版本，不负责选择最新编译器。

升级后的第二轮完整 `python3 scripts/check.py` 已实际 wait 并退出 `0`：
Python `131 run / 1 skipped`，workspace ordinary Rust
`2440 passed / 0 failed / 23 ignored`（app 子集 `1132 passed / 2 ignored`），
14 项 doctest（其中含 2 项 compile-fail）、fmt、workspace 严格 Clippy 和三组
AI controller 均通过。全新 target 的 app 清单为 `1134` 项，并核对了监控、
工作台 chrome 和窗口契约具体用例名。首轮门禁因缓存编译 `E0514` 退出 `1`；
具体根因仍未确定，失败原件保留。第二轮使用空 target 和显式当前工具链路径，
不把后续成功反推为首轮根因。

依赖切片已本地提交为 `98f8f9d`，作者门禁为 `AUTHOR_GATE_PASS`；首轮新独立
复核发现上述 UI 状态灯 P2，返回 `REQUEST_CHANGES`，后续从本页顶部修复接续。
公司内网暂不可用，只做本地提交，不执行 push。隔离实现
worktree 已归档，日志已复制到父工作树。Windows/Linux、原生桌面与真实远端等
边界仍开放；旧 `0.9.0` 原生包不能代签新依赖，锁屏导致的主题/紧凑窗口
`HOLD` 继续保留。详见[依赖升级记录](testing/records/2026-11-14-dependency-refresh.md)。

## 2026-11-14 主机监控面板视觉层级与紧凑几何收口

本片在 `31ca92b feat: refine workbench chrome` 上继续完善主机监控 rail，源码、
测试和首轮文档已固定在本地提交
`aab1718f6682a82860ded8f0510ab7ccb843a902`；公司内网暂不可用，不执行 push。

本次内容：

- 控制区显示监控对象、采样状态和诊断动作；资源指标、网络接口、监听端口、
  文件系统和 Linux 能力状态使用现有 registry、Palette、组件 token 和完整的
  双语 AccessKit 分组语义；
- 资源指标与网络接口采用卡片层级，监听端口使用协议/状态/端点的两级布局，
  探测结果公开独立语义；状态颜色只读取主题语义 token；
- 窄 rail 中的磁盘设备选择器继续使用固定 `160px` 内层视口，移除会让 GPUI
  内层滚动末端只露出半行的额外 gap，末条设备现在要求 bounds 完整落入视口后再点击；
- Linux 能力状态把状态徽章和检测年龄放到详情列元信息行，长英文能力名称可以换行，
  状态徽章保持单行并保留键盘可达性；
- `workspace/view.rs` 同步收录 Rust 1.100.0 `cargo fmt` 对上一片工作区代码的
  格式归一化，未改变工作区业务行为。

当前证据：

- 监控定向回归 `21 passed / 0 failed`，磁盘末条滚动选择和能力状态双语可达性均有
  独立回归；
- app 测试清单为 `1134` 项，本片新增层级回归已进入二进制；
- 完整 `python3 scripts/check.py` 已通过：依赖版本策略、Python `131 run / 1 skipped`、
  `cargo fmt --all -- --check`、workspace 严格 Clippy、workspace ordinary Rust
  `2440 passed / 0 failed / 23 ignored`（app 子集 `1132 passed / 0 failed / 2 ignored`）、
  14 项 doctest（含 2 项 compile-fail）以及 readiness/observer/small-stack 三组
  AI controller。首轮文档误把 2454 个通过结果当作 ordinary，又重复列出
  compile-fail；本次按原始日志纠正，未重新定义测试或改变其执行结果；
- 已确认当前本地 active/default toolchain 为 Rust `1.100.0`，项目没有
  `rust-toolchain.toml`、`.cargo/config*` 或 `rust-version`，Cargo 默认跟随开发机或
  CI runner 的 active toolchain；不把本地最新版本写成项目 MSRV。

独立严格只读复核已固定上述 SHA，在独立 worktree 和 Cargo target 中返回
`APPROVE`，P0/P1/P2/P3 均为 `0`。独立回执为 app 清单 `1134` 项、监控 `21/1113`、
层级回归 `1/1133`、磁盘几何 `1/1133`、工作台布局 `12/1122`（通过/过滤），
app 严格 Clippy、fmt 和差异检查均退出 `0`。复核中的 app 全量命令被中止并退出
`130`，未计为独立通过；作者完整门禁与独立定向证据分别保留。

macOS/Windows/Linux 原生窗口、IME、VoiceOver/NVDA、真实远端监控、Podman 联调、
签名安装和像素级验收仍保持开放。详见
[ADR 0125](adr/0125-monitor-panel-visual-hierarchy.md) 和
[测试记录](testing/records/2026-11-14-monitor-panel-visual-polish.md)。

本片精确 macOS arm64 release 包已完成构建和静态检查，但桌面专用通道本次实际
读回锁屏。三套输入的截图摘要完全相同，不能证明真实主题或语言变化；原生视觉
仍为 `HOLD`。本次自行启动的四个进程均已终止并 `wait`，没有安装应用。来源、
摘要和失败证据见[原生观察记录](testing/records/2026-11-14-monitor-panel-native-smoke.md)。

## 2026-11-14 响应式工作台紧凑连接库收口与完整门禁

响应式工作台上一片的实现基线为
`e1d410d2d61753f2e90296981199ed2b4b3d3703`；上一轮紧凑布局源码固定在
`9194d7da6c7fc628bc3d8f7c1155afb5c91e31f1`，当前源码与测试补强已固定在
`a33e4c3b154867e4ef542cd9ef6a1193aee31c19`。本片没有新增依赖，仍保持直接
registry 依赖使用 `x.y`、Cargo.lock 保存解析版本和三套主题通过 registry、
Palette、组件与 chrome token 扩展。

本次收口内容：

- AI 助手列继续按窗口宽度 34% 和 360px 上限计算；720px 以下的最低宽度为
  200px，720px 及以上为 240px。900×580 目标仍保持 240px，480px 空工作区
  打开 AI 时快速连接表单不再被最低宽度挤压；
- 磁盘设备列表改为固定 160px 高度，导航 header 增加后最后一行仍可滚动到达；
- workspace layout 测试和 workspace manifest 清理已适配 Rust 1.100.0 的严格
  Clippy；首次中文工作区测试与生产完整 AX label 对齐；
- 480px 窄窗口下，英文快速连接表单限制在自身区域滚动，Connect 动作在
  480×900 目标中滚动后仍可达；
- 480×580 低高度窗口将快速连接区域收紧为 `120px`，内容顶部对齐并在自身区域
  内滚动；连接库保留约 `48px` 的正向列表视口；
- 窄宽度连接库工具栏切换为图标优先布局，保留完整双语 AX label 和 tooltip；
  加密同步使用 Lock 图标，批量操作栏同步收紧间距并保留完整语义；
- 连接库回归扩展为 12 条连接记录，分别验证首条和末条记录、末条滚动后的全部
  动作 bounds，以及实际点击末条编辑动作；
- 新增工具栏与批量栏的中英文 `Role::Group`、AX label、按钮 bounds 和批量
  Move 动作回归；
- 新增 480×580、480×900，三套主题、明暗模式和中英文矩阵回归，检查连接管理器、
  文件夹树、操作栏、列表视口、首条连接及全部动作的真实 bounds 和 List/ListItem
  语义；两个尺寸均挂载真实保存连接，确认保存连接和快速连接的 Connect 动作在
  滚动后可达，快速连接滚动不会改变连接库 sibling bounds。
- 连接库旧帧的 Edit 回调现在按当前连接 UUID 重新解析；连接已删除、失效或不再
  可编辑时失败关闭，不能把旧绘制帧的动作转发到新记录。对应提交为
  `6d1f1a1`，并加入两条 stale painted Edit 回归。
- session-tab reveal key 纳入当前导航 rail 宽度；Connections `236px` 切换到
  Monitor `260px` 后会重新计算可见区域，关闭动作不会因旧布局结果被裁切。对应
  提交为 `a33e4c3`，并加入 workbench 回归。

最终源码 `a33e4c3b154867e4ef542cd9ef6a1193aee31c19` 上的 `python3 scripts/check.py`
已退出 `0`：依赖策略、Python
`131 passed / 1 skipped`、fmt、workspace 严格 Clippy、workspace Rust 测试、14 项
doctest 和 readiness/observer/small-stack 三组 AI controller 均通过；app 完整
测试为 `1127 passed / 2 ignored`，测试清单共 `1129` 项，新增测试已实际进入二进制。
当前 focused 回执为
design `4 passed / 1125 filtered out`、workbench `11 passed / 1118 filtered out`、
stale painted Edit `2 passed / 1127 filtered out`、连接库相关过滤测试
`84 passed / 1045 filtered out` 和 session-tabs `6 passed / 1123 filtered out`；
快速连接、工具栏/批量栏、窄窗主题/语言/高度矩阵各为 `1 passed / 0 failed`，
磁盘监控独立回归、首次中文启动回归和 `keelshell-core` workspace layout `12/12`
也已通过。

此前针对文档快照与源码提交 `78091e684f95cba5faecc54571992142f5fcd766` 的独立
严格只读复核返回 P0 `0`、P1 `1`、P2 `1`、P3 `2`。P1 stale painted Edit
和 P2 session-tab reveal 已分别由 `6d1f1a1` 与 `a33e4c3` 修复，并通过上述
回归和完整门禁；本次文档提交同步修正复核指出的旧 SHA 与旧测试清单。新的独立
针对包含本片文档修订的提交
`251a8ee0398f5927685ae36f2f486e2d6d77d71d`，新的独立严格只读代码与功能复核
已经在独立 worktree 和独立 Cargo target 中完成，merge recommendation 为
`APPROVE`，P0/P1/P2/P3 均为 `0`。复核实际确认 app 测试清单为 `1129` 项，
design `4/1125`、workbench `11/1118`、stale painted Edit `2/1127`、
session-tabs `6/1123`、core workspace layout `12/12`，Clippy、fmt 和差异检查
均通过；作者门禁的 app 全量 `1127 passed / 2 ignored` 仍单独保留为作者回执。
独立复核没有重新完成宽 `library` 过滤集和 app 全量测试，也没有把这些中止命令
写成独立通过。当前状态为
`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。第一次复核尝试因无法形成可审计
正文被终止，不能计入批准。

480×580/480×900 的 headless GPUI 几何和受控交互现在已经覆盖；当前源码还完成了
精确 macOS arm64 包结构检查和启动级原生短验，完整窗口级 AX/布局验收仍未关闭。
Windows/Linux 原生桌面、IME、VoiceOver/NVDA、真实 SSH/Podman、外部 AI provider、
签名、公证、安装升级和像素/多显示器验收仍保持开放。完整记录见
[响应式工作台紧凑修复](testing/records/2026-11-14-responsive-workbench-follow-up.md)
和[macOS 精确包短验](testing/records/2026-11-14-responsive-workbench-native-smoke.md)。

## 2026-11-14 Rust 工具链读回

本机当前 `rustc 1.100.0 (0ef1bc386 2026-11-09)`、`cargo 1.100.0
(3acc79c0c 2026-11-05)`，`stable-aarch64-apple-darwin` 为 rustup 的
active/default toolchain。本仓库没有 `rust-toolchain.toml`、`.cargo/config*` 或
环境变量覆盖；`cargo metadata --format-version 1 --no-deps --locked --offline`
显示 5 个 workspace crate 的 `rust_version` 均为 `null`。

因此当前编译默认跟随开发机或 CI runner 选择的 active toolchain，当前本地命令
实际使用 Rust 1.100.0。`Cargo.toml` 不需要写入 `rust-version = "1.100"`：
该字段声明最低支持版本（MSRV），不会选择最新编译器；写入本地最新版本还会把
环境事实错误地变成项目兼容性限制。项目继续不 pin Rust toolchain，直接 registry
依赖保持 `x.y`，`Cargo.lock` 保留解析后的完整版本。验证和决策记录见
[Rust 1.100.0 工具链记录](testing/records/2026-11-14-rust-1-100-0-toolchain.md)。

## 2026-11-13 当前片：响应式工作区层级

当前 worktree 基于源码基线
`e1d410d2d61753f2e90296981199ed2b4b3d3703`，实现了最小响应式工作区视觉切片：

- `900×580` 紧凑布局保留 AI、新建工作区窗口、凭据库和配置备份/恢复入口；
- 紧凑入口使用图标或短文案，同时保留完整双语 AccessKit label 和 tooltip；
- AI 列按窗口宽度的 34% 计算并限制在 `240..=360px`；
- 连接导航和主机监控仍在同一 rail 中互斥切换，连接页使用 `236px`，监控页使用
  `260px`，没有改变现有导航状态、按钮路由或 focus handle；
- `workspace-navigation-rail` 使用 `Role::Group` 和 `Workspace navigation`，
  header 使用 `ChromeTokens::pane_header_height` 并公开连接/监控双语语义；
- 视图继续只消费 registry、Palette、组件和 chrome token；没有新增依赖，没有修改
  `assistant.rs`、`files/**`、`snippet/**`、`session/core`。

新增 `workbench_layout` 回归已经过 focused 编译和运行：
`10 passed / 0 failed / 1113 filtered out`。覆盖 Frost、Graphite、Lavender，
明暗模式和中英文，检查关键入口可发现性、容器 bounds、完整 AX label、AI 打开后
终端预算、导航 rail `Group` role、header label、236/260px rail 宽度和 token 高度。
focused app Clippy（`--all-targets -- -D warnings`）、fmt 和 `git diff --check`
均通过。ADR、产品说明和测试记录分别见
[ADR 0124](adr/0124-responsive-workbench-polish.md)、
[原生工作区](product/NATIVE_WORKBENCH.md) 和
[测试记录](testing/records/2026-11-13-responsive-workbench-polish.md)。

## 交付边界

提交前的最终 focused 回执已执行并保留：

```text
cargo fmt --all -- --check
cargo test -p keelshell-app --locked --offline design::tests -- --test-threads=1
cargo test -p keelshell-app --locked --offline workbench_layout -- --test-threads=1
cargo clippy -p keelshell-app --locked --offline --all-targets -- -D warnings
git diff --check
```

结果为：fmt check 通过，design `4 passed / 0 failed / 1125 filtered out`，
workbench `11 passed / 0 failed / 1118 filtered out`，focused all-targets Clippy
通过，`git diff --check` 通过。这些命令验证当前源码和受控 GPUI 渲染树。
macOS/Windows/Linux 原生窗口、IME、
VoiceOver/NVDA、真实 SSH/Podman、最终安装包、签名、公证、像素对比度和多显示器
几何仍未由本片覆盖。

## 2026-11-08 当前片：工作区窗口切换器

当前源码候选在既有多窗口生命周期上增加窗口切换器。`AppWindows` 注册表为
每个窗口保存轻量 `WindowSummary`，`Workspace` 顶部显示窗口数量，展开列表显示
会话数和当前主机/快速连接空态，点击后激活真实原生窗口。窗口摘要不共享 SSH、
终端、文件、AI 或远程面板实体；新建窗口仍经过原配置闸门、维护状态检查和八窗口
上限。窗口序号由进程生命周期内的单调计数分配，关闭前置窗口后保留后续窗口序号。

源码已固定提交为
`373d4157ae816a29a74aa72cf490f688e3563457`。定向结果已通过：窗口入口 `3/3`、
全部窗口生命周期 `19/19`、modal/focus `12/12`、工作区布局 `8/8`、
主题 `9/9`；新增中文/英文与 Frost「霜白」、Graphite「曜石」、Lavender「雾紫」
组合回归，英文无障碍标签使用英文标点。窗口列表已接入统一 modal scope，增加
Dialog 语义、全屏输入隔离、focus trap、Escape 关闭和工作区焦点恢复。作者完整工程
门禁已通过，独立严格只读复核以该源码基线返回 `APPROVE`，P0/P1/P2/P3 均为 `0`。
设计、产品和测试记录分别见
[ADR 0123](adr/0123-workspace-window-switcher.md)、
[工作区窗口切换器](product/WORKSPACE_WINDOW_SWITCHER.md) 和
[测试记录](testing/records/2026-11-08-workspace-window-switcher.md)。

当前定向结果使用重新编译的 `1,121` 项 app 测试清单。源码提交后的完整
`python3 scripts/check.py` 已使用命令会话 `63031` 退出 `0`：Python
`131 run / 1 skipped`，app `1,119 passed / 2 ignored`，workspace 其余 Rust
测试、14 项 doctest、严格 Clippy、依赖策略和 readiness/observer/small-stack 三组
AI controller 均通过；`__eh_frame section too large` 仍只是链接器 warning。
独立复核已完成；下一步重新构建绑定当前源码 SHA 的 macOS 包并执行有限原生多窗口
观察。旧 `package-r4` 只代签主题/语言短验，不代签本片新增窗口切换器。

## 2026-11-08 当前片：编辑器格式与合并迟到读回组合回归收口

本片在源码基线
`1d0898025bcd714550de8d702d5c8026242844bf`
上新增两个真实 GPUI/SFTP 组合回归，生产代码没有修改：

- `independent_editor_format_no_newline_keeps_lf_policy_across_empty_and_plain_saves`
  验证无换行远端文件首次插入 LF 后，连续保存带换行、空内容、再次无换行和后续
  带换行内容仍保留 LF 插入策略，不增加 BOM 或 CRLF；
- `merge_adopt_save_keeps_late_newer_draft_and_rebases_next_save` 验证合并采用、完整
  保存审核、发布期间较新草稿、成功目标读回、新编辑基线和下一次 CRLF 保存的
  完整时序。

完整工程门禁第二轮 `python3 scripts/check.py` 已退出 `0`：

- Python：`131 run / 1 skipped`；
- workspace Rust：`2,437 passed / 23 ignored`；
- `keelshell-app`：`1,115 passed / 2 ignored`；
- 14 项 doctest、依赖版本策略、严格 workspace Clippy 和 readiness/observer/
  small-stack AI controller 全部通过；
- `cargo fmt --all -- --check` 与 `git diff --check` 均退出 `0`。

第一轮完整门禁已在 Python `131 run / 1 skipped` 后于格式阶段退出 `1`，原因是新增
断言尚未按 `cargo fmt` 展开；格式化后重新启动完整门禁并通过。失败事实和最终回执
保存在[组合回归测试记录](testing/records/2026-11-08-editor-format-merge-combinations.md)。

源码实现已固定提交为
`9134cfaa771057a0f246ead47479be3706248415`。独立严格只读复核已以该 SHA 为基线
完成两轮检查：首轮发现并由本次文档修订关闭一个 P2 状态文案问题；第二轮已返回
`APPROVE`，P0/P1/P2/P3 均为 `0`。精确 macOS 包、local/tracking/remote SHA
回读、Windows/Linux 原生窗口、IME、VoiceOver、真实 SSH/Podman 长流程和外部 AI
provider 仍未验收；既有旧包不能代签本片新增测试。

## 2026-11-08 本片源码与 macOS 包收口

本片源码提交为 `e5891d1c8101c00944979ec2761718d376ae60d5`，已推送到
`feature/main-visual-integration-20261106`；local、tracking、remote 三者已读回
一致。完整工程门禁已在该源码候选上通过：Python `131 run / 1 skipped`，
workspace Rust `2,435 passed / 23 ignored`，app `1,113 passed / 2 ignored`，
14 项 doctest、依赖策略、严格 Clippy 和三个 AI controller 均通过。

当前 exact package 为
`work/native-cua-20261107/package-r11/KeelShell.app`，manifest 的
`build.commit` 绑定上述源码 SHA，GUI/MCP 为 arm64 Mach-O，实际最低版本为
macOS `15.0`。manifest、GUI、MCP SHA-256 分别为：

- `54ed0e165a27c760f4672673bbde16d3572efaff815b50b6b7c629927b49f8d6`
- `c6afd91a3cdb219a3974c92b90f0d2310d70c16a45ce1efc068bffe8e22cc139`
- `94eaf83f8bbb23ae4db7cc190c9c06332ddbc19b4bf9246dcdf35f43b3645d8f`

已解锁 macOS 桌面短验确认默认工作台、连接库空态、三套主题 Frost「霜白」、
Graphite「曜石」、Lavender「雾紫」和中英文切换可见；测试使用隔离数据目录，
没有安装或替换已有应用，进程与隔离数据已回收。

源码提交后的两次独立只读复核尝试均未返回可审计正文：第一次重复轮询旧会话后
被系统停止，第二次隔离 agent 在中断请求后仍无报告。当前必须记录
`NON_AUTHOR_REVIEW_UNAVAILABLE`，不能把作者门禁或原生短验写成非作者批准。

## 2026-11-08 内联合并审核视口候选

当前分支 `feature/main-visual-integration-20261106` 的源码基线为
`ed75c9802509b54f4c57dc38f8573fe0f43aeee1`，本片的四个源码变更文件
`files/merge.rs`、`files/review_viewport_tests.rs`、`files/transfer_tests.rs` 和
`files/conflict_merge_tests.rs` 已随源码提交
`e5891d1c8101c00944979ec2761718d376ae60d5` 收口。它们将内联三方合并审核收口为固定
`220 px` 卡片；
正文独立滚动，动作区两行固定，展开按钮移入动作栏。正文滚轮通过 review ID 绑定的
处理器显式更新并截断 `body_scroll`，旧帧和新审核之间失败关闭；切换冲突、完整版本
和冲突选择方案会复位正文滚动。

截至本交接片，专项结果为：

- stale replacement frame：`1 passed / 0 failed / 1114 filtered out`；
- conflict merge：`27 passed / 0 failed / 1088 filtered out`；
- review viewport：`12 passed / 0 failed / 1103 filtered out`；
- editor format：`13 passed / 0 failed / 1102 filtered out`；
- `cargo fmt --all -- --check` 与 `git diff --check`：退出 `0`。

本片随后按[共享 target 指南](testing/SHARED_TARGET.md)失效五个项目包并完成真实
重编。第一次重编在严格 Clippy 阶段发现并修复了未使用构造器、测试专用清理函数、
`then_some` 风格和未使用测试参数；最终 `python3 scripts/check.py` 退出 `0`：
Python `131 run / 1 skipped`，workspace Rust 测试 `2,435 passed / 23 ignored`，
其中 app 为 `1,113 passed / 2 ignored`，14 项 doctest、严格 Clippy、依赖策略和
readiness/observer/small-stack controller 全部通过。完整证据保存在[本片测试记录]
(testing/records/2026-11-08-inline-merge-review-viewport.md)；设计决策见
[ADR 0121](adr/0121-inline-merge-review-viewport.md)。首次视口回归曾因外层尾部裁掉
正文命中区域、并因 stop propagation 阻止正文默认滚动而失败，修复后精确重跑通过。

后续工作：按共享 target 规则重新执行完整工程门禁，固定本片源码并取得可审计的
独立严格只读复核，再生成绑定新源码的精确包。Windows/Linux 原生窗口、IME、
VoiceOver、真实 SSH/Podman 长流程和外部 AI provider 仍未验收。

## 2026-11-08 编辑器格式与合并迟到读回组合回归

源码实现已固定在
`9134cfaa771057a0f246ead47479be3706248415`，本片包含两个真实 GPUI/SFTP 回归：

- `independent_editor_format_no_newline_keeps_lf_policy_across_empty_and_plain_saves`
  从无换行远端文件开始，验证首次 Enter 使用 LF，连续保存空内容和无换行内容后仍
  保留 LF，后续编辑不增加 BOM 或 CRLF，并核对四份原始备份；
- `merge_adopt_save_keeps_late_newer_draft_and_rebases_next_save` 验证读取合并、
  Adopt、完整 Save review、发布期间较新草稿、成功读回、新基线和下一次保存的
  精确 CRLF 字节。

定向结果为：编辑器格式 `14 passed / 0 failed / 1103 filtered out`，冲突合并
`29 passed / 0 failed / 1088 filtered out`，两个新增用例精确运行均为
`1 passed / 0 failed / 1116 filtered out`。格式、差异检查已通过。生产代码没有
修改；产品、ADR 和失败证据见
[组合回归测试记录](testing/records/2026-11-08-editor-format-merge-combinations.md)
和 [ADR 0122](adr/0122-editor-format-and-late-readback-regressions.md)。

首次接入的测试上下文编译错误已修复，并重新确认新增用例出现在当前测试清单中。
并行切片曾覆盖第一个测试文件的未提交内容，已按返回差分重新应用并重跑；这一
事实保留在测试记录中。第二轮完整工程门禁已通过；首次独立复核的唯一 P2 是提交
后状态文案未同步，本次文档修订已补齐固定 SHA 和复核状态。精确新包和原生验收仍
按上方当前交接顺序待执行。文档修订后的第二次独立复核已确认 P2 关闭并给出
`APPROVE`。

## 2026-11-08 快速操作中心生产布局与键盘收口

本片源码已提交为
`c13724a9eab9df3d97fdb3ed1eaf25b5dcfabffa`，并推送到
`origin/feature/main-visual-integration-20261106`；源码提交的 local、tracking、
remote SHA 已读回一致。该提交继续完成 Workspace 快速操作中心。动作目录仍由纯
Rust 元数据、双语标题、
关键词、平台快捷键和会话上下文过滤组成；Workspace 只负责模态生命周期与已有
动作映射。当前生产布局已收敛为单一真实根节点：`command-palette-modal` 覆盖
模态内容区，`command-palette-card` 承载搜索、结果滚动区和计数栏。结果行有稳定
尺寸、列表语义、选中状态和主题 token，新增主题仍只扩展 registry/Palette/token。

`Command/Ctrl+K` 已接入现有 Workspace action binding；输入框、方向键、Enter、
鼠标点击和 Escape 共用现有 modal/focus 机制。Enter 通过模态根的捕获键盘路径
触发现有 Workspace 流程，关闭后恢复工作台焦点。快速连接现在进入真正的一次性
连接模态，主机监控会选择 Monitor 导航面板；快速操作中心不直接执行 SSH、文件、
AI provider 或持久化写入。

该源码提交的定向 GPUI 回归为 `16 passed / 0 failed / 1091 filtered out`，
覆盖纯动作目录、双语、焦点恢复、连接管理器 Enter 路由、一次性快速连接、主机
监控路由、首末会话边界、平台快捷键和 `900×580` 卡片/结果区边界。此前失败是
关闭后对已卸载元素调用 `find`，已改为 GPUI 的 `try_find(...).is_none()` 语义；
补充主机监控回归时还曾因测试夹具没有挂载 `monitor-scroll` 节点失败，随后改为
验证稳定的生产 `navigation-monitor` 语义。测试专用渲染分支和诊断 `eprintln!`
已删除。随后完整
`python3 scripts/check.py` 已退出 `0`：依赖策略、Python `131 run / 1 skipped`、
格式、workspace 严格 Clippy、workspace tests、doctests 和 readiness/
observer/small-stack AI controller 均通过。workspace app 单元测试为
`1105 passed / 2 ignored`，core、mcp、session 的单元与集成测试均通过；显式依赖
Podman、OpenSSH 或真实 Linux 采样的可选测试仍按边界保持 ignored。当前只剩固定
源码交付已完成；新的原生多平台、真实 SSH/Podman 与外部
AI provider 验收不由本片 headless 门禁代签。

新的独立严格只读复核已针对该源码提交完成，P0/P1/P2/P3 均为 `0`，范围内
merge recommendation 为 `APPROVE`；复核工作树保持未修改。文档收口提交会继续
保留本片完整门禁、精确重跑和源码 SHA 证据。

本次完整门禁首轮曾在 Python 阶段因 4 个既有
`test_windows_native_crash.PreparationTreeTests` 用例的 `124` 超时退出而退出
`1`；精确重跑这 4 项为 `4 passed / 0 failed`，随后完整门禁再次执行并退出 `0`。
该失败与精确复跑均保留在[测试记录](testing/records/2026-11-08-command-palette.md)中。

当前边界：macOS/Windows/Linux 原生窗口、完整 IME/VoiceOver、真实 SSH/Podman
长流程、外部 AI provider 和当前源码包验收仍不由 headless GPUI 回归代签。新建
窗口、分屏和布局保存暂时继续使用工具栏入口，后续加入动作目录只需扩展纯动作
注册和 Workspace 映射。详细记录见[快速操作中心测试记录](testing/records/2026-11-08-command-palette.md)、
[产品说明](product/COMMAND_PALETTE.md)和[ADR 0120](adr/0120-command-palette.md)。

## 2026-11-07 连接库视觉层级与主动作候选

本片最终源码已提交为
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9`，当前连接库行在宽表格和窄卡片中统一表达
selected 状态：行公开 `aria-selected`，使用 `Palette.surface_active` 和
`Palette.accent` 左边界，非选中悬停使用 `surface_hover`。活动连接库中的
Connect 使用 GPUI Kit `primary` 变体，Edit/Move/Copy/Trash 保持次级动作；
回收站继续只提供 Restore 和永久删除。没有增加主题 ID 分支，Frost「霜白」、
Graphite「曜石」、Lavender「雾紫」继续通过 registry/Palette/token 驱动，新增
主题只扩展注册定义。

验证已完成：连接库定向 `27 passed / 0 failed`，selected-row 精确回归
`1 passed / 0 failed`，app 严格 Clippy、格式和差异检查通过；完整
`python3 scripts/check.py` 退出 `0`，Python `131 run / 1 skipped`、
app `1089 passed / 2 ignored`，workspace 其它测试、doctests、依赖策略和三组
AI controller 通过。首次新增测试错误地把可访问标签当成可见文本的失败已保留，
并已修正为验证可访问标签与语义状态；最终提交还收敛了 selectable role 和选中
行悬停状态。设计见[ADR 0119](adr/0119-connection-library-visual-hierarchy.md)，
回执见[连接库视觉层级记录](testing/records/2026-11-07-connection-library-visual-hierarchy.md)。

新的独立严格只读复核已经以
`48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9` 为代码基线完成，P0/P1/P2/P3 均为
`0`，merge recommendation 为 `APPROVE`；复核实际执行 selected-row 精确回归
`1 passed / 0 failed / 1090 filtered out`，并核对了行状态、原生 selectable
语义、宽窄布局复用、主动作层级和主题 token 解耦。
当前包不包含 `48eb9f5f73fddd2ccfa14cf630e70b28f4d108c9`，因此不能把既有 macOS 包写成本片原生通过；当前源码
macOS 包、Windows/Linux 原生窗口、完整键盘/IME/VoiceOver、真实 SSH/Podman
长流程和外部 AI provider 仍待独立验收。

## 2026-11-07 主题选择器 exact package 与复核状态

主题选择器源码已提交并推送到
`feature/main-visual-integration-20261106`，源码 SHA 为
`37b8eaf7d65fe31133c5e5058848341c50c8a820`；local、tracking 和远端 SHA 已读回
一致。该提交已通过完整工程门禁：Python `131 run / 1 skipped`，app
`1088 passed / 2 ignored`，workspace Clippy、Rust tests、doctests、依赖策略和
三组 AI controller 均通过。

当前 exact source 已重新构建
`work/native-cua-20261107/package-r10/KeelShell.app`，bundle 结构检查退出 0，
manifest 精确绑定上述 SHA。解锁 macOS 短验确认三套主题均可切换，明暗模式按钮
保持可见，空连接库和主题对话框 AX 元素可读；测试进程已按 PID 读回并回收。该
smoke 仍不覆盖真实 SSH、分屏、900×580 accessibility frame、IME、VoiceOver、
签名、公证、安装、Windows/Linux 原生桌面。

针对该 exact source 已启动三次新的独立严格只读复核，但均在有界等待内没有返回
可审计正文，随后停止；没有任何复核任务修改、提交、推送或清理工作树。当前状态
保持 `NON_AUTHOR_REVIEW_UNAVAILABLE`，不能把本片写成非作者批准。包、smoke 和
复核边界见[主题选择器记录](testing/records/2026-11-07-theme-picker-layout.md)。

## 2026-11-07 主题选择器布局与可见性修复

本片继续推进用户选定的三套主题方向：Frost「霜白」、Graphite「曜石」、
Lavender「雾紫」全部保留在项目中，并通过 registry、语义 Palette、组件形状、
Chrome/Typography token 解耦。主题卡片目录现在拥有独立滚动区域，明暗模式控制
固定在弹窗可见动作区；主题卡片的工作台预览和真实生产按钮均保持注册表驱动，
后续新增主题只需添加主题定义和 token。

定向验证已通过：主题模块 9/9、磁盘监控主题联动 1/1、格式、差异检查和应用
严格 Clippy 均通过。随后 `python3 scripts/check.py` 退出 0，Python
`131 run / 1 skipped`，完整 app 测试为 `1088 passed / 2 ignored`，workspace
其余测试、doctests、依赖版本策略和 readiness/observer/small-stack AI
controller 均通过。既有 `__eh_frame section too large` linker warning 保留，
不影响退出码。详细证据见[主题选择器记录](testing/records/2026-11-07-theme-picker-layout.md)。

本片当前待执行交付顺序仍是：固定源码提交后启动新的独立严格只读复核，按
P0/P1/P2/P3 返回路径/行号、真实测试计数、限制和 merge recommendation；复核
通过后推送分支并读回本地、tracking、远端 SHA。macOS/Windows/Linux 原生窗口、
900×580 accessibility frame、IME、VoiceOver、安装包和真实外部 provider 继续
保持各自独立验收边界。

## 2026-11-07 工具栏远程上下文布局修复

本轮修复了 macOS 短验发现的真实布局缺陷：紧凑工具栏中的“远程工作区”上下文
在窄窗口会被压成逐字竖排。源码提交
`2d05525c00d21455420d25cce675bacdedf8a73a` 已在本地完成，包含上下文最小宽度、
标题和值的单行省略、1280px 品牌/紧凑动作断点拆分、1280px 布局回归以及开发机
绝对路径记录修复。

当前验证结果：

- `keelshell-app` 定向布局、主题和更新面板回归均通过；
- 当前源码 `cargo test -p keelshell-app --locked --offline -- --test-threads=4`
  为 `1087 passed / 2 ignored`；
- `python3 scripts/check.py` 退出 `0`，Python 131 passed / 1 skipped、格式、
  严格 workspace Clippy、workspace tests、doctests、依赖策略和三组 AI controller
  均通过；linker `__eh_frame section too large` warning 仍按历史边界保留；
- 当前精确 macOS 包为 `work/native-cua-20261107/package-r9/KeelShell.app`，
  manifest 绑定源码 `2d05525...`，包结构检查退出 `0`；manifest/app/MCP SHA-256
  分别为 `8cceb76d...`、`524ae37f...`、`94eaf83f...`；
- 解锁后的 macOS 短验启动当前包，观察到工具栏 `Remote workspace` 单行显示，
  主题对话框可打开/关闭，进程和 `data-r9` 已清理。外部 900×580 调整仍受 GPUI
  无边框窗口 accessibility frame 限制，本次不把紧凑尺寸原生验收写成通过。

当前 exact tip 已完成一次独立只读复核，复核结果为 P0/P1/P2/P3 均为 0，
范围内 merge recommendation 为 `APPROVE`。复核实际验证了工作区布局、工具栏
断点、上下文最小宽度、双语 AX 标签、格式和差异检查；它没有把作者的完整工程
门禁或 macOS 原生短验重复计入独立证据。此前 exact HEAD 复核提出的窄布局和绝对
路径问题已由本轮处理。Windows/Linux 原生窗口、完整键盘/IME/VoiceOver、真实
SSH/Podman 当前包联调和外部 AI provider 仍保持开放。

## 2026-11-07 Reef / reef-template 参考边界

本次新增文档切片 [ADR 0118](adr/0118-reef-reference-boundary.md)，把 Reef 与
reef-template 的工程参考价值固定为 workspace resolver 3、workspace 版本/元数据
继承、配置分层、显式生命周期与 shutdown、owner 归属和证据管理。KeelShell
继续使用自己的 GPUI Kit 原生桌面分层，不引入 Reef 依赖。

本轮只读检查记录了 Reef revision
`75ccbf5d864fadb543ecfa62fc9871c7cff04153`；该 checkout 在检查时已有一处
`Cargo.toml` 工作树修改。检查覆盖 Reef workspace、`reef-compose` 的 feature
装配和 `application` 生命周期实现、Core 配置读取，以及模板的
`crates/app` 继承、`conf/` 三层配置、入口和工程同步文档。reef-template
revision 为 `03bd71c096fdd941e389584c44bdcb40dfed0cdf`，检查时工作树干净。
两处源仓库均未被本任务修改、提交或清理。

边界已明确：不复制 server runtime、全局 Compose、HTTP/MQ/DB/telemetry 等
服务端中间件，不引入私有 Git 依赖、内网 URL、模板 Liquid 或服务部署约定。
本轮只做 Markdown 文档和差异验证，未把静态源码检查写成 Reef 最新版本或
KeelShell 原生/远端功能验收。未来任何真实 Reef 接入都必须另建 ADR，固定
可公开依赖、API revision、配置、owner 和平台验收。

## 2026-11-07 会话、连接库与 AI 助手界面增强

本片已提交并推送到 `feature/main-visual-integration-20261106`，精确提交为
`dd6c88b0710bb56ca785eeeb83ee0835a1b77ede`；当前工作树、tracking 和远端 SHA
一致且工作树干净。
本片包含连接库范围标题与空态、会话条数量与双语 AX、上一个/下一个会话快捷键，
以及 AI 助手的六阶段工作流轨道。AI 回复到达后会自动滚动到回复审核区，避免
长上下文把审核入口推到首屏之外；知识保存动作固定在助手底部审核栏，Agent 停止
并编辑问题会在最新布局完成后按实际几何显露问题输入框。

定向验证已通过：连接库 26/26、AI 助手 6/6、会话条 5/5，快捷键专项 1/1，
`cargo check -p keelshell-app --locked`、格式和差异检查通过；长 MCP、Agent 编辑
问题和知识保存回归均为 1/1。AI 定向测试首轮曾因回复区域不可见失败，补充响应
滚动锚点后精确重跑为 2/2；Agent 编辑问题第一次和一次旧布局修补也曾失败，最终
改为 render 后 `on_prepaint` 几何滚动并精确重跑为 1/1。所有失败事实保留在
[本轮测试记录](testing/records/2026-11-07-session-library-ai-ui-polish.md)。

设计决策见 [ADR 0117](adr/0117-session-navigation-and-assistant-workflow-surface.md)；
产品流程见 [AI 助手审核工作流](product/AI_ASSISTANT_WORKFLOW.md)。Frost「霜白」、
Graphite「曜石」、Lavender「雾紫」继续由主题 registry、Palette、组件形状、
Chrome token 和 Typography token 提供，视图不按主题 ID 分支，新增主题只需扩展
注册定义和语义 token。

完整 `python3 scripts/check.py` 已重新执行并退出 0，包含 Python 131/1 skipped、
格式、严格 workspace Clippy、workspace tests、doctests、依赖策略和三组 AI
controller。当前提交已完成新的独立严格只读复核：P0/P1/P2/P3 均为 0，
复核建议为 `APPROVE`；复核使用两个定向用例，各 1 passed，保留
`__eh_frame section too large` linker warning 及共享 target 只读限制。当前精确
源码已重新失效构建并生成 `package-r6`，manifest 绑定 `dd6c88b...`，结构检查
退出 0；解锁后的 macOS 短验确认 Frost、Graphite、Lavender 三套主题均可切换，
AI 助手侧栏和其 AX 元素可见，运行进程已关闭。Windows/Linux 原生桌面、完整键盘
焦点、IME、VoiceOver、当前源码包的长流程、Podman/SSH 真实联调和外部 AI provider
仍不能由本轮 headless 或短范围 smoke 代签。

## 2026-11-07 AI 助手窄栏工作流修正

在上述初始提交之后，`58fd2d248302958675b0652b7bc15b8aee77b580` 修正了 AI
助手窄栏工作流轨道：六个阶段以完整阶段项换行，分隔符附着前一个阶段项，
当前宽度可以直接看到“保存知识”，不再依赖横向滚动。新增 360×760 中英文 GPUI
布局测试，确认六个阶段均在轨道边界内可见。

该提交的定向验证为窄栏专项 1/1、assistant 专项 7/7、app Clippy 和
`cargo check -p keelshell-app --locked` 通过；新的独立严格只读复核返回
P0/P1/P2/P3 均为 0，并建议 `APPROVE`。复核确认 1087 项 app 测试库存，
知识审核 footer 相关 3 项与窄栏专项合计 4/4 通过；首次不匹配过滤器的
0/1087 结果没有计入验收。

当前精确源码包为 `work/native-cua-20261107/package-r7/KeelShell.app`，
manifest 绑定 `58fd2d2`，结构检查退出 0；macOS 短验观察到六段轨道完整换行
为两行，助手 AX 元素可读，测试进程已关闭。该提交与文档提交均已推送，最终
HEAD/tracking/远端 SHA 已读回一致。跨平台原生桌面、完整键盘焦点、
IME、VoiceOver、Podman/SSH 真实联调和外部 AI provider 仍保持开放。

## 2026-11-07 连接身份栏最终候选

本轮实现已完成源码与测试修复，当前候选包含 pane 连接身份栏、恢复状态语义
修正、主题 typography token 以及 live IPv6 端点显示修复。源码仍保持 UI、core、
session、AI 分层：

- 每个终端 pane 都显示会话标签、真实 SSH 用户/主机/端口、`SSH / UTF-8`
  和带文字的生命周期状态；
- 单 pane、横向/纵向分屏和恢复占位共用同一身份栏渲染路径；
- 未保存的一次性连接显示 `未保存连接 / Unsaved connection`，连接档案缺失
  显示 `配置已移除 / Profile unavailable`，两者使用不同语义色；
- Frost、Graphite、Lavender 仍全部可选，主题由 registry、Palette、组件形状、
  chrome token 和 typography token 组成，新增主题只需添加注册定义；
- pane 标签区域已收紧身份栏宽度，保留分屏标签与关闭动作的点击命中空间。

当前源码定向回归和完整工程门禁均通过：Python 131 passed/1 skipped，
`keelshell-app` 1079 passed/2 ignored，core 197 passed，session 434 passed/16
ignored；workspace integration、MCP、SSH loopback、transfer、doctest、readiness、
observer 和 small-stack AI controller 也通过。严格 workspace Clippy、格式和
差异检查通过。app 单线程与最终并行重跑、session 四线程重跑均达到相同计数。
首轮完整门禁中新增测试曾触发 `needless_borrow`，已修复并保留失败事实；当前还
不能把该候选写成已获独立复核批准。

IPv6 P2 已由源码提交 `b2f875458e42cf374cb3d388d808e986a415c2cd` 修复：初始连接
和重连路径统一调用 `jump_host_picker::endpoint`，身份栏显示
`operator@[2001:db8::8]:2202`；helper 定向测试 1/1，完整工程门禁再次退出 `0`。
该提交之后的文档更新只记录证据和状态，不改变这次包构建的 Rust 输入。

`package-r3` 仍保留为源码提交 `2589d3b` 的详细人工 GUI 观察包；
当前源码精确包为 `work/native-cua-20261107/package-r5/KeelShell.app`，manifest
绑定 `b2f875458e42cf374cb3d388d808e986a415c2cd`，结构检查退出 `0`，并完成了
当前包的短范围主题/双语 native smoke check。两份包的证据边界在测试记录中分开。

上一轮固定提交 `2589d3b` 的独立报告提出的证据问题已由 `9d02440` 及后续文档
提交修复：Hallmark preflight 已指向当前 pane 记录，ADR 计数已同步，旧包补齐
源码提交、构建退出码和 manifest/app/MCP hash。固定 `9d02440` 的旧复核正文曾
另外指出 IPv6 显示与包 provenance 两项问题；前者已由 `b2f8754` 修复，后者由
`package-r5` 的精确 manifest 绑定收口。针对当前 exact source `b2f8754` 新开的
两次独立只读复核任务均在有界等待内没有返回可审计正文，随后已停止；它们没有
修改、提交、推送或清理目标工作树。因此当前状态是
`NON_AUTHOR_REVIEW_UNAVAILABLE`，不能把旧报告、代理启动或超时写成当前提交批准。

完整 app 并行负载下，route/split 夹具曾在 10 秒 UI 就绪等待内超时；当时
gateway 和 target 都已认证，跳板通道及 SSH 会话仍存活，`connect_route` 仍在
进行。由于生产 SSH 操作期限为 15 秒，测试就绪等待改为有界 20 秒，route
server session 改为有界 120 秒以覆盖完整 split/close 循环。生产 reconnect、
SSH 生命周期、命令审核和 AI 权限没有改动。session 还曾出现一次 traceroute
夹具抖动（129 passed/1 failed/4 ignored，`traceroute_hop_limit` 实际得到
`timeout`）；精确重跑、四线程重跑和最终 workspace 门禁均通过，失败事实已保留。

旧提交 `d9072ed` 的 Avicenna 只读报告发现 1 个 P1 与 3 个 P2；P1 已由
`1e63165` 修复，三个 P2 已在当前源码中关闭。该旧报告不替代当前提交的独立
复核；当前 exact source 的两次新复核没有形成正文，后续维护仍必须固定最新提交
重新审查并返回 P0/P1/P2/P3、路径/行号、真实计数、限制和 merge recommendation。

macOS 原生包的详细人工观察绑定源码提交 `2589d3b` 的
`work/native-cua-20261107/package-r3/KeelShell.app` 与隔离数据目录
`work/native-cua-20261107/data-r3`，回环端点为 `hdh@127.0.0.1:22222`；已观察
单 pane、横/纵分屏、合并、关闭面板、三主题、双语、主题切换后的身份栏与会话标签、
真实 SSH 连接、断开和恢复状态。当前源码的 `package-r5` 已完成结构检查和短范围
主题/双语 smoke check；900×580 调整因 GPUI 无边框窗口 accessibility frame 异常
仍未通过。Windows/Linux 桌面、
完整键盘焦点、IME/VoiceOver 矩阵和 Podman 真实联调仍未验证。Headless GPUI
测试不能代替这些边界。

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

## 当前候选接续顺序

1. 提交当前源码、测试和文档候选。
2. 固定新提交启动独立严格只读复核，返回 P0/P1/P2/P3、路径/行号、真实测试计数、
   限制和 merge recommendation；发现 P1/P2 时修复并重新门禁、重新复核。
3. 复核通过后推送当前功能分支，读回本地、tracking 和远端 SHA。
4. 使用已解锁 macOS 桌面完成三主题、900×580、横/纵分屏、连接中断、键盘/AX
   观察；Windows/Linux 原生和 Podman 真实联调继续分别验收。

## 历史交付顺序（2026-11-06 交接片）

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
