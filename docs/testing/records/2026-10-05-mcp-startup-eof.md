# MCP 初始化回复与输入 EOF — 2026-10-05

状态：最终作者工程门禁、新非作者独立复审及根整合门禁均已通过，见[整合记录](2026-10-05-ai-modal-mcp-integration.md)。新 `a19d0c1` CI 的 macOS/Windows EOF 回归实际通过；Linux 在 app 文件布局目标先失败，未执行 MCP，见[三平台记录](2026-10-05-integration-ci.md)，因此原 Linux 失败尚不能关闭。

## 触发证据

精确 `40d092ca61c4c229acb4a004d2852dd858e86aed` 的 [Quality 37244888512](https://github.com/cyruss648/keelshell/actions/runs/37244888512) 最终失败。macOS、Windows 成功；Linux 的 `unread_stdout_then_eof_still_exits_within_shutdown_deadline` 在子进程成功退出断言失败，没有超出三秒截止。旧断言未附带 stderr，具体子进程诊断未知；完整失败日志保留，不自动重跑来覆盖。

根在独立 worktree 使用官方 SDK 构造确定性调度：写 actor 完整 flush 初始化正回复后，暂不重新 poll 启动 future；实际关闭输入，读 actor 发布 EOF，再恢复 future。原生产源码返回 `Err(Startup)`，回归实际 exit101。20 次旧 macOS stdio 子进程观察均成功，仅为诊断对照，不是 Linux 通过证据。

## 修复和回归

成功的 typed `InitializeResult` flush 使用连接内 atomic 位记录，启动关闭分类在自有 I/O join 后读取终态。正回复后正常 EOF 可以成功退出；预初始化 ping 的成功回复和缺少协议元数据的错误回复均不能将未建立会话升级为正常启动。预算耗尽与 I/O 失败继续拒绝，不改变认证或执行规则。设计见 [ADR0049](../../adr/0049-mcp-initialization-eof-classification.md)。

进程断言补充退出状态和静态 stderr，便于后续失败诊断。早期候选套件输出与缺少整套源码冻结的状态保留，不能代替最终源码验证。

最终冻结源码的作者 `scripts/check.py` 实际 exit0：1034 普通、8 文档、6 脚本测试，fmt、x.y 策略、严格 workspace/all-targets Clippy 及默认/显式 2 MiB 的完整 CLI 控制器通过，11 项按既有政策忽略。461 个输入在整次 480.782 秒运行前后保持 SHA，完整日志 SHA `9640bf98c50820e950800e052a30f642200317711c634b8c2f0e77cc182e87c4`。此项作者门禁不能代替独立复审、根整合或新目标 CI。macOS 受控流与本机进程不代表 Linux CI、Windows/Linux 桌面或正式发布签名通过。

新非作者独立复审通过，无剩余 P1/P2。独立 MCP 全套 68 普通与 1 文档测试通过，包含 11 项真实 stdio 与三个确定性 EOF 场景；五个私有追加场景确认 flush 失败或 Pending 不升级、成功初始化后 reader I/O 错误与 33 帧预算仍拒绝、外部取消原语义不变，以及正常 EOF 后双 I/O actor Drop。原与私有源码的 all-targets 严格 Clippy、格式和 x.y 策略通过，自有检查进程已退出、TMP 已空。作者 10 份与复核 23 份证据经根逐项核验并复制保存，复核 manifest SHA `e514d691cb62088ee5aa4ce2ae1c9b7922d40c80a75400c3868225ead7deb589`。本机独立复审不关闭原 Linux CI，须由新提交的新流水线确认。
