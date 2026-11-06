# 文本格式与审核式路由诊断组合 — 2026-11-06

状态：进行中。源码、工程门禁、开发包、原生、精确提交CI与发布安装分别验收。
时间使用Asia/Shanghai；原始UTC回执保留。下列`work/next-batch-20261106/`
为本地ignored原件入口，不表示公开附件。

## 最新状态

最终r3的543项工程输入、21源码路径通过完整门禁和新包检查，
Gauss的`combined-r3`批准限定源码，`combined-r4`另批准门禁和新开发包；
两份报告开放P0–P3均0，不代签后续原生。
2026-11-06 04:43:21 UTC启动的新runtime-r3已实际结束：
外层28510由父实际wait、exit0/reaped，797.330164/900秒，没有重启或延时。
38组单窗霜白浅色/英文AX/JPEG覆盖BOM+CRLF完整审核取消与确认，
完整目标/原备份字节核对，以及真实SSH UI的UDP IPv4/IPv6一跳结果。
inline编辑器底部动作可达性待修；合并采用/保存、late-save与连续零换行保存保持
NOT_RUN。新owner的container/network/image/private已清理，原runtime-r2失败/HOLD不变。
Gauss的native-r1批准已观察子集和基础设施收口，允许限定交付；
新增P2 `NR1-P2-01`保持OPEN，完整native继续HOLD。见
[原生记录](2026-11-06-editor-traceroute-native.md)。
以下按阶段保留历史事实，不把旧“待验”视为新的当前结论。

## 来源与组合

基线为已交付的`fa7c05914e0eefc0f24ab32f8af690bdc50a63ed`。
编辑器r3来自10路径/538输入冻结，SHA256为
`85e9d1332fa49828f3b3e1e3229a69e032124f0da3fc7439f7f63b1884a23d4a`；
Copernicus必要差分复审关闭三项P2，新的单片完整门禁通过。
traceroute r2来自12路径冻结，完整作者树1032文件不等于工程输入数；
Archimedes的`source-r2`批准限定源码，关闭r1编译P1和lint P2，未批准父组合或原生。

父先导入editor r3，再叠加获批traceroute r2。两片唯一公共路径
`crates/keelshell-core/src/lib.rs`以精确fa7c059为base做干净三方合并。
编辑器模块与四个公开类型、traceroute三个公开类型同时保留；
其他20个源码路径分别逐字节匹配作者冻结，范围外工程不变。

组合冻结为**543项工程输入、21个源码路径**：
`final-source-r1/freeze.json` SHA256
`6ac76fd88bbf245e49663961d8f801e4e7b8ec88478e02e1313adc208ffc68de`。
每项工程原字节另存于同目录。`parent-combined-import-r1.json`记录实际装配。
新的非作者Gauss独立复核组合、来源与后续门禁/包；审查未完成前不授予组合批准。

## 实际工程验证

共享Cargo槽在前作者明确释放后由父独占，jobs2/四测试线程。
只失效五个KeelShell项目crate，保留第三方缓存。

| 阶段 | 实际状态 |
| --- | --- |
| `project-clean-r1` | exit0、实际wait/reaped，543输入前后不变 |
| `workspace-inventory-r1` | exit0、实际wait/reaped，五个项目包均从父绝对源码目录重新编译 |
| 新增库存 | editor25与traceroute17具体名全部存在，见`inventory-verification-r1.json` |
| `full-gate-r1` | exit1、外层34218实际wait/reaped，417.610秒；测试源码缺文件级cfg触发remote-only guard，原失败保留 |
| `remote-only-r2` | 只补测试文件首行`#![cfg(test)]`，原生产、guard与断言不变；5项passed，exit0/实际wait/reaped |
| `session-inventory-r2` | exit0/实际wait/reaped，7个session traceroute新增名重新编译核对；其余库存归属未变 |
| `full-gate-r2` | exit0、外层54681实际wait/reaped，558.243秒；2355 ordinary/23 ignored、14 rustdoc、Python131/1 skip，42新增及基线204名实际pass；仍含版本流缺陷，不签最终交付 |
| `session-inventory-r3` | 修后生产Python嵌入的session从父源码重编，7新增名齐全，exit0/实际wait/reaped |
| `full-gate-r3` | 2026-11-06 04:02:13 UTC启动，04:11:49 UTC终态，575.972秒；外层5546实际wait、0/reaped；2355 ordinary/23 ignored（含app1047/2 ignored）、14 rustdoc、Python131 run=130 passed+1 skipped；42新增与基线204具体名均实际pass |
| 新包 | `workspace-build-r1`及外层88131实际wait、0/reaped，12.186秒；`package-macos-r1`与`inspect-macos-r1`均0/reaped；6成员与41资产完整读回，`combined-r4`限定批准 |
| 组合原生 | runtime-r3取得38组AX/JPEG，BOM/CRLF取消与确认、UDP双栈一跳由native-r1限定批准；开放P2和完整native HOLD保持 |

`--list`仍会执行部分`harness=false`的自有集成程序；库存命令不是完整门禁通过。
单片2338或17/37项结果不与组合测试相加，不冒称最终组合数。
最终派生汇总为`full-gate-r3-summary-r2.json`及`full-gate-r3-exec-readback-r2.json`。
首次汇总误留旧r2日志摘要、Python耗时文字及adoption标签；旧原件保留，
新的派生汇总从r3原日志读回，未重跑或改写门禁。正常应用与测试程序分别核对。
新app SHA256为`9e83d83af253ffe3c3aba29997450ea75f4c70a6dbe9311e61469228bd0158b9`，
MCP SHA256为`e29f58d15f0c26c6634033f2eb0efed62b5b71dc37fcbfbab6bbbc9d28ed78a6`；
原件入口为`native/package-final-readback-r1.json`、`native/package-assets-readback-r1.json`。
初始冻结和失败门禁不覆盖。父r2仅增加测试文件级cfg，543输入/21路径不变；
新冻结SHA256为`ec74721f1450241cd76b6909aeca7968b86d2bb1335949c85efca28e9cb11e29`，
原/新完整字节与精确前缀保存在`final-source-r2/freeze.json`。
Gauss的r1批准限定组合源码，不签未完成的门禁、包或原生；r2差分另审。

## 真实工具发现与修订中边界

父从本地固定公共Ubuntu digest新建自有工具镜像，构建期apt联网、禁止pull，
170秒TERM/5秒KILL及外层240秒原期限；原生运行期仍使用internal网络。
构建与外层90494均exit0/实际wait/reaped，75.695秒；工具query容器与构建context
已清理，公共base未变，新image按精确owner留待原生使用。
镜像为`0223b4051699eaa0b626fa0762a8bd0f7c7e12ffc7299f0ef4d0bec4e7d5c4ca`。

真实`traceroute --version`为exit0、stdout空、stderr恰好2.1.5两行文本。
r2生产及原生准备只接受stdout，不能准入这款真实工具；Dewey的r3修正为退出0、
stdout空且stderr精确等于固定版本字节，既有生命周期拒绝不变。
独立字节常量的正控及10个错误流/双流/分流/多余/截断/非零/其他版本负例通过，
Python14项实际pass。Archimedes新的`source-r3`批准限定源码并关闭CR2-P1-01；
旧r2报告与错误不回写，Gauss父组合差分和最终门禁/包另收口。

父在r2门禁实际结束后，仅叠加两个Python文件，测试cfg前缀已与作者r3同字节。
新`final-source-r3/freeze.json` SHA256为
`1b5c7cf42d8a4d9ede5de6fcca29caf11da5daeaef59fc8e580ea4a1172590bd`，
仍543工程输入/21源码路径，生产parser/argv/预算未改。Carver同步修原生准备版本准入，
并在作者离线测试中修正其独立预审所指出的journal回收与最终清理期限缺陷；
新的非作者准备复核尚未关闭这些发现，尚未启动原生。

同一image的普通UID1001、cap-drop-all、network=none只读raw工具probe实际观察：
UDP IPv4 `127.0.0.1`、IPv6 `::1`均一跳、exit0、stderr空；
ICMP IPv4为exit1与精确权限错误。外层91509实际wait、0/reaped，三个容器均退出并移除。
这只验证实际工具的输出/权限边界，不是Rust parser、SSH或GUI验收。
原件为`image-preparation/`与`raw-tool-probes-r1/`，没有外部目标或业务数据。

进一步将冻结中真实`icmp.py + traceroute.py + probe.py`及实际worker按Rust的
拼接与base64协议，在同一普通UID1001只读、network=none容器执行：
旧r2 red实际返回`unsupported_environment/unsupported_implementation`；
r3 green实际UDP IPv4/IPv6均`success/destination_responded`、一跳精确地址和RTT，
ICMP为`permission_denied`。前台28793实际wait、0/reaped，三个容器均退出并移除；
45秒总预算实际1.206秒。原件`production-probe-r1-red/`、
`production-probe-r2-green/`保留完整源码、请求、帧与数值回执。
这覆盖真实生产Python适配器，不冒称Rust响应parser、SSH或原生GUI通过。

2026-11-06本批官方registry检查34包/104要求通过；manifest/lock未改变，
直接registry要求仍为`x.y`，保留锁定patch；详见`dependency-freshness-r1*.json`。

## 原生准备与未验范围

真实SSH/SFTP最小运行候选位于
`work/editor-format-20261106/native-runtime-r2/`。旧runtime-r1和Curie三项HOLD保留。
新候选需要新不可变包绑定、
自有普通Linux OpenSSH与合成文件、真实host-key pin及新的非作者准备复核。
原先监控开发包不含本片，不用于本片验收。

基本计划为BOM+CRLF Unicode编辑、完整审核取消、重新确认、全部目标和原备份读回。
独立无BOM合并夹具用于保留新插入的头部U+FEFF。没有可信时序证据的late-save保持
NOT_RUN；可选traceroute只有在真实2.1.5工具准入后才从真实review UI验证loopback。
准备或离线测试通过不代表GUI运行、工具成功或清理完成。

### 首次真实夹具启动失败

runtime-r2的25成员冻结、新包绑定及Curie `report-r2`准备复核通过后，
父于2026-11-06 04:30:34 UTC实际运行。外层49450已实际wait、exit1/reaped，
1.112秒结束。应用未启动，桌面库存未观察到KeelShell；
四项保存/合并检查及GUI诊断均为NOT_RUN。
受限root先将0700合成目录交给UID1001，再写入seed，因没有DAC_OVERRIDE而发生
`PermissionError`。原始container stderr与exit1保留。父进一步确认，相同权限
也会阻止root观察器读取0600文件或执行固定竞争写，须一并修正。

14条工作阶段host命令均数值退出且回收；夹具容器和网络已按精确归属移除，
但非正常container退出触发清理HOLD，`verified=false`、image/private保留。
原r2冻结、执行标记、900秒时钟、失败与清理报告均不覆盖或重启。
Carver正准备独立runtime-r3，仅调整seed创建顺序，并在root读取控制元数据后
将合成文件观察/固定竞争写永久降为UID/GID1001；不增加cap或放宽文件权限。

新尝试使用独立owner `01e8d214a619471b8ad93786b6083afe`。
同字节Containerfile及既有有界builder的新image构建已实际wait、0/reaped，
32.387/240秒、9条命令均0/reaped；query容器/context清理，公共base保持。
image ID为`c8bb344eb234d3c6b7309e7f149f7aac05f99f05456b5993e85f4814079384c2`。
原件在`image-preparation-r2/`。新的源码差分、最终绑定复核和真实原生仍待验；
新构建不改变旧失败结论。

Windows/Linux真实桌面、完整主题/语言/尺寸/键盘/IME/辅助技术、多窗口、
供应商账户、完整外部MCP及签名安装继续开放。上一批fa7c059的三平台CI
与本片组合工程、原生互不代签。整个产品目标保持进行中。
