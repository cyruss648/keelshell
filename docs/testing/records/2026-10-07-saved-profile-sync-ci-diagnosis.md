# 保存资料同步：诊断分支 CI 定位

状态：已读回精确诊断提交的三平台终态与 Linux 完整日志，并完成新的非作者源码和证据复核。尚未确定原失败原因，新的阶段观察已在独立分支提交推送，六个限定测试与严格 Clippy 通过，新 CI 仍运行；本记录不表示生产修复。

此前诊断基线的精确提交为 `dd32392aac8dd7ba67f81dac0c41777970b2297b`。其 [Quality 37506388205](https://github.com/cyruss648/keelshell/actions/runs/37506388205) 已实际 `completed/failure`：

| 原生 CI job | 终态 |
| --- | --- |
| Linux 112416018776 | completed/failure |
| macOS 112416018852 | completed/success |
| Windows 112416019068 | completed/success |

Linux app 测试实际为 547 通过、1 失败、2 ignored，耗时 632.76 秒。唯一失败为 `approved_saved_profile_retarget_does_not_replace_captured_schedule_or_authenticated_session`：人工批准后原 18 秒等待结束仍未观察到元数据和磁盘更新。失败观察记录 busy=true、review=None、密码缓冲已清空、一个确认选择、wire=[0,0]；从批准后的首个观察到失败为 18.072598 秒。inspect 点击到待审内容为 7.846934 秒。

原业务断言、18 秒批准等待和 45 秒计划没有放宽。已有日志没有 blocking 开始、核心 service 返回、待发布恢复、Tokio join、bridge 或 foreground 接受的阶段时间，故不能归因于某一段、死锁或合法 Stale。源码中的两个顺序 Argon2id 调用使用 64 MiB、3 次迭代、1 lane；共享密钥派生及 CI 并发属于候选因素，不是已证明原因。后续 test-only 观察记录独立操作的提交、blocking 开始、service 返回和 foreground 完成，不改变生产权限、加密或完成条件。

该次原目录同步 old!/new! 字节测试通过，仅证明本次结果；不能把它追溯为主线 `0cca8ac` 的原 Quality 37481523251 字节失败已修复。Linux 的后续 OpenSSH 互通步骤被跳过，未取得该提交的互通或桌面验收。

根读回实际终态和 315,708 字节 Linux 原始日志，SHA-256 为 `354e70c91b018dd2b84ac2523defba5f71ee02618d6da7874c3b12ba14ea9277`。终态获取实际退出 0；原尚未结束时的抓取失败及 ANSI 输出处理失败单独保留，不改写为成功。新的非作者完整复核精确提交源码、主副本相关生产等价关系及 61 项证据材料；根再次逐份读回正文、命名空间和模式。临时原始日志与完整回执留在忽略的 `work/` 中，仓库不携带真实凭据、私有工作树绝对路径或原始进程环境。

当前主副本后续 717 输入工程检查的成功属于[组合范围](2026-10-07-recursive-mirror-main-combination.md)，不替代此 Linux 原失败，也不表示新的阶段诊断已合入。

新的四路径 test-only 阶段观察与独立补充在 `0f1a57756a78771dc0d4e2ca14703d55925841bf` 提交推送，逐 ref 一致、工作树干净。三个真实面板用例、两个 recorder 用例及原保存资料审批用例均实际执行通过；格式、依赖策略和 locked app/all-targets 严格 Clippy 通过。675 输入门禁前后相同，原生产 service、加密和 18 秒／45 秒断言保持不变。根全文读回 771 份封包正文／20,185,970 字节和 675 份提交源码，确认实际 wait、进程组与临时目录终态；此为限定诊断验证，不是原 Linux 原因或生产修复。该增量尚未合入主树，[新 Quality 37523395932](https://github.com/cyruss648/keelshell/actions/runs/37523395932) 尚未取得终态，不预报三平台成功。
