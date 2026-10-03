# 一次性 SSH 快速连接验收记录

日期：2026-10-04  
范围：空工作区快速连接、显式保存入口、直连路线作用域

## 验证内容

- GPUI 回归测试填写主机、端口、用户名并切换密码认证；点击连接后进入现有登录提示，路线标记为临时路线，连接库和状态存储均保持为空。
- 取消一次性连接后重新加载状态，确认没有连接配置或最近连接写入。
- “保存为连接”只把草稿复制到标准连接编辑器；未点击编辑器的保存按钮前，状态存储仍为空。
- Core 路线测试确认直连路线可以在没有保存 profile 的情况下生成规范化身份和主机密钥作用域，且不会出现在 `AppState::connection_route` 中。

## 命令

本记录随功能提交更新。提交前至少运行：

```text
cargo fmt --all -- --check
cargo test -p keelshell-core --test routes --locked
cargo test -p keelshell-app --bin keelshell-app quick_connect_ --locked
cargo clippy -p keelshell-core -p keelshell-app --all-targets --locked -- -D warnings
python3 scripts/check.py
```

## 本次结果

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test -p keelshell-core --test routes --locked`：27 项通过。
- `cargo test -p keelshell-app --bin keelshell-app --locked`：246 项通过。
- `python3 scripts/check.py`：整仓测试、文档测试和依赖策略通过；OpenSSH 互操作测试按脚本配置保持忽略。

## 证据边界

这些测试验证内存路线、持久化边界和 GPUI 交互，不代表已经连接真实生产主机。密码、私钥口令、跳板、代理和高级重连设置仍由持久化编辑器处理；Windows/Linux 桌面原生验收另行记录。
