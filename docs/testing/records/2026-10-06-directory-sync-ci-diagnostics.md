# 目录同步提交级 CI 与测试诊断

## 精确提交与实际失败

任务历史提交 `0cca8ac59d12afdc68512b45db8830810835c634` 的 [Quality 37481523251](https://github.com/cyruss648/keelshell/actions/runs/37481523251) attempt 1 已实际结束：Windows 和 macOS 成功，Linux 失败，整体为 failure。这不代表三平台原生桌面验收。

Linux app 测试结果为 547 passed、1 failed、2 ignored。唯一失败是 `files::transfer_tests::directory_sync_reviews_exact_content_and_uploads_only_after_confirmation`，在原提交 `transfer_tests.rs:618` 的最终字节断言中，期望 `new!`，实际仍为 `old!`。该失败没有记录同步 worker 的终态、状态消息或确认点击后的准入状态；尚不能区分未准入、合法拒绝或执行错误，原因未确定。此前同 ID 保存资料审核测试在这次 Linux 运行中通过；不能据此追溯证明旧失败原因或把本次失败视为同一问题。

根已下载精确 attempt 元数据、三项完整原始 job 日志以及整个 attempt 的 ZIP，并读回全部 33 个 ZIP 文件。三个原始日志分别为 Windows 538,971 B、macOS 538,196 B、Linux 305,831 B；Linux 日志 SHA-256 为 `f7bee6b2a1a06f0a2d08d637bd4ad1126dd7e6397a242c7b07e9449308772147`，ZIP 为 471,281 B，SHA-256 为 `febc30c639fe95af8895ac1d827bbba38e40405de7d30463cbbe541474f0f0e7`。完整材料保存在忽略的私有 work 目录，不重新解释旧日志。

## 诊断增量与验证边界

原测试的 `idle` 条件仅表示 `busy` 已清除，失败或拒绝也会进入 idle。新增 test-only 观察在确认点击后记录 busy、worker、pending，在原最终字节断言失败时附带实际双语状态、终态与原子写入次数。保持原 12 秒等待、原确认次数和全部字节／目标保留断言；不增加重试，不改生产代码，不将未知失败改为通过。

整合镜像的先前 686 输入本机完整工程门禁已通过，包括同一既有同步测试；这只证明该源码与本机运行，不能宣称 Linux 原因已解决。新诊断与两项独立组合测试的当前源码仍需重新运行主树门禁及非作者限定复核；新提交级 CI 和 Linux 定位仍开放。
