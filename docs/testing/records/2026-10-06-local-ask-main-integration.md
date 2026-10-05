# 本地 Ask 进度的主工作副本整合 — 2026-10-06

当前结果：**第二次完整主树门禁通过，功能已在cfac提交并推送**。两项test-only补充经新的非作者复核后整合，根对最终260工程输入实际验证1171普通+8doc+6脚本和严格检查/两控制器；新版macOS双程序构建、57打包回归及标准包原生结构检查通过；新版窗口已完成中文浅色的部分原生流程，最小窗口和完整矩阵仍待完成。第一次失败及原因未知保持，不把后来通过推断为原故障原因。MCP始终由KeelShell向外部智能体提供服务；内置API/本地CLI Ask独立。

当前精确源码CI整体failure：Linux通过、Windows额外小栈控制器总期限超时、macOS未取得运行器。原日志与边界见[本次CI](2026-10-06-local-ask-progress-ci.md)，新版窗口及清理见[部分原生记录](2026-10-06-local-ask-progress-native.md)。下方运行、尚未推送和待复核是各阶段历史时点，不覆盖本段。

## 导入与输入绑定

根基于已推送的bd9f5e导入10份代码和候选文档，设计计划仅应用对应单句差量并保留较新历史。LOCAL_AGENTS另采用独立实跑复核的指南单句：隐藏助手保留请求，最后Entity owner释放才请求取消。14份普通候选全文与作者冻结字节相等；计划和指南差量分别核对。

260份Rust/Cargo/工具链输入与作者只有3份已审测试差异：SystemRoot测试及模型目录夹具两文件，各自等于对应最终门禁/独立复核源码。依赖、Cargo.lock与工具链保持。首次导入辅助脚本误猜作者输入文件名而失败，没有启动Cargo；原失败保留。改用实际输入文件名完整核对后才执行门禁。

## 根实际运行

`python3 scripts/check.py` 在314.919997秒exit1，外层未超时，leader实际回收，原进程组未观察到survivor；private TMP为空后删除。260工程输入前后逐bytes/SHA相等，没有测试期间修改Rust源码。

| 检查 | 实际结果 |
| --- | --- |
| 格式、x.y依赖策略 | 通过 |
| scripts单元测试 | 6通过 |
| workspace/all-targets/locked严格Clippy | 通过 |
| 普通Rust测试 | 部分930通过/1失败/2ignored；不是完整workspace通过 |
| 默认自托管CLI控制器 | 实际输出future5040字节并退出成功；不计普通test-result |
| 文档测试、额外2MiB控制器 | 未到达 |

失败为`keelshell-mcp/tests/ipc_transport.rs:287`的`binary_rejects_partial_or_invalid_environment_without_stdout_fallback`：等待`command.output()`的2秒期限返回`Elapsed`。该harness实际9通过/1失败，三组env哪一组失败、子进程退出/管道状态与原因没有记录，保持未知。MCP入口及该测试没有由本Ask增量修改；这不证明Ask导致该超时，也不能推断启动/runtime/调度中的具体机制。

原完整日志94533字节，SHA-256 `59bf7b5659d0dc2ebcc459a7ab23cd9a8cc62f9b98f1792df694c8a593a5e3a5`；门禁收据、before/after260输入、首次导入失败及完整作者/复审封包存于ignored整合证据目录。源保持冻结，新的非作者在独立工作区诊断自有MCP子进程启动与退出，不改原2秒deadline或失败事实。

后续有限独立诊断没有复现产品P1/P2：原10项IPC在另一target实跑通过，原三envpair的9次直接自有binary观测均exit1/空stdout/精确静态stderr/实际reap与双EOF；私有Rust三pair观测保持原2秒并通过。不同inode首次执行约0.265秒，不控制全局OS缓存，不能解释原超时。原18份绑定源码已恢复等于Git和根MCP字节。首次cp60秒、观察器PID-count误解及无initialize却期待EOF成功的3项私有准备/预期失败完整保留；另有正确initialize后EOF成功的独立用例，不改写原失败。

有限诊断88payload/89regular/90归档成员已根全部逐bytes/SHA读回，manifest SHA-256 `b1543d361242fb5516e64fc69bcede7612cc80f7ea400ccc39c5420fdb3d486b`，tar3223818字节/SHA-256 `bf72d719bdac1d00fd136317477bb0d030b8e3b2f0bbbdc69dd0eacb0e0bc1c0`。它证实测试源码没有失败后的显式awaited-reap、无迭代/pipe阶段记录和capture配额；不能证明历史泄漏或原超时机制。新的test-only候选另行加32KiB/pipe和显式bounded清理回执，原2秒与三pair保持，生产入口不改；完整门禁、非作者复核和根整合尚未完成。

## 独立并行失败与剩余验收

已推送bd9f5e的macOS源码CI在自托管CLI控制器的预算后代监听关闭断言失败。其已完成80普通通过/0失败/2ignored，不能与本根MCP测试失败混成同一件事；timeout/cancel分支、出生身份与端口原因未知，另一个独立工作区诊断。

新Ask完整门禁、确认问题后的修复及新非作者复核、新精确源码CI、macOS供应商CLI/最小原生窗口/语言主题，以及Windows/Linux原生仍开放。没有新Release、安装更新或授权Codex MCP业务验收。


## 两项 test-only 候选的独立收敛

预算诊断作者最终完整门禁1158普通+8doc+6脚本通过，114payload/115归档经根读回；新的非作者默认/2MiB、wrong-PID/缺PID与ACK绝对期限反例、恢复258输入后的格式/x.y/严格Clippy通过，194payload/195归档经根读回。根仅导入该测试差量和新记录，保留Ask全部进度测试；唯一fixture调用冲突同时保留继承管道与ACK开关。实际根合并版默认32.209秒、2MiB11.239秒退出0，future5040字节、原立即TCP判断保持。此时MCP守卫尚未导入，完整主树门禁尚未执行，不能将定向控制器通过写成整仓成功。

MCP守卫作者完整门禁424.285秒通过1164普通+8doc+6脚本，两控制器4624字节，518既有tracked及其中258编译输入前后相等；两份候选生产不变，保留原三pair/2秒，新增32KiB/pipe与失败时1秒bounded kill/wait。78payload/80归档已根全量读回，新的非作者正在另行运行原16项和受控read-error/迟到完成/预取消/spawn失败边界。原根失败及原observer容量中断保持；作者通过不代替该候选复审或新的根完整门禁。


## 第二次根完整门禁

两项test-only候选经非作者限定PASS导入后，最终260工程输入相对第一次失败仅预算/MCP两测试变化，生产Ask、依赖manifest、Cargo.lock与工具链保持逐字节相等。预算三方fixture交汇的两项定向控制器另已通过。第二次`python3 scripts/check.py`实际254.295536秒exit0；42个普通harness共1171通过/0失败/11原ignored，4个doc harness共8通过，6个脚本测试。fmt、x.y及workspace/all-targets/locked严格Clippy通过，默认与显式2MiB控制器均实际完成、future5040字节、24条预算诊断。MCP入口及其原三pair和16条生命周期回归在最终工作副本通过。

260份Rust/Cargo/工具链输入在门禁前后逐bytes/SHA相等。直接leader实际wait/reap、原隔离numeric group读回不存在，空private TMP已删除；此包装器不证明未观察的逃逸后代。完整日志126679字节/SHA-256 `4e0084ece9dc3004f87b95b9ea72202376770cbdd6394950d115defe5b6a5df8`。第一314.920秒失败、两个历史原因UNKNOWN及所有作者/复核的非零证据保持。新精确源码CI、新原生供应商CLI/900×580窗口、其他平台桌面、Release和安装更新尚未验收。

## macOS 构建和标准包检查

在第二完整门禁的同一份260工程输入上，以macOS最低15.0显式构建GUI与MCP两项arm64程序，实际55.983秒退出0。57项packaging回归通过，命令实际1.925秒退出0。标准打包与目标OS结构检查均实际退出0：两项Mach-O架构、最低版本、动态链接、可执行权限、plist及清单摘要通过。根对所有包文件与原构建程序分别读回核验，输入仍与完整门禁相等。

标准包仅放在新的ignored staging目录，未安装、未签名、未公证、未发布；dirty构建清单的commit为空，不能冒充正式Release来源。构建/打包包装器的leader实际wait/reap，原numeric group新观察不存在，空private TMP已删除；该范围不是完整逃逸后代普查。新版窗口、供应商CLI与最小900×580交互尚未执行，源码CI在提交推送后另行核验。
