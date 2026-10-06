# 目录镜像最终观察撤权修复：非作者复核 — 2026-10-06

结论：本次非作者复核通过，范围仅为最终远端来源 LSTAT 返回后、本地删除分派前
新增的授权检查。没有发现该三行窄修复的新 P1/P2；不等同于完整镜像、主树组合、
真实桌面或 Windows/Linux 原生验收。基线为精确 `7f50d60a59034dd145c40414c334e8bfc4e09f67`。

修复作者冻结的 881 份正文、66,827,834 字节完整读回后，在新工作树逐份复核
663 个输入。相对原作者生产代码仅两行解释注释及 `self.authority()?`，其余正文
相等。该检查同时拒绝已关闭会话及调用方撤权，位于 pending 与本地 syscall 之前。
重验证、远端写入以及同步本地闭包的授权路径另有检查；未改变它们的生产正文。

新增四个有限 TCP/SSH/SFTP 生命周期用例覆盖常规文件和空目录：第一项真实完成后，
第二项的最终 LSTAT 已进入并被独占 gate 阻塞，此时撤权。修复版返回 `Closed`，
第二项和后续项保留，同一 scope 重新授权后可完成原审核的剩余项；没有制造未知
pending 或隔离。另两例直接丢弃最终观察中的 future，保留目标且同一 owner 可继续。
这些是库级调用者的显式授权测试，不表示 UI 自动恢复或重放已取消计划。

对精确旧生产正文使用最终测试源码再次反验：两个撤权用例均实际失败，退出 101，
4.528622750 秒。旧版返回 `Ok(())` 并删除第二个目标，后项保留。
断言之前已经关闭 SFTP/SSH、join 自有监听任务并确认原端口拒绝连接。
原日志 2311 字节、SHA-256 `35f69be8465f5dad2e810994e78af770b20d87b90a058d3fb7c3ececb4b644fb`，失败源码与实际
wait 收据保留。第一次旧版反验同样失败，未被改写为通过。

新增一个 headless GPUI 用例通过真实控件：先批准本地镜像，首项完成后将第二项
最终来源检查阻塞，点击取消，保留完整中文正文与后项；旧计划不自行恢复。
重新生成计划并第二次人工确认后才删除剩余项。逐项 Completed 与后项
CancelledBeforeWrite 保持；当前项可因外层取消选择而保守显示 Unknown，或由
授权检查先返回而显示 Rejected。实际目标与重新取得 owner 证明本用例的未派发
边界，不能仅从保守 UI 标签反推出删除已发生。

最终相关检查实际退出 0，23.787179417 秒：5 个 core 镜像用例、
18 个 TCP/SSH/SFTP 镜像用例、9 个 headless GPUI 镜像用例以及严格 workspace /
all-target Clippy。32 个不同用例包含新增 5 个，不重复累计专项运行。
665 输入、14,335,293 字节在这一检查前后相等。恢复旧代码反验后，
最终修复正文逐份匹配上述输入，新增四例又实际退出 0。
这不是重新执行完整 workspace 普通/doc/ignored/平台矩阵；原修复作者的完整门禁
仍属于其原 662 输入，根整合需要自己的完整检查与新产物绑定。

首轮新增测试因已消费 gate 对象再次借用而编译失败；之后严格 Clippy 分别发现
新增测试中的未使用 import 和 workspace 禁止的 `expect`。均只修测试，保留每轮
原正文、日志与退出码，没有添加 lint 豁免。导入/应用 helper 的收据及来源地图
schema 假设错误也保留为失败；后继完整逐正文读回证实实际候选相等。只读工具的
路径/语法探测失败仅保留原工具观察，不伪造子进程 wait 收据。

执行检查使用独立增量 CoW 缓存。复制期间持有两个只读 Cargo 锁并检查非自有 FD，
689,986 份源缓存元数据前后相等，682,684 常规文件采用不同 inode；33 份选样完整
正文相等。缓存复制不是干净构建。所有 owned 检查保存真实 wait、实际进程组缺失
及私有 TMP 清理；不声称全进程普查。原失败材料保留至根消费后按工程约定归档。

此记录在工程检查后新增，封存前的格式、依赖策略、链接、补丁和正文读回另行保存。
未启动/安装应用、未使用客户机器、未推送或发布。外部 writer 的最终检查到删除
之间竞争、递归非空子树设计及真实平台验收继续开放。
见[镜像设计](../../product/DIRECTORY_MIRROR.md)与
[修复作者记录](2026-10-06-directory-mirror-independent-review.md)。

The non-author review passes only the three-line final-LSTAT revocation repair.
Four new real loopback lifecycle cases and one new headless consumer case pass;
the exact original production body fails both final-source revocation controls.
The final changed-area gate passes 5 core, 18 TCP and 9 headless cases plus strict
workspace Clippy, with equal 665 source inputs. This does not establish complete
root, desktop or cross-platform acceptance. Original failures remain unchanged.
