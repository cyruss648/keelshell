# 有界目录比较验证记录

日期：2026-10-04

## 覆盖内容

- `keelshell-core::compare_directories` 对两组相对路径快照执行确定性排序和元数据比较。
- 路径重复、绝对路径、`.`/`..`、反斜杠、控制字符、超长路径和超过 10,000 项的快照在比较前拒绝。
- 文件类型、已知大小或已知修改时间冲突报告为 `Changed`；缺失字段报告为 `Uncertain`，不会静默当作相等。
- `keelshell-session::SftpSession::snapshot_tree_limited` 在真实 loopback SSH/SFTP 夹具上递归读取树，验证排序、深度边界、条目边界和零上限拒绝；不跟随符号链接。
- `keelshell-app::snapshot_local_directory` 在临时本地树上验证相对路径排序、文件根拒绝、深度边界和条目边界；实现是阻塞适配器，必须由后台 worker 调用。

## 已执行命令

```text
cargo fmt --all
cargo test -p keelshell-core --locked directory_compare -- --nocapture
cargo test -p keelshell-session --locked sftp_tree_snapshot_is_sorted_bounded_and_depth_explicit -- --nocapture
cargo test -p keelshell-app --offline directory_compare -- --nocapture
```

结果：核心单元测试 3 项、核心公共集成测试 2 项、真实 SSH/SFTP loopback 测试 1 项和 App 本地适配器测试 2 项通过。

## 边界

本记录证明快照适配器和比较引擎的边界，不证明本地目录扫描已在 UI worker 中接通，也不证明 Windows/Linux 原生文件选择器、内容哈希同步、差异应用、删除策略或生产服务器并发写入语义。当前结果是只读数据，仍需应用层审核计划后才能设计同步操作。
