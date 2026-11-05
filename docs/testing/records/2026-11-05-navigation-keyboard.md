# 连接树键盘与折叠验证 — 2026-11-05

## 身份和范围

基线 `967c588803df493d06d42cf96221bd691aba46b9`，当前工作分支
`feature/navigation-keyboard-20261105`。本轮补连接树的折叠、键盘浏览、搜索恢复、
UUID 选择与激活失效；不改变连接认证、主机密钥、命令审核或 AI 发送规则。
设计见 [ADR 0099](../../adr/0099-keyboard-connection-tree.md)。

本地原始材料位于 `work/navigation-keyboard-20261105/`，包含各次命令、输入摘要、
日志、退出回收、原生夹具准备和后续独立审查。源码、受控 GPUI、真实原生和外部 CI
分别记账。修订候选的新 run 已取得限定原生观察，当前为 **PARTIAL_OBSERVED**；
输入通道有效性及完整键盘隔离仍 OPEN；新的非作者已完成原件和冻结文档复核，
只签 **PARTIAL_OBSERVED**，未授予完整native或Hallmark finish。
第一候选、窗口访问受阻的 run 和新 run 分别保留，不能混用包或运行的结果。

## 开发检查

- 2026-11-05T01:44:28Z 开始的官方依赖 freshness 检查通过；进程退出 0并回收，
  本轮无依赖变更。报告 `dependency-freshness.json`。
- `candidate-1` 误用新的本地 target，编译阶段主动停止以复用既有构建目录。
  子进程实际退出 -15、执行会话回收；不是测试失败或测试通过。
- `candidate-2`、`candidate-3` 为编译不通过的开发候选，分别保留 GPUI API 用法及
  测试按键参数类型修订之前的错误。没有把未执行的行为测试计为通过。
- `candidate-4 → candidate-5` 的键盘测试文件相同，12 passed / 2 failed 变为
  14 passed / 0 failed；仅 `tree.rs` 的 FocusHandle 加入 Tab 停靠配置。
  这是开发候选内的修订，不是原始 main 的反例。后续拆分键盘模块并在方向移动时
  清除 armed，不能把 candidate-5 的执行自动延伸为最后源码的执行。
- 第一候选 `source-clippy` 与 `native-build-r1` 退出 0并回收。
  `full-gate-r1` 同样退出 0并回收：2,068 ordinary Rust / 23 ignored，
  含 app 875 / 2 ignored；10 rustdoc（一项仅编译），Python 128 run / 1 skipped，
  格式、严格 Clippy、依赖策略及附加进程控制器通过。497 个构建/门禁输入一致。

## 独立审查与修订

第一轮非作者结论为 **HOLD_SOURCE，P1=1、P2=2**：

| 发现 | 修前证据与修订 |
| --- | --- |
| NAV-SR-001 / P1：树中长按键在鼠标切换后可能进入另一终端 | 生产 GPUI 事件的三个专项在隔离 transport 写入或提前切换断言失败。修订将按键归属与激活候选分开，在相同窗口的绑定动作之前拦截，再由工作区/模态捕获重复和松键；双激活键取消该次激活。 |
| NAV-SR-002 / P2：删除/过滤后丢失旧父链 | 两项回归分别观察到错误首项选择。修订保留旧可见父链，选择最近的可见祖先，全部消失才回首项。 |
| NAV-SR-003 / P2：主题测试未实际安装家族 | 新实际 ID 断言得到 frost 与 graphite 不相等。测试设置改用 `apply_settings`，检查三家族 × 浅深 × 中英文的实际 ID、Palette、选择、焦点、折叠和草稿。此为测试覆盖修订，不是生产主题失效。 |

`review-regressions-before` 为 13 passed / 6 failed；
`after-1` 因嵌套模块可见性编译失败，未执行行为检查；
`after-2` 为 19 passed / 0 failed，以上进程均已退出并回收。
P1 与父链的两个新增测试文件在修前/修后字节相同，
详见 `review-regression-binding.json`。主题测试的设置确实变化，不声称这一项同测试不变。
`after-3` 再补搜索 Down 的持有键归属、跨窗口隔离和漏松键恢复，
**21 passed / 0 failed**，退出 0并回收。
目录按钮另按实际原图纠正为靠左排列，继续使用现有主题 token。

修订候选的源码冻结清单为 `candidate-review-r2-inputs.json`。
`native-build-r2` 与 **`full-gate-r2` 均已退出 0并实际回收**。
修后门禁为 **2,075 ordinary Rust / 23 ignored**，其中 app **882 / 2 ignored**，
10 rustdoc（一项仅编译），Python 128 run / 1 skipped；
格式、workspace/all-targets 严格 Clippy、依赖策略及附加进程控制器全部通过。
499 个构建/门禁/检查后输入相同，原日志摘要与逐项清单见
`full-gate-r2-summary.json`、`post-gate-r2-inputs.json`。
修订程序 SHA-256 为
`1f180f138479fb9143ad764700fff53b80262e8fafdc46fac4f64059030c28b6`；
非作者源码再审 **PASS_LIMITED_SOURCE，开放 P0/P1/P2/P3=0**，
NAV-SR-001/002/003 在源码与已有回归证据范围关闭。原始 HOLD 保留；
reviewer 读取执行日志与源码，没有重跑 Cargo 或原生。
非作者终态复核 **APPROVE_SCOPED_SOURCE_AND_EVIDENCE_ACCURACY**，
新增 P0/P1/P2/P3=0，已核 499 个当前输入、五个包成员、12 份文档、
四对有效旧包原图/AX、失败审计及清理回执。报告为
`independent-review/final-r2.md/json`；该结论只认可所列源码与证据表述，
**当时 native 为 OPEN / r2 INTERACTION_NOT_RUN**，Hallmark 未作 native ship 结论。
该次独立终态复核时修后原生尚未启动；后续窗口访问受阻和新 run 的限定观察另记，
不能把旧审批扩展为后续原生审批。499 个冻结输入复查相同，
见 `native/frozen-after-core-start.json`。另一个独立工作副本的纯 core
布局模型与持久化提交 `3e2f41f9…` 已合入并推送 main，本机完整门禁及提交绑定
独立审查通过，精确 Quality `41963286647 / attempt 1` 已读回三平台 success，
46 项新增布局测试在每个平台通过。该提交没有导入本导航候选，也没有改变
本候选源码、包或测试结论；上述 CI 不验收导航候选。
当前未提交、未推送，无本轮精确远端 CI。

## 修订候选原生限定观察 — run 5c76b54d

新 run **`5c76b54db2de4dd89cff9793187208c6`** 于
**2026-11-05T09:43:13Z** 启动同一 `native/package-r2a/KeelShell.app`，
程序 SHA-256 为 `1f180f138479fb9143ad764700fff53b80262e8fafdc46fac4f64059030c28b6`。
`app-process.json`、`package-binding.json` 和运行中的审计绑定该应用身份；
499 个工程输入及五个包成员保持冻结。本节时间均为 UTC，09:49:24Z–10:02:47Z
对应上海 17:49:24–18:02:47。此前 03:49Z 的窗口受阻 run 另列于下节。

原件为 `native/captures-r2/` 下 **54 对 AX/JPEG** 及 `manifest.json`。
全部原图像素为 **2880×1866**；清单的时间和 run ID 是捕获归属记录，
文件名不是操作成功证明。文档回读对全部 AX、图像尺寸、原始审计事件和包摘要
进行核对，抽查原图的选择、焦点边框、外观及草稿；逐文件摘要和实际抽查范围见
`native/documentation-readback-r2.json`，这些摘要是本次回读计算，并非捕获时签名。

### 原始捕获能够支持的范围

| 捕获编号 | 原件观察与边界 |
| --- | --- |
| 01–08 | 经原生认证/主机密钥流程连接同一受控 OpenSSH 的 `fixture` / `fixture_b` 两账号。05、08 的监控入口均显示“继续”，与分别暂停后的状态一致；两账号不是两台服务器。 |
| 09–10 | 命令草稿为 `NAV-COMMAND-DRAFT-KEEP\n# 未执行`，Ask 草稿为 `NAV-ASK-DRAFT-KEEP\n不发送`，其中 `\n` 表示原文换行。仅填写，未运行或发送。 |
| 11–19 | 霜白浅色中文下，搜索框 Tab 进入树、Home 选未归档、Left 折叠、Right 展开及进入首子项、Left 返回父级、End 选择并显露末尾空根、Up 选末条连接、Tab 离开至管理按钮均有选择/像素变化。AX 的 focused element 仍为窗口，不能据此证明 GPUI 内部焦点或 VoiceOver 完整正确。 |
| 20–25 | 根目录折叠后，`needle` 查询返回多项并展开匹配路径；23 的精确 `nav-test-01.invalid` 查询显露深层连接。24 仅选择离线测试项，没有激活；25 清查询恢复折叠，并回退到可见测试根。 |
| 26–32 | 无匹配查询及 Down 后保持空结果。`LOCAL SSH A` 查询实际同时匹配 A/B；29、30 文件名误写 A，原图/AX 仍为 B。31 明确选择 A，32 的 Return 才切到已有 A 会话；B 仍连接。旧 B 命令目标保留但失效，执行按钮禁用，没有自动重绑定到 A。 |
| 33 | 隐藏导航后未见 `q` 改动可见草稿；单张图不能证明没有远端写入，仍受下面的审计及正控边界限制。 |
| 34–36、41 | 34 为曜石深色中文，35 为雾紫浅色中文，36 为雾紫浅色英文；此前为霜白浅色中文。41 再显示 Ask 时两份完整草稿仍在、命令执行与请求预览禁用；没有配置模型服务、没有发送。此为外观与内容保持的有限组合，不是三主题浅深×双语完整矩阵。 |
| 37、39、40 | 文件名声称空目录或 End/Right，但实际选择仍在本地 SSH 根；未确认相应键盘导航变化。 |
| 38 | 缩窗尝试未改变所捕获窗口，原图仍 2880×1866；不能声明紧凑 OS 窗口、最小客户区或响应式矩阵通过。 |
| 42–46、48–49 | 点击可改变选择，随后 Home/Up/Tab/End 等未产生预期导航变化；切回中文也未恢复。49 的 Down duration=100 同样没有导航变化。 |
| 47 | 点击搜索框并 `typeText` 确实写入 `nav-test-01.invalid`，结果随之变化；文本输入的这一观察不证明方向键或终端输入通道有效。 |
| 50–51 | 点击终端后分别 `pressKey x`、`typeText x`，均未 Enter；原图中有可见字符，但两个后续服务端审计仍记录 PTY 输入 0。端到端正控未达到预期，不能写成“终端完全无字符”或“完整输入隔离通过”。 |
| 52–54 | Raise 后点击测试根可使其折叠，但 End/Right 没有确认导航变化；重置 CUA、重绑定同一包后也未改善。准确原因 **UNKNOWN**，不能归咎语言、主题、CUA 或产品中的某一层。 |

### 审计窗口与失败正控

以下文件均位于 `preparation/runs/5c76b54db2de4dd89cff9793187208c6/audits/`。
从 `snapshot.events` 重算结果与各自 `summary` 一致；不能只采用
`native/r2-parent-close-readback.json` 的汇总或 comparison 的布尔结论。

| 原始快照 | UTC 时间 | 实际结果 |
| --- | --- | --- |
| `before-navigation-53d59d6e.json` | 09:49:24Z | healthy，24 个事件；两个已就绪且仍存活的 shell；PTY 累计 0；既有被拒绝 exec 22 次／9,240 字节。 |
| `after-navigation-be92fbba.json` | 10:02:23Z | 与 before 的事件序列和两个 live shell 身份相同。 |
| `after-navigation-settled-54a40dce.json` | 10:02:47Z | 同上；PTY 增量 0、新增 exec 0，既有 22 次／9,240 字节单独保留。 |
| `terminal-key-positive-7183ae42.json` | 10:04:09Z | `pressKey x` 后仍为相同事件及两条 live shell，PTY 0；正控未观察到预期服务端输入。 |
| `terminal-text-positive-a1bdd9e2.json` | 10:04:45Z | `typeText x` 后仍同上，PTY 0；正控未观察到预期服务端输入。 |
| `final-82fe00ee.json` | 10:07:07Z | 累计事件及计数仍相同，但 `live_shells=[]`、`app_identity_live=false`；这是关闭后的读回，不是两会话仍就绪的验收。 |

before、after、settled 和两个正控中的 shell 身份分别为
`fixture / pid 17 / kernel_birth ["linux", 9758917]` 与
`fixture_b / pid 50 / kernel_birth ["linux", 9764823]`。
`comparison-353a1472.json` 引用的 before/settled 文件摘要相符，其
`zero_input_and_exec_during_window=true` 只证明这个窗口没有记录到新增 PTY 输入或 exec。
50–54 在该窗口之后，不能塞进同一 comparison 的覆盖范围。
两个未 Enter 的终端正控未建立输入通道有效性，因此**完整键盘隔离仍 OPEN**；
早期可见导航成功、后期按键无变化、服务端零计数分别记账。
旧 r1 的 +62 exec 失败保留，不被新窗口覆盖。

### 关闭、清理和待审状态

应用经 UI 关闭；本 run 的 `app-exit.json` 记录应用 PID 11210 **exit 0 / reaped**。
UI 操作和父执行会话 **78897 exit 0 / reaped** 的来源为父操作记录，
`native/r2-parent-close-readback.json` 明确引用父 `functions.write_stdin(78897)`
的实际返回；文档回读没有重新 wait，也不把摘要当作全部原生成功。
`cleanup.json` / `final.json` 于 **10:07:08Z** 记录 verified=true：
容器、网络和私有目录清理、宿主端口拒绝、应用及 payload 线程收尾。
`preparation/builds/98ee906126ac49ebb05280b168952abd/image-cleanup.json`
于 **10:07:48Z** 记录自有镜像 `915903dd…` 已移除、`shared_base_removed=false`。
回读核验了这些原件与父汇总所列摘要；没有重新运行 Podman 或 GUI 检查。

当前原生结论为 **PARTIAL_OBSERVED**，不再把修后候选统称为 INTERACTION_NOT_RUN。
独立 reviewer Anscombe 已完成原件及相关源码复核，报告为
`independent-review/native-r2-observations.md`，SHA-256为
`d5aa45e2391262fcdcc7ded25cf1c2816feba035c0a84f98d97580b10be09f38`。
54对原件、499工程输入、五个包成员和12份冻结文档均匹配，没有新增可确证源码P1/P2；
只签所列观察子集，后期根因仍UNKNOWN，完整输入隔离OPEN，不授予Hallmark finish或ship。
comparison只覆盖11–49；50/51有可见字符但PTY仍0，不能确认其为远端回显或本地组合文字。
47→48可见焦点从搜索到树；52查询下只剩一个折叠根，End不动无判别力；
53/54有后代的折叠根未展开。最后两个live shell快照为10:04:45Z，final已无live shell。
建议的真实主题modal/语言串联与前后字符及非文本键正控仍未执行，不能填为通过。
源码、工程输入、包和旧原件保持冻结；本次仅文档回写，没有重跑 Cargo、GUI、Podman，
没有提交、推送或发布。

## 历史：修订候选首次启动与窗口访问受阻

用户通知已解锁后的前三次 CUA 查询仍报告 Mac locked，当时没有启动新应用或
容器；见 `native/unlock-recheck-20261105.json`。随后应用清单可读，父任务重新
构建自有镜像，启动独立 run `33a13f5964264e82a8ade810da109dc0`。
`app-process.json` 记录 r2a 程序于 **2026-11-05T03:49:02Z** 启动，
PID 51865，程序摘要与上述修订候选一致；这是实际进程启动，不能继续标为“未启动”。
访问应用窗口时 CUA 再次返回 Mac locked，未取得 AX 或截图、没有交互验收，
也没有显式连接 SSH。清单可读不等于窗口可操作，接口反馈不证明用户未执行解锁。

本 run 的最终审计 `audits/final-a936bf95.json` 为 healthy=true、event_count=0，
shell_ready、shell_input 和 exec_refused 均为 0。没有两条就绪会话，**空审计
不能作为导航零输入验收通过**。未建立 before/after 通过对照，也未补齐 r1 缺口。

应用退出 -15并已回收，supervisor 会话 31463 实际退出 0并由父任务回收。
`cleanup.json` 与 `final.json` 均确认容器/网络消失、宿主端口拒绝、私有目录删除；
精确包路径进程查询为空。自有镜像 `806f98c7…` 已于 03:51Z 删除，
共享基础镜像保留，旧 build receipt 不再是可运行镜像。
汇总见 `native/r2-launch-blocked-readback.json`。
后续只读桌面清单查询仍返回 Mac locked，没有再启动服务；新查询单独保存于
`native/unlock-recheck-after-r2-launch.json`。该阶段以新 build、新 run 和分别暂停
两会话监控后的新审计窗口作为接续条件；后续实际观察见上方 run `5c76b54d…`，
本节窗口受阻、空审计及退出记录保持独立。

## 第一候选原生观察

包为 `native/package-r1/KeelShell.app`，程序 SHA-256：
`ab2d5b91141953a68701ff336c1fb2c03f98e0c504a25253b80b6fc64cfe3f7a`。

1. `cc9e7784…` 的种子把 `group` 写成固定值，违反已有 folder path 不变量，
   StateStore 正确拒绝加载。原始拒绝画面保留，未建立 SSH，也不是产品导航失败。
   新增夹具检查修前失败、修后 27 项全部通过；旧 generator 与检查结果保留在
   `preparation/seed-fix/`，没有改写旧运行种子或修改生产校验。
2. `37e8d636…` 使用修正种子和同一包，实际加载 27 条连接、10 个目录。
   明确标注的 25 条 `.invalid` 仅供浏览；通过 UI 显式连接同一本地 OpenSSH
   服务的 `fixture` / `fixture_b` 两个账号。首次主机指纹与本 run 回执一致后接受，
   未把两个账号说成两台服务器。
3. 关闭首 run 后观察到了额外的同包进程；第二 run 的 `02-startup-seed-r2`
   实际捕获该无种子的额外实例，**无效**。按包路径及内核出生身份终止并确认消失；
   它不属于监督器子进程，退出码及 wait 状态未知。回执为
   `native/extra-instance-cleanup.json`，不把消失冒充 wait/reap。
4. 重新绑定监督器应用后的 `03–06` 为四对有效原始图像/AX，均为
   **2880×1866、霜白浅色、中文**。观察到 Home 选未归档、Left 收起、
   Right 展开/进入首个子项、End 选择并显露末尾空目录；
   两条会话与 `NAV-COMMAND-DRAFT-KEEP` 草稿保持。
   AX 包含树/行和 selected；此工具始终报告窗口焦点，不能据此宣称完整 AX 焦点或 VoiceOver。
5. 下一批 Up/Home/折叠操作因系统再次锁屏被工具拒绝，没有新的成功捕获。
   Tab、搜索恢复、会话激活、三主题/中英文/小窗矩阵及修后原生均未完成。

两个真实 shell 在 before/after 和后续稳定快照中均已就绪，日志健康、前缀连续，
观测到的 **shell 输入累计及增量都是 0 字节**。但 before 的 exec 拒绝为 28 次，
after 为 90 次，窗口内增加 **62 次**，因此组合的“零 shell 输入且零 exec”检查
**FAIL**。周期性监控仍在请求；本记录不把这些请求过滤掉或改基线伪造通过。
夹具不保存命令正文，不能逐次证明这 62 次都来自监控。
后续可在新的独立 run 通过原生监控暂停入口分别停下两会话轮询后再验，
但旧失败及它的范围必须保留。没有 SFTP subsystem，文件区明确显示拒绝，
本轮不把空文件列表当作文件传输成功。

首 run 应用退出 0，第二 run 因 STOP 被监督器终止、应用退出 -15；
两个 supervisor 均退出 0，父任务实际 wait/reap，应用子进程也已回收。
各自的容器、网络、宿主端口和私有凭据目录均完成清理。
误用的独立 `target/` 编译缓存已删除，回执为 `unused-target-cleanup.json`；
共享构建目录保留。确认系统仍锁屏后，已删除本轮自有镜像（共享基础镜像未删），
其 `image-cleanup.json` 保留准确 ID；后续原生需重新 build 新自有镜像。
修后最初打包遗漏 target 导致预检 KeyError，原 manifest 保留；
`native/package-r2a/` 使用实际 host target 重新打包且所有包成员摘要相同，预检通过，
仅该新目录保留。打包与预检不代表原生执行。
已合入且无 worktree 使用的 `feature/net-native-acceptance-20261105` 本地分支已删除。
当前主工作副本和被其它任务使用的旧 worktree 保留，不绕过管理器的保护。

## 验收计划

生产 GPUI 事件检查目录父子移动、空目录、折叠/搜索恢复、滚动显露首尾、
单一 Tab 停靠点、真实会话身份重排、删除/过滤后的旧松键拒绝，以及隐藏后的零终端输入。
原生包使用隔离配置与自有 SSH 夹具检查实际像素、AX 和键盘结果；测试种子明确标注，
未连接的测试配置不冒充在线服务器。三主题、语言和窗口尺寸只记录实际执行的组合。

本轮早期中文导航和有限外观/草稿观察已记录；接续先完成新独立复核及输入异常定位，
在有效输入正控成立后再验后期键盘与完整隔离。紧凑 OS 窗口、完整主题×语言×尺寸矩阵、
长按跨焦点的修后原生复核仍 OPEN，不能由源码回归或零审计计数关闭。

Windows/Linux 桌面、VoiceOver/IME、按首字母跳转、折叠偏好跨重启保存及完整产品目标
仍需独立验收。
