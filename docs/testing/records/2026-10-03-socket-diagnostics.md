# 监听端口诊断记录 — 2026-10-03

## 范围

本次新增远程 Linux 监听端口诊断。它只在已建立的 SSH 会话中执行固定的 `ss -H -lntup` 命令，读取 TCP/UDP 监听行并显示在监控面板；用户可以手动触发，资源监控的周期刷新不会自动发送该命令。

解析器限制协议为 TCP/UDP、单行字段和总输出大小，并保留本地地址、对端地址、状态与 `ss` 提供的进程文本。输出只作为诊断数据展示，不会重新拼接为可执行命令，也不代表服务已经接受过连接。

## 验证

- `cargo test -p keelshell-session`：监听行解析、未知协议、截断行和现有 SSH/SFTP 回环测试通过。
- `cargo test -p keelshell-app --all-targets --locked`：GPUI 监控面板与其他应用测试共 56 项通过。
- `cargo clippy -p keelshell-app --all-targets --locked -- -D warnings`：通过。
- `cargo fmt --all -- --check`：通过。

## 边界

- 夹具和当前开发机没有提供真实 Linux 主机的服务探测证据；尚未完成 Windows/macOS 适配，因为该诊断目前明确是 Linux `ss` 能力。
- 没有进行端口连接探测、服务健康检查、进程重启或配置修改。
- `ss` 输出中的进程字段依赖远端权限，缺失时会保留为空；这不改变地址和状态的读取语义。
