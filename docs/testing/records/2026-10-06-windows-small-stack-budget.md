# Windows 小栈控制器整体预算

日期：2026-10-06。状态：候选实现，本机完整门禁通过；尚待独立审查与新 Windows CI。

## 已核验的原始失败

[Quality 37377310693](https://github.com/cyruss648/keelshell/actions/runs/37377310693) attempt 1 的完整 head 为 `2f7ca1ee4a3abc248bc76e30eac1911110b5c76f`，终态 `completed/failure`。三个实际 job 的 checkout 日志均包含该精确提交，步骤及完整日志已读取保存。

| 平台 | 实际结果 |
| --- | --- |
| Linux | 1179 普通通过、12 ignored、8 doc；默认与 2 MiB 控制器各 626 条阶段记录、38 次 TCP，future 5080 字节；原 OpenSSH 互通步骤成功 |
| macOS | 1179 普通通过、11 ignored、8 doc；默认与 2 MiB 控制器各 626 条阶段记录、38 次 TCP，future 5080 字节；原 OpenSSH 互通步骤成功 |
| Windows | 1160 普通通过、11 ignored、8 doc；默认控制器完成 626 条阶段记录、38 次 TCP，future 5440 字节；额外 2 MiB 控制器在 45 秒整体预算失败，尚未完成 |

Windows 默认控制器耗时 73.497253 秒；26 次 `ConnectionRefused` / 10061 共耗时 52.451005 秒，已超过原小栈整体预算。小栈入口完成 17 次拒绝探测，累计 34.235470 秒；序号 369 开始 Claude 进度用例 4，序号 370 在 45.000854 秒记录 `deadline_exceeded`。这不证明正在处理的单次操作挂死、泄漏、栈溢出或供应商行为异常；该次超时后具体进程清理仍未另行采集。

首次两个 job 日志 GET 实际 exit 1，原因是 GitHub CLI 拒绝将控制字符输出。后续限定读取允许原始控制字符进入隔离文件，两个操作实际 exit 0；没有将错误输出改写为成功，也没有把 CLI 输出保护失败记作产品失败。完整原日志 SHA-256：Linux `fe81552dfb2ef55b9c5e7d163e0c799eac668df598f17b75855a0bc2a50a6bfc`；macOS `9f6ab2964e4304ccc09d642417b1c1e8b84c3afd7e8c25a0cae2b99a5b6c30b0`；Windows `7d1c31061d68ab0a792a76d3641a0ad34c60bab6278db13b3466672d4521f807`。原始读取和解析材料保留在 ignored `work/ai-redaction-main-integration-20261006/`。

## 候选改动与门禁

见[ADR 0057](../../adr/0057-windows-small-stack-controller-budget.md)。只更正 Windows 小栈完整控制器的整体测试预算为 90 秒；其他平台仍 45 秒，2 MiB 栈与全部具体操作期限、断言及 TCP 调用保持。产品代码、依赖和凭据处理没有修改。

本机隔离工作树完整 `scripts/check.py` 实际 exit 0，耗时 557.425 秒：1179 普通通过、11 ignored、8 doc、6 脚本、格式、`x.y` 依赖策略及严格整仓 all-targets Clippy 通过。默认与 2 MiB 控制器各完成连续 626 条阶段记录、38 次原 TCP 调用及 5080 字节 future；耗时分别 10.779466 与 10.670365 秒。完整日志 355007 字节，SHA-256 为 `ff16c7c98f0179cebaf3e973a7a309a84927e04aa7027d9274bee39b0da3206f`。

261 项工程输入的早期快照是在门禁启动后采集；结束快照与该早期快照全部相等，不称其为启动前冻结证明。与已核验主线相比仅该测试控制器源文件改变；82 个原断言、夹具尾部完整字节、原 TCP helper 及 11 个调用位置保持。候选源文件 SHA-256 为 `bb647f36c71ff39ba57a79cdef64b37b9a5c4ffcfa176b72e61e6df2c51e459f`。该段日志、两份输入快照、scope 读回及执行收据保存在 ignored `work/windows-small-stack-budget-20261006/`。

执行器已明确 wait/reap 进程组 leader，原数字 PGID 不再存在；这不是逃逸后代普查。格式及完整门禁的两个私有临时目录经空目录检查后删除。所有失败证据保持。独立审查和新 Windows 执行继续开放；现阶段不报告已修复、三平台全绿或原生桌面通过。后续通过不会覆盖本次 failure 或此前 unknown 结果。
