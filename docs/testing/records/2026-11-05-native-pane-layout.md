# 原生分屏工作区接入记录

日期：2026-11-05，Asia/Shanghai。基线
`77aa69c04ad0dd625a5d827552335878de993ee7`，候选分支
`feature/native-pane-layout-20261105`。

## 范围与状态

本增量接入嵌套左右/上下分屏、逐叶标签和选择、分隔条与键盘比例调整、
保留会话的合并、活动面板聚焦、紧凑窗口回退和显式布局保存。
重连保留面板与稳定标签身份，并继续退休旧运行授权。
重启恢复、多窗口和完整原生矩阵仍未完成。

首包对应的完整门禁与非作者源码再审均已通过所述范围；受控 macOS 原生已完成
下述子集，首包拖动与紧凑窗口未通过。工具区选择与单 move 短拖动已完成修订，
52 项专项、`full-gate-r4` 和 `source-r3` 限定源码复审通过。
修后包先遇到窗口接口失败，之后在独立 run `acceb50f1da74d12bf027efec66166a4`
取得 35 对原始 AX/图像及四通道审计，覆盖工具选择、两轴拖动、三个分隔条键盘、
限定主题/语言、聚焦/合并/关闭及保存。原失败历史保持，紧凑 OS 窗口和完整矩阵
仍为 OPEN；新增 r2 证据已通过新的非作者 **APPROVE_SCOPED_COMBINED**，
开放 P0/P1/P2/P3=0；Hallmark finish 对所检子集为 **ship**。
本记录为提交前验收快照；提交、推送和精确 CI 由该切片 `delivery-readback.json` 另记。
原连接树候选及其冻结包、输入保持独立。
证据目录为 `work/native-pane-layout-20261105/`。

## 已执行

- 官方依赖 freshness 开始于 `2026-11-05T04:39:02Z`，
  完成于 `04:39:19Z`：34 个 registry 包、104 条声明/使用通过；
  进程退出 0并回收。本轮没有新增依赖，仍按 `x.y` 声明。
- `check-r1` 退出 101：三处控件 ID 使用 `(&str, String)`，
  与 GPUI 的转换接口不匹配。改为完整稳定字符串 ID 后，
  `check-r2` 的 `cargo check -p keelshell-app --locked` 退出 0并回收。
  后续源码继续修改；该检查不代表最终候选通过。
- 用户通知电脑解锁后重新查询窗口接口，原生 apps 仍为空且工具返回
  `Mac locked`。这是工具访问失败，不能据此判断用户设备的实际锁定状态。
  该次检查尚未启动应用或 Podman，不将空审计计为零输入或原生验收通过；
  后续实际运行分别记录于下文。
- 基线文档提交的精确 Quality `41967541069 / attempt 1` 在
  `2026-11-05T04:53:20Z` 仍为 `in_progress`。原始 GET 与回执保存在
  `baseline-ci/`；不沿用前一功能提交的成功结果。

- 后续精确终态为 **failure**：macOS/Windows job success，Linux Rust gate
  在 `local_agent_directory_process.rs:287` 报
  `owned request ended before readiness: Err(ScratchFailed)`，后续 OpenSSH 步骤
  skipped。失败发生在未含本候选的基线文档提交；原因尚未确认，不能称作
  已诊断的偶发故障。`baseline-ci/final-readback.json`、原始 run/jobs 和 Linux
  完整日志保留。首次日志下载因 gh 拒绝转义序列失败，第二次仅向文件输出成功。
- 首轮生产 Clippy 拒绝 `add_remote_tab` 的 8 个参数；已将会话创建参数整理为
  `NewRemoteTab`。该失败保留，后续门禁结果按轮次记录于下文。

- `pane-tests-r1`：42 passed / 1 failed，失败为测试查找不存在的弹窗控件 ID；
  修改为实际连接管理器 ID。`pane-tests-r2` 为父新增键盘测试缺少类型 import 的
  编译失败。`pane-tests-r3`：45 passed，包含跨窗口拖动与 Enter 键盘激活。
- 父任务发现关闭非活动会话会切走活动面板。`inactive-close-before` 用实际
  关闭按钮复现失败；按原 EntityId 直接关闭后，`pane-tests-r4` **46 passed**。
  修前/修后测试文件 SHA-256 完全相同，仅两个生产输入变化，见
  `inactive-close-regression-binding.json`。所有进程均退出并回收。
- 模型新增 34 项、工作区新增 11 项 GPUI 场景；专项过滤还包含一项迁移后的
  归属回归。真实 SSH 重连测试也加入了原面板/稳定标签 ID 保持的断言，等待
  全仓检查读回。GPUI 场景使用生产渲染和事件，终端由隔离通道提供；不属于
  OS 原生输入或真实 SSH 联调结论。
- `full-gate-r1` 退出 1并回收：格式和严格 Clippy 通过，app 为
  **912 passed / 1 failed / 2 ignored**；后续 core/session/rustdoc 等未完成。
  唯一失败是旧会话栏测试反复挂载 pair 后手工拼接 tabs，却保留了最后 pair 的
  局部顺序。只在最终组装 fixture 时创建一致的初始面板，不改生产合同或原断言；
  `session-tabs-r2` 的四项测试全部通过。`full-gate-r2` 已退出 0并回收：
  **2,152 ordinary passed / 23 ignored**（app **913 / 2 ignored**）、11 rustdoc、
  Python 128 run / 1 skipped，附加进程控制器、格式、严格 Clippy 和依赖规则通过。
  503 个输入在结束后完全相同，见 `full-gate-r2-summary.json`。这是下面 P2
  修复前的候选，不能直接当作键盘修订后的完整门禁。
  门禁输入共 503 个；本轮 app 测试链接器提示 `__eh_frame` 超过 compact unwind
  表限制，该警告保留于日志，不将本地测试构建称作无警告发行构建。

## 独立审查与键盘修订

新非作者 Herschel 的 `source-r1` 为 **HOLD / P2 NP-SR-001**，其它 P0/P1/P3 为 0。
平衡四叶布局中，各叶按钮只能调整直接父 split，根左右 split 缺键盘入口。
原报告和已读输入摘要保存在 `independent-review/`，不改写原 HOLD。

新增真实 Tab/方向键回归在修前失败：三个分隔条均无法通过 Tab 到达，
`balanced-keyboard-before` 退出 101并回收。当前修订给每个 Kit 分隔条增加
稳定焦点、Tab stop、方向键、Splitter AX 语义、比例值和双语提示；拖动与
键盘/AX 共用结构和弹窗代次检查。鼠标拖动保留原 SSH 键盘焦点。
首个修后检查因 AX callback 的值参数与 `cx.listener` 引用签名不兼容而编译失败，
已改为弱实体回调。`pane-tests-r5` 退出 101并回收：46 passed / 1 failed，
唯一失败为 Kit 内部 Observed 在 caller 绑定焦点之后才建立，未记录该焦点，
`ElementSnapshot::focused()` 因缺少观测绑定拒绝断言。没有修改 Kit 私有实现；
测试改为读取生产保存的真实 FocusHandle，仍用真实 Tab 和方向键输入。

`balanced-keyboard-r6` 随后实际失败，0/3 分隔条可达。明确显式焦点句柄不会
继承 element 的 Tab 属性后，在句柄创建处启用 `tab_stop(true).tab_index(0)`；
`pane-tests-r7` 退出 0并回收，**47 passed**。两次 503 输入仅 `pane_view.rs`
不同，测试文件完全一致，见 `balanced-keyboard-regression-binding.json`。
测试确认错误轴按键不改变布局、正确轴只调整当前分隔条、其它完整归属与
选择保持、无终端 Write。最初 `balanced-keyboard-before` 的测试观测版本不同，
不能与 r7 声称同文件负控；r6/r7 才是这一修订的同测试对照。

父任务继续发现新可聚焦分隔条缩窗隐藏后未回到惰性工作区。
`compact-divider-before` 失败；加入弹窗场景后的 `before-r2` 中，直接缩窗仍失败，
弹窗场景因未等待异步 action 分发而停在控件定位，不能称其生产问题已复现。
补上有界任务队列推进后，`compact-divider-before-r3` 的两个场景均在
“隐藏分隔条必须回到惰性工作区”断言失败。当前修订在紧凑/聚焦隐藏时迁移
焦点，并记录弹窗返回的分隔条身份，恢复前重新检查其可见性；
`pane-tests-r8` 已退出 0并回收，**49 passed**。r3/r8 使用同一最终测试文件，
503 输入仅 `pane_view.rs`、`view.rs`、`modal_scope.rs` 三个生产文件不同；
测试后全部输入保持，见 `compact-divider-regression-binding.json`。
修后覆盖直接缩窗、真实连接管理器弹窗期间缩窗再返回、惰性工作区接收后续
按键、Tab 可继续移动以及零终端 Write。两项测试只证明所测焦点路径，
不外推所有弹窗、运行授权或原生 OS 输入。以上失败及原 HOLD 均保留。

Herschel 的 `source-r2` 为 **APPROVE_SCOPED_SOURCE**，开放 P0/P1/P2/P3=0，
NP-SR-001 在冻结源码与受控回归范围内关闭。两组回归、49 项专项和 503 输入
绑定均经只读核对，reviewer 没有重跑 Cargo 或启动 GUI。七个生产文件和最终测试
在 `2026-11-05T06:02:22Z` 再次核对无漂移，详见 `independent-review/source-r2.md`
和 `input-hashes-r2.json`；这不包含最终包、最终文档或原生检查。

修后 **`full-gate-r3` 已退出 0并回收**：**2,155 ordinary passed / 23 ignored**
（app **916 / 2 ignored**）、11 rustdoc、Python 128 run / 1 skipped；
格式、严格 Clippy、依赖规则及附加进程控制器均通过。503 输入结束后保持相同，
详见 `full-gate-r3-summary.json`。前轮失败与 linker 的 compact unwind 警告保留。

## 原生准备

本轮后续重置 CUA 会话后，应用清单读取成功且 KeelShell 显示未运行，
见 `native-interface-recheck-r3.json`；该结果只证明清单入口可用，不证明窗口操作。
已将既有有界 NET supervisor 和只计数量的 OpenSSH 夹具复制至本轮
`preparation/`，保留来源摘要并使用独立 owner label，旧导航副本未修改。
种子仅含两条真实 loopback 测试连接，AI/更新后台请求关闭、没有预信任主机密钥；
两账户的命令由 ForceCommand 拒绝，PTY 输入只记录字节数，不存正文。
独立镜像和 macOS 开发包构建均退出 0并回收，随后完成下述实际运行。

## 首包 macOS 原生观察

程序 SHA-256 为
`0855f7b47aa74425396481c7afe0d6ee6b645c5d462e7e2f0cfc1f97a063b165`。
503 个门禁、构建和捕获后输入相同，另有 41 个 assets 输入单独绑定；
503 不包含这组全部 assets。包构建于未提交源码，manifest 没有 commit/workflow
身份。`native/package-binding-r1.json` 是构建时回执，其 `not run` 是当时状态；
后续运行、审计和清理由 `native/verification-r1.json` 记录。

独立 run `6fb7579cf0f941e299878eafb1f188da` 在 `06:06:27Z` 启动。
经原生私钥认证、核对夹具主机指纹并确认信任后连接 A 账户，在同一认证 SSH
连接上打开四个独立 Shell 通道；B 账户未连接。ForceCommand 不执行命令，
SFTP 拒绝是该夹具的预期边界，不能算作产品 SFTP 验收。

34 对原始 AX/图像保存在 `native/captures-r1/`。32 张为 2880×1866，
27/28 为系统缩放后的 3840×2098；清单与摘要见 `capture-manifest-r1.json`。
不能用捕获文件名代替实际内容：

- 01 是私钥认证，02 才是主机密钥确认；03 是信任后再次认证，04 才已连接。
- 05–08 逐步形成四叶，09 是叶内按钮使右上下目标 50%→45%，其首个 Tab
  尚未到分隔条。10 再用 Tab 到右上下分隔条，Down 使 45%→50%；
  11 根左右分隔条 Right 使 50%→55%；12 左上下分隔条 Up 使 50%→45%。
  截图有对应焦点线，AX 有比例与双语说明；不把 AX 报窗口焦点当作读屏验收。
- 18 曜石深色、19 雾紫浅色、20 雾紫浅色英文保留四叶和未执行命令草稿；
  初始霜白浅色中文另见 08–13。21 从草稿尝试 Shift-Tab 后未见根比例变化，
  不能按文件名记作新的英文键盘通过。22/23 聚焦活动叶后恢复原四叶和比例。
- 24 显式保存四叶，25 合并左侧两个叶，26 切换合并叶内标签。31 关闭第四个
  会话后收拢空叶；32 合为单叶并保留其余三个会话，33 再保存。
  两份保存读回确认 5500/4500/5000 比例、连接引用和存活标签 UUID 保持；
  移除的只有显式关闭的第四个标签。不含重启恢复。
- 14–16 的左右拖动及 28 的上下拖动均未观察到比例变化；30 拖拽窗口角未
  缩小窗口。27/29 的系统缩放确实放大/恢复窗口，但没有进入紧凑回退。
  接收事件字段未知，不能直接将这些结果归因于生产代码或工具。
- 29/30 在还原窗口宽度后，右侧长提示只显示末尾 `ute.`；这是终端显示观察，
  尚未确认 resize/reflow 原因，不以它否定或证明布局归属。

六份审计由 `native/verify-r1.py` 核对 owner、容器、应用身份、完整事件前缀和
Shell 的 PID/kernel birth。初始、三个分隔条键盘后、主题/聚焦/合并后均是
相同四个 live shell；显式关闭后只剩原前三个。关闭事件计数仍是 0，
“已关闭第四个”依据 live identity 缺席与界面结果，不虚构 `shell_closed` 事件。
所有这些操作至正向对照前 raw PTY 为 **0 字节**。34 显式向首终端输入 `x`
后，只有该通道增加 **1 字节**，没有按 Enter。后台监控另有 exec 探测并被拒绝：
正控前 1,280 次/537,600 字节，正控后 1,318 次/553,560 字节；
不能写成全部请求为零。

应用通过窗口关闭按钮退出 0并回收；父执行会话 92576 也退出 0并回收。
`06:23:36Z` 回执确认自有容器、网络、私有目录消失且端口拒绝连接；
`06:24:15Z` 移除本轮自有镜像，未删除共享 base。没有安装、发行标签、
真实业务服务器或模型发送。首包原生证据只绑定上述输入，不覆盖后续修订。

## 原生观察后的修订

分屏会重新展开已经收起的 Files，并覆盖 Commands/Tunnels 选择。
独立只读评估确认基线已有该行为，属于可恢复的 P3 体验问题；
本轮决定一起修订，使分屏保持原工具区选择和实体，新连接仍默认展开 Files。
新增真实 loopback SSH 和生产按钮回归；首次 `tool-choice-before` 为缺少
`ToolPanel` import 的编译失败，不能视为行为反例，修正后继续验证。

`tool-choice-before-r2` 实际复现收起状态被分屏改为 Files。短拖动的
`short-drag-before` 也实际复现：两轴在一次目标 move 后仍为 5000，预期 6500；
两次目标 move 和原多次移动测试通过，共 2 passed / 1 failed。

首个修复尝试在 canvas 添加 `on_drop`，`native-observations-after` 仍
16 passed / 1 failed，短拖动未修好；Files 选择回归已通过。
改为在 divider `on_drag` 创建时应用首次鼠标位置，沿用 `resize_pane` 的
身份/代次守卫，不增加全局输入状态。`after-r2` 的短拖动、双 move 和 Files
均通过，但原两个守卫测试仍用首次合法移动前的比例作基线，故 15 passed /
2 failed。测试调整为在首次移动后、弹窗/关闭之前截取基线，仍断言之后的旧
payload 不改变完整归属/比例，并保持新拖动与零终端输入的正负对照。
最终测试已在禁用首次位置应用时重跑：`short-drag-before-final-tests`
**51 passed / 1 failed**，只失败于单 move 两轴比例。恢复后
`native-observations-after-r3` **52 passed**，均退出并回收；
503 个输入只有生产 `pane_view.rs` 不同，完整最终测试相同，见
`short-drag-regression-binding.json`。
Files 的整个测试文件与行为失败轮相同，修后覆盖两轴、四种工具选择、
原面板实体保持和真实 SSH 无输入；但该两轮的其它 pane 测试/生产文件也变化，
不称作全输入单文件对照，见 `tool-choice-regression-binding.json`。
修后 **`full-gate-r4` 已退出 0并回收**：**2,158 ordinary passed / 23 ignored**
（app **919 / 2 ignored**）、11 rustdoc、Python 128 run / 1 skipped；
格式、严格 Clippy、依赖策略与附加进程控制器通过，503 输入与门禁前相同。
终态和逐项计数见 `full-gate-r4-result.json`、`full-gate-r4-summary.json`。
首包拖动保留 OPEN，受控复现不证明 CUA 旧失败的唯一原因。

Herschel 的 `source-r3` 为 **APPROVE_SCOPED_SOURCE**，开放 P0/P1/P2/P3=0。
既有工具选择 P3 与受控确证的单 move P2 在所审源码/回归范围关闭。
reviewer 完整阅读四个变化文件的 delta，核对 503 项工程输入与 40 项捕获输入，
并确认五份历史审查材料未变；hash 核对不表示阅读了全部 503 个文件。
reviewer 没有重跑 Cargo、Podman 或 GUI；不构成原生或最终组合批准。
详见 `independent-review/source-r3.md`、`input-hashes-r3.json`。

## 修后包与窗口接口失败

`native-build-r2` 退出 0并回收；新包程序 SHA-256 为
`5ce34862d2c8dffc40828291d4e6f05039888fcd4f3d672ca643b6a0e17039b8`。
503 个门禁/构建/启动尝试后输入一致，另有 41 个 assets 单独核对；
manifest 的 commit/workflow 仍为空，属于未提交源码的开发包。
所有 manifest 文件摘要一致，见 `native/package-binding-r2.json`。

run `904d404d221246f49f4b67d3702dfde7` 于 `06:54:01Z` 受监督启动该包。
使用完整包路径读取窗口，以及重置接口后读取清单，均返回 `Mac locked`；
没有 AX、截图、交互或显式 SSH 连接。不能据工具错误推断设备实际锁定，
也不能把健康的空审计当作零输入或原生验收通过。
`native/r2-interface-unavailable.json` 与 `r2-launch-blocked-readback.json`
保留这次尝试。

应用按 owner 结束，退出 -15并回收；父执行会话 18611 退出 0并回收。
`06:55:24Z` 读回容器、网络、私有目录均已消失，两类端口拒绝连接；
`06:55:56Z` 移除本轮自有镜像，未删除共享 base。
新包与冻结输入保留，以便窗口接口可用时重新建立独立验收 run。
这次失败尝试没有取得修后原生验收，后续独立运行见下一节；本候选尚未提交/合入/推送。

限定交接复核 `independent-review/handoff-r4.md` 为
**APPROVE_SCOPED_HANDOFF**，所核对快照没有开放 P0/P1/P2/P3。
reviewer 重算门禁日志计数、503 工程输入、41 assets 及五个包内文件摘要，
交叉核对失败尝试和清理回执；没有重新执行测试、GUI 或 Podman。
其签署范围为记录中的文档/证据快照，不包含本段结果回写，也不签署修后
原生、最终 combined 或 Hallmark finish。

## 修后包独立 macOS 原生运行（r2）

本节新增结果只绑定程序 SHA-256
`5ce34862d2c8dffc40828291d4e6f05039888fcd4f3d672ca643b6a0e17039b8`
及独立 run **`acceb50f1da74d12bf027efec66166a4`**。
`preparation/runs/<run>/app-process.json` 记录应用于
**2026-11-05T09:12:02Z** 启动，PID 73980，并绑定 kernel birth 和包摘要；
监督器 PID 73870。本节时间均为 UTC，上海时间加 8 小时。
原 `904d404d…` 窗口失败及首包 `0855f7b4…` 的 34 对捕获均保留，不回填为通过。

父任务运行 `native/verify-r2.py` 退出 0，原回执
`native/verification-r2.json` 的核验时间为 **2026-11-05T09:35:23.521207Z**。
其 503 项 `full-gate-r4` / `native-build-r2` / 捕获后当前输入相同，
35 对 AX/JPEG 的摘要和原始尺寸均已核对；每张原图都是 **2880×1866**。
另 41 项 assets 与构建清单分别绑定，不并入 503。包内五个文件保持 manifest
摘要；commit/workflow 仍为空。构建时 `package-binding-r2.json` 的 `not run`
及启动时准入材料的 `NOT_RUN` 是当时快照，后续运行以本节和上述回执为准。
文档作者读取原始 AX、选定图像、保存 JSON、审计事件和退出/清理材料并核对摘要，
未重跑 verifier、Cargo、GUI 或容器；这不是新的独立审查结论。

本次仍使用自有隔离 OpenSSH 夹具。01 为私钥认证，02 核对夹具主机密钥，
03 信任后再认证，04 已连接；A 账户 `fixture` 在同一认证连接中打开四个
独立 Shell 通道，B 账户未连接。ForceCommand 拒绝 SFTP/exec，不执行命令；
工具区保留的观察不代表 SFTP、命令执行或隧道业务通过。

### 捕获顺序与实际结果

原件位于 `native/captures-r2/`，文件名与时间见该目录的 `manifest.json`。
下表比例均为 AX 中的目标比例，不是从显示缩略图估算的像素比例。

| 捕获 | 实际观察及限制 |
| --- | --- |
| 05 / 06 / 07 | 分屏分别保持已收起的 Files、选中的 Commands、选中的 Tunnels；形成两叶、三叶、四叶。08 为收起工具区后的四叶。 |
| 09 | 使用半尺寸坐标的拖动选中了终端文字，三个比例仍为 50%；保留为失败尝试。 |
| 10 / 11 | 按原图像素坐标拖动：根左右 50%→58.47%；左上下 50%→62.18%。修后包两轴拖动有实际结果，但不反推 r1 失败的唯一原因。 |
| 12 / 13 | 12 从多行草稿尝试 Shift-Tab，仍在草稿中；13 是叶内按钮先把右上下 50%→45%，随后 Tab 在草稿中添加两个尾空格。不能按文件名将两张记成分隔条键盘成功。 |
| 14–20 | 从导航搜索框顺序 Tab 到分隔条：15 左上下焦点，16 Up 使 62.18%→57.18%，17 错误轴 Right 不变；18 右上下 Down 使 45%→50%；19 根左右 Right 使 58.47%→63.47%，20 错误轴 Up 不变。截图焦点线和 AX 比例相互对应。 |
| 21 / 22 / 23 | 曜石深色中文、雾紫浅色中文、雾紫浅色英文；初始霜白浅色中文见 05–20。只覆盖这些组合，不是三主题×明暗×语言的完整矩阵。 |
| 24 / 25 / 26 | 聚焦活动叶、还原四叶和原比例，再显式保存四叶布局。 |
| 27 / 28 | 两次缩窗尝试未生效，原图仍为 2880×1866，AX 仍有三个分隔条；紧凑 OS 几何与回退保持 OPEN。 |
| 29 / 30 / 31 | 合并左侧两叶后四个 shell 仍 live；切换合并叶内标签；仅显式关闭第四个会话，空叶收拢。 |
| 32 / 33 / 34 / 35 | 合为单叶并保留其余三个会话，再保存；34 用 Previous SSH session 选首会话，35 显式向首终端输入 `x`，没有按 Enter。此后没有再次保存布局。 |

后期草稿原文为 `printf 'pane-review'\n# 草稿保留，不执行  `，末尾两个空格来自
13 的人工 Tab。20–25 与 33 的 AX 提取值完全相同；不能据此称草稿全程与初始
字节相同。AX 的 focused element 仍报告窗口，分隔条键盘结论来自实际比例变化
与截图焦点线；不把这些材料写成 VoiceOver 验收。

10/11 的右侧终端只显示夹具长提示末尾 `execute.`，后续调整时又出现换行正文。
这是保留的终端 resize/reflow 显示观察，原因未确认，不据此宣称完整终端重排
或文本显示验收通过，也不以显示变化推断 shell 身份改变。

### 六份审计、保存与退出

`preparation/runs/<run>/audits/` 的六份运行中审计核对了相同 owner、容器、
应用 PID/kernel birth、准备输入摘要、递增捕获时间及完整事件前缀。
初始四个 shell 的 PID 为 17/39/93/139，kernel birth 分别为
9559209/9561476/9564562/9566322；每次均按 PID 和 kernel birth 共同匹配。
拖动、键盘、主题、聚焦及合并后仍为这四个身份，显式关闭后只剩原前三个。
`shell_closed_count` 始终为 **0**；关闭判定依据 live identity 中第四个缺席及
原生界面结果，不虚构 `shell_closed` 事件。

| 审计时点（UTC） | live shell 数 | raw PTY 累计字节 | 监控 exec 拒绝次数 / 命令字节 |
| --- | --- | --- | --- |
| 09:15:23，四叶初始 | 4 | 0 | 110 / 46,200 |
| 09:20:19，分隔条后 | 4 | 0 | 582 / 244,440 |
| 09:24:16，主题/聚焦/合并后 | 4 | 0 | 962 / 404,040 |
| 09:24:52，关闭第四个后 | 3 | 0 | 1,012 / 425,040 |
| 09:25:55，正控前 | 3 | 0 | 1,088 / 456,960 |
| 09:26:33，正控后 | 3 | 1 | 1,132 / 475,440 |

正控新增唯一一条 `shell_input`，1 字节，目标为首 shell PID 17 /
kernel birth 9559209。它与 35 的显式 `x` 操作相符；审计只记录数量，不记录
输入正文。零 raw PTY 不代表全部 SSH 请求为零，监控 exec 探测已单独列出。
退出后的 `final-b8abccb7.json` 保留 raw PTY **1**、live shell **0**，
监控累计 1,146 次/481,320 字节；仍没有 `shell_closed` 事件。

`native/saved-four-layout-r2.json` 与 `saved-single-layout-r2.json` 的实际顶层键
为 **`layout`**。四叶根左右/左上下/右上下依次为 **6347/5718/5000 bps**；
单叶保存保留同一窗口 UUID、三个存活标签 UUID 和同一连接引用，只有显式关闭
的第四个标签被移除。标签字段仅 `id` 与 `connection_id`，没有运行授权或凭据。
单叶保存时 `active_tab` 为原四叶遍历的 **`original_tabs[1]`**
（`859429e3-b73c-4c50-9baf-1bbe25c07c9b`）；34 才切到首通道做正控，未再次保存。
保存读回不包含重启恢复。

应用正常退出 **0/reaped**；监督器退出 **0/reaped**，父执行会话
**35928** 已回收。`09:26:46Z` 的 `cleanup.json` / `final.json` 记录自有容器、
网络、私有目录消失，两类端口拒绝，app clean、payload threads closed。
自有镜像在 **09:28:09Z** 清理，共享 base 保留。清理依据本 run 的冻结回执，
文档作者没有重新执行容器、端口或进程探测。没有安装、发布或生产环境验收。

**最终 combined：APPROVE_SCOPED_COMBINED；Hallmark finish：ship for inspected subset。**
新的非作者 `independent-review/combined-r5.md` 在 **10:04:21.833993Z** 完成无漂移核对，
开放 P0/P1/P2/P3=0。reviewer 逐张检查35张原图及对应AX，重算七份原始审计，
核对布局保存、进程/资源清理、503工程输入、41assets及11份冻结文档。
其亲自执行的315项只读证据断言与10项冻结核对，不是新产品测试；工程测试、
构建及原生操作均只复核既有记录。精确阅读范围和文件摘要见
`independent-review/input-hashes-r5.json`。

`source-r3` 的
APPROVE_SCOPED_SOURCE 只覆盖其源码/受控回归；`handoff-r4` 的
APPROVE_SCOPED_HANDOFF 只覆盖 07:06:19Z 前核对的旧交接快照，不能代签本次
09:12:02Z 之后的原生材料或本轮文档；新增材料的批准来自上述新复核。
本次状态回写不倒签原冻结字节；后续提交绑定、精确 CI、发布和安装仍独立验收。

## 基线 CI 诊断边界

基线 Linux `ScratchFailed` 的独立只读阶段诊断保存在
`baseline-ci/diagnosis/analysis.md` 与 `input-hashes.json`。
日志只能将失败等待点限定到 Codex held-cwd 或 wait 两例，未保留具体操作、
errno 或关闭前结果；源码还允许 scratch 关闭失败覆盖此前结果。
历史根因仍 UNKNOWN。补案例/请求阶段和有界错误来源观测属于后续建议，
本轮没有实施或重跑 Linux；不以本机 r4 通过改写原 run 的失败。

## 资源

复用本聊天受保护的主工作副本；先前已合入的布局模型本地分支已删除。
保留工作树，未绕过管理器的主工作副本保护。并行作者只修改约定文件，
没有创建额外 worktree、容器、分支或应用进程。检查统一使用共享 target。

## 后续验收

修后专项、完整门禁与非作者源码复核已通过；本节 r2 的限定原生材料已读回，
最终 combined 与所检子集的 Hallmark finish 已通过新的非作者复核。
完整主题×明暗×语言×尺寸、紧凑 OS 几何、IME、VoiceOver、Windows/Linux 桌面、
重启恢复、多窗口以及重连/审核目标的完整原生矩阵仍为 OPEN。
原连接树候选的 499 输入及 r2a 包保持独立，不使用其材料补记本候选；
也不把本轮 503 输入的包记为导航候选原生通过。受控 GPUI 测试不关闭 OS 缺口。
