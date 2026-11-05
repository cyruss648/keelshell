# ADR 0103：CI 容量观测与串行夹具寿命

- 日期：2026-11-05，Asia/Shanghai
- 状态：六文件修订、新 profile 完整门禁与 Nash 组合复核通过；新提交绑定及精确 CI 为 PENDING

## 依据

诊断提交 `3d8476716ab80f0cec8b8c731a8cde0bcd7601a3` 已推送 feature，
未合入 main。其 Quality `41994734898 / attempt 1` 的 Linux 原日志
第 744 行报告剩余 70 MB，第 957 行在 `selected-cancel / Codex` 记录
`OwnedExecutableWrite / StorageFull / raw_os_error=28`，关闭 scratch 成功，
请求返回 `ScratchFailed`。这确认该次副本写入收到 ENOSPC；
不能据此确定所有磁盘占用来源，或倒推旧 run `41967541069` 的根因。

串行目录控制器的同名变量遮蔽会让前一夹具继续拥有临时根目录和程序副本，
直到外层作用域结束。相同的新寿命断言在显式关闭前退出 101、关闭后退出 0。
该对照证明前一根目录的释放时机，不是 CI 磁盘峰值或新 profile 收益测量。
具体原始证据与状态见[容量记录](../testing/records/2026-11-05-ci-storage-capacity.md)。

## 决策

1. 每个串行场景完成原请求、子进程回收及既有断言后，显式消费夹具并关闭
   `TempDir`；共享夹具先用 `Arc::try_unwrap` 确认没有残留 owner。
   下一夹具创建时断言前一根目录已不存在。helper 只保留路径用于测试，
   不保留句柄或目录 owner；关闭失败明确使测试失败。
2. 在 `[profile.dev.package."*"]` 配置 `debug = 0`，面向非 workspace
   依赖；workspace 保持 `[profile.dev] debug = 1`，test 继承 dev。
   既有 Argon2/SHA2 优化、debug assertions、overflow checks、release
   profile 和锁定依赖保持。此配置旨在控制测试产物占用，实际大小与容量
   变化须另行测量；不由配置文本声称收益。
3. 构建缓存键增加 `dependency-debug-0` 分代，隔离旧 profile 的 target
   缓存；本切片不清理共享缓存或其它任务资源。
4. `scripts/check.py` 在每个检查命令前后记录 workspace/temporary 两种
   位置角色的 total/used/free 字节，失败后的观测由 `finally` 执行。
   读取失败只保留可用性及数字 errno/winerror；不输出路径或错误正文。
   输出损坏或关闭不能覆盖原命令结果。这是时点容量观测，不是峰值、
   inode 统计、自动清理或遥测上传。

## 不变量与验收

既有测试断言、四线程入口、Ask/控制器期限、取消与子进程回收规则不变；
生产完整可执行文件校验、复制、审核和错误优先级继续保持。
本切片不以删测试、延长期限或忽略 ENOSPC 获得通过。

夹具修前/修后对照均早于 profile 修改。新 profile `full-gate-r1`
现已退出 0并回收：2,126 ordinary Rust / 23 ignored、14 rustdoc、
Python 131 run / 1 skipped，506 个工程输入前后一致。
本机全仓与专项调用的夹具程序分别测得 10,078,320 和 9,921,664 字节；
两者分开绑定，不代表完整 target、磁盘峰值或远端容量收益。
Nash 对六文件源码为 **APPROVE_SCOPED_SOURCE**，无开放 P0/P1/P2/P3；
随后 `combined-r1.md` 为 **APPROVE_SCOPED_COMBINED**，无开放 P0/P1/P2/P3，
文档 P3 已关闭；新提交绑定及精确 CI 仍 **PENDING**，不宣称远端已修复。

原提交 Windows 单 job 重跑已 success，整体 attempt 2 仍 failure：
Linux/macOS 的时间、步骤和原日志均与首次相同，属于继承结果。
首次 Windows preparation 失败原因仍 UNKNOWN；重跑不含新六文件修订。
原生导航与分屏按各自冻结输入及回执独立验收，不纳入本切片。
既有设计见[ADR 0102](0102-local-ask-scratch-diagnostics.md)、
[ADR 0075](0075-development-kdf-build-optimization.md)和
[ADR 0084](0084-development-sha2-build-optimization.md)。
