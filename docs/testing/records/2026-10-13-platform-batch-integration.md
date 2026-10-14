# 批量传输与本地 AI 生命周期整合

## 范围

2026-10-13 从远端主线 `ecab604` 恢复开发。在独立分支
`feature/platform-batch-integration` 组合批量传输 `0220f2f` 和 Unix
清理修复 `991473f`；共同祖先为 Windows 测试控制器修复 `af9465c`。
两项生产代码不重叠，交接和路线图保留双方历史并增加本轮状态。
保存快照分支没有自动并入，CDB 诊断改动也不属于本次产品组合。

独立逐文件审计确认，最初组合共 846 个受版本控制的路径；除两份手工
合并文档外，844 个路径的字节、Git mode 和 index blob 与两个原提交的
预期联合相同。这个检查证明导入完整性，不代替本轮测试。

## 原提交平台证据

- 主线 `ecab604` 的 [Quality 37726023963](https://github.com/cyruss648/keelshell/actions/runs/37726023963)：
  macOS/Linux 成功，Windows 在 Python 控制器的原后代存活断言失败。
- Unix 分支 `02f2e59` 的 [Quality 37868547352](https://github.com/cyruss648/keelshell/actions/runs/37868547352)：
  三平台全部成功。该提交包含 `991473f`，不含批量传输增量。
- 批量分支 `0220f2f` 的 [Quality 37876026424](https://github.com/cyruss648/keelshell/actions/runs/37876026424)：
  macOS/Linux 成功；Windows 明确报告
  `browser_modifier_ranges_hidden_filters_and_single_actions_use_current_paths`
  失败，随后应用测试进程以 `0xc0000409` 异常退出，没有输出最终断言详情。
  前一个失败和进程异常是两个独立观察，不能把最后一个打印的测试当作
  崩溃触发者，也不能据此断言栈溢出的具体位置。

## 隐藏文件夹具修正

生产本地浏览器在 Windows 使用 `FILE_ATTRIBUTE_HIDDEN`，在 Unix 使用
文件名的点前缀。原批量测试仅写入 `.hidden`，在 Windows 上始终是可见
文件；因此切换隐藏项时，选区由四项降为三项的原断言得不到对应输入。
应由测试夹具设置并核实真实隐藏属性，保留生产语义、全部选择断言、原
时间限制和四线程执行。准确 Windows 运行结果需本次提交自己的 CI。

## 本轮执行记录

新的独立审查确认一个普通文件审核身份问题：Save 待审时，仍可点击远端
另一文件的 Edit，以 Read 审核替换原 Save；旧确认只比较可选 batch ID，
两者都是 `None`，可能批准新 Read 并丢失草稿。旧取消也可撤销新审核。
修复将全部五个新审核入口集中到分配独立 UUID 的函数，确认/取消绑定
完整审核身份，同时保留 batch plan UUID 和原执行前检查。
两项真实按钮回归和设计见 [ADR 0088](../../adr/0088-file-review-action-identity.md)。

Unix AI 库组合专项已实际通过 101 项、1 项 ignored；这只验证该本机库
测试范围，未替代后续 workspace 门禁及平台运行。

首次完整检查在 Clippy 依赖编译阶段主动停止，以应用已确认的 Windows
夹具修正；自有进程实际退出 `-15` 并回收，尚未进入 Rust 测试，不记为
完整门禁通过。格式、直接依赖 x.y 策略及 57 项打包脚本回归已通过。
最后的完整检查实际退出 `1`，848 个工程输入在检查前后保持相同。
直接依赖 x.y 策略、格式和全 workspace/all-targets 严格 Clippy 通过；
65 项脚本回归完成（64 通过、1 项 Windows 原生跳过）。workspace Rust
测试在 AI 库停止，结果为 100 通过、1 失败、1 ignored。失败为
`first_signal_and_observation_failures_stay_typed_on_reentry_and_drop`，
Unix 夹具的 `request` 在 `stream.write_all(&[byte])` 返回 `BrokenPipe`，
位置 `unix_cleanup_tests.rs:383`。此前 AI 库专项的 101 通过不能覆盖这次
完整检查失败，原因尚未定位，不修改原期限或断言以使其通过。

自有检查进程已实际回收、进程组不存在；夹具记录 leader 已回收、group
不存在并保留失败 scratch。后续 workspace/app/session/doc、readiness、
observer IO 和 small-stack 阶段没有到达，不能记为通过。

文件审核指针回归分别保留旧实现负控、第一版 UUID 修复以及临时诊断的
两项失败。诊断确认旧回调 expected/current UUID 不同，但拒绝后仍有
事件传播。最终确认/取消回调在身份检查前停止传播，完全相同的两项
GPUI 指针回归实际通过（2 通过，退出 0，26.86 秒）。独立审查确认
全部提案赋新身份、旧事件拒绝和新绘制取消正控制；这只关闭限定行为
问题，不代表完整工程门禁或原生桌面验收。

## 保存进度与重新克隆

用户要求先提交全部本地代码，再删除项目目录并从远端重新克隆。本轮
以 `feature/platform-batch-integration` 保存进度检查点并推送；不合入
main、不创建发行标签，也不将当前组合记为已完成的功能验收。新的
原生包/桌面检查及精确提交 CI 尚未验收。后续首先处理上述失败，然后
重跑完整检查，继续原路线图。

删除前逐一核对本地分支与远端 SHA、提交后的干净状态，并在仓库外私下
保存内部恢复 refs 的 Git bundle/元数据及忽略的失败日志。普通源码和
本轮文档纳入公开功能分支；内部恢复快照和原始运行材料不上传。确认
没有活跃消费者后删除旧目录和可再生缓存，再从 GitHub 克隆至原路径，
检出保存的功能分支并核对提交、树、逐文件内容及干净状态。实际迁移
收据保存在仓库外，重新克隆完成后暂停目标。

## 验收边界

历史批量传输的 macOS 受控原生三上传/三下载、审核取消零写入和队列
入口证据继续保留原范围。本轮不继承其程序哈希，也不以合并或编译证明
新的原生行为。Windows 应用异常、三平台桌面、最小窗口和辅助技术、
完整外部 MCP、本地 CLI 自身登录，以及签名公证/安装更新仍需各自验收。


## 用户恢复开发后的夹具故障定位

用户随后明确恢复目标。原失败根的 member stderr 在第 223 行记录
`set_read_timeout(Some(100 ms))` 返回 `EINVAL`，其 PID/group 与同轮
保存的身份及主检查日志一致；member 随后退出，父端第 383 行写请求
收到 `BrokenPipe`。这定位到本次测试夹具的直接故障，不证明操作系统
为何拒绝 setter，也不能据此关闭其它历史 CleanupFailed。

候选仅将自有 Unix fixture 通信改为安全非阻塞读写。操作字节与完整
nonce 共用单次 100 ms 绝对期限，受 member/leader 原 10 秒生命期上限
约束；部分读写、Interrupted 或 WouldBlock 不重置期限。完整 nonce
仍是退出条件；Q 的回复保持尽力发送，以兼容 Drop 的既有单向退出请求。
父端 8 秒期限、生产 3 秒清理、一次信号许可及原身份/终态断言不变。

新增真实 socket 控制覆盖完整身份响应、分段 nonce、错误/残缺 nonce、
无数据/部分数据的父期限、已过期请求及对端关闭。专项、完整工程和新
非作者复核结果随后分别记录；此候选当前不声明已通过。

## 2026-10-14 夹具修复与拖放验证

完整本机 `python3 scripts/check.py` 实际退出 0，耗时 1078.53 秒，848 个当时工程输入在检查前后保持不变。收据为忽略目录中的 `work/2026-10-13-resume/full-check-result.json`。该结果覆盖格式、严格 Clippy、workspace 测试、readiness、observer IO 和 small-stack；应用套件为 740 通过、2 ignored。它不能解释随后修复的 macOS 更新器失败，也不能解释 Windows `0xc0000409`。

精确 `cddec26` 的 [Quality 38333873552](https://github.com/cyruss648/keelshell/actions/runs/38333873552) 已完成且整体失败：Ubuntu 成功，macOS 与 Windows 失败。macOS 的直接故障是更新器夹具继承非阻塞套接字后 `read_exact` 返回 errno 35；现用单次 2 秒请求期限和 8 秒夹具寿命做非阻塞安全读写，Drop 不再二次 panic。新增分段、超时和提前关闭回归。更新器测试现为 53 通过。

拖放上传复用批量审核。六项 GPUI 回归及完整文件模块 162 项通过，四线程执行。拖放提示放在现有批量工具栏中，文件区最小高度保持 64。此证据不是 macOS、Windows 或 Linux 原生文件管理器拖放，也未证明远端字节在真实桌面流程中的往返。
