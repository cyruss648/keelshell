# 命令片段领域与存储回归 — 2026-10-03

## 实现范围

`keelshell-core/src/snippets.rs` 为已有 `Snippet` 和 `AppState.snippets` 增加三个显式编辑入口：

- `insert_snippet(Snippet)`：追加调用方提供 UUID 的片段，拒绝重复身份。
- `update_snippet(Snippet)`：按相同 UUID 更新原位置；未知或歧义身份报错，不隐式新增。
- `remove_snippet(Uuid)`：返回被删除的完整片段，保留剩余条目顺序；未知或歧义身份报错。

三个方法都在候选快照上执行修改，完整校验后才替换调用方的内存状态。失败保持数据与 `SnapshotRevision` 不变；成功也保留原 revision，随后必须通过同一 `StateStore` 显式保存并采用其返回的新快照。错误只含静态字段及约束，不拼入命令文本。

沿用现有 schema 和限制：最多 2,000 个片段，名称 120 个 Unicode 字符、描述 2,048 个 Unicode 字符，最多 32 个标签、每个 64 个 Unicode 字符。命令正文最多 65,536 个 UTF-8 字节，保留多行、Tab 和首尾空格；拒绝空白正文、CR、ESC、NUL 和其他控制字符。没有新增依赖或改变磁盘格式。

## 验证

本机执行时设置独立临时目录和四个测试线程；全部夹具使用临时文件及虚构内容。

```sh
rustfmt --edition 2024 --check crates/keelshell-core/src/snippets.rs crates/keelshell-core/tests/snippets.rs
cargo test -p keelshell-core --test snippets --locked -- --test-threads=4
cargo clippy -p keelshell-core --all-targets --locked -- -D warnings
cargo test -p keelshell-core --locked -- --test-threads=4
```

- 定向格式与严格 Clippy 通过。
- 新增集成回归 **13 项通过**，包括稳定 UUID/顺序、同名不同身份、重复/缺失/歧义 ID、中文字符和 UTF-8 字节边界、多行/Tab/终端控制字符、集合上限、无关配置失败时的原子性。
- 真实 `StateStore` 测试验证新增、编辑、删除与重开磁盘后的内容一致；内存编辑不创建文件、不改 revision。
- 两个独立 store 读取同一版本后，较早提交成功；较晚更新或删除返回 `Conflict`，磁盘字节保持为胜出版本，失败草稿保持可编辑。
- 单条合法但累计超过 4 MiB 的存档返回 `TooLarge`，不覆盖原文件，也不清空草稿。
- 完整 core 回归通过 **122 项单元/集成测试及 1 项文档测试**。

日志位于 ignored `work/snippet-domain-{format,tests,clippy,core-tests}.log`。第一次格式检查发现新文件排版差异，按 rustfmt 处理后通过；无测试或 Clippy 失败。

## 边界

命令片段作为用户明确保存的明文保存在本地配置。领域层不会推测并移除秘密，也不会从终端输出或会话历史自动创建片段。以上检查只证明领域规则及本机存储语义；片段界面、会话目标绑定、候选选择、原生平台交互和远程执行须由应用集成记录另行验证。
