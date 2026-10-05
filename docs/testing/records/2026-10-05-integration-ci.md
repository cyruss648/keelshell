# Ask、弹窗与 MCP 整合的三平台 CI — 2026-10-05

状态：精确提交 `a19d0c17f2e0e298b97805991f01e2329d494d4f` 的 [Quality 37253864420](https://github.com/cyruss648/keelshell/actions/runs/37253864420) 已结束 **failure**。macOS 和 Windows 成功，Linux 的文件布局 GPUI 测试失败。此结果与[本机整合及限定原生检查](2026-10-05-ai-modal-mcp-integration.md)分别记录。

后续文件场景同步修复已通过作者、根整合 1060 普通/8 文档/6 脚本及新非作者限定复审，保持原产品与测试期限；原 CI failure 不变，新提交 CI 待确认，见[同步修复记录](2026-10-05-files-scene-readiness.md)。确定性探针只证明该竞态类别，不能反推本次 CI 缺失的后台时序。

## 本次实际结果

| 平台 | Rust 和脚本 | 打包 | 系统 OpenSSH |
| --- | --- | --- | --- |
| macOS 26 | 1053 普通、8 文档、6 脚本通过；11 项普通测试按既有政策忽略；格式、严格 Clippy、x.y 和两种完整 CLI 控制器通过 | 57 项通过 | 9 项实际互操作通过，清理回执通过 |
| Windows 2025 | 1033 普通、8 文档通过；11 项普通测试按既有政策忽略；脚本为 5 通过、1 跳过；格式、严格 Clippy、x.y 和两种完整 CLI 控制器通过 | 53 通过、4 跳过 | 平台策略跳过，没有相应 artifact |
| Ubuntu 24.04 | 已执行普通目标合计 455 通过、1 失败、2 忽略，其中 app 为 375 通过、1 失败；6 脚本通过，文档未执行；未形成全仓通过结论；前置格式、严格 Clippy、x.y 已通过，workspace 测试退出 101 | 57 项通过 | 因前序失败而跳过，没有相应 artifact |

macOS、Windows 的 MCP 初始化 EOF 三项回归和真实 stdio 退出检查实际通过。Linux 在 app 目标中已失败，尚未执行 MCP 目标，不能用本次结果关闭旧 Linux EOF 失败。

## Linux 失败与后续范围

唯一失败目标为 `files::transfer_tests::workspace_layout_tests::real_file_rows_and_controls_survive_transfer_comparison_editor_and_review_states`。真实 SSH/SFTP 文件场景完成首组 900×580、AI 隐藏的 Running 测量后，GPUI 条件等待超过原 12 秒期限；日志记录 app 为 375 通过、1 失败。现有记录没有提供该次运行的具体前后台任务先后证明，因此不能直接宣称产品暂停缺陷、MCP 回归或调度负载是已确认根因。

布局测试使用每个 WRITE 200 毫秒的夹具延迟维持传输窗口；测量期间传输可能先完成，是新确定性探针的候选。后续修复必须保持真实传输、全部状态和布局断言，明确测试同步与后台完成顺序，不通过重跑旧 CI、跳过用例或扩大产品期限关闭失败。新候选、独立复核、根工程检查和新提交 CI 分别记录。

完整原始 Linux job 日志保存在 ignored `work/ci-a19d0c1-independent-20261005/linux-job-log-raw.stdout.log`，162759 字节，SHA-256 `8103c668f55b45b2584ba78cb09ee8d5262e98dce7cde979bf0784120bccea84`；原失败上下文和最初尚不能获取日志的读取失败也保留。macOS 的本次 OpenSSH artifact、原日志及 Windows 独立计数另行保存。macOS 清理回执只覆盖已观察身份，不能扩大为未知进程普查。

新的非作者只读审查结果为 `EXACT_CI_FAILURE_CONFIRMED`。82 份冻结证明的 manifest SHA 为 `31e0810f25e72ab2399f82480a2927c6b93006ae598da70d26d018c4d4d5fc19`；报告 SHA 为 `c7888fce2e347508a65c80633a70985143dcef0f5973154202a0f7a2e5884a8a`。API head 与三个真实 checkout 均匹配，根逐项核验 bytes/SHA 后复制到新的 ignored 验证范围。此复核只确认本次结果和保存材料，没有接受修复候选原因或启动 GUI、模型及新服务。

## 验收边界与清理

上述记录是本次实际 CI 的源码、测试、打包及 macOS OpenSSH 结论，没有 Windows/Linux 原生窗口、屏幕阅读器、供应商模型、正式六目标 Release、签名、公证或安装更新验收。MCP 方向仍为 KeelShell 向外部智能体提供服务；[Codex 的两轮业务失败](2026-10-05-codex-authorized-mcp-failure.md)保持原状态。

此前三个已完成增量的 managed worktree 已生成可恢复 Git 归档，真实目录及三个旧功能分支已删除；两个主目录独立审查 target 已删除。根在清理前再次逐项核验 241 份作者/复核复制材料，源码与失败证据、实际客户端尝试和当前开发包保留。新文件布局修复使用新的独立工作区。
