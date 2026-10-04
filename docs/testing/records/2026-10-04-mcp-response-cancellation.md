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

所有数据、进程、内存 stream 与 loopback 都由测试拥有。macOS 上实际
stdio 进程不代表 Windows/Linux 结果；没有启动 GUI、连接客户机器、发送
模型请求、改写 installed app、运行 update helper、安装或发布。供应商
MCP 互通、实际业务和新源码远端 CI 保留独立证据边界。
