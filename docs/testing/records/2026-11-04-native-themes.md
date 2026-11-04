# 2026-11-04 — 三套原生主题与组合验证

最新状态：主题实现、知识库和隧道管理的组合源码通过本机完整门禁、分项独立复核及下述限定 macOS 原生检查。完整门禁之后仅修复依赖检查器的非成员 dev-dependency 判定，已单独重新执行受影响的完整 Python suite、依赖策略和官方 freshness；Rust 源码、Cargo/CI 配置与打包脚本保持不变。本文保留前序失败，不以最后通过覆盖历史。

## 范围

霜白、曜石、雾紫原生主题目录，独立明暗偏好，旧设置兼容，未知主题 ID 保留，统一 Kit 组件颜色及圆角，终端默认色与原生选择器。

基于 `4cd1809` 的组合候选同时包含本轮知识库、隧道规则管理与依赖升级。不能把各作者的限定测试直接当作最终组合通过。

## 已执行

- 官方 registry 在线 freshness：2026-11-04T10:18:12Z，104 个 Rust 声明/使用检查通过；报告在忽略的 `work/themes-20261104/dependency-freshness.json`。直接要求为 `x.y`，锁文件保留精确版本。
- 初次主题 token/adapter 测试：4 passed。覆盖目录有效性、浅深变体文字/动作/输入边界对比度、系统模式解析和 Kit 与自有 Palette 同步。此时尚未导入终端默认色变化，不外推为最终组合结论。
- 初次编译报告一个已移除的未使用 import；链接器还报告大型测试二进制 unwind 元数据提示。均保留在初次日志。

## 验收边界

最终工程与复核结果见末段。下方 macOS 原生主题检查已完成其限定范围。

受控 GPUI 测试不等同于 macOS/Windows/Linux 原生桌面验收；颜色数值测试不证明实际屏幕、远端 ANSI/OSC 配色或全部语法高亮可读性。没有安装覆盖、签名、公证或发布验收。

HTML 原型是独立设计证据；初次窄屏复核曾被中断，原失败保留。五项 P2 已修复，新的独立 reviewer Euclid 在精确修后源码中未确认 P0/P1/P2。最终浏览器结果见下方。

## 后续检查历史

- Workspace 主题专项：**7 passed**，含 900×580 中英三预设可达、正常保存/Conflict、系统通知和会话/草稿身份保持。
- 完整 gate 首轮停在 Clippy：三个测试 `expect` 和新版 Kit 使 AI 测试一处 `.into()` 冗余。均修正，不放宽 lint。
- 第二轮通过 Python **116 tests（1 skipped）**、格式和严格 Clippy；完整 Rust 在原有 `closing_profile_and_manager_restore_visible_keyboard_target` 发生默认栈溢出，因此本轮 gate 仍失败。终端主题 11 项新增回归在此次中断前均显示通过，但不宣称整个 app suite 通过。
- 相同失败用例单独复现；自有 macOS crash 的调用链显示连接列表按钮分配叠加于巨大的 Workspace render 栈帧。拆分 modal dispatch 与 workspace builder 后，原条件/原栈/原四线程的单例 **1 passed**。后续完整门禁仍需执行；没有增加线程栈或放宽测试。
- Python 3.15.2 / Pillow 13.0.0 的图标校验通过（35 图标、ICO/ICNS）；打包脚本 **57 tests passed**。
- 新非作者 Mendel 的主题源码审查未发现 P0/P1/P2；原生和最终日志复核尚待后续。另一个 NET reviewer 提出 1 P1/4 P2，修复前组合不得合入。

完整失败、修复日志与受限 crash 栈摘录位于 `work/themes-20261104/`；未将其它应用诊断或客户数据写入证据。

## 解锁后的 macOS 原生检查

使用本轮隔离开发包和 `KEELSHELL_DATA_DIR`，没有覆盖安装，也没有读取用户现有连接或凭据。受控 SSH/SFTP fixture 只绑定随机 loopback 端口，终端只回显、不执行 shell 命令；监控的 exec 请求被 fixture 拒绝，不作为监控通过。

首次解锁尝试发现自建配置的文件模式为 `0644`。应用拒绝加载和保存；原始截图、应用正常退出 0 和修正记录保留。只把这份自有测试配置调整为 `0600`、目录调整为 `0700`，重新启动后继续。

真实窗口操作完成：

- 核对 fixture 日志与界面中的同一主机指纹后连接，实际取得终端欢迎输出和 SFTP 列表。
- 在命令编辑器保留未发送的 `printf theme-draft-unsent`，依次使用霜白、曜石、雾紫、霜白、曜石；每次切换后会话入口、文件列表和完整草稿仍在，终端默认背景随主题改变。
- 主题页显示三套抽象预览与明确的推荐明暗模式，保存成功状态实际可见；英文主题页和工作区均实际打开。
- 正常退出返回 0；再次启动同一隔离配置，界面恢复曜石、深色和英文，磁盘设置读回一致。没有宣称临时 SSH 会话跨重启恢复。
- 最后正常退出返回 0；fixture 返回 0，原端口拒绝连接，所属临时根已经删除。

原始 PNG、AX、进程退出记录、包摘要和 `native-theme-results.json` 位于 `work/themes-20261104/native/`。截图为 2880×1866 物理像素，不据此推断最小 OS 逻辑窗口验收。本次主题开发包早于最后 NET 修复导入；不把它作为最终 NET 组合原生证明。

仍未验收：Windows/Linux 原生、OS 外观变更通知、最小原生几何、IME/VoiceOver、外部 OpenSSH/full shell、AI 模型发送、签名公证与发行安装。

## HTML 原型最终复核

修后真实浏览器分别检查 A/B/C 的 1440×900 和 900×580，以及 A 的 375×844；这七个 viewport 的文档宽度均未超出 viewport。实际点击验证开发组主机与新 session tab 同步、目录切换清空选区并禁用下载、专注模式隐藏工具区后通过命令历史恢复工具区。源码与本地引用检查确认四个入口的静态截图和无脚本导航；未单独声称浏览器禁用 JavaScript 的动态验收。

Hallmark 静态检查为 0 FAIL / 8 WARN；命令请求了 `--render`，但工具未找到 puppeteer-core，因此它自己的 render tier **未运行**。原始告警与实际浏览器测量分别保留，不把退出 0 写成 render 通过。原型所有主机、指标、文件和回答都标为示例，不连接服务器、不执行命令、不调用模型。

八条告警逐项处理：五条对比度告警来自静态扫描混用了不同主题和被覆盖的 base token；实际 computed style 的正文比值为 11.195–15.790，选中项文字为 5.152–6.607，全部高于 4.5。大面积 accent 告警指向未被页面使用的 scaffold `data-slot` 规则。F4 保留选中标签/切换按钮的背景作为导航状态，与主提交动作区分；F24 保留工作区细边界及外部轻阴影，分别用于密集内容分隔和原型画布分层。原始 WARN 没有隐藏；理由同时进入 `.hallmark/log.json`。

## 组合 gate 第三轮

完整 gate 第三轮通过脚本、格式和严格 Clippy，应用测试 **812 passed / 3 failed / 2 ignored**。原栈溢出测试已通过；三项失败为 AI 诊断步骤和本地知识保存控件在滚动正文中的可见性。

知识库入口移至助手标题行，避免单独增加一行推移全部正文。生成诊断计划和确认知识结果会展开新内容，原三项测试增加真实 scroll-container 滚动后继续原可见性、指针和绑定断言；没有删除断言、放大窗口、调整线程栈或延长超时。中间 header 试验的编译失败和可见性失败也保留。最终 focused 结果分别为诊断 **1 passed**、capture save **1 passed**、workspace knowledge **6 passed**、knowledge library **12 passed**；这些过滤集有交集，不相加作为独立用例总数。新的完整 gate 与独立复核继续进行。

## 最终组合门禁与独立复核

`full-gate-v4-result.json` 记录 `python3 scripts/check.py` 实际退出 **0**，开始/结束之间 **929 个输入文件完全相同**，耗时约 512 秒。普通 Rust 测试 **2,007 passed / 0 failed / 23 ignored**；rustdoc **10 passed**（其中 1 项仅编译，9 项运行）；app **819 passed / 0 failed / 2 ignored**。Python 初次完整执行 **116 项 / 1 skipped**。格式、workspace/all-targets 严格 Clippy、x.y 策略及额外 readiness、observer I/O 和 2 MiB 控制器检查均通过。

主题、终端默认色、Workspace render 拆分和助手滚动修正由 Mendel 独立复核；AI 知识库由 Bernoulli 独立复核；NET 两轮修复由 Curie 复核并核对精确前后源码及父任务日志；HTML 五项修复由 Euclid 复核。上述限定源码范围最终均未确认 P0/P1/P2。NET 最终 focused 为 **17 + 13 passed**。真实远端取消超时服务器场景未运行，不能由拒绝取消测试推断通过。

依赖工具 reviewer Pascal 随后发现非工作区成员的 dev 依赖被错误要求出现在根锁文件的 P2。修复把声明遍历、工作区成员与根锁图分开；新增 4 项回归。原检查器在同样反例中实际失败，修后检查器 **53 项通过**；完整 Python suite 重跑 **120 项 / 1 skipped**、依赖策略与 **2026-11-04T11:45:03Z** 的官方 freshness **104 项通过**。该末轮只改两个 Python 文件；不声称 v4 原 929 输入仍是完全相同的最终文档/脚本快照。原始证据在 `work/themes-20261104/dependency-review/`。

修后 Pascal 复核 P0/P1/P2 为 0，独立 53 项检查器测试、3 项附加隔离控制及实际仓库 104 行离线重放通过。Mendel 最后回读原生 AX、截图、进程和 gate 清单，并核对 448 个 Rust 文件及 Cargo 输入未变，主题/助手范围最终签核 PASS。

## 最终原生包补充核对

同一 Rust 输入构建并通过 macOS arm64 staging，主程序 SHA-256 为 `6d92ada1ddeff800bac062599611949ab70899b77fc39aba407da277272898e3`。实际打开知识库，确认搜索、标题、摘要的中英文 AX 名称与用户输入值分开呈现；切换语言后条目内容保持。最终主题页实际显示三套目录项、当前选择及独立明暗设置。程序正常退出 **0**。

最终原图和 AX 为 `13-final-*` 至 `17-final-*`，退出为 `app-exit-final.json`，位于 `work/themes-20261104/native/`；它补充的是主题入口与可访问名称检查，没有重新执行 SSH 联调或 NET 原生流程。三主题切换时保留 live fixture/文件/草稿及重启持久化的证据仍属于上文较早原生包，来源没有合并混写。三平台完整桌面、系统外观通知、最小 OS 窗口、IME/VoiceOver、外部 shell、签名安装仍开放。
