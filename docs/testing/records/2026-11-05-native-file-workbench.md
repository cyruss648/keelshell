# 原生文件工作区与渲染栈验证 — 2026-11-05

本轮基线为 `3a1473e241263f0b531af16e4e1dd8fd46fc65b0`。
源代码、工程门禁、受控 macOS 程序、平台 CI 与公开发行分别记录。
最终候选门禁、独立复核和提交绑定以收尾段为准；早期失败记录保留。

## 改动范围

- 默认展示本地/远程真实列表，文件操作与传输设置互斥展开，输入实体和草稿保留。
- 正常导航采用标题/操作与路径两行；审核/断开快照保留紧凑路径，
  修改路径继续撤回旧提案。批量上传/下载和队列入口常驻。
- 已有编辑、比较、冲突和传输结果保持可达，设置折叠不代表结果关闭。
  隐藏输入的键盘焦点回到可见入口或惰性文件面板。
- 生产工作区和布局夹具共用 `FilesPanel::workbench_height`：
  900×580 窗口中为 212px，1440×900 中为 340px。
  正常导航/紧凑导航/操作条外高分别为 61/29/30px。
- 紧凑审核正文上限 32px，保留完整文本、两轴滚动与显式展开。
  三主题继续通过统一语义 token 呈现，没有主题 ID 业务分支或新依赖。

设计见 [ADR 0097](../../adr/0097-file-workbench-disclosures.md)、
[文件工作区指南](../../product/FILES_WORKSPACE.md)。

## 文件布局候选的工程检查

本地忽略证据根为 `work/native-file-workbench-20261105/`。

| 运行 | 实际结果与后续处理 |
| --- | --- |
| `focused-r1` | 对二进制 crate 使用 `--lib` 的命令错误；没有测试通过结论 |
| `focused-r2` | 新控件缺少 `Selectable` trait 导入，已修正 |
| `focused-r3` | 双语言文件操作专项通过 |
| `files-r1` | 171 passed / 8 failed；审核时隐藏路径破坏既有提案撤回路径，改为保留紧凑路径，并修正展开入口的测试导航 |
| `files-r2` | 178 passed / 1 failed；紧凑导航按钮超出半像素，修正尺寸 |
| `app-r1` | 844 passed / 2 ignored；早于最终焦点和几何调整 |
| `full-gate-r1` | 严格 Clippy 拒绝新增测试中的三处 `expect()`；改为测试内显式失败闭包 |
| `review-negative-r1` | 真实手工冲突输入测试在新增 manual guard **之前已经通过**；不是失败反例 |
| `queue-focus-r1` | 编辑器和手工冲突输入的两项队列焦点测试通过 |
| `files-r3` | 175 passed / 10 failed；改为生产 212px 后暴露 29px 操作视口内的 32px 控件及旧滚动断言假设 |
| `files-r4` | 183 passed / 2 failed；余下两项本地导航夹具未计外边框，已修正夹具外高及内部高度断言 |
| `full-gate-r2` | 退出 0、进程已回收；普通 Rust **2,034 passed / 23 ignored**，含 app 846 / 2 ignored；rustdoc 10 passed（一项仅编译），Python 128 / 1 skipped；格式、严格 Clippy、依赖策略与额外进程控制器通过 |

`native-build-r2` 和 `full-gate-r2` 的 **489 个输入一致**。
这次完整门禁早于后续工作区渲染栈修复，不能替代修复后的门禁。
官方依赖 freshness 开始于 `2026-11-04T19:01:59Z`，104 条声明/使用通过；
报告为 `dependency-freshness.json`。本轮没有修改依赖版本。

## 独立检查与失败证据

本地导航作者与主实现分开工作；其工作副本已归档、分支已清理，
原文件、补丁和验证保存在 `local-navigation-author/`。
非作者 Lorentz 初查提出手工冲突输入未列入 queue focus guard 的源码风险。
新增真实冲突测试在旧 guard 下也通过，不能把该风险写成已复现的 P2；
补全 guard 属防御性覆盖。Lorentz 已在 `p2-reassessment.json/.txt` 中撤回
该 P2，状态为 **WITHDRAWN_UNPROVEN**；不是“复现后修复/关闭”。
后续 `source-review-r2` 对布局候选报 P0/P1/P2=0，早于渲染栈改动和最终原生复核。

初查同时指出：旧独立布局场景给文件区 232px，而实际工作区只有 212px。
本轮已让两者调用同一个生产高度函数，并保留随后失败和修复的记录。
几何测试不等于操作系统窗口验收。

## 受控 macOS 原生证据

使用隔离数据目录和真实本机 SSH/SFTP 夹具；shell 仅回显，不能执行命令。
凭据是夹具专用公开测试值。未连接客户机器，没有模型调用或发行安装。

| 包内程序 SHA-256 | 实际覆盖 |
| --- | --- |
| `8e7f61111f3367220f40ef6a483c467ae839410230ef5b81268be76bf38ec146` | 首包 01–02：霜白浅色中文、真实双列表、文件设置与文件名草稿；早于最终几何/焦点调整 |
| `14c5b8ef98908b08655c02a9b806acba3eb55883216d08abb30a113db3e30624` | 布局修后包 04–10：三主题各自浅深中文、设置互斥切换与草稿保留；11–22：900px 宽英文、审核展开/取消、真实上传和目录下载、完成队列 |

04–10 原始 JPEG 为 2880×1866；11–22 为 1800×1866。
结合启动尺寸、2 倍像素与标题栏，这些分别对应 1440×900、900×900 客户区。
12–16 的高度拖动没有成功，文件名中的 small/minimum **不证明 900×580**。
本轮该尺寸的原生几何仍未验证。`capture-manifest-r2.json` 保留每张的尺寸/摘要。

- 05 记录在文件设置输入 `retained-final-file-draft`、切至传输再返回后保持；
  命令草稿 `printf "unsent-native-draft"` 在主题、语言、队列切换后仍未发送。
- 17–19 完整上传审核展开后取消；夹具目标 `notes.txt` 仍不存在。
  `review-cancel-readback.json` 保存独立文件系统读回。
- 重新准备并明确确认后，34 字节 `notes.txt` 上传完成，两端 SHA-256 相同；
  下载 `/bin` 的两个测试文件，共 30 字节，本地/远端逐文件摘要相同。
  20、22 记录真实完成队列 1→2；内容回执为 `upload-content-readback.json`、
  `download-content-readback.json`。
- 900px 宽、显示传输结果时，原生滚轮尝试未移动远程列表。正确点击可选中
  可见行，但尚不能据此区分工具事件、布局或滚动实现原因；后续专项复核需闭合。

03 错误捕获了旧 bundle 的空工作区，明确无效。
旧捕获闭包持有旧 App；后续改为把目标 App 作为显式参数并保存完整 AX。
退出后读取 App AX 又拉起空配置实例；该实例随后关闭，`listApps` 确认
`isRunning=false`。原自有包和夹具均退出 0、已回收。无效捕获、关闭索引失效
及初始化配置权限/结构错误均保留，不能算作原生通过或产品回归。

### 渲染拆分后的最终包

`native-build-r3` 退出 0、已回收；包内程序 SHA-256 为
`132881a279779de29f813ecad5c30276bfa5b3387d66b962304ebbd7ce80a668`。
构建、最终门禁启动和捕获后的源码检查共 **490 个输入一致**；
回执为 `native/package-r3-gate-binding.json`。提交身份另由收尾读回绑定。

- 23–27 实际为 1440×900：霜白浅色、曜石深色、雾紫浅深中文，以及雾紫深色英文。
  文件与传输设置互斥切换后，`retained-r3-file-draft` 保留；
  命令草稿 `printf "unsent-r3-native-draft"` 始终未发送。
- 28–35 实际为 900×900 的雾紫深色英文。26、27 的初次缩宽尝试未成功；
  后续沿最右边缘拖动才缩至 900px。28、29 的高度拖动仍未成功，
  **不能称为最终包 900×580 原生验收**。原始 JPEG/AX 和逐图摘要见
  `native/capture-manifest-r3.json`。
- 30–32 展开完整审核再取消，独立文件系统读回确认目标仍不存在。
  重新审核并确认后，33 显示上传 34 字节、队列 Done 1，两端内容摘要一致。
  35 显示下载目录的两个文件共 30 字节、队列 Done 2，逐文件内容摘要一致。
  三份 `native/r3-*-readback.json` 保存取消、上传、下载的实际读回。
- 34 在传输结果显示时对远程行发送原生滚轮，仍显示相同前三行。
  此观察保留在 `native/scroll-attempts-r3.json`，没有宣称修复 OS 滚动。
- 最终应用和新夹具均退出 0、已回收；关闭后用应用清单确认
  `isRunning=false`，没有再次读取已退出 App 的 AX。

此前布局包的六种主题画面与此最终包的限定范围分开，不能拼成最终包全矩阵通过。

### 独立滚轮专项

Lorentz 在独立源码副本中，使用生产 `Workspace` 和真实 SFTP，
分别挂载 900×900、900×580 场景，完成 34 字节上传并保留结果面板，
随后派发像素和行单位滚轮事件。两个测试均通过：最后一行进入可见区域、
可实际选中、反向滚动返回顶部，且没有新增写入或审核。
该专项退出 0、进程已回收，几何没有出现列表越过可见主体的假设问题。

两项测试已去掉诊断打印后纳入 `files/remote_wheel_tests.rs`。
没有修改生产滚动实现；受控事件通过不证明 OS/CUA 原生事件通过。
原生无滚动的原因仍 **UNKNOWN**。原测试源码、构建输入、几何和进程回执
保留在 `independent-review/remote-wheel-probe/`，一次性源码副本已清理。

## 上一提交的跨平台失败与本机栈诊断

精确基线 `3a1473e` 的 Quality **41891939556** 为 failure：

- Linux job `126791331424` 在应用测试中栈溢出，SIGABRT / Cargo 101。
  日志命名测试为 `capture_reload_propagates_busy_and_failure`。
- Windows job `126791331254` 栈溢出，状态 `0xc00000fd`；
  命名测试为 `profile_sync_close_and_history_cannot_restore_retired_mirror_approval`。
  同一测试在该次 Linux 日志中通过。
- macOS job `126791330849` 取消，runner ID 0、步骤为空；
  annotation 指向托管 runner 容量，未执行测试。
- 两个已执行平台的格式/Clippy 均通过。测试入口名称不等于确切故障语句。

原日志与 API 回执在 `previous-ci/triage/`。对空日志的首次下载尝试无有效证据；
显式允许终端转义后取得日志，并与 archive 字节核对。

Zeno 对拥有独立副本的 macOS arm64 测试程序进行六次有界诊断：
程序 SHA-256 为 `a2b50b06e77452bb83bfbc7eead3e9d0ce249e747cb7106bb82252eecf62e0d6`。
两项测试在请求 2 MiB 线程栈时通过，1 MiB 时栈溢出；
512 KiB 时由 LLDB 捕获 `Workspace::render_workspace` 入口栈探测访问 guard page。
反汇编证明该函数入口分配 **1,101,856 字节**，尚未进入业务场景。
调试器进程正常结束不代表测试通过，两个被调试程序均由控制器终止并回收。

该副本与完整构建输入的密码学绑定未证实；它与本轮原生包分别计身份。
本机结果支持缩小渲染函数栈帧，但**历史 Linux/Windows 的确切根因仍 UNKNOWN**。
没有提高 CI 栈预算来隐藏失败。报告、原始栈帧、请求的栈设置及收尾在
`previous-ci/triage/small-stack-r1/`。

工作区拆分后，新的输入绑定测试程序
`dcba56db8f16ed3b363e9906ccd3df0dea69ce13e1f9b8db1ef92bc297bee20f`
将 `render_workspace` 入口分配降到 **62,208 字节**。
原两项准确场景在请求 2 MiB 栈时均通过。另行捕获的 1 MiB 故障已转移到
`TunnelsPanel::render`（入口分配 974,800 字节）以及
`FilesPanel::local_browser_view`（312,112 字节，另有调用者帧）。
512 KiB 的故障位于 quick-connect 路径下的 retained-button 元素转换。
这些是分别捕获的实际边界，不能用 512 KiB 堆栈推断 1 MiB 故障。

本轮不声称全应用支持 1 MiB/512 KiB 调试线程栈，也没有给 CI 设置更大栈。
工作区的大帧确实减小；其它渲染帧预算和历史平台唯一根因仍独立保留。
准确命令、两轮构建和局部反汇编在
`previous-ci/triage/frame-refactor-r1/revision-2/`；
`boundary-1m-r1/` 保存 1 MiB 调试器捕获与 2 MiB 测试通过回执。

## 收尾与开放边界

最终 `full-gate-r3` 退出 0、进程已回收：普通 Rust **2,036 passed / 23 ignored**，
其中 app **848 passed / 2 ignored**；rustdoc **10 passed**（一项仅编译），
Python 运行 **128 项 / 1 skipped**，格式、严格 Clippy、依赖策略和额外进程控制器通过。
完整原日志、逐项汇总、输入和退出回执均保留。490 个输入与最终原生构建一致。
非作者最终组合复核为 **APPROVE_SCOPED_CHANGE，P0/P1/P2=0**：
覆盖已记录源码/测试、完整本机门禁、限定原生范围和同步文档，
没有关闭下方原生/平台边界。33 个候选文件的 SHA-256/Git blob 清单及报告在
`independent-review/final-combined-review-r3*`；reviewer 允许在提交前替换待定结论，
提交后的准确 blob 与门禁输入绑定由交付回执记录。
原生 finish review 对上述最终包已捕获范围给出 **ship**，无 P0/P1/P2；
这不是完整平台验收。reviewer 独立核对程序、13 组截图/AX、490 输入和原日志，
并在清理前重新计算了仍在本地的上传源与下载文件摘要；没有重跑本轮完整 Cargo 门禁。

诊断作者已移除 3 份复制测试程序和 60 个临时目录，完整符号表压缩后校验保留，
没有其自有进程残留；生产源码保持冻结。独立滚轮测试的副本已清理，
编译过的确切测试源码、输入和原日志保留。自有原生应用和夹具也已退出回收。
随后清理旧布局包和隔离测试数据/传输文件，逐文件摘要与旧包 manifest 保留在
`native/final-native-cleanup.json` 及旁边的保存副本；最终 r3 开发包保留。
夹具临时根目录已由退出清理。没有删除共享缓存或其它任务持有的 worktree。
提交/推送/CI 将由精确提交回执绑定；没有创建标签或覆盖安装。
完整主题×语言×尺寸矩阵、Windows/Linux 桌面、OS 外观通知、IME、VoiceOver、
原生文件拖放和完整产品/AI/发布安装目标继续保持 OPEN。
