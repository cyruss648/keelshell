# SFTP 权限修改验收记录

日期：2026-10-03

## 范围

验证远程文件面板的审核式 POSIX mode 修改：独立八进制输入、不可变选中路径、确认后 SFTP `SETSTAT`、符号链接拒绝、双语反馈和刷新边界。

## 结果

- `cargo test -p keelshell-session --test ssh_loopback set_permissions_changes_only_the_remote_mode_bits -- --nocapture`：通过。
  - 临时真实 TCP SSH/SFTP 服务器上的 `/mode.txt` 从 `0644` 改为 `0640`。
  - 文件内容大小保持 7 字节。
  - `10000` 被类型化为无效 mode，未发送有效修改请求。
- `cargo test -p keelshell-session --test ssh_loopback reviewed_permissions -- --nocapture`：2 项通过。
  - 目录可以从 `0700` 改为 `0750` 并返回新 metadata。
  - 旧 mode 快照、符号链接目标和符号链接父目录均在 `SETSTAT` 前拒绝。
- `cargo test -p keelshell-app --bin keelshell-app files::tests -- --nocapture`：8 项通过。
- `cargo test -p keelshell-app --bin keelshell-app files::transfer_tests::reviewed_permissions_change_uses_the_exact_selected_entry_and_refreshable_result -- --nocapture`：通过。
  - 真实 GPUI 面板先生成确认条，再执行不可变选中项，确认结果要求刷新列表。
- `python3 scripts/check.py`：通过。包含格式、Clippy、依赖策略和工作区全量测试。
- `python3 -m unittest discover -s packaging -p 'test_release.py' -v`：18 项通过。
- `python3 -m unittest discover -s packaging -p 'test_publish.py' -v`：29 项通过。
- GitHub Actions Quality `37119306924`（提交 `790bf1c`）：macOS 26、Ubuntu 24.04、Windows 2025 全部通过；macOS/Linux 另有 4 项 OpenSSH 互通通过，Windows 按工作流条件跳过该服务器步骤。
- `cargo check -p keelshell-app --all-targets`：通过。

## 实现边界

验证使用受控临时 SSH/SFTP peer，不代表任意生产服务器的 ACL、所有权、扩展属性或 Windows 原生权限语义。应用只修改 POSIX mode 字段；执行前会复核目标和父目录，执行后读回 mode；服务器端权限策略仍可能拒绝操作。符号链接在 UI 和 worker 两层拒绝，避免把链接目标当作可审核的固定路径。
