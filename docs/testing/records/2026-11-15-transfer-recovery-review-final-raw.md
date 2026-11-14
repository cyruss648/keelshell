KeelShell 独立、严格只读代码与功能复核

审查目标：e2b00af3cb8bcae67d54cbf69cf04e8baab2b7cd
审查范围：Discard / persisted plan progress 成功 mutation 后的 recovery store reload、旧 load callback generation 拒绝、新增 GPUI 交错测试、claim 生命周期、错误/失败路径与回归风险。
结论：APPROVE
P0/P1/P2/P3：0/0/0/0

关键证据：
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1079-1109：load_recovery_records 在后台读取 store，并捕获 session、load generation、mutation generation。
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1120-1142：apply_recovery_load 在写入 recovery_records 前同时检查三项身份；旧 callback 会被直接拒绝。
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1144-1175：成功的 persisted recovery progress mutation 先推进 mutation generation，再启动新的 recovery store reload；失败时保留当前状态并返回错误。
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/parallel.rs:858-897：Discard 成功时推进 mutation generation、移除当前记录并启动新的 recovery store reload；失败路径只报告错误，不伪造成功或刷新为错误快照。
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:1610-1661、1774-1865：persisted plan 的 claim 只有在计划结果有效且 progress 成功持久化后才重新交回 review；失败、旧 session、挂起或 stale review 路径释放/丢弃 claim，不把旧计划重新放回 UI。
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/parallel.rs:730-815、297-320：claim request 绑定 session 和 record ID；旧 session、claim 不匹配、记录消失和失效审核均 fail closed，局部 claim 在返回时释放。
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/transfer_recovery.rs:251-260、317-439：claim 由 OS 文件锁持有，upsert/remove 校验 journal 与 record ID，Drop 释放 claim；core claim/store 回归通过。
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/parallel_tests.rs:799-888：GPUI discard 交错测试保存旧快照，执行 durable mutation，调用旧 callback，再等待由生产 load_recovery_records 启动的新异步快照；最终验证旧记录和并发完成记录均不会回流。
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/parallel_tests.rs:891-1012：GPUI persisted plan progress 交错测试验证旧 callback 不能覆盖新进度，且新快照只保留当前 durable record。
- /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/parallel_tests.rs:218-335、459-556、558-635、744-796：实际 GPUI discard 点击、计划进度持久化后取消、确认失败 claim 释放、并发终态 reload/清空列表均有独立回归。

测试与检查：
- `CARGO_TARGET_DIR=/private/tmp/keelshell-independent-review-e2b00af3/target rtk proxy cargo test -p keelshell-app --locked files::transfer_tests::parallel_tests:: -- --nocapture`：19 passed / 0 failed / 0 ignored；app 清单为 1176 tests，1157 filtered。
- `CARGO_TARGET_DIR=/private/tmp/keelshell-independent-review-e2b00af3/target rtk proxy cargo test -p keelshell-app --locked parallel_tests::stale_recovery_load_cannot -- --nocapture`：新增两个交错测试 2 passed / 0 failed。
- `CARGO_TARGET_DIR=/private/tmp/keelshell-independent-review-e2b00af3/target rtk proxy cargo test -p keelshell-core --locked transfer_recovery -- --nocapture`：core transfer_recovery 单元 8 passed / 0 failed；其他 integration binaries 为 0 matching tests。
- `CARGO_TARGET_DIR=/private/tmp/keelshell-independent-review-e2b00af3/target rtk proxy cargo test -p keelshell-core --locked --test transfer_recovery -- --nocapture`：3 passed / 0 failed。
- `CARGO_TARGET_DIR=/private/tmp/keelshell-independent-review-e2b00af3/target cargo fmt --all -- --check`：exit 0。
- `CARGO_TARGET_DIR=/private/tmp/keelshell-independent-review-e2b00af3/target rtk proxy cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`：exit 0。
- `git diff --check 5fdd783e9459fb9890e2c6735ada808ec6aeb597..e2b00af3cb8bcae67d54cbf69cf04e8baab2b7cd`：exit 0。
- 已知 app linker warning：`__eh_frame section too large`；没有改变测试或 Clippy 退出码。

交错测试的边界：测试在真实 GPUI window/context 中执行，旧 callback 通过生产 apply helper 注入以确定性复现顺序，新快照由生产后台 reload 路径实际产生；它没有阻塞真实文件 I/O 来制造不可预测的调度抖动。该边界不构成当前修复的 P0-P3 缺陷，实际 discard 点击和 persisted plan worker 路径另有回归覆盖。

未验证边界：本次没有执行完整 scripts/check.py，也没有把 headless GPUI/loopback 证据扩展为 macOS/Windows/Linux 原生三平台、真实客户 SSH/SFTP、Podman、外部 AI、IME、辅助技术、签名/公证、安装升级或发行验收。作者 full gate 由主线程另行提供，不能替代这些边界。

隔离与清理：reviewer 使用 detached worktree `/private/tmp/keelshell-independent-review-e2b00af3/worktree`，独立 target `/private/tmp/keelshell-independent-review-e2b00af3/target`，未修改目标源码工作树、未格式化写回、未 commit/push/merge，未使用共享 target；复核后已移除 reviewer worktree、target、日志目录和 Git worktree 元数据。原始报告文本保留于 `/private/tmp/keelshell-independent-review-e2b00af3-raw-report.txt` 供主线程归档。
