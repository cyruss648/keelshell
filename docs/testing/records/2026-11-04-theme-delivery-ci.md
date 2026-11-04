# 2026-11-04 — 主题组合提交与 CI 回读

## 已提交的功能

三主题、知识库管理、命名隧道管理与依赖更新已经合入并推送 `origin/main`：
`c36687126312191191236f1e9c8840abb7b4d808`。推送后远端 SHA 读回一致；
没有创建发行标签或覆盖安装。功能、原生范围与前序完整门禁见
[主题验证记录](2026-11-04-native-themes.md)。

## 最新源码与验收范围

后续限定测试修复和诊断已推送至 `7782979351291abfc95966b286f824fca683deb6`。
精确 Quality `41848787327` 的 macOS/Windows 已成功，Linux 在 MCP SFTP 夹具准备失败；
整个 run 失败，三个完整原日志均已回读。各项结果见
[原生 Job 记录](2026-11-04-native-barrier-diagnostics.md)；此前失败继续逐次保留。
不能把本机通过或 CI 步骤名称代替实际平台测试结果。

macOS 原生证据分两份：较早的主题包覆盖受控 SSH/SFTP 连接下切换主题、保留草稿
和重启恢复；最终组合包补验三个主题入口及中英文知识字段 AX 名称，未重跑 SSH fixture。
后续提交仅修改测试输入，不把这些检查扩展为完整知识管理、NET 原生、Windows/Linux
桌面或发布安装验收。具体包摘要与范围分别保存在 `native-theme-results.json`
和 `native-final-results.json`；后续 Rust 测试源码确实变化，不能声称整个 Rust 源树不变。

新的非作者 Huygens 已逐个检查 `c366871` 之后至 `7782979` 的四个提交：只改变五个被测试模块
守卫隔离的 Rust 文件、一个 Python unittest 文件及文档；其它 538 个非 Markdown
输入相同，包括生产源码、Cargo 配置/锁文件、资源、打包和 workflow。
主题指南与 ADR 0095 的内置注册扩展边界正确，本次限定交付复核 P0/P1/P2 均为 0。

早期主题包的可执行文件已清理，当前不能重算其字节摘要；原结果、manifest、启动路径、
退出回执及保留的截图/AX 相互一致。最终包的五个文件均重新核对 manifest，
其程序摘要为 `6d92ada1ddeff800bac062599611949ab70899b77fc39aba407da277272898e3`，
实际退出 0 且已 reap。该包在功能提交前构建，manifest 没有 commit/workflow 身份；
源码连续性不等于当前 HEAD 的原生执行。复核材料在
`work/theme-integration-20261104/final-delivery-review/`。

## 原提交的 Windows 失败

精确提交的 Quality run `41824639165`，Windows job `126556142575` 在
`scripts/check.py` 的 Python 阶段失败，尚未执行该 job 的 Cargo 检查。
Windows Server 2025 runner 使用 Python 3.15.2、Rust 1.99.1。
此前独立打包回归为 57 项、4 skipped；Python suite 为 120 项、
1 failure、1 skipped。

失败项为 `test_real_timeout_owns_descendant_with_detached_stdout_and_stderr`：
进程身份文件尚不存在，ACK 未创建，控制器已超时退出 124。
原始 receipt 显示直接进程已回收、活动进程为零、输出管道已排空，
没有清理错误。其它成功的进程树用例不能替代这一用例的身份和后代断言。

完整原日志保存在忽略目录
`work/theme-integration-20261104/ci/original-windows.log`；
原 run/jobs JSON 同目录保存。不把这个结果归因为已证实的 Python 版本回归。

## 限定测试修复

原测试把总期限设为 5.4 秒，而 `run_owned` 为回收保留 5 秒。因此 Python
解释器启动、创建后代及身份发布实际共享不到 0.4 秒；owner 构造也会消耗该期限。
源码确认了这个预算分配缺陷，但日志不能确定本次 runner 启动延迟的底层原因。

仅修改 `scripts/test_windows_native_crash.py`：

- 为夹具准备预留 5 秒，加原控制器的 5 秒清理预留，形成单一、最多 10 秒的总期限。
- 完整身份发布后，先持有原生进程句柄并核验 Job membership，再把同一期限缩短到
  `min(原截止时间, 当前时间 + 5 秒清理预留 + 0.4 秒执行)`，随后发送 ACK。
  不能延长或复活已到期的期限。
- 成功绑定身份后的执行与清理继续断言小于 3 秒；总运行时间单独断言小于 10 秒。
  原先把准备耗时也混入 3 秒断言的方式不再使用。
- 保留实际超时、SIGINT、非零退出优先、原生句柄终态、进程树为空、管道排空及
  空/部分身份发布拒绝断言；生产控制器、应用源码和 workflow 均未修改。

新增真实延迟发布反例：父进程已创建后代，但等待观察者在至少 0.6 秒后放行身份发布。
修后通过身份绑定与原回收断言；只把同一测试的总期限恢复成原 5.4 秒时，
实际在身份发布阶段失败。另有确定性回归覆盖正常绑定、接近外层截止时间和已过期
三种状态，确保不会重新授予时间。

## 本机验证与复核

首次受影响完整 Python suite：122 项、1 skipped、退出 0。
旧预算反例：1 项，预期的身份发布断言失败，无其它 error。
新的非作者 Tesla 复核没有确认 P0/P1/P2，独立模块检查为 63 项
（62 passed、1 Windows-native skipped），另有 7 个反例测试方法通过。
其中 4.65 秒后发布身份仍遵守原外层期限；超过准备上限不发 ACK 并完成清理；
移除期限收紧会触发原小于 3 秒断言；篡改 reap、active count、pipe 或 error
回执均被原断言拒绝。18 个所属进程组无剩余成员，私有临时根为空。
修后完整 `python3 scripts/check.py` 实际退出 0，耗时约 592 秒；
543 个已跟踪非 Markdown 输入在运行前后摘要相同。普通 Rust
2,007 passed / 23 ignored，app 819 passed / 2 ignored，rustdoc 10 passed
（其中 1 项仅编译）；Python 122 项 / 1 skipped。格式、workspace/all-targets
严格 Clippy、x.y 依赖策略及额外 readiness、observer I/O、2 MiB 控制器检查全部通过。
运行期间更新的交接/测试 Markdown 不纳入这 543 个输入不变声明。
本机为 macOS/POSIX；以上不构成 Windows 原生句柄测试或 Windows 桌面验收。

修后门禁原始输出、输入摘要和独立复核材料保存在
`work/theme-integration-20261104/ci/`。

该修复已提交推送为 `9d4517e653e8484436c6a10a1e81229dbccb1103`，
远端 main 已读回一致。Tesla 的最终复核将提交中的 543 个非 Markdown 输入
及 5 份文档逐一绑定到已通过门禁/已复核内容，P0/P1/P2 为 0。
原 `c366871` 的 macOS/Linux 两个 job 最后均成功，三份原始日志已完整保存。
新修复的 Quality `41828929466` 中 macOS 因另一个既有 Unix 分片 nonce 测试失败，
详见[独立后续记录](2026-11-04-unix-fixture-fragment-readiness.md)。
新 run 的 Windows/Linux 最终均成功。Windows 七项真实进程树控制已回读，
包括延迟身份发布、原父子句柄终态和空清理错误；不扩大为 Windows 桌面验收。
后续 Unix 修复 `7b43af2` 的 macOS/Linux 成功，Windows 更新器测试失败，
具体后续见[更新器记录](2026-11-04-updater-transport-fixture.md)。

## 资源清理与后续边界

本轮原生 app、SSH/SFTP fixture、两个 HTML 服务均已正常收尾，所属端口拒绝连接。
NET 临时 worktree、分支和约 6.5 GiB 的独立缓存已清理；测试、原失败和最终开发包
先复制到主 checkout 并核对摘要。旧开发包、图标验证 venv 和已结束子 agent 已清理。

`dependencies-20261104` 与 `ai-04-knowledge-library` 两个受管理 worktree
归档时均被应用管理器以 “Another active task still uses this worktree” 拒绝。
保留它们及对应分支，不使用 shell 删除绕过活跃使用保护。清理回执为
`work/theme-integration-20261104/worktree-cleanup.json`。

本轮限定原生结果没有关闭 Windows/Linux 桌面、完整主题×明暗矩阵、系统外观通知、
最小 OS 窗口、IME/VoiceOver、真实 NET 超时、完整知识上下文模型发送或发行安装。
工作区布局迁移和整个产品目标仍在继续。
