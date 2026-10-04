# 有界内容校验与目录同步计划验证记录

日期：2026-10-04

## 覆盖内容

- `keelshell-core::hash_directory_content` 对 SHA-256 输入执行 64 MiB 字节上限，超限直接拒绝。
- 两侧相同内容摘要在修改时间不同的情况下报告 `Same`；摘要不一致报告 `Changed`；只有一侧摘要或缺失其他必要字段时报告 `Uncertain`。
- `plan_directory_sync` 对改变和单侧路径生成复制操作，对目标侧独有路径按策略保留或生成显式删除操作；不包含自动执行入口。
- `DirectorySyncPlan::confirm` 只接受当前计划的审阅指纹；错误指纹拒绝，正确指纹只返回无写入能力的确认回执。
- 公共集成测试验证来源摘要、来源大小和已有目标的类型/大小被携带到复制操作，显式删除也绑定目标类型/大小；不确定行在计划阶段被拒绝。

## 已执行命令

```text
cargo fmt --all
cargo test -p keelshell-core --locked
cargo clippy -p keelshell-core --all-targets --all-features --locked -- -D warnings
```

结果：核心库 76 项单元测试、公共目录比较与同步计划集成测试、3 项文档测试和严格 Clippy 通过。

## 证明边界

本记录证明摘要计算、比较分类、计划确定性和确认令牌边界，不证明真实本地或远程文件读取、SFTP 写入、并发修改检测、符号链接处理或 Windows/Linux 原生文件交互。应用层尚未接入摘要收集和同步执行，计划仍需未来的 transport worker 在写入前重新校验。
