# 工作区组合验证记录

日期：2026-11-05，Asia/Shanghai。分支：`feature/workspace-integration-20261105`。
本记录维护组合的实际状态，不把独立分支的检查结果当作组合验收。

## 输入与范围

| 输入 | 精确提交 | 既有证据 |
| --- | --- | --- |
| 原生分屏 | `9db9cbc1dcfd22c05a2e8a5dfb183877c8ff9f65` | [分屏记录](2026-11-05-native-pane-layout.md)：完整门禁、限定macOS原生和新非作者组合复核；精确CI `42009970970/1` 为Linux文件布局等待失败，其余两平台success，详见下文。 |
| Ask诊断 | `3d8476716ab80f0cec8b8c731a8cde0bcd7601a3` | [诊断记录](2026-11-05-local-ask-scratch-diagnostics.md)：有界事实队列与错误来源，原CI失败保留。 |
| CI容量 | `72a19ea5b7d47402c79d0ced73d442570c7ae9fd` | [容量记录](2026-11-05-ci-storage-capacity.md)：显式夹具关闭、debug配置、容量记录、完整门禁与独立提交绑定；精确CI `42009720436/1` 已三平台success。 |

共同基线为 `77aa69c04ad0dd625a5d827552335878de993ee7`。
连接树原始候选基线为 `967c588803df493d06d42cf96221bd691aba46b9`，499工程输入与
门禁、r2a包保持一致；其54对新增原生捕获已完成新非作者复核，仅PARTIAL_OBSERVED；
后半段输入异常根因UNKNOWN，完整输入隔离OPEN。
随后将其10个冻结源码文件经三方patch导入组合，没有改动原工作树。
本组合没有安装应用、发布标签或修改用户真实连接。

## 组合过程

只读 `merge-tree` 和实际 `merge --no-commit --no-ff` 均显示工程源码无冲突。
冲突限于 HANDOFF、ROADMAP、CAPABILITIES 的状态段落。保留各切片的来源、
原失败和未验边界，新增本组合状态。旧分屏交接作为明确的历史快照保留，
只调整因目录移动产生的Markdown相对链接；原始字节摘要及转换回执位于
`work/workspace-integration-20261105/document-resolution-r1.json`。

导航patch应用退出0、没有冲突。7个文件与原导航源码逐字节相同；
`workspace.rs`、`modal_scope.rs`、`view.rs` 三个文件组合了两个UI切片，
需要新的非作者交互复核。来源、patch摘要与逐文件结果见
`navigation-import-preparation-r1.json` 和 `navigation-import-readback-r1.json`。

分屏不可变提交绑定另发现非阻断P3：两个当前能力表格仍写combined/finish待审。
本组合只纠正这两句，所有OPEN条件保持；冻结B提交与旧审查不改写，
限定修订回执为 `b-document-p3-correction-r1.json`。新的限定复核
`work/native-pane-layout-20261105/independent-review/doc-p3-followup-r1.md` 已批准，
该两句范围开放P0/P1/P2/P3=0；不签整个组合。

## 验证状态

- 新官方依赖检查退出0并回收；使用当前Cargo.toml/Cargo.lock，没有新增或升级依赖。
- 提交前再次核对官方来源：11:59:34–11:59:51 UTC的`dependency-freshness-r2`
  退出0并回收，34个registry包、104条requirements通过，errors为空；版本要求仍为x.y。
- `full-gate-r1`：**PASSED，exit0/reaped**，exec90038、child67248；515工程输入
  前后相同。共享target、2 jobs、原1500秒预算与四线程测试入口保持。
  **2,191 ordinary Rust passed / 23 ignored、14 rustdoc、Python 131 run / 1 skipped**，
  其中app933/2ignored。格式、严格Clippy、依赖策略及附加进程控制器通过。
  `full-gate-r1-readback.json`、原日志和进程回执保留；文档测试包括compile-only/compile-fail，
  不等同14项运行时用例。链接器既有compact-unwind警告保留，不称无警告发行构建。
- 新的非作者组合源码/证据审查为 **APPROVE_SCOPED_COMBINED_SOURCE_AND_EVIDENCE**，
  P0/P1/P2=0；515工程输入、23文档、41assets及五个包成员均绑定，
  579个本地链接目标存在。两处仍写C原件复核待收回的文字构成一项非阻断P3；
  本轮已纠正，并由 `document-followup-r1` 限定复核关闭，开放P0/P1/P2/P3=0。
  报告在 `independent-review/combined-r1.md/json`，后续文档批准不扩大原审查范围。
  候选e331372已推送，精确Quality42025002190/1已三平台success，见下方精确CI终态。
- 独立分支的门禁和开发包继续只签各自输入。组合macOS arm64开发包已构建并分目录打包，
  构建/打包均退出0并回收；515门禁/构建/打包/当前输入相同，41assets另核对，五个包成员摘要匹配。
  程序SHA-256为 `33be2e685a280b6bfd2094d4707a67c12b8a14ccac6e2b92d34f64a877154831`。
  `native/package-readback-r1.json` 保留构建时点结果，manifest的commit/workflow为空；
  随后的本包原生运行见下文，未安装。
- 历史Linux失败、失败输入正控、缩窗未生效和完整平台矩阵均保留。
- 原生重启恢复、多窗口与完整供应商验收仍未完成。

## 组合包的限定 macOS 原生观察

程序 `33be2e68…` 在本地受控SSH run `3f7689e959484901bc4365d98819d8ad`
运行，保存 **53对2880×1866完整AX/JPEG及捕获时间、run和程序摘要**。
门禁、构建和打包的515项冻结输入一致，五个包成员摘要再次核对。
两条真实账号连接后分别暂停监控；25条offline记录仅作导航，没有激活连接。
以下结论以原始捕获和逐段审计共同界定，不把文件名或画面中的字符作为服务端输入证明。

| 捕获 | 实际观察 | 服务端审计与边界 |
| --- | --- | --- |
| 10–13 | 普通`x`可见但未确认提交；第一次Right无增量；Escape后可见`x`消失；随后Right仅1字节 | `x`为0、Right为0、Escape为0、第二次Right为1而非预期3。保留失败，不猜这1字节内容或IME根因。 |
| 14、16–18 | A粘贴`x`，B的Right与粘贴`x`，A fresh Right | 独立窗口分别为1、3、1、3字节，均无新增exec。粘贴不代签普通字符按键。 |
| 19–28 | 命令/Ask草稿、隐藏导航后`q`、曜石深色与雾紫浅色保存、切英文 | 该段PTY/exec均零增量；27的End与28的Down没有预期移动，不能记为完整导航成功。 |
| 29–34 | 独立Escape之后，Down、End、Home、Right实际移动所选行；End滚动到空根，Home回到Unfiled | Escape另段测量为0；之后导航段PTY/exec均为0。恢复现象不证明内部输入事件根因。 |
| 35–41 | 搜索`LOCAL SSH`，Tab后Down移动到Alpha；随后A/B分别做粘贴与Right对照 | 搜索/树段为0；A/B后对照各为1和3字节，原两条shell身份保持。分屏前累计PTY17字节。 |
| 42–44 | 左右分屏建立第三条真实shell，新B监控通过UI暂停 | 生命周期变化单独记账，建立三shell新基线；初始化监控增量另计。 |
| 45–47 | 点击分隔条再Right未改变50%；Tab显示紫色焦点后Right改为55% | 点击路径PTY增加3，不能算隔离成功；Tab路径独立段PTY/exec为0。 |
| 48–53 | 分屏中树End选中真实B；新B粘贴与Right成功；合并后保留三条会话，保存显示`Saved locally` | 树段为0，新B分别为1、3；合并/保存段为0，三shell身份保持。保存只有UI确认，本轮未做保存后的磁盘读回。 |

命令草稿 `# NAVIGATION_DRAFT_ONLY_DO_NOT_EXECUTE` 和Ask草稿
`NAVIGATION_ASK_DRAFT_ONLY_DO_NOT_SEND` 保持；没有执行命令、发送模型或安装应用。
最终live audit累计 **PTY24字节、32次拒绝的exec请求/13,440字节**，三条shell仍活且没有
shell_closed事件。夹具拒绝exec不等于产品未发监控请求；整次run不能写作PTY为0。
原始普通字符提交、输入恢复原因、完整键盘隔离仍OPEN。

应用由UI退出，`app-exit.json`为exit0/reaped；父wait监督器会话70792得到final路径，
之后执行会话已回收、原PID不存在。工具未展示监督器外层数值退出码，因此该字段留空，
不以应用退出码代填。11:40:00 UTC的cleanup/final均verified，精确容器、网络、私有目录
已清理、端口拒绝连接；11:40:39 UTC清理专用镜像，共享base保留。
回执为 `native/verification-r1.json`、`native/parent-close-readback-r1.json`；
新的非作者`native-r1`已批准限定原生证据，状态PARTIAL_OBSERVED；
53对原件、28份原始审计与22个区间重算一致，无可确证源码P1/P2。
不授予完整native或finish，也不自动签后续变化的最终文档。

该包只签原515输入。原生结束后，仅文件布局测试加入诊断；原包与原件不重新归属新测试输入。
紧凑OS窗口、完整主题×明暗×语言×尺寸、IME/VoiceOver、Windows/Linux桌面、
重启恢复、多窗口和真实供应商验收仍开放。

## 来源提交的精确CI终态

这些结果只签各自提交，不替代组合CI。B的原始日志、artifact和observer回执保留在
`work/native-pane-layout-20261105/current-ci-r2/`；父启动的exec48913已退出0并回收，
全部启动子进程已回收。旧main筛选错误的收集器已停止，空查询页保留；没有重跑实际GitHub run。
A的exec34765与75个子进程均回收，外层exit0；macOS artifact补取exec1068也exit0/reaped，
原下载失败保留。两份ZIP匹配API摘要，Linux/macOS各有11项OpenSSH通过。
原始188文件已逐摘要复制到组合材料的`source-ci-a/`，回执为
`source-ci-a-copy-readback.json`，原件未改写。

| 来源 | 平台 | Ordinary Rust passed / ignored | app passed / failed / ignored | Rustdoc | Quality结果 |
| --- | --- | --- | --- | --- | --- |
| A `72a19ea` / `42009720436/1` | macOS | 2,126 / 23 | 868 / 0 / 2 | 14 | success |
| A | Linux | 2,121 / 24 | 868 / 0 / 2 | 14 | success |
| A | Windows | 2,061 / 18 | 859 / 0 / 2 | 14 | success |
| B `9db9cbc` / `42009970970/1` | macOS | 2,158 / 23 | 919 / 0 / 2 | 11 | success |
| B | Linux | 1,089 / 7（执行止于app） | 918 / 1 / 2 | 未执行 | failure |
| B | Windows | 2,098 / 18 | 910 / 0 / 2 | 11 | success |

B的52项分屏专项在三平台均通过。Linux唯一失败为
`files::transfer_tests::workspace_layout_tests::real_file_rows_and_controls_survive_transfer_comparison_editor_and_review_states`。
日志2628–2630行报GPUI条件等待12s超时；该12s为测试等待预算，不能当作墙钟耗时。
最后布局记录为900×580、assistant=true的running-transfer；缺少最终传输phase，
当前根因UNKNOWN，不推断为磁盘容量或已确证的产品缺陷。
此失败与A旧ENOSPC分开诊断；旧失败不会被A的新成功覆盖。B的Linux OpenSSH步骤
因前序失败未执行，macOS有成功证据；Windows按工作流条件跳过。

原始材料位于 `work/workspace-integration-20261105/`。
后续实际执行后再追加命令、输入、结果、回收和独立审查回执，不提前填写通过。

## 文件布局失败的后续诊断

新的非作者诊断核对A/B/当前的16个相关文件字节一致；B日志没有失败场景的最终phase
或真实耗时，12秒为GPUI测试时钟预算。源码可定位两个测试前提缺口：未确认WRITE
期间串行测四组外观，10秒hold fallback不能保证5秒传输idle预算仍有效；UI写入
Pausing后，worker向session转交暂停请求是异步步骤，原测试随即释放WRITE，没有建立
第二次转交先于释放的保证。两者均不能直接认定为本次CI根因或产品暂停缺陷。

限定修订先补失败前阶段耗时、真实phase与等待末快照，保持原超时、断言和线程数。
候选及原诊断保存在 `file-layout-ci-diagnosis/`。仅修改测试文件，摘要
`e164ea1bb6b23d0b02b5545f75b24c3212c04cd78e5f2c897794e8f6d85c06f9`；
新增111行、替换原一次phase等待，判定仍为真实Paused及12秒原GPUI预算。

macOS arm64专项各执行一次：完整矩阵1passed，**176条/176个独立布局组合**；
四个真实上传均经过Running→Pausing→Paused，暂停时65,536字节，恢复后786,432字节
精确相等，subsystem/handle最终为0。四次running测量分别约0.848、0.679、0.415、
0.534墙钟秒；这是本次本机场景，不能补填原Linux失败耗时。
owned hold清理专项1passed，提前退出/取消均释放资源。两次执行保持jobs2、
四线程入口和同一修订515输入，exit0/reaped，无残留自有进程组。
回执在`file-layout-ci-diagnosis/execution-r1/`。
修订后的`full-gate-r2`已退出0并回收（exec36492、child93525），修订515输入前后
完全一致，较r1只改变该测试文件。**2,191 ordinary/23ignored、14rustdoc、
Python131run/1skip**，app933/2ignored；格式、严格Clippy、依赖策略及附加进程控制器通过。
回执为`full-gate-r2-readback.json`。新的非作者`diagnostic-r1`批准限定诊断源码及两次证据，
开放P0/P1/P2/P3=0，已绑定候选提交中的e164ea测试文件；没有做负控，
原失败和两处时序前提缺口不会被本机门禁覆盖。原生包仍只绑定r1的515输入。

## 候选提交与精确CI终态

`e3313722b20b0c3f5461ea8a4f194fec51c0adea` 的两个父提交为B的9db9cbc和A的72a19ea；
515项r2工程输入与23项文档r5均逐一匹配Git对象，提交后工作区干净。
推送退出0，2026-11-05 12:11:12 UTC创建Quality push run42025002190。
推送时main为77aa69c。未发布或安装；后续主线和文档提交以对应Git回执为准。

观察器仅启动一次，exec79169/PID29803，12:14:56 UTC启动、13:44:56 UTC到期，
由Aquinas保留外层执行会话。74项候选定义/来源摘要与精确提交一致，包括新增诊断后的
文件布局目标。20:40:44 Asia/Shanghai取得终态并结束收集，原期限未延长，
没有重跑或取消CI。原始API、三平台日志、解析结果及附件在`current-ci/`。

| 平台 | Ordinary passed / ignored | app passed / ignored | Rustdoc | Packaging Python run / skipped | Scripts Python run / skipped | 74项候选 | Quality |
| --- | --- | --- | --- | --- | --- | --- | --- |
| macOS | 2,191 / 23 | 933 / 2 | 14 | 57 / 0 | 131 / 1 | 全部通过 | success |
| Linux | 2,186 / 24 | 933 / 2 | 14 | 57 / 0 | 131 / 1 | 全部通过 | success |
| Windows | 2,126 / 18 | 924 / 2 | 14 | 57 / 4 | 131 / 1 | 全部通过 | success |

Linux/macOS各有11项本机OpenSSH互操作通过；两份附件分别含7个文件，ZIP摘要
匹配GitHub API，临时目录删除和所观察进程停止均有回执。Windows按工作流条件跳过，
三平台CI均不代表各平台桌面、发布或安装验收。
新增诊断的文件布局目标这次三平台通过，但成功libtest不输出内部诊断：
真实phase/timing没有被远端日志观察到，不能填写为零或解释旧失败根因。
旧B失败和两处时序前提缺口保持OPEN。

观察器在原5400秒期限内完成，75个子进程全部exit0/reaped；外层exec79169已实际
wait/reap且exit0，后续检查原PID及75个子进程PID均不存在。
`current-ci/exec-readback.json`单独记录外层退出，不以子进程汇总代填。
最终结果为`current-ci/final-readback.json`、`test-readback.json`、
`evidence-manifest.json`和`final-readback-manifest.json`。
本节状态增量发生在候选提交之后；后续文档提交保持515项工程输入不变，
精确CI仍只签e331372，最终文档另做限定复核。

## 输入补查准备

候选CI期间准备普通字符、文本提交和回车路径补查。r2开发包的构建/打包均
exit0/reaped；515项工程输入与full-gate-r2一致，五个包成员摘要与r1逐项相同，
程序仍为33be2e68。`native/package-readback-r2.json`记录该检查，r2包未执行或安装，
不能将r1原生记录改归r2，也不增加输入验收范围。

测试镜像构建owner为`5653c1ba75aa4c17a3ab43ba98e381e0`，12:25:49 UTC开始、
12:28:49 UTC结束。公开Debian包下载/安装所在的Containerfile `timeout 180s`
步骤返回124；wrapper记录`timeout=False`，并非外层240秒监督预算超时。
exec66413最终exit1/reaped。原始`build-result.json`、`build.log`、命令回执和
`context-cleanup.json`保留于`preparation/builds/5653c1ba75aa4c17a3ab43ba98e381e0/`；
失败报告为`preparation/reports/failure-4d0e45eb8cb94e1e819841dc96977fa7.json`。

构建context已删除，没有image receipt；按本次owner核对的容器和镜像均为空，
外部构建容器库存中没有本次新增项。没有启动原生run、应用进程或私有凭据目录，
没有删除其他项目容器或共享base。父回执为`native/input-followup-preparation-r2.json`。
该补查状态为**NOT_RUN**，保留输入路径OPEN；不以重复下载或放宽超时替代失败分析。
