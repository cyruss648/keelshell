# 远程文本UTF-8 BOM与换行保持测试记录— 2026-11-06

本记录区分隔离源码、完整工程、新包与原生验收。
以下`work/editor-format-20261106/`为本地ignored原件目录，不表示公开附件。
时间使用 Asia/Shanghai；原件中的 UTC 时间保持原样。

## 当前父导入续记

作者下列“父536未变”“尚未导入”描述对应各隔离验证的实际时点。
父在traceroute作者明确释放Cargo槽后，于2026-11-06 03:26 UTC将获批r3的10路径
导入开发分支，538项工程输入与作者冻结逐字节匹配。03:34 UTC又叠加已获非作者
源码批准的traceroute r2，最终543项工程输入、21路径；共享`core/lib.rs`以fa7c059
为基线做干净三方合并，同时保留两片导出。父已使五个项目crate产物失效，
新库存包含编辑器25项与traceroute17项具体新增名；最终组合完整门禁已实际0/reaped，
2355 ordinary/23 ignored（含app1047/2 ignored）、14 rustdoc、Python131/1 skip，
42新增与基线204具体名实际pass，新包构建和macOS包检查完成。
这些后续状态没有回写作者r2/r3冻结、旧失败或独立报告。
Gauss的combined-r4已限定批准新包和工程原件。
同包runtime-r3已取得BOM/CRLF完整审核取消和确认的真实macOS观察，
目标29字节与唯一原备份22字节独立核对；inline合并操作可达性待修，
merge-adopt/save保持NOT_RUN。限定非作者原生复核与未验范围见
[原生记录](2026-11-06-editor-traceroute-native.md)，组合见
[组合记录](2026-11-06-editor-traceroute-integration.md)。

## 来源与范围

基线为 `850b77b77b576f3bd2527883ffef9d97a666afd0` archive，加父18个冻结源码，
共536工程输入。实施只发生于 ignored `implementation-source/` 无`.git`副本；
r2与r3均为538工程输入、10个改动路径（8个既有、2个新增），r3只修订其中4路径。
父536、父Rust、正式docs、Git、依赖及不可变package-r2未由本片修改。

- 当前r3冻结与10路径摘要（本地`work/editor-format-20261106/freeze-source-r3/freeze.json`），SHA256
  `85e9d1332fa49828f3b3e1e3229a69e032124f0da3fc7439f7f63b1884a23d4a`；
  完整补丁（本地`work/editor-format-20261106/editor-format-source-r3.patch`） SHA256
  `41cc313db5e85e49a0503523a41fbeabda2349eba951a26c873ebf9119cfce33`。
- 历史r2冻结与10路径摘要（本地`work/editor-format-20261106/freeze-source-r2/freeze.json`），SHA256
  `7ccdddf2931af98692ec2e5741db60c912fe2195c42e313a70242154c9cc8ac7`。
- 可导入补丁（本地`work/editor-format-20261106/editor-format-source-r2.patch`），SHA256
  `0eb613bd6329fe61a46e8098386a522cd554c3c6a85ce0b157c25920e909deaf`；
  独立基线副本实际应用exit0、10/10摘要匹配，未导入父树。
- 专项完整命令、逐名通过与回执（本地`work/editor-format-20261106/verification-summary-r2.json`）；
  作者r2回执（本地`work/editor-format-20261106/handoff-final-r2.json`）保持当时事实，不回填新门禁结果。

## 专项真实结果

| 范围 | 结果 | 实际命令回执 |
| --- | --- | --- |
| core文档格式 | 7 passed / exit0 | `verification-r1/core-text-document-r2.json` |
| app格式 | 10 passed / exit0；已含于下列23条，不重复累计 | `verification-r1/app-editor-format-r1.json` |
| app冲突合并 | 23 passed / exit0 | `verification-r1/app-conflict-merge-r2.json` |
| app搜索/patch | 4 passed / exit0；与冲突组交集1条 | `verification-r1/app-editor-search-r1.json` |
| session审核保存 | 9 passed / exit0 | `verification-r1/session-editor-reviewed-r1.json` |
| core/app/session all-targets严格Clippy | exit0 / `-D warnings` | `verification-r1/owned-clippy-r2.json` |
| ownedfmt check | exit0，仅10路径，`skip_children=true` | `verification-r1/owned-rustfmt-check-r2.json` |

专项去重 **42 passed = core7 + app26 + session9**，包含全部 **21个新增**：
core7、app11（10个GPUI/loopback与1个搜索/patch）、session3。
覆盖BOM与内部FEFF、LF/CRLF/混合/CR、实际Enter、最终字节/行预算、
格式变化重读（含远端恢复）、取消零写、审核stale、较新草稿保留、完整读回与布局。
GPUI三主题/双语/两尺寸测试不是实际桌面、辅助技术或IME验收。

专项Cargo均jobs2、4测试线程，共享target。普通验证每命令固定最大895秒
（880秒执行+15秒清理）；项目clean固定最大135秒；ownedfmt最大75秒。
每条均真实wait/reaped，实际秒数与数值exit保留在JSON。

## 首失败与归属证据

`app-conflict-merge-r1`为22过1失败、exit101：旧测试期望原LF文件保存为CRLF。
仅修测试段，保留草稿CRLF并新增精确pending LF/raw review、确认前零写与保存后
LF/clean断言；23条重跑通过。Clippy首轮exit101的12个新core测试unwrap违规改为
带上下文helper；未放宽lint、断言或期限。r1→r2生产字节不变，旧日志与冻结保留。
早期dep-info绝对路径证明脚本失败和app linker warning也保留。

首次切换到副本前只使5个项目crate失效，4279第三方fingerprint目录保留。
已逐名核对app1039/core193/session153清单，包含本片21名与基线knowledge14名；
knowledge14在专项阶段仅核对清单，没有另跑整套专项。
二进制SHA、dep-info与绝对编译目录分别保存，不能以源码摘要/exit0代替inventory。

## r2完整门禁续记

<!-- FULL_GATE_STATUS_BEGIN -->
r2原冻结时完整门禁未run，原回执保留。父后续明确重新授权独占Cargo槽，
`verification-full-r1/` 在同归属r2上只启动一次 `scripts/check.py`；
原起点为2026-11-06 09:57:25.413788，1500秒总预算包含清理，不延长期限。
三份新清单命令均exit0、wait/reaped；准备执行器的session日志解析失败exit1已保留，
未运行门禁前发生，修正后沿用原绝对deadline继续，无重复inventory命令。
gate与外层92709均实际wait/reaped、数值exit0，10:09:34.641885结束，
从原起点总729.228秒。2334 ordinary passed/23 ignored，app1037/2 ignored已含其中，
14 rustdoc；Python131 run=130 passed+1 skipped；依赖策略、workspace严格Clippy、
全仓格式检查和三个附加控制器通过。21个新增名全部实际pass，父536/副本538前后不变。
后处理曾误选后续MCP零测试main汇总，首次断言失败也保留，修正按app二进制身份选择；
没有重跑或改写gate。见完整实际汇总（本地`work/editor-format-20261106/verification-full-r1/gate-summary.json`）。
该轮仍是r2，独立审三项P2不因完整门禁exit0而关闭。
<!-- FULL_GATE_STATUS_END -->

## r3三项P2修订与受影响验证

r2独立审原报告（本地`work/editor-format-20261106/independent-review/source-r1.md`）为REQUEST_CHANGES，P0/P1零、
P2三项。三条 `independent_editor_format_r1_` GPUI回归在修复前真实运行，
0过3失败/exit101，见原日志（本地`work/editor-format-20261106/verification-r1/app-independent-review-red-r1.log`）。
之后只修四个自有源码路径：core文档类型、app保存采用、合并采用和格式测试。
较新草稿与格式配对、无BOM合并不误删新增头部FEFF、无换行保存保留CRLF策略，
均覆盖真实两次保存/完整目标与备份、一个/两个头部FEFF、内部FEFF、空版本、
BOM有无与保存期间新增草稿。另增一条core策略矩阵测试。

| r3受影响检查 | 实际结果 |
| --- | --- |
| 三条独立GPUI回归 | 3 passed / exit0，已含在冲突组 |
| core文档格式 | 8 passed / exit0 |
| app冲突合并 | 26 passed / exit0 |
| app搜索/patch | 4 passed / exit0，与冲突组交集1条 |
| core/app/session all-targets严格Clippy、10路径ownedfmt | 各exit0 |

本轮受影响专项去重37条，所有回执538摘要与r3一致、均实际wait/reaped；
session专项没有为此无关测试fixture改动而重复运行。新增源码累计25条
（原21+新core1+新app3），本轮四个新增均实际pass。
新库存app1042/core194，三个具体独立回归与core新增名均已核对。
见r3验证汇总（本地`work/editor-format-20261106/verification-summary-r3.json`）、
r3冻结（本地`work/editor-format-20261106/freeze-source-r3/freeze.json`）、
四路径最小差分（本地`work/editor-format-20261106/editor-format-r2-to-r3.patch`）。
完整补丁已实际应用基线副本exit0、10/10摘要一致，538仍未导入父536。

<!-- R3_FULL_GATE_STATUS_BEGIN -->
Copernicus的正式差分审（本地`work/editor-format-20261106/independent-review/source-r2.md`）关闭三项P2，
限定r2→r3差分新增P0/P1/P2/P3均零，批准该冻结进入新的完整工程门禁。
`verification-full-r2/`在同归属r3上只启动一次`scripts/check.py`，未再次clean；
先核对现有二进制与dep-info归属，再新列app1042/core194/session153库存，
本片25名与基线knowledge14名齐全。

原起点2026-11-06 10:35:44.874332，总预算1500秒包含inventory与清理，
绝对deadline为11:00:44.874332。gate和外层61769均实际wait/reaped、数值exit0，
10:45:25.876496结束，总580.999秒，未延长期限。
2338 ordinary passed/23 ignored，app1040/2 ignored已含其中；
14 rustdoc、Python131 run=130 passed+1 skipped、依赖策略、全仓格式、
workspace all-targets严格Clippy及三个附加控制器通过。
25个新增名全部实际pass，父536与副本538前后不变。
见本轮汇总（本地`work/editor-format-20261106/verification-full-r2/gate-summary.json`）及
逐名执行结果（本地`work/editor-format-20261106/verification-full-r2/new-25-results.json`）。
作者已明确释放Cargo槽（本地`work/editor-format-20261106/cargo-slot-release-r2.json`），后续未再运行Cargo；
下一个源码归属须先只失效五个项目crate并核实新测试库存，保留第三方target。
原r2报告与门禁保持历史事实，不回写为r3。
<!-- R3_FULL_GATE_STATUS_END -->

## ACK、读回与未验边界

测试独立读取合成备份并断言完整原始字节；生产只要求备份写入成功ACK后才发布
目标，没有独立完整读回备份。目标发布ACK后app仍完整读回全部字节，逐Vec比较，
只有完全一致才采用保存基线。同长末字节变化回归证明ACK/长度相等不能代替全量
目标读回；失败保留旧基线/草稿与已完成备份，不自动重放或回滚。
这不承诺备份崩溃耐久性、远端CAS或owner/ACL/特殊mode保持。

Copernicus `01a1ae8e-64fe-7370-8701-bcea30304ece` 的r2审请求修订记录保留，
r3必要差分复审已关闭三项P2。作者交付时538尚未导入父536，后续父导入见本页开头续记。
新包构建已由父在最终543输入组合上完成；macOS限定BOM/CRLF原生观察见开头续记，
完整编辑格式、Windows/Linux桌面、IME/辅助技术、安装/签名/发布仍待验。
父536既有gate/package不签本片538或后续组合543。
