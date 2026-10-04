# MCP 错误响应取消修复 — 2026-10-04

基线 `37ba14d0ec23ab6b6b1c280af86425a8aef42016`，分支
`feature/mcp-response-cancellation`。复用干净 managed Windows checkout，
保留 `be9590f2` 分支和旧证据；未改根或已冻结 companion worktree。

## 原失败与机制

[Quality 37210745161 Linux job](https://github.com/cyruss648/keelshell/actions/runs/37210745161/job/111461250972)
在 `malformed_shapes_and_unsupported_actions_have_bounded_protocol_errors`
的 3 秒 read 超时；同次 Windows/macOS 成功不改变整体失败结论。
原完整日志复制到 ignored `work/mcp-response-cancellation/` 并实核 SHA-256
`a563e0dc16cfb0018e5031ea77efcff11dd776ff97e780eeb8f52600eff1602d`。

根当前源码的 40 次/4 并发复查有一次 3.012 秒超时，原 receipt/probe 保留。
作者轻 instrumentation 仅在 timeout 打印 last ID，使用根真实 binary 与
原 3 秒期限，200 次/4 并发的 case129 于 3.039 秒失败，确定为 ID 4；源、
失败日志和完整结果保留。没有扩大期限或降低并发。

真实锁定 `rmcp 3.5.0` 的 standalone 确定性探针：旧 ID3 send 占有 writer，
receive 消费 ID4/method9 后等待输出锁，被取消；释放旧 send 再次 poll 后
只输出 ID3/-32602，预期 -32600 永久丢失，断言 exit101。最初 harness 的
Tokio rlib 身份和 ID 构造编译错误日志也保留；修正 harness 后的真实 SDK
运行失败才是产品证据，没有改 dependency 或 SDK。

## 实现与本地验证

[ADR0042](../../adr/0042-mcp-message-ownership-during-receive-cancellation.md)
记录两个拥有的 I/O task、有界 queue、官方 codec、flush acknowledgement、
原 budget、EOF 与 AEAD cancellation 和共同 2 秒收尾；没有额外 underlying
shutdown。五项确定性 unit regression 覆盖排队/部分写入时接收取消、随后
有效请求、EOF、满 queue 和 task 析构，并检查 budget 与 shutdown 调用。
新增真实进程回归用四个并发客户端各收齐 24 条混合回复，核对全部正常 ID、
固定错误、无重复和私有标记；另保留 initialize 前 EOF 的静态 startup failure。

最初 MCP64普通+1doc、首轮整仓门禁，以及原场景400次/4并发、新混合场景
40次均通过。之后补充 reader 先分类 EOF/失败再发布连接取消的顺序和
startup EOF 回归，最终源码重新完整验证，早期成功日志保持独立。

最终 `scripts/check.py`：971 普通测试、8 doctest、6 Python script 通过，
格式、strict workspace all-targets Clippy、x.y policy 通过；完整默认 CLI
控制器及追加 2 MiB 控制器均通过。MCP 65 普通+1doc 包含其既有认证 IPC、
方向 AEAD、撤权、背压与洪泛测试。10 项外部配置测试忽略：8 项 OpenSSH，2 项实际安装版供应商 CLI。
`whole-gate-final.log` 与 `gate-summary-final.json` 保留完整结果；上游
`block 0.1.6` future-compatibility warning 不作为新增 strict-Clippy 失败。

`process-stress-final.json` 绑定最终 MCP binary SHA-256
`9e2aad9974f2ff13e267a9defb17529f7729231412c804fbad6d5ab846fcf2d9` 与测试 binary SHA-256
`cf00e8ac5eea5f4f825b5de69bd3609e983887c2e2cdb87038d11d3eee094eaa`：原错误场景200次/4并发无失败，最长0.016秒；
四child混合回复40次/4并发无失败，最长0.043秒；startup EOF200次/4并发无失败。
三种场景全部保留原3秒内部read和外层有界执行。47项本基线打包回归也通过；
本分支没有companion打包改动，不把它写成root后续57项包的验收。

`cleanup-final.json` 确认此checkout的owned MCP进程0、private TMPDIR空，
压力期间3个源码hash未变，最终binary hash匹配。9个源码/config/doc文件及
独立patch已在`review-ready-source.json`冻结；原失败日志与成功receipt各自
有SHA-256。新源码三平台 CI待完成；此分支未push或merge。

## 新的独立复审

2026-10-05，全新 reviewer 在未参与实现或方案设计的情况下完成复核，
无可复现P1/P2。9个文件的前后hash、staged tree
`94053dbdb2fa284ca32fb6ae375436b27166cf5d`、patch与冻结receipt一致，
未修改源码、文档、index或根checkout。ignored
`work/mcp-response-cancellation-independent-review/review-report.md` SHA-256为
`379b41e6b475b2ce6d6f1b58860b8944fe3f2c4caea661a792801962a303f1cd`；
原审核receipt为`review-receipt.json`，后续状态文档delta另行核对。

Reviewer独立重编译真实旧SDK probe，预期exit101重现同一丢响应机制；
targeted fmt、strict all-targets Clippy及MCP65普通+1doc通过，含五项新unit
回归。自写直接stdio harness在原3秒read、4并发下完成原场景200次、
混合24回复40次、startup EOF80次，均无产品失败；原场景最大case0.114秒、
最大read0.100秒。最终320个owned子进程全部reap，private TMPDIR空。

首次自写harness错误要求ID-less错误显式带`id`字段，收到正确-32600后
触发KeyError；修正为与原Rust测试等价的缺字段/null语义后通过。初版
harness、320份初始运行结果与诊断全部保留，没有把harness失败写成产品
timeout。独立package test重新构建的实际压力binary SHA-256为
`3c8dbf75eea3114bd37e8da6299619a05b404c2b060452d330bf4a1e63203744`，
与作者整仓构建binary分别留证；源码hash始终未变。独立复审未重复整仓
门禁，也未执行GUI、真实安装、供应商客户端或客户SSH操作。

## 证明边界

### 根整合 — 2026-10-05

根`5afdc8a`整合修复`93f35cae615a97340be67c8f78c03c473b0e296d`，
没有冲突，四项生产/测试文件SHA-256与独立通过的冻结版本一致。
根完整`python3 scripts/check.py`通过988普通测试、8 doctest、6脚本、
默认及追加2 MiB完整CLI控制器、格式、strict all-targets Clippy和x.y策略；
10项外部配置测试保持忽略。含companion的57项打包测试全部通过。
日志`work/mcp-final-integrated-gate-20261005.log`的SHA-256为
`912c9a9976334b48a998c0fad21332250693c75ca7c7e9b86d948692312fe448`。
门禁时staged tree为`6b23b350ed6cf584fea567ffdff76be590620216`；随后只补本文与
交接中的证据状态，生产/测试源码未变。最终native包及远端CI单独追加。

归档前，根将该checkout全部2108个ignored work regular files（44,575,120 bytes）
复制至`work/mcp-response-cancellation-archive-evidence-20261005/`并逐份SHA比对；
包含旧Windows失败/成功证据、原SDK失败、harness错误、作者与fresh审查记录。
`preservation-manifest.json` SHA-256为
`30a3ecbd3045db30fdea11b844f87de027eaea0db8177b1478b163a700b9a421`。

SDK作者与独立reviewer的验证中，所有数据、进程、内存 stream 与 loopback 都由测试拥有。macOS 上实际
stdio 进程不代表 Windows/Linux 结果；没有启动 GUI、连接客户机器、发送
模型请求、改写 installed app、运行 update helper、安装或发布。供应商
MCP 互通、实际业务和新源码远端 CI 保留独立证据边界。


### 标准 macOS 包的首次原生尝试 — 2026-10-05

根使用同一门禁源码以`MACOSX_DEPLOYMENT_TARGET=15.0`构建GUI与MCP，
由标准`package.py --mcp-binary`生成双程序`.app`；两者架构、依赖、最低
系统版本及六文件归档清单检查通过。开发包`build_commit:null`，未安装或
发布。ignored `work/mcp-final-native-20261005/bundle-receipt.json`绑定：

- GUI SHA-256 `59879b62e9ff05058a8c3459ab23e1416d8e6467da0c4595c32843bde1e8b655`。
- MCP SHA-256 `1e972b92f24b4d2be5ddf6e46a35c5b3a8930c88171e1009949e07bbad5cdbdc`。
- ZIP SHA-256 `36ade8b763c0f2ab1e67006a9a560af14668024e99af90f6500721f6c6a5fc13`。

中文/暗色的隔离数据只含自有loopback SSH夹具。真实GUI完成认证、
核对指纹、37字节终端选区、六项能力及canonical `/`授权；OS复制的启动
配置仅经masked辅助GUI传入独立stdio进程内存。客户端实际发现七项工具，
读回指定37字节文本、七条SFTP目录和70字节含中文文件；未授权监控返回
FORBIDDEN；精确短命令提案到达PendingReview。

首次原生窗口中，命令卡的批准/拒绝行在固定底栏下方，wheel操作未见
正文移动；独立GPUI geometry/wheel检查尚在进行，暂不断定原因。客户端
等待人工操作的240秒期限到达，未执行命令；这是未完成的UI验收，不是
已证明的旧SDK丢响应机制，也不把读取成功记成批准流程成功。原失败
client receipt、console与app日志保留，后续修复/重试分别另存。

owned monitor于900秒期限终止GUI（exit -15），fixture正常退出0并删除
临时远程树；客户端adapter退出0。辅助GUI随后通过Cmd-Q退出。
`cleanup-receipt.json`确认五个owned PID不存在、夹具监听消失、private
TMPDIR为空，profile不含临时MCP能力或密码。新的批准/拒绝/运行中撤权、
重启默认关闭和Windows/Linux原生仍需分别验收。


### 原生复核与首条提案回归 — 2026-10-05

第二次标准包另存`work/mcp-final-native-pass2-20261005/`。源码提交
`34cff1b`的生产文件与前次门禁/首次包相同；再次标准构建、双程序打包、
原生结构检查及六文件归档检查通过。GUI/MCP SHA与首次包相同，ZIP
SHA-256为`fe7122e091569a7592db3448780637eb44ee17367eec8c0f49b0dc385e9c98e3`；
仍为开发包（build_commit:null），没有安装或发布。

本轮GUI/真实SSH/SFTP/独立stdio完成前次所有读取检查、首次精确命令
批准成功并显示中文stdout/stderr、第二提案人工拒绝、第三挂起提案
Running后关闭授权。GUI显示OutcomeUnknown且不含输出，旧stdio失效
（adapter exit1）；这不证明远端进程停止。之后浅色/英文下重新只授予
ListSessions，Cmd-Q退出、同一隔离profile重启，确认默认关闭、没有
授权/选区/提案，复制与授权按钮禁用，英文/浅色设置保留。

本次CUA在正文发送`down`时处于顶部未移动；发送`up`时正文向上移动，
精确命令和审批/拒绝按钮完整进入视口，随后实际点击完成流程。这是
此次原生自动化输入的观察，不推导系统滚轮设置或所有用户设备行为。
独立GPUI探针亦否定零滚动范围假说，没有修改生产布局来处理未证明
缺陷。首次240秒未完成的尝试仍保留，不覆盖其receipt。

独立只读探针在冻结副本上使用既有grant按钮及后台canonical校验，
验证37字节实际read_selection与`/approved`授权。六/七项工具、中文/
英文、1440×900/900×580形成八种state；32项原始MouseMoveEvent与
ScrollWheelEvent覆盖正文中央/左空白边缘、Pixels/Lines单位，要求
审批按钮实际位移并完整进入正文、固定footer不动。该夹具仅验证
SFTP会话布局，无SSH执行，不代替上方combined原生业务。最终receipt
SHA-256 `3f2dd6d14b0491619c3ef5844d51da3680f914352ab454bc42394343c56ed0d9`，
最终log SHA-256 `721b87224f9f48524655cd978ee9c2f6581f98bef1c5671da8f54d7fb0d7e638`。
探针准备期间四份失败log、初版helper说明和最终正常按钮setup分别保留。

根将有意义的首条短提案场景整理为独立测试module；只新增test覆盖，
无生产源码变化。新的独立审查与最终门禁结果另行追加。原生owned
GUI、restart、Probe、client、adapter、fixture均退出，fixture root删除、
监听消失、private TMPDIR为空，profile无临时MCP能力/密码，见本轮
`native-flow-receipt.json`及`cleanup-receipt.json`。

整合提交已推送开发分支，远端exact ref核对及ahead/behind 0/0；
[Quality 37217653868](https://github.com/cyruss648/keelshell/actions/runs/37217653868)
已在macOS/Ubuntu/Windows全部成功，macOS/Linux独立OpenSSH互通也成功；完整CI日志另存并校验，不代表Windows/Linux原生GUI。SDK author worktree已可恢复
归档，全部2108份ignored证据已SHA复制保留，已合SDK本地分支删除。
供应商MCP客户端、Windows/Linux原生窗口、六目标Release、签名/
公证与实际安装更新继续保留验收边界。

根新增test整理的两次编译失败（TestAppContext读取方式、平台事件所有权）
分别保留targeted与targeted-repaired日志；修正后专项通过1项，最终回归
source重新冻结，不作为生产transport失败。


### 新增回归的最终根门禁与独立审查 — 2026-10-05

冻结基线`34cff1bd6607dc1c15a2048ae724928ae99ea267`，四文件staged tree
`c21c023b7b267367804306874d711c49991dade0`；新增回归source SHA-256
`20260f553f1322fa2192dbf36326623ce6afff88b8acbd92aa1290e12cca8abd`。
生产源码不变，只补测试与验收记录。最终根完整workspace门禁通过989普通、
8文档、6脚本测试、默认及额外2 MiB完整CLI控制器，格式、全工作区all-target
strict Clippy与x.y策略通过。10项外部配置测试未提供环境而忽略；不以此
代替供应商CLI或OpenSSH业务验收。门禁log SHA-256：
`799c9759a3acf9cb476e33874b5077e2b05d57fbdb934f93e5d68a5dee863f44`。
打包源码未变化，沿用此前57项本地打包回归，不重复计入Rust测试数。

新开独立reviewer未参与实现或原探针，完整导出并冻结429份index文件，
验证前后SHA一致；独立source/CoW target上MCP UI专项9项通过（正文6.88秒），
格式、app all-target strict Clippy及x.y通过，private TMPDIR为空；没有
可复现P1/P2。其新委派只读证据reviewer核对18项probe evidence、32原始
事件/8种geometry、标准包/原生flow/清理receipt，也无P1/P2。报告SHA-256
`7f54baa51f48b8fb375013541d8e8cfaada1faf9049a9f8a293434f104c37456`；
receipt SHA-256
`1fea49e1e17eacebe3fad2e8d2db1e6209721e9265504cd6152c1725375468a9`。
未把独立专项或probe当作原生OS输入或新一轮整仓检查。上游`block 0.1.6`
future-compatibility notice保留，不是strict Clippy失败。

生产提交34cff1b的Quality37217653868完整日志SHA-256
`b98e220f4a5c8675aab6cf50a261cd1e2131d5a0da7e4e5a9e737e61277fa372`：
macOS/Ubuntu各988普通+8doc，Windows968普通+8doc；两种完整CLI控制器
三平台均成功。打包macOS/Linux各57通过，Windows53通过/4个Unix条件跳过；
脚本macOS/Linux6通过，Windows5通过/1条件跳过；macOS/Linux各8项独立
OpenSSH成功，owned进程/临时数据清理成功。此CI基线没有本次新增回归，
新增源码提交的远端CI另行记录；Windows/Linux原生GUI、供应商MCP与正式
发布/安装边界保持不变。根与reviewer完整失败和成功日志均在ignored work保留。
