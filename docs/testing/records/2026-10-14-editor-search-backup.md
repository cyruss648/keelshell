# 远程编辑器查找、替换与保存前备份 — 2026-10-14

范围是展开草稿编辑器中的字面查找、替换当前、全部替换，以及审核保存前的原内容备份。查找和替换只修改本机草稿；取消审核不写远端。保存沿用原审核、元数据复核和原子发布。发布前先把已复核原内容写入同目录隐藏文件 `.{name}.keelshell-backup-{uuid}.txt`。备份失败不发布新内容。目标发布结果未知时，已完成备份保留，原文件继续使用原隔离规则。目标被其它操作占用时，备份可能已经写入，但替换仍被拒绝。

## 已执行验证

- `cargo test -p keelshell-session --locked -- editor_reviewed --test-threads=4`：6 项通过。新增回环测试在目标临时文件关闭确认被持有时读回原文件和完整备份，释放后读回替换内容。
- `cargo test -p keelshell-app --locked -- editor_search --test-threads=4`：3 项通过。包含字面边界测试，以及真实 GPUI 点击：查找、单次替换、全部替换、取消零写入、确认后目标与备份内容分离。
- `python3 scripts/check.py`：实际退出 0。依赖策略、67 项 Python 测试（1 项跳过）、格式、严格工作区 Clippy、工作区测试，以及默认和显式 2 MiB 本地智能体控制器均通过。

## 未关闭边界

本记录不是原生文件管理器、macOS/Windows/Linux 桌面、辅助技术或 Windows `0xc0000409` 验收。备份不是版本历史或事务回滚；发布结果未知时不会自动删除备份或重放目标写入。

# Remote editor search, replacement, and pre-save backup — 2026-10-14

Scope covers literal find, replace current, and replace all in the expanded draft editor, plus an original-content backup before reviewed save. Search and replacement modify only the local draft; cancelling review writes nothing remotely. Saving retains the existing review, metadata recheck, and atomic publication. Before publication, the rechecked original is written to `.{name}.keelshell-backup-{uuid}.txt` in the same directory. Backup failure prevents publication. When target publication has an unknown result, the completed backup remains and the original file keeps its existing isolation rules. If another operation owns the target, the backup may already exist while replacement is still refused.

## Verification run

- `cargo test -p keelshell-session --locked -- editor_reviewed --test-threads=4`: 6 passed. The new loopback test reads both the original file and its complete backup while the target temporary close acknowledgement is held, then reads the replacement after release.
- `cargo test -p keelshell-app --locked -- editor_search --test-threads=4`: 3 passed. This includes literal bounds and real GPUI clicks for find, one replacement, replace all, cancellation with zero writes, and separated target/backup contents after confirmation.
- `python3 scripts/check.py`: exited 0. Dependency policy, 67 Python tests (1 skipped), formatting, strict workspace Clippy, workspace tests, and both default and explicit 2 MiB local-agent controllers passed.

## Boundaries still open

This record is not native file-manager, macOS/Windows/Linux desktop, assistive-technology, or Windows `0xc0000409` acceptance. The backup is neither version history nor transactional rollback. An unknown publication is not automatically cleaned up or replayed.
