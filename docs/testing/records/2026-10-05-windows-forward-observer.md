# Windows 远端监听释放的被动观察 — 2026-10-05

状态：最终测试修正的作者95项、12次专项、严格工程门禁、根整仓及fresh独立复审通过，无剩余P1/P2。修正提交`896073a`已推送main，新Quality37238796819三平台全部成功，macOS/Linux实际OpenSSH回执与159份证据经根核验。本次测试修正的三平台源码CI边界已关闭，产品桌面/供应商MCP/Release边界保持独立。

## 原始失败与定位

提交`f09651b742ecd987e9ebe88ca5aa7e51e0bd8e8f`的
[Quality37235821726](https://github.com/cyruss648/keelshell/actions/runs/37235821726)
结束failure。macOS26、Ubuntu24.04全部成功，Windows2025的
`unknown_remote_forward_allocation_disconnects_and_releases_listener`失败。
Windows按平台实际回环suite93项，92通过/1失败，10.51秒；认证完成177.1352ms、准备/操作预算3秒，
随后打印`one bound allocated listener`，最终返回`Elapsed(())`。
原始日志、完整分平台计数、两份9项OpenSSH实际回执及132项manifest均保留，见[上一夹具记录](2026-10-05-ssh-timeout-fixture-stability.md)。

根、作者和独立审查分别核对冻结源码与阶段日志：打印分配marker之前，1秒二次bind与`AddrInUse`断言已经完成；
cleanup helper会把其超时转换为具名静态错误，生产请求使用`SessionError::Timeout`。
打印之后剩余唯一会向顶层传播裸`Elapsed(())`的await，是最后1秒`TcpStream::connect`观察。
它位于精确`Timeout("remote forward")`、`session.is_closed()`和监听lease归零的条件之后。
这是控制流与日志定位，不证明Windows内核延迟原因，也不将观察超时当成连接拒绝。

## 修正范围

只修改`crates/keelshell-session/tests/ssh_loopback.rs`最后一个资源观察：
继续先等待SSH关闭、持有真实listener future的lease计数归零；
然后在原来同样的1秒期限内，单次bind精确原`127.0.0.1:实际分配端口`，要求成功且`local_addr`完全一致，再drop新测试listener。
分配前相同bind必须返回`AddrInUse`，分配后相同端点必须可重新取得，形成对实际OS监听资源的被动观察。
IO或Elapsed继续使测试失败；没有重试、没有扩大期限，也没有通过pending TCP连接改变被测监听。

此前150ms认证准备证明、3秒操作/清理预算、响应gate、精确操作超时、SSH关闭、监听计数、关闭后exec失败断言全部保留。
共享SFTP夹具、生产transport/API/超时/依赖及应用界面均未改。
新的macOS测试只能证明本机行为，Windows仍必须由新提交的实际CI确认。

独立只读方案审查核对锁定Tokio1.53.1/Mio1.2.3的bind实现与同端点前后观察，支持该最小方案；
没有改reuse选项或将Mac结果当Windows验收。
后续独立探针需验证：即使伪计数显示0，只要真实listener仍持有端口，post bind仍须失败；
真正drop后同端点可取得。这是资源因果负控，不是仅重复计数实现。

作者强制清理自有session package并观察候选实际编译，以防切换源码时复用旧artifact。
新冻结、95项回环/各6次专项/工程门禁、fresh独立复审、根整仓、新提交及新CI结果按实际完成后追加。
旧408认证失败、旧TCP探针副作用候选、共享源中断、f096 Windows观察失败及其原manifest不覆盖。

## 最终冻结与作者结果

`ssh_loopback.rs`最终40555字节，SHA-256为
`2a4426a200e622cfe49ef0111a67722831b2bf9278886e6982340edf317295b0`；
共享SFTP夹具保持`c38f71963abcefa4ba34ec37608b67968996034efbccc7b80d339a7d8f71bf66`。
根256源码/工程freeze相对于f096 CI仅这一个测试文件变化，生产runtime、应用、依赖及打包文件保持。

| 作者最终检查 | 结果 |
| --- | --- |
| 强制重新编译的真实SSH/SFTP回环 | 95/95，7.68秒，outer34.268秒；binary SHA-256为`ad8a26cc5e1aad57acccdad3a447f94d231d2ae8ee5c6a46807007a1ba561aa2`，新endpoint rebound marker及实际dep-info核对 |
| 预定两目标各6次 | 12/12，无失败后重试；认证156.536–158.753ms，操作超时与清理3.012285–3.015207秒，每次forward均实际复绑精确端点并确认SSH关闭 |
| 工程检查 | workspace格式、x.y依赖策略、session all-targets严格Clippy通过，后者9.126秒 |
| 范围与owned进程 | 只修改最终observer与说明，原3秒操作/清理和1秒观察期限保持；owned编译/测试进程为空，0700 TMP为空，自有缓存已在根核验后清理 |

作者收到新observer具名错误建议时已实际编译，因此保持已开始检查的源码，不在冻结窗口追加编辑。
最终`.await??`继续传播Elapsed/IO为失败；只有bind成功和完整`local_addr`断言后才能打印rebound成功。
作者55份证据经根逐SHA/bytes核验，manifest SHA-256为
`b39ae6cc1c3520b62cd96fcb9a09665f21ede7175803f3d9c9cfc650805cc541`，
报告为`a419994effc87a33285186a0634bf30925e79db197483db8c44571efa3e098ec`。
独立方案审查13份材料也经根核验，但只读方案结果不替代下一个最终源码复审或原生WindowsCI。

## 根最终整仓

最终256源码/工程文件在完整gate前后hash一致，仅一个测试文件相对于f096变化。
`scripts/check.py`通过1031普通、8文档、6脚本、workspace格式/all-targets严格Clippy/x.y以及默认/显式2 MiB完整CLI控制器，
两个future均4624字节；11项默认ignored为两项供应商opt-in及九项独立OpenSSH。
gate255.009秒，日志104772字节、SHA-256为
`614ce45a64cfbf499076679bf14d8957cbb9c9812134ea58aabaff0df1526a87`。

打包及GUI/MCP生产源码未变，上一轮根57项打包与双程序build、本机9项OpenSSH及f096的macOS/Linux各9项CI互通保留在原范围，
不重复运行或改成新的执行，也不把编译迁移为新包/桌面验收。新的三平台CI将按新提交实际记录。

## 最终独立复审

独立源码/缓存在清理其自有session package后实际重新编译：95/95回环、7/7私有因果探针、workspace格式/x.y及session all-targets严格Clippy通过，无剩余P1/P2。
259个root/source-final源码/工程文件前后hash一致，探针副本只另加7项测试；私有探针不计入根1031普通数量。
探针明确证明：伪lease0不能让仍持有真实socket的相同端点bind成功；真正drop后可取得完整端点。
另外实际forward监听保持120ms直至明确cancel、取消后SSH继续可用；旧100ms准备在认证尚未完成时精确超时，gate早到许可、显式释放和10秒失败兜底均验证。

独立49份证据经根逐SHA/bytes核验，manifest SHA-256为
`af5ab94b32465f046ad1051bc3dbc25f7fa0d1956cab398a85f1b1e821f8a5a2`，
报告为`37564eb15f3a459d8f0a008061188410f818514cfa3b35a53a8058a0821161d9`。
原只读方案13份与旧f096审查62份均保持；两套新scope binary、dep-info、源码/探针及raw结果独立保留。
owned编译/测试PGID均已退出、0700 TMP为空。作者target与独立审查两个target及各自空TMP已实际删除；
根逐SHA/bytes重新核对原55/49份证据，并确认五条路径的exists/lexists均为false。
作者新增清理回执SHA-256为`41fbc656936395a7a5eadd33f569317fb5b090cf6e13a4b024beee69dd672999`，
独立审查为`a87945e5e8f8aa0f92201b003d91e960de912861fa419d7ed9f9e532e4622f68`。
原报告与manifest不重写，源码、冻结binary、dep-info、7项探针与失败日志保留；根公共target未动。
这是本机macOS Rust进程及真实回环SSH/SFTP，不能替代新提交的WindowsCI、三平台桌面、供应商MCP或Release/安装验证。

## 提交与新CI

测试修正提交`896073ad81e5f9868e6f5eb6fe8c4fcc0f7df348`以fast-forward合入并推送main。
根逐项读取提交tree，256源码/工程hash与最终gate freeze一致；`git ls-remote`核对精确远端SHA，
当次工作区clean、main/origin ahead/behind为0/0，`feature/windows-forward-observer`已非force删除。
七条提交路径为一个测试文件、五条已有文档及本记录，没有修改生产程序、依赖、打包或发布标签。
408与f096失败run和证据继续冻结。

根本机gate、256源freeze、作者/独立材料核验、cache清理、阶段诊断与提交推送核对共15份冻结材料，
manifest SHA-256为`3464a7476f7c0f1bcd786d1c2adad3ce611b0153949534c5a53efc0e2feca1cb`。
CI证据在另一个私有目录建立manifest，不向原15份冻结材料回填。

新[Quality37238796819](https://github.com/cyruss648/keelshell/actions/runs/37238796819)
精确绑定896提交，attempt1实际completed/success，三平台job全部成功；
根直接API读回run SHA/status/conclusion一致，两个目标case在三个平台均实际ok，没有取消或重跑。

| 896实际平台 | 普通通过/失败/忽略 | 文档通过 | 脚本执行/跳过 | 打包执行/跳过 | 默认/显式2 MiB CLI future | SSH回环 |
| --- | --- | --- | --- | --- | --- | --- |
| macOS 26 | 1031/0/11 | 8 | 6/0 | 57/0 | 4624/4624字节 | 95通过，8.70秒 |
| Ubuntu 24.04 | 1031/0/12 | 8 | 6/0 | 57/0 | 4624/4624字节 | 95通过，13.12秒 |
| Windows 2025 | 1011/0/11 | 8 | 5/1 | 53/4 | 4968/4968字节 | 93通过，10.23秒 |

三者x.y策略、格式、workspace严格Clippy、普通/文档测试与完整显式小栈控制器均完成；
集合与平台之间不相加。Linux额外默认忽略项为`/proc`手工验收；两项供应商测试仍默认opt-in。
Windows平台条件编译产生93项回环，不能把本机95项强加到其统计。

新macOS ARM64 artifact11317295070实际9通过、22.937秒，62个kernel birth identities；
新Linux X64 artifact11316519025实际9通过、9.269秒，59个birth identities。
独立代理及根均实际读取ZIP，digest与GitHub metadata一致，zip成员与落盘材料逐字节一致，
result/tests.log及日志字节数匹配；owned/observed stopped、temporary removed全为true，
ancestry_unverified为空、无cleanup错误。Windows OpenSSH按平台跳过，不计作通过。

新CI159份证据逐bytes/SHA与0600权限核验通过，manifest SHA-256为
`1d3048b163c543dceaf0d6473b3b2c41f326fc335b022f01d02d70580ba47c3a`，
report.md为`a8884e76ffff2b8253ec39f2d3e218683e072025c1ac0f086220588de2ee0665`，
report.json为`1858675d7ab8451d3c3d6ac92b74dfaa5a5348ae7a3548d37edb890b07247122`。
API/gh原始日志保留，running N、实际test行与summary交叉一致；本次无parser/读取失败。
原f096的132份与408的49份失败证据保持，原开发包与其源freeze不改归属。

本结果关闭单测试文件的三平台源码CI；不代表Windows/Linux GUI、真实供应商MCP工具循环、云模型、六目标正式Release、安装或自动更新通过。
