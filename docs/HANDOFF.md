# KeelShell 当前交接 — 2026-11-05

本入口供新的 AI 会话接续，时间使用 Asia/Shanghai。先读 [AGENTS.md](../AGENTS.md)，
再按下面的切片记录核实源码、门禁、原生、CI 和交付状态。旧的详细交接已移到
[组合前历史快照](history/2026-11-05-handoff-before-workspace-integration.md)，
其中“当前”、暂停和清理指令只代表历史时点，不构成新的操作授权。

## 当前工作

当前工作副本复用原分屏工作树，分支为 `feature/workspace-integration-20261105`。
本次候选提交组合以下已审查的提交和冻结导航源码，尚未合入 main；原515输入的本机组合门禁与非作者源码/证据审查已通过，组合包53对原生捕获正在独立复核，精确组合CI在提交推送后另行绑定。原生结束后的文件布局测试诊断已通过新的完整门禁，测试修订另待非作者结论：

| 切片 | 精确输入与已验证范围 | 当前边界 |
| --- | --- | --- |
| 原生分屏 | `9db9cbc1dcfd22c05a2e8a5dfb183877c8ff9f65`，父为 `77aa69c`。完整门禁 2,158 ordinary / 23 ignored、11 rustdoc、Python 128 run / 1 skipped；503 输入相同。限定 macOS 原生及新的非作者组合复核通过。 | 已推 feature；Quality `42009970970 / attempt 1` 为 failure：macOS、Windows success，Linux唯一文件布局状态等待失败，根因UNKNOWN；52项分屏测试三平台通过。独立提交绑定批准，两处状态同步P3已在组合中纠正并限定复核；完整原生矩阵未完成。 |
| Ask 诊断与 CI 容量 | `3d8476716ab80f0cec8b8c731a8cde0bcd7601a3`、`72a19ea5b7d47402c79d0ced73d442570c7ae9fd`。容量修订完整门禁 2,126 ordinary / 23 ignored、14 rustdoc、Python 131 run / 1 skipped；506 输入相同。新的非作者源码、组合和不可变提交绑定通过。 | 已推 feature；Quality `42009720436 / attempt 1` 已三平台success。这是新容量修订提交的通过，不回写旧失败，也不解释B的新文件布局失败。 |

源码自动组合没有冲突；冲突仅在 HANDOFF、ROADMAP、CAPABILITIES 的状态段落。
随后以三方patch接入导航的10个冻结源码文件，也没有冲突；7个文件保持导航原字节，
workspace、modal_scope、view三个文件组合了分屏与导航变化，必须重新审查交互。
本分支完整门禁 `full-gate-r1` 已**退出0并回收**，父执行会话 **90038**。
**2,191 ordinary Rust / 23 ignored、14 rustdoc、Python 131 run / 1 skipped**，
app为933/2 ignored；**515工程输入前后一致**。格式、严格Clippy、依赖策略及
附加进程控制器通过；共享target、2 jobs和1500秒预算未改变。
新的官方依赖检查退出0并回收。新的非作者组合复核为
`APPROVE_SCOPED_COMBINED_SOURCE_AND_EVIDENCE`，P0/P1/P2=0；
一项非阻断P3涉及两处仍写C原件待审的状态文字，已由document-followup-r1限定复核关闭。
完整门禁与新的非作者组合复核不继承来源分支结论。
当前 main 仍为 `77aa69c04ad0dd625a5d827552335878de993ee7`，以后续 Git 读回为准。
新的macOS arm64组合开发包已构建，程序SHA-256为
`33be2e685a280b6bfd2094d4707a67c12b8a14ccac6e2b92d34f64a877154831`；
515项冻结输入一致，41assets另行核对；已执行下面的原生子集、未安装，
不能替代来源包或完整平台验收。
组合材料放在 `work/workspace-integration-20261105/`，正式记录从
[组合记录](testing/records/2026-11-05-workspace-integration.md)继续。

## 新组合包的原生子集与资源

run `3f7689e959484901bc4365d98819d8ad` 已结束，原始AX/JPEG共53对，均2880×1866。
最初普通`x`按键可见但PTY0，首次Right0，Escape后Right1而非预期3；
随后两账号粘贴与fresh Right及导航后对照均独立成功。普通字符路径与恢复根因仍UNKNOWN，
不能以粘贴代签，也不把现象归咎IME。
导航/隐藏q/两主题切换/语言段PTY与exec均零增量，但该段End/Down没有移动；
单独Escape后方向、Home/End、搜索Tab导航有实际移动及独立零增量。
分屏建立第三条shell并暂停新监控；点击分隔条+Right未改变比例且PTY增加3，
Tab聚焦后Right由50%变55%且该段为0。合并保留三shell，保存仅有UI确认。
最终累计PTY24字节、32exec/13,440字节，命令与Ask草稿保持、无命令执行或模型发送。

应用由UI退出且exit0/reaped；父wait70792得到final，执行会话已回收、原监督器PID不存在，
外层数值退出码未展示，不代填。11:40:00 UTC清理容器、网络、私有目录，
11:40:39 UTC清理专用镜像，共享base保留。没有仍活的本轮测试应用/容器。
新非作者正在核对53对原件、分段审计与源码；见组合记录和
`work/workspace-integration-20261105/native/verification-r1.json`、`parent-close-readback-r1.json`。
原生结束后新增的文件布局测试诊断通过full-gate-r2：2,191 ordinary/23ignored、
14rustdoc、Python131run/1skip，app933/2ignored，修订515输入前后一致；
exec36492退出0/reaped。两项exact专项各1passed，布局176/176组合。
官方依赖freshness-r2在11:59:34–11:59:51 UTC通过104条要求，无版本变动。
原包证据保持r1归属；Linux旧失败与两处测试时序前提缺口仍OPEN。

## 导航原始候选与原生边界

`feature/navigation-keyboard-20261105` 的源码与 r2a 开发包仍独立，基线为 `967c588`。
修订完整门禁 2,075 ordinary / 23 ignored、10 rustdoc、Python 128 run / 1 skipped，
499 输入一致；非作者源码与旧证据准确性复核通过。源码已按冻结摘要导入当前组合，
原候选工作树未修改；新原生证据和开发包仍只属于原499输入。

解锁后同一程序 `1f180f13…` 在 run `5c76b54db2de4dd89cff9793187208c6` 取得54对原件。
早期中文树导航、折叠、搜索、已有会话切换和草稿保持有可观察结果；后半段方向键、
Home/End等没有预期导航变化，最后两个终端正控也没有确认服务端输入，重绑定也未恢复，原因 UNKNOWN。
搜索文本输入和鼠标/AX 点击仍可改变界面。不能据此归咎语言或直接认定产品缺陷。
两条真实 SSH 会话在09:49:24–10:02:47 UTC审计窗口内保持身份，raw PTY为0、
新增exec为0；初始化已有22次拒绝/9,240字节另计。正控未成功，完整键盘输入隔离仍OPEN。
原件中29/30仍是B，31选A/32激活才切A；37/39/40没有选到文件名声称的空根；
38缩窗未生效。文件名不是验收结论。

应用和监督器均退出0并回收，10:07:08 UTC清理自有容器、网络、私有目录；
10:07:48 UTC清理专用镜像，共享base保留。新的非作者已逐项复核54对原件、源码路径、499工程输入、包及12份冻结文档，
未发现新增可确证源码P1/P2；只认可PARTIAL_OBSERVED，不授予完整native/finish。
独立记录与设计合同已接入组合，原证据保持。组合源码审查和原生验收另行进行。
接续该树的 `work/navigation-keyboard-20261105/native/r2-parent-close-readback.json`
及 `independent-review/`；旧失败和旧包证据保持，禁止混用分屏包补记导航验收。

## 已审查切片的必要边界

- 分屏：修后程序 `5ce34862…` 的 run `acceb50f…` 有35对2880×1866原始AX/JPEG，
  四个真实Shell、两轴拖动、三个分隔条键盘、工具选择保留、聚焦/合并/关闭和保存读回。
  原四个会话合并后保持，只显式关闭第四个才剩原三个；最后独立输入正控增加1字节，
  监控exec另计。缩窗未生效；后期草稿保持包含人工Tab加入的两个尾空格。
  四叶保存比例6347/5718/5000，单叶保存活动标签为原遍历第二项，后续正控后未再保存。
  新的 `combined-r5` 为 APPROVE_SCOPED_COMBINED，开放P0/P1/P2/P3=0，
  Hallmark finish只签所检子集。详见[分屏记录](testing/records/2026-11-05-native-pane-layout.md)、
  [布局合同](product/WORKSPACE_LAYOUT.md)及[ADR 0101](adr/0101-native-session-pane-ownership.md)。
- CI容量：串行夹具显式关闭，下一夹具检查前一目录已删除；非workspace依赖dev debug=0，
  缓存新分代，检查命令前后记录数值容量。原断言、期限与四线程入口保持。
  旧诊断提交Quality `41994734898`的Linux实际ENOSPC不解释更早`41967541069`的UNKNOWN。
  Windows限定重跑成功，attempt2的Linux/macOS为继承结果，整体仍failure。
  本机两种夹具程序10,078,320/9,921,664字节不是远端或完整target节省量。
  见[容量记录](testing/records/2026-11-05-ci-storage-capacity.md)、
  [诊断记录](testing/records/2026-11-05-local-ask-scratch-diagnostics.md)及
  [ADR 0103](adr/0103-ci-storage-capacity.md)。

## 用户要求与工程约束

- 只管理远程SSH；不接入本地终端、远程桌面或串口。保持真实原生GPUI Kit应用和
  macOS、Windows、Linux目标；编译、受控GPUI与仿真不能替代各平台桌面验收。
- 霜白、曜石、雾紫全部内置，主题通过注册表和语义token扩展；业务组件不按主题ID分支。
  使用既有Hallmark原生映射，见[设计合同](design/HALLMARK_NATIVE.md)。
- UI/core/session/AI分层；阻塞I/O离开UI线程。公共库API有rustdoc，解释不明显约束，
  使用类型化错误和有意义的测试。保持中英文README同步。
- AI上下文发送必须显式审核；生成的命令另行审核执行。主机密钥变化fail closed；
  凭据默认临时，显式保存仅进系统凭据库或加密vault，不能写入普通配置或日志。
- registry依赖要求仅`x.y`，保留Cargo.lock patch，不pin Rust工具链；每轮查官方稳定版本。
  最近容量切片freshness为2026-11-05 17:01:46–17:02:04，104条通过；组合另留新回执。
- 用户已授权公开GitHub、提交推送、标签触发多平台Release和本地Podman真实联调；
  不涵盖遥测上传或覆盖安装。仓库中不写具名专有SSH产品比较。
- 每个功能完成后新开非作者评审。先完成已授权工作，不反复请求确认。
  源码门禁、开发包、原生子集、CI、公开发行与安装分别验收，失败证据不得改写。

## 下一步与资源

1. A/B/C原515组合门禁与非作者源码/证据复核通过，文字P3限定复核已关闭；
   C原件仅PARTIAL_OBSERVED。收回新组合53对原生独立复核，完整输入隔离仍OPEN。
2. A的精确CI三平台success，B精确CI因Linux文件布局条件等待失败；失败与原件保留。
   B observer已退出0并回收。文件布局已定位测试前提与诊断缺口，确切根因UNKNOWN；
   阶段诊断、限定验证及新完整门禁已通过，候选提交的精确CI与两份限定复核并行；
   复核及精确CI有实际结论后再判断main合入，不提前签完成。
3. 继续紧凑OS窗口、完整主题×明暗×语言×尺寸、IME/VoiceOver、Windows/Linux桌面，
   并推进重启恢复、多窗口、真实供应商/CLI和审核目标的完整原生矩阵。
   完整产品目标仍active，具体能力缺口见[路线图](ROADMAP.md)及[能力清单](product/CAPABILITIES.md)。
4. 仅在功能证据与提交保存后归档无消费者的受管worktree，清理自有分支、进程、容器、网络、镜像。
   管理器曾拒绝仍被使用和主工作副本的归档，不能用shell绕过；不删共享target/base image。

当前没有本轮KeelShell或SSH测试容器在等待用户。A的CI observer执行会话34765已退出0并回收，
75个子进程全部回收，macOS artifact补取会话1068也退出0并回收；精确原件188文件已逐摘要复制到组合的source-ci-a目录。
B的observer执行会话48913已退出0并回收，全部启动子进程已回收；两者只读GitHub并有期限。B原收集器6419因错误沿用main筛选已停止并回收，
空查询页保留；修订收集器仍绑定同一run且不延长原观察总期限。资源状态以终态回执为准。
本地`work/`证据被Git忽略，分别位于对应切片的工作副本；公开文档中的路径不代表已发布附件。
