# 文件列表键盘与状态语义 — 2026-11-16

基线 `e709d02d86ccfbb1a01677f7cda618b64050765b`。
状态 `ENGINEERING_PASS / NATIVE_PARTIAL_OBSERVED`；最终完整门禁和非作者源码
复核已通过，另一个新非作者限定批准原生证据，原生观察仅覆盖下述子集。
归档及提交推送回执分别记录，
不将其合并成完整产品验收。本批集中开发后统一集成、专项修复和最终完整门禁。
用户于 2026-11-16 明确远端恢复可用，代码应及时提交并推送，取代此前仅本地交付。

父负责 GPUI 接线、两个原生列表、现有重命名/删除审核入口、状态辅助技术标签及
受控集成。独立作者在受管 worktree 实现通用选择模型，仅修改 selection.rs；
逐字节导入 SHA-256
`74187bb0e3417ab9766f82ca824dc8a883fc112b46e677c069a337719daffe4c`。
作者独立 rustc wrapper 实际 `31/0/0`，含原 3 项与新增 28 项，库存逐名相等；
Windows surrogate 条件用例未在本机编译或运行。作者未运行 Cargo、共享 target，
没有提交或新建 feature 分支；报告及回执已收取，agent 已关闭，受管 worktree
实际路径和 Git 记录均 absent。

唯一 Cargo target 继续归父 checkout：
`work/recovery-batch-integration-20261115/target`。没有切换来源，不清第三方缓存。
实际工程输入、程序、库存和验证结果在完成后逐项绑定，不能按预期计数。

首轮键盘专项 r1 在测试 trait/索引类型、观察器包装类型及借用适配处编译失败，
实际 `exit 101 / reaped / 14.494s`；未执行测试。修后 r2 实际
`exit 101 / reaped / 26.101s`，`7 passed / 3 failed / 0 ignored`。
三项失败分别为两个状态 ID 尚未加入测试观察器，以及 APFS 拒绝非法 UTF-8 名称
的实际文件创建。原件保持；状态观察入口补齐，原生文件系统浏览与注入的非法
PathBuf 界面反例分别记录，不把注入值当作实际文件创建成功。

键盘 r3 实际 `exit 101 / reaped / 25.808s`，`10/3/0`。名称 Enter 的订阅
尚未在同一次 App update 内投递，路径反例又使用不发 Change 的静默 set_value；
改为真实用户路径输入/鼠标改选，并在应用更新边界后观察，不改生产审核 guard。
r4 实际 `exit 101 / reaped / 19.223s`，`12/1/0`，两项重命名通过，含确认前
不写、单独确认重命名/删除后的完整 SFTP 内容/目录读回。剩余真实几何反例：
末行底为 `557px`、滚动视口底为 `556px`，偏移已达到 `-591px` 最大值。
原因是 GPUI 的内容范围排除了滚动节点自身的 1px 边框；现将焦点边框移到外框，
行可见性仍按内部滚动视口作严格断言，不放宽像素要求。r3/r4 原件保留。

r5 键盘专项实际 `exit 0 / reaped / 12.617s`，`13/0/0`；
文件模块 `files-regression-r1` 实际 `exit 0 / reaped / 159.260s`，`341/0/0`。
app 具体库存 1330 名，13 个新键盘与 31 个选择模型名均存在并实际通过；
模型中 28 项为本批新增，原 3 项不重复算新增。
应用严格 Clippy、正常 arm64 GUI/MCP 构建、暂存包和实际 Mach-O 检查通过。
首次完整门禁 r1 的 Python/fmt/Clippy 和部分 Rust 已执行，但非作者指出列表
FocusHandle 默认为非 Tab stop。该 P2 与指南不一致，父立即停止旧门禁，
实际 `exit -15 / reaped / 106.653s`，不算完整成功。
新增真实 Tab/Shift+Tab 分发红例 `browser-tab-red-r1` 实际
`exit 101 / reaped / 17.470s`，`0/1/0`，两列表均未被 Tab 到达；
随后仅将两列表句柄显式加入 Tab 导航，待新专项、冻结与复核。

修后 r6 实际 `exit 0 / reaped / 16.767s`，14 项键盘全部通过。
新 app 库存 1331 名；新增默认名共 42 项（28 模型、14 GPUI），逐名绑定以最终
完整门禁解析为准。r2 冻结 717 项，集合 SHA-256
`d849a4fc427ef1b2abb114fabb88f5fa6c67a167b1e35d0021e7026e737e7166`。
相对 r1 仅 files.rs 和 browser_keyboard_tests.rs 两项改变，其余 715 项保持。
新非作者 r2 复核关闭 P2-001，开放 P0–P3 均为 0，独立预构建执行键盘 `14/0/0`、
模型 `31/0/0`，库存逐名一致；没有独立重新编译，完整门禁/原生不代签。
源码复核 agent 已实际关闭。修后正常 GUI/MCP 构建与 package-r2 检查已通过。
原报告签署时完整门禁/原生 PENDING 的文字保留，父最终结果另行绑定如下。

旧 package-r1 的原生 Frost 会话独立核对主机 pin、连接真实 localhost SSH，
观察到列表/选区/完整状态文本及实际方向键。自动 connect 流首次在把 Files tab
误认成 CheckBox 时 `exit 1`，失败保持；随后的只读 AX 接续分别保存，不把整体
脚本写成通过。该会话已停止，三个受管进程停止回执为 true，两端口关闭，
私有目录 absent。最终 Tab 修订由下述 package-r2 新原生回合另行观察，
旧截图不代签新源码。

## 最终完整门禁与程序绑定

`full-gate-r2` 实际 `exit 0 / reaped / 未超时 / 724.429846375s`；
`final-gate-summary.json` 逐项核对 717 项完整工程路径及字节，零漂移。
普通 Rust `3210 passed / 0 failed / 48 ignored`，rustdoc `14 passed`；
工程 Python `284 run = 283 passed + 1 skipped`，session helper `25/25`。
fmt、严格 workspace Clippy、直接 registry `x.y` 政策与三组 AI controller 通过。
app 库存 1331 名与门禁实际执行名逐项相等；42 个新增默认名全部通过，
其中 28 个选择模型、14 个 GPUI。它们包含于总数，不另行相加。

`normal-build-r2`、`package-r2` 和 `inspect-package-r2` 均实际成功。
正常 GUI 为 `199,098,288` 字节，SHA-256
`5ae23593849a5027e71f1a9461dd0fe3129d7adc54b1f22b110694d67464ca9b`；
MCP 为 `14,548,464` 字节，SHA-256
`0e5efdcbe2069fb4ea3c809a99419c557d11ba0ba18e7ba8e5fc6ea0e9bde816`。
`normal-artifacts.json` 核对暂存包全部文件与 manifest，门禁后再次确认正常程序与
包内字节相等；包在未提交候选中构建，manifest 的 commit 为 null，最终源码由
717 项冻结与精确 Git blob 另行绑定，不能声称包已经自带源码提交号。
没有安装、签名、公证或发行。

## 实际 macOS 观察与未通过范围

`package-r2 / native-frost-r2` 仅覆盖 macOS arm64、Frost Light、中文、
默认窗口和两个隔离 localhost sshd；主机 pin 单独核对后才连接 SSH/SFTP。
观察与完整失败说明从持久证据中的 `raw/native-notes.md.gz` 读取。

| 范围 | 实际结论 |
| --- | --- |
| 浏览/选区/可见性 | Home、方向、Shift 范围、Space、Escape、32 项超限整次拒绝、Page、End 和末行位于列表内框的 AX 几何已观察。首个 Home 未见变化的失败保留；后续 helper 记录 U+F729 并另轮成功，根因未证明。 |
| 改名 | F2 填名聚焦、名称 Enter 完整审核、重复 Enter 不执行、取消不写；重审后单独确认，旧名 absent，新文件全部 28 字节与种子摘要相同。审核时列表按设计卸载，脚本误要求列表仍存在的失败保留。 |
| 刷新/删除 | 一轮脚本误假定改名后自动重载而失败；后续显式 F5 读到新名，再 Delete 审核，重复 Delete/Enter 不确认，单独确认后目标 absent。指南说明需手动刷新。 |
| 本地 | 实际输入路径、Enter 读取、Home/Enter 打开目录、Backspace 上级、中文文件选择及填写传输路径；种子保持，没有启动传输。 |
| 正向 Tab | 大目录 97 次、单文件子目录 19 次分别到达两列表。 |
| 反向 Tab | 单文件目录回合 180 次没有观察到列表，最终 AX 焦点在命令文本区；另从传输框出发的点击被窗口边界 guard 拒绝，未发点击。原生反向入口保持 HOLD，受控 GPUI 通过不代签。 |
| 其他失败 | 首轮正反向脚本超时而未持久化完整反向逐事件原件；单文件脚本把 `/var` 与服务端 canonical `/private/var` 作精确比较失败。各脚本原退出码保持，不由接续观察回填整体成功。 |

本批 Graphite/Lavender 原生 `NOT_RUN`；Windows/Linux 桌面、IME、VoiceOver、
完整尺寸/焦点/主题矩阵继续开放。删除取消未取得本批原生字节读回；
重命名审核有完整 AX 文本，长路径右侧超出当时正文视口，完整横向滚动未验收。

第二个新非作者限定批准上述原件与陈述，开放 P0/P1/P2/P3 均为 0；
实际独立核对 717 项工程输入、包内五文件、正常程序和原生会话 SHA、
完整 28 字节、失败/接续与清理边界，亲自查看六张 PNG。
执行八次命令批处理、37 个顶层命令组、四个只读子命令及六次图片查看；
没有执行测试、编译或原生重放。12 份分场景回执保持三份 exit 0、九份 exit 1。
报告签署前已核对工程输入，父随后推进源码提交；最终按精确 Git blob 再次绑定，
不依赖口头输入不变说明。原报告及原回执保持；父单列
`final-native-review-binding.json` 澄清确认输入不变的来源为父，而非用户。
该 reviewer 已实际关闭，原生反向 HOLD 与其他未验范围不因零新增问题而关闭。

## 资源收口

`native-frost-r1` 和 `native-frost-r2` 的 `session-final.json` 以及
`native-cleanup-final.json` 核对六个根进程停止、四次端口关闭和两个私有目录
absent；147 条内核 birth 身份当前零活匹配。用户中断后原外层 PTY 句柄已不存在，
不从该事实推断外层退出码，另保留 `interruption-resource-readback-r1.json`。
旧门禁 r1 的唯一空临时根已在进程/lsof 核对后回收，
`interrupted-gate-temp-cleanup.json` 保留实际依据。没有创建容器或用户 chat。
本批作者与两个 reviewer 共三个 agent 已关闭，受管作者 worktree 已回收；
没有活动本批 agent。父归档、Git blob 绑定及重复临时文件回收回执在交付时另列。

## 提交、推送与持久原件

用户恢复推送要求后，父先读取远端：当前功能分支尚不存在，main 的远端独有差分
仅 `docs/PROJECT_STATUS.json`。父没有重置、合并或强推 main；将此前 159 项本地
提交推到当前功能分支，实际成功、reaped，三方读回为 `e709d02`。
随后对本批 717 项工程输入逐项核对 Git index blob 和当前文件，零 mismatch，
提交源码及正式状态文档为 `6d80c259b9a80facbf0f2fdeea986e18978c714f`。
推送实际 `exit 0 / reaped / 未超时 / 2.408s`，本地、tracking 和远端再次一致。
独立源码/功能复核与工程门禁已满足才提交；原生证据报告随后归档为单独文档提交。

[持久证据](2026-11-16-file-browser-keyboard-evidence/README.md)无损封存 491 份原件、
源字节数 `93,201,142`，并保留三个逐字节未修改的便捷截图。
14 份可重建暂存包、helper、预构建测试副本以完整字节数和摘要登记，不重复提交二进制。
归档 verifier 同时核对完整原件/预览、归档 Git index 或精确提交，以及源码
`6d80c25` 的全部 717 个工程 blob；源码先提交不代表当时已包含后续原件。
归档绑定、归档推送与清理回执单列。后续只改文档时不重跑工程输入未变的完整门禁。

设计见 [ADR 0145](../../adr/0145-file-browser-keyboard.md)，使用见
[键盘指南](../../product/FILE_BROWSER_KEYBOARD.md)。
批次工作目录 `work/file-browser-keyboard-20261116` 保存所有实际命令、失败与回执。
本批 registry 依赖未新增；两轮官方最新检查均 45 秒超时，SIGTERM 后实际 reap，
不能签所有依赖均为最新。Rust/Cargo 当前为默认 stable 1.100.0，无 toolchain pin
或 rust-version。三主题继续共用 registry/Palette/token。
完整产品、三平台原生、IME/辅助技术和发行保持开放。
