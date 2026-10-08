# Windows CDB 启动参数窄修正 — 2026-10-08

状态：精确af9465c功能分支，两个脚本候选已导入隔离Git工作副本。本机脚本回归、格式、依赖策略和严格Clippy已经通过；实际Windows/CDB运行尚未完成，不准声明已修复原应用异常。根的Unix未提交增量与SFTP批量作者不混入此分支。

原37732063328已实际通过Windows Job控制器，但首CDB控制未启动目标、debugger退出0x80070002；其余四控制与固定原706应用诊断未到达，原0xc0000409应用原因仍UNKNOWN。现在只把-netsyms参数构造成单一-netsyms:no项，保留完整目标/argv尾部、五CDB控制、原固定源码、期限、Job身份及cleanup；不得把macOS的命令构造测试当CDB解析器运行。

Microsoft官方[CDB命令说明](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/cdb-command-line-options)的语法行采用冒号形式，参数标题采用空格形式；本记录保留这处资料差异，不以它确定历史0x80070002原因。新增三项命令边界回归，覆盖含空格Windows原生路径、选项/目标唯一边界及目标参数尾部。原59Windows脚本/6OpenSSH测试正文保持；精确旧helper配新contracts的离线负向结果保留于前置审查，不作为native失败或成功。

已有作者与新非作者源码/离线证据复核仅允许精确导入与新的组合检查，见忽略材料windows-cdb-launch-author-20261008-v1和windows-cdb-launch-independent-20261008-v1。新的组合源码/证据复核和提交准入单独完成，不继承该前置结论。

## 当前组合验证

在隔离工作副本的同一832项输入上，以下四个实际命令均退出0，完整stdout/stderr分别保存并读回。owner均已wait/reap，所属进程组消失，专用TMP为空且实际删除；这只证明命令自己的收尾，不能推断未运行的Windows夹具终态。

| 检查 | 实际结果 |
| --- | --- |
| Python脚本回归 | 68项运行，67项通过，1项Windows原生检查在macOS跳过 |
| 直接依赖x.y策略 | 通过 |
| cargo fmt --all --check | 通过 |
| cargo clippy --workspace --all-targets --locked -- -D warnings | 通过，157.50秒 |

源码与物理mode在上述命令前后保持一致。Clippy使用串行root target租约，缓存不作为源码或测试结果；该租约已经释放。依赖block 0.1.6的future-incompatibility提示是已保留的编译器提示，不是本次Clippy失败。原始回执位于忽略材料windows-cdb-current-worktree-preparation-20261008-v1。

此分支没有修改Rust生产源码；父提交的完整workspace结果属于父提交，没有在此分支重新执行完整Rust workspace、doc或本地智能体控制器。真实CDB参数解析、五项Windows CDB控制、固定原706应用诊断及原0xc0000409原因继续开放，后续必须验证精确提交。完整产品、GUI及发行未由本切片关闭。

新的非作者组合审查已全文核对三路径、原始工程回执和完整输入，并独立执行相同68项脚本回归，实际退出0（67通过、1项Windows原生跳过）。未确认P1/P2；只准入三路径的功能分支提交、推送及原Windows诊断工作流，不准入主线合并、标签发布或原生成功声明。复核后本记录仅补充这一状态，两个脚本与其它工程输入保持审查版本；实际Windows结果须另行记录。
