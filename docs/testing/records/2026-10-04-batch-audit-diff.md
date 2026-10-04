# 批量摘要审计与文件差异验证记录

日期：2026-10-04

## 覆盖内容

- 批量命令在显式审核后完成时写入摘要审计：目标数、成功/失败/未知/未启动计数、取消和停止等待项标记。
- 审计记录只保留命令 SHA-256 与已保存配置 ID；序列化结果不包含命令正文、输出、主机地址或凭据。
- 片段编辑器与凭据设置模态关闭时，会触发待保存审计刷盘；GPUI 回归重新读取 `StateStore` 验证落盘结果。
- 远程文件编辑器可针对读取基线和当前草稿生成有界 unified diff；相同内容显示无变更，超限或非 UTF-8 输入会拒绝预览。

## 已执行命令

```text
cargo fmt --all -- --check
cargo test -p keelshell-core --locked diff::tests
cargo test -p keelshell-app --locked workspace::tests::command_workflows -- --nocapture
cargo check -p keelshell-app --locked
```

结果：核心差异算法 7 项测试通过；核心批量审计及其存储兼容测试随核心全量测试通过；工作区命令流程 12 项（含两项模态关闭刷盘回归）通过；应用检查通过。

提交 `3620580` 的 GitHub [Quality 运行 37169231359](https://github.com/cyruss648/keelshell/actions/runs/37169231359) 已于 macOS 26、Ubuntu 24.04 和 Windows 2025 全部成功；Windows 任务中的 Linux-only OpenSSH 步骤按矩阵条件跳过，macOS/Ubuntu 的 OpenSSH 步骤按流水线执行。

## 边界

这些测试证明有界数据结构、序列化兼容性和 GPUI 事件回调，不证明 Windows/Linux 原生桌面交互、远程生产服务器上的 SFTP 持久化、完整操作日志或自动恢复传输。现有三平台 Quality 仍需在本轮提交后重新运行。
