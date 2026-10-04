# 有界目录比较验证记录

日期：2026-10-04

## 覆盖内容

- `keelshell-core::compare_directories` 对两组相对路径快照执行确定性排序和元数据比较。
- 路径重复、绝对路径、`.`/`..`、反斜杠、控制字符、超长路径和超过 10,000 项的快照在比较前拒绝。
- 文件类型、已知大小或已知修改时间冲突报告为 `Changed`；缺失字段报告为 `Uncertain`，不会静默当作相等。
- `keelshell-session::SftpSession::snapshot_tree_limited` 在真实 loopback SSH/SFTP 夹具上递归读取树，验证排序、深度边界、条目边界和零上限拒绝；不跟随符号链接。
- `keelshell-app::snapshot_local_directory` 在临时本地树上验证相对路径排序、文件根拒绝、深度边界和条目边界；实现是阻塞适配器，必须由后台 worker 调用。
- 文件面板的“比较目录”操作只接受绝对本地目录和当前远程目录，在 worker 中读取两侧快照；结果卡片显示五类统计并最多展示 100 条路径，不提供同步写入、删除或覆盖动作。

## 已执行命令

```text
cargo fmt --all
cargo test -p keelshell-core --locked directory_compare -- --nocapture
cargo test -p keelshell-session --locked sftp_tree_snapshot_is_sorted_bounded_and_depth_explicit -- --nocapture
cargo test -p keelshell-app --locked files::worker::tests::local_snapshot_adapter_is_bounded_and_honors_cancellation -- --nocapture
cargo test -p keelshell-app --locked
```

结果：核心目录比较单元测试、核心公共集成测试、真实 SSH/SFTP loopback 快照测试、本地快照适配器测试和完整 GPUI 应用测试通过。当前完整应用测试为 267 项；核心库 69 项，会话库整仓门禁为 64 项，独立批量集成为 14 项，SSH loopback 为 95 项。

## 边界

本记录证明快照适配器、后台本地扫描函数、比较引擎和文件面板只读结果卡片的边界，不证明 Windows/Linux 原生文件选择器、内容哈希同步、同步计划、差异应用、删除策略或生产服务器并发写入语义。当前结果是只读数据，仍需单独的审核计划后才能设计同步操作。GitHub [Quality 37176372172](https://github.com/cyruss648/keelshell/actions/runs/37176372172) 已在三平台完成构建、测试和打包检查。
