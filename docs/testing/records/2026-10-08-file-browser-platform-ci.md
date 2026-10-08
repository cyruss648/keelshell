# 文件浏览平台 CI 跟进 — 2026-10-08

状态：本机 `feature/browser-platform-controls` 的完整工程检查与 57 项打包测试已实际通过；新的非作者最终源码／工程证据复核无 P1/P2 阻断，新提交／推送后的精确 CI 另行验收。此次只修改测试，没有新的原生 GUI 结果。

## 原提交结果

精确主线 `7c11954761b75243d82e1b52282aeb822e271b12` 的 [Quality](https://github.com/cyruss648/keelshell/actions/runs/37683724603) 已结束。

| 平台 | 结果 | 完整原日志 SHA-256 |
| --- | --- | --- |
| macOS | success | `72b00587e9f8d4c82458f54f3f70d8cf4b8fb43d4a4b539cb03a139c2e0395e2` |
| Windows | failure：本地导航拒绝测试 | `863b0219ec0dd5c065a8ad87a698297583b66db9209267f7ad6c8353415723a3` |
| Linux | failure：夹具组准入 120 秒超时 | `419935a19c9f0d5484c06d35a99e8c43308dc02b8e973bc983e8a89fc0d4e33a` |

Windows 的 `late_local_navigation_cannot_replace_the_last_explicit_directory` 在 failed-navigation listing 断言失败。隔离夹具返回 canonicalize 后的 Windows verbatim 路径，`second.join("..")` 在进入生产校验前归一化为合法父目录。原测试没有保持其意图中的 ParentDir 输入。新输入使用原生 OsString 字面拼接，并先断言确有 ParentDir；原最新目录、失败后无旧 listing、业务断言与 12 秒等待保持。新增 Windows 条件控制直接证明 join 的归一化及字面路径在 I/O 前返回 UnsupportedPath。此控制在 macOS 不执行，需要新 Windows CI，不能视为本机已验证 Windows。

Linux 的 `wrapped_file_actions_still_review_the_exact_selected_remote_target` 在创建 SSH peer、目录、窗口和业务动作前准入超时；app 704 passed／1 failed／2 ignored，855.47 秒。历史准确 owner 和原因仍 UNKNOWN。测试专用观察模块只保存固定标签、数字和 Weak；记录实际 permit 与 queue 清理阶段，在超时后输出有界快照，原失败保持。七项 observer 控制、原 FIFO／清理／跨 App／worker 控制与四线程完整门禁已在本机实际通过，见[诊断记录](2026-10-08-file-fixture-admission-diagnostics.md)。不延长 120 秒、不加重试、不忽略或串行绕过。

诊断候选 v1 的非作者静态审查发现五处测试 expect 违反 workspace deny；本次改用既有 Checked trait，错误标签及全部原断言保持。该 P2 是静态配置冲突，原候选未运行 Clippy；本次实际编译及严格检查另记结果。新观察不更改生产业务。

原三份日志、失败下载/解析探针和只读分析材料保留在忽略的 `work/ci-7c11954-20261008-v1`、`work/browser-windows-parent-path-review-20261008-v1`、`work/browser-linux-ci-admission-review-20261008-v1`。本次结果记录于 `work/browser-platform-controls-main-20261008-v1`。

## 本机正式检查

同一 813 输入、16,016,468 字节在检查前后保持一致。`python scripts/check.py` 实际 wait=0，耗时 1,181.77 秒：依赖 x.y 策略、6 项 Python、格式、全 workspace／all targets 严格 Clippy、1,776 项普通 Rust 和 10 项 rustdoc 全部通过，22 项 ignored 未执行。原默认／2MiB 控制器各 626 条阶段以及十项准备失败和目录观察 IO 控制通过；没有调用供应商 CLI 或模型。应用 712 passed、0 failed、2 ignored，包括新增七项 observer 和原三项 FixtureGroup 控制。

同输入 57 项打包测试实际 wait=0，1.33 秒。两次所属进程组均消失，私有 scratch 为空并移除。根完整读取原日志和输入，工程日志 SHA-256 为 `4d0702439aee168703a47c6514a6256ec74fa8684c4f121d39155c1773d5655f`，打包日志为 `ecf489f5e75a46ab0a0c67866b6ee916222b29709f0dc32aedccb9245e14b520`。随后只更新四份 Markdown 状态，所有非文档输入保持。本次没有重建应用包或新增原生流程，因为修改均为测试；不继承此前原生结果为本轮验收。

新 Windows 条件控制仍未在本机执行，Linux 历史超时原因仍 UNKNOWN；必须分别读取精确新提交的三平台 CI 结果，不把本机成功表述为三平台通过。

新的非作者已独立读取完整 7 路径改动、两份原日志、两套 813 冻结输入和最终四份状态文档，当前 809 非状态输入保持；独立解析检查数量与上述一致，并检查实际进程组／scratch 均不存在。源码与限定工程证据无 P1/P2 阻断。没有运行新的 GUI 或替代 Windows/Linux CI；小审查记录保留在忽略的 `work/browser-controls-final-review-20261008-v1`。

## 精确修复提交 CI 结果

`a717400bf791bad43a293db50a02a64fb58bcfc7` 的 [Quality 37696576523](https://github.com/cyruss648/keelshell/actions/runs/37696576523) 已结束，macOS 和 Linux 成功，Windows 失败；三平台整体仍未通过。三份完整 job 原日志已实际下载，首次因工具拒绝终端转义的失败记录保留，成功的 v2 原文没有覆盖它。

| 平台 | 结果 | 完整原日志 SHA-256 |
| --- | --- | --- |
| macOS | success | `a470475e74c6b71d8ec5ef22ab9a23b8c587faaec979d5ca9831d8f8d5544214` |
| Linux | success | `7565e399b8cfe094e8e70a1894c02e704912c5919d6bcec458c34798fc0425b2` |
| Windows | failure：应用测试进程异常退出 | `3ec1d3eb878e32457931744f883ce2dde015dae82ee0db1c012ad497955d9399` |

Windows 的四线程应用测试进程以 `0xc0000409 / STATUS_STACK_BUFFER_OVERRUN` 异常退出，原日志没有单项 Rust `FAILED` 或准确触发者，不将该状态码直接归因为栈溢出。包装检查通过，后续完整 workspace 未完成；新的只读独立审查与有界诊断正在核对，保持原测试断言和期限。Linux 新成功不追溯为旧 120 秒超时准确原因已查明。本轮日志和下载回执保留在忽略的 `work/ci-a717400-20261008-v1`；没有新的 Windows/Linux 桌面 GUI 验收。
