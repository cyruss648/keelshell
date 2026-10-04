# SSH 超时夹具的阶段与资源清理 — 2026-10-05

状态：最终夹具的作者验证、根整仓、本机OpenSSH及新独立复审通过，无剩余P1/P2。修复提交与新CI另行追加；原408的远端失败保留。

## 原失败与范围

生产提交 `40824f00ae9385f380b36c0abecf41b043c11a4e` 的
[Quality 37232614315](https://github.com/cyruss648/keelshell/actions/runs/37232614315)
结束为 failure：Ubuntu/Windows 成功，macOS 的
`directory_limit_failure_and_timeout_release_remote_handles` 在
`SSH connect/authenticate` 返回 Timeout，尚未进入其目录操作与清理断言。
原套件为94通过/1失败，workspace退出101；macOS后续文档、小栈和OpenSSH步骤没有完成。
原日志、分平台实际数量与49项哈希核验保留在[整合记录](2026-10-05-workspace-workflows-integration.md)。

原测试将100ms同时用于SSH准备和SFTP请求；另一远端端口分配超时测试采用相同准备方式。
连接限时是产品的正常行为，本次修改仅针对测试应先进入受测操作的前提，不更改生产超时、API或重试规则。

## 候选修复

两个目标场景使用夹具既有的3秒设置完成连接，密码认证引入可控150ms延迟，明确超过旧100ms准备限时。
目录读取与远端监听分配使用仅在目标测试启用的响应 gate；测试观察真实READDIR/目录句柄，或监听器生命周期计数为1且二次bind返回`AddrInUse`，之后才接受精确操作超时。
端口分配未确认期间不发起TCP连接探针；只有SSH已关闭、拥有监听的future释放且计数回到0以后，才验证连接失败。
超时后释放gate，并有界验证句柄/监听清退、共享SSH可继续读取或不确定远端分配后的SSH关闭状态。
gate本身有10秒失败兜底；其他夹具的默认响应保持原有行为。

作者在独立前修复源码中仅注入150ms认证延迟，两个旧100ms场景均确定性在连接阶段失败。
这是同一失败阶段的受控复现，不证明GitHub当次调度的具体耗时或操作系统原因。
第一次候选全套运行错误复用了前修复源码的编译缓存，未执行新阶段日志；原失败与artifact/dep-info保留，
该运行不作为新修复结果。后续使用独立缓存或清理自有package，绑定实际编译、源码、dep-info与二进制。

## 保留的候选与中断证据

第一版gate候选虽通过95项及目标各6次，但其pending TCP连接探针可能触发服务器向尚未登记的转发通道请求，
被客户端拒绝后自行结束监听循环。因此这些结果不能证明操作超时造成监听清理，均保留为superseded候选，不能用于最终验收。
最终版本让生命周期lease随真实listener future移动，future即使在首次poll前被abort也会释放计数；
二次bind只观察已有监听，不通过accept路径改变被测状态。

作者撤销冻结后，在根确认停止前修改了共享测试源；根停止了自己拥有的进程组53155并确认该组为空。
原根`final-gate`回执exit=-15、264.442秒，日志SHA-256为
`ec8c35c209e022410f7e46e46b39a8c2c5bb571ca3a035327f03a84cab69b345`。
它明确归类为source-scope interrupted，所有中途通过项均不计为最终结果，而非把它改写成完整通过或产品测试失败。
同步运行的57项打包检查通过，其输入未被这两个session测试文件改变；结果仅按未变化的打包范围保留。

最终冻结`ssh_loopback.rs`为`326a5de599633fdc2c1175082670b5d33d7e5252cba12530d03a7a4f3e947864`，
共享SFTP夹具为`c38f71963abcefa4ba34ec37608b67968996034efbccc7b80d339a7d8f71bf66`。
作者新编译版本95/95通过，目标各6次均观察150ms认证完成、精确操作超时及资源清理；
新独立复审和根整仓已重新执行通过，不引用旧候选的复审结果。

## 最终源码的本机验证

| 检查 | 结果与归属 |
| --- | --- |
| 作者完整真实SSH/SFTP回环 | 95/95，测试7.64秒；明确重新编译，binary SHA-256为`ef71504aa86363170f6234d3b16931cbcdf1e398611b9df055c3228fee97e4e8`，三个最终阶段marker及dep-info核对 |
| 作者预定专项 | 两目标各6次，共12/12；认证157.832–158.953ms，操作超时加清理3.013344–3.017016秒；没有失败后追加重试 |
| 作者工程门禁 | workspace格式、x.y策略、session all-targets严格Clippy通过；无作者owned Cargo/test进程，0700 TMP为空 |
| 根最终整仓 | 1031普通、8文档、6脚本全部通过，11项普通ignored为2项供应商opt-in与9项另跑OpenSSH；195.249秒，格式、workspace all-targets严格Clippy、x.y及默认/显式2 MiB完整CLI控制器均通过，两个future均4624字节 |
| 根打包 | 57项通过，1.702秒；打包源码未被本次两文件修改改变，按该独立未变化范围使用 |
| 根本机系统OpenSSH | 9项通过，22.478秒；自有回环SSH/SFTP/exec互通，owned/observed清理与临时目录移除通过、ancestry_unverified为空，60个累计birth identity；不代表客户主机或桌面验收 |
| 根GUI/MCP构建 | 两程序native build通过，32.370秒；只证明构建，不新增开发包或桌面操作验收 |
| 新独立复审 | 无剩余P1/P2；259个源码/配置在root、冻结副本及前后保持；最终95项回环、6项独立copy-only探针、格式/x.y/session all-targets严格Clippy通过。私有6项不加到根普通测试数量中 |

根冻结256个源码/工程文件，最终门禁前后全部hash保持；相对于原生UI开发包快照只有这两个测试夹具不同。
最终整仓日志SHA-256为`13b3f728b0b5390018f29692b12519fc8e39bb47eb3e73d1497a5e59cf80a8cf`，104354字节；
打包日志为`71eb2374f862c2a894aeda8cd84be8648a02c6abd7068f2d07a43f71ffb9e83f`；
OpenSSH外层回执日志为`f1a7fc7dd3d2d545a02a075f490a682016f46c9e9dc41c15387c4a3287c96302`，实际9项和清理由其`root-openssh/result.json`及tests日志证明。
作者95份证据经根逐项SHA/bytes核对，manifest SHA-256为`daec6e999d8cebcffa20f5c2d886d346f8a355489af513447d150a5abf96cd17`，
report为`f0a0892dade7d82382078c4875f1c620018e3ffaa6d58d2d552ead78b8780db0`。

本次根构建GUI SHA-256为`26ad9e962f4ebb9ec37b186db89748f9e34f42595f997db22ed3555737180e58`，
MCP为`069bbb239b5382f5c5203172ffd9fe080f7dc124c1fe1ab2de2e7d2a3477f53d`；
构建日志SHA-256为`ed041714f3775986043b8632d351a765169c4308a1f9b1b6fa3d0f7b7ab95964`。
没有把本次构建GUI与旧原生包字节等同，也没有沿用旧包的commit或签名/安装结论。

新独立探针证实：bind观察120ms不关闭原监听，正常确认后的显式cancel释放lease并保留可用SSH；
gate保持阻塞直到显式释放、提前释放许可不丢失，未释放时10秒兜底返回错误；
旧100ms准备预算在认证未完成且gate/句柄/监听均为0时精确返回连接认证超时。
独立首轮APFS复制90秒超时和dep-info绝对路径假设诊断错误均保留；后者按实际Cargo相对路径与receipt cwd核对，不能当作产品失败或省略编译绑定。
独立manifest62项经根逐SHA/bytes验证，SHA-256为`789cf6001371a47895240d660af1c8ccbadce562fc1d0ff1a57357a6a477c209`，
报告为`2351d781e36896873fd4232b32e1fe585a22a0a5fdb4851b88746156f6e96926`。
两个审查scope的源码/二进制/失败独立保存，自有cache在根核验后另行清理，不改原manifest。

提交推送与新提交CI的完整回执另行追加。
原UI开发包仍归属408工作树快照；这些测试夹具改动不能将其重写成新提交的标准包、Release或新增桌面验收。
