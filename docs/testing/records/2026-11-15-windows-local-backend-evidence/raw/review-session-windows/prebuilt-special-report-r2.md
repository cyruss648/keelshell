**r2 六项预构建专项独立执行报告**

记录时间 UTC：2026-11-15T13:42:37.578076+00:00。执行主体为本 reviewer；构建主体为父。环境 Darwin arm64。副本在 review-session-binaries-r2，所有输出在 review-session-windows；未访问原始 target，未独立重编译。Native NOT_RUN。

父副本 [receipt.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-session-binaries-r2/receipt.json) SHA-256 `af22cf3be08ea51c6e0244c84537ee66bd8a925f3e6d20b73d6011b6cb82fb16` 绑定工程 r2 `1fb9b4b343d25634fed4bc6cf9ec15d12ac7049562fd83f977960d9d0b78e1c9`。父称原件/副本前后相同；本 reviewer 独立检查副本本身的前后哈希，未读原件来验证其陈述。

| 副本 | 每次枚举/执行前后 SHA-256 | 独立 --list 库存 |
| --- | --- | ---: |
| session | `f49acffd8e45528769264228cb481777e5e3195c32cbdcc5fb4e32bd1ed03d3c` | 194 |
| app | `f19a98c17ca52619cc13e7c1e16332958c7b594691291099833f45b75eccaebc` | 1239 |

两个 --list 均 exit0/reaped，30s 外层界限；输出与父给的 inventory 字节一致。以上是副本 host 库存，不是 Windows 编译库存或 PE --list。

每项测试命令均通过 Python subprocess argv 直接执行指定副本；cwd 为本 checkout，stdin=DEVNULL，timeout=45 秒。测试内部使用独立临时数据和有界 barrier。无超时、无失败、无 ignored，合计 6/6；以下耗时为外层实测 wall time。

| 命令（副本名称及参数） | 通过/选择 | exit / reaped | 耗时 | 原始输出 |
| --- | ---: | --- | ---: | --- |
| `session mutation_worker --nocapture` | 3/3 | 0 / true | 0.019s | [stdout](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-session-windows/prebuilt-r2-session-mutation_worker.stdout.log) / [stderr](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-session-windows/prebuilt-r2-session-mutation_worker.stderr.log) |
| `session local_unicode_case_aliases --nocapture` | 2/2 | 0 / true | 0.007s | [stdout](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-session-windows/prebuilt-r2-session-local_unicode_case_aliases.stdout.log) / [stderr](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-session-windows/prebuilt-r2-session-local_unicode_case_aliases.stderr.log) |
| `app archive_unobserved_worker_result --nocapture` | 1/1 | 0 / true | 0.014s | [stdout](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-session-windows/prebuilt-r2-app-archive_unobserved_worker_result.stdout.log) / [stderr](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-session-windows/prebuilt-r2-app-archive_unobserved_worker_result.stderr.log) |

handoff 的 3 项为 stage 取消窗口、publication 取消窗口及正常观测正控，执行生产共享 worker、scope 和 reservation，并进行真实 host 文件写入/rename。Unicode 两项覆盖 missing 组件及真实 staging 后的 Unknown；app 一项验证实际错误映射、close hold 与 Uncertain phase，不是 GPUI 窗口全流程。

[枚举命令/哈希回执](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-session-windows/session-combined-r2-prebuilt-inventory.json) 与 [执行命令/计数/哈希回执](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-session-windows/session-combined-r2-prebuilt-execution.json) 保存完整绝对命令、exit、reaped、外层界限和 stdout/stderr SHA。session/controller integration 本轮没有副本，动态 controller 证据仅从父回执读回。没有运行全库、大组或 Windows PE。
