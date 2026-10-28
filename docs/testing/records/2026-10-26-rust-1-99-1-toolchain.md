# Rust 1.99.1 工具链与项目版本限制记录 — 2026-10-26

## 范围

本记录对应用户要求：本机切换到 Rust 1.99.1，清理旧 Rust 工具链，并取消项目配置对 Rust 版本的强制限制。

项目层面现在不包含 `rust-toolchain.toml`，workspace 和各 crate manifest 不声明 `rust-version`，`mise.toml` 只保留任务而不管理 Rust 版本，CI 也不安装或选择旧的精确工具链。Windows 原生诊断脚本不再把 1.98.1 写死为准入条件，而是检查实际运行环境为 Windows x64 MSVC Rust 工具链并把实际 release 写入收据。

直接 registry 依赖的 `x.y` 声明规则、应用 `Cargo.lock` 的解析版本、依赖安全边界和跨平台原生验收边界保持不变。历史记录中的 Rust 1.98.1 是对应运行时的事实，不会被回写成 1.99.1。

## 本机工具链读回

已安装并设为默认：Rust `1.99.1-aarch64-apple-darwin`，`rustc 1.99.1 (58cf8f9c3 2026-10-13)`，`cargo 1.99.1 (5f94df478 2026-08-27)`。

已卸载：`1.98.1-aarch64-apple-darwin` 和重复的 `stable-aarch64-apple-darwin`。当前只保留显式的 `1.99.1-aarch64-apple-darwin`。

## 工程验证

以下结果绑定本记录对应提交，命令在 macOS Apple Silicon 本机使用默认 Rust 1.99.1 执行：

| 检查 | 结果 |
| --- | --- |
| `cargo metadata --no-deps --format-version 1` | 通过；5 个 workspace package 的 `rust_version` 均为 `null` |
| `cargo fmt --all -- --check` | 通过 |
| `git diff --check` | 通过 |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` | 通过；0 errors，保留既有 `block 0.1.6` future-incompatibility 提示 |
| `cargo test -p keelshell-core --locked --offline` | 505 passed |
| `cargo test -p keelshell-ai --locked --offline` | 176 passed，5 ignored |
| `cargo test -p keelshell-app --locked --offline -- --test-threads=4` | 761 passed，2 ignored |
| `python3 scripts/check.py` | 退出 0；Python 67 passed，1 skipped；workspace Rust 合计 1930 passed，0 failed，23 ignored |

AI 全量测试第一次运行时有一个隔离代理用例在子进程已退出且输出已完整写出后出现 `pipe EOF deadline`；该用例单独重跑通过，随后 AI 全量测试重新运行得到 `176 passed，5 ignored`。这次收尾抖动不改变代码断言结果，但保留在记录中以区分测试观察问题与产品失败。

本记录不宣称 Windows、Linux 或 macOS 发行包的原生桌面验收，也不宣称发布签名、公证、安装覆盖和完整产品目标已经完成。

## 独立复核与远端读回

独立 reviewer 会话 `01a180b2-e860-71a1-8f04-b3481c439384` 在隔离 worktree `/Users/hdh/.codex/worktrees/9884/keelshell` 只读审查了 `80aa7207c1b67963cbd8b54c0ba2f990ec71aad6`。结论为 `MERGE`，P0/P1 均无。复核确认 `try_update` 是 Rust 1.99.0 对 `fetch_update` 的更名，调用参数、闭包重试、`Result` 判定和 memory ordering 未改变；toolchain 配置、CI、Windows 收据解析和 Cargo.lock 边界一致。

复核提出两个 P2 跟进项：源码使用 `try_update` 形成 Rust 1.99.0+ 的实际编译下限，但项目不再提供机器可读的 `rust-version`，后续可在文档中明确该事实；Windows `rustc -Vv` release/host 解析可抽成纯函数并增加缺失字段与错误 host 的直接反例测试。两项均不阻止本次合并。

提交已推送并读回：`HEAD=80aa7207c1b67963cbd8b54c0ba2f990ec71aad6`，`origin/main=80aa7207c1b67963cbd8b54c0ba2f990ec71aad6`，`HEAD...origin/main=0 0`。reviewer 的 session crate 并行全量运行另观察到两个未改动的临时目录路径夹具失败；MCP 的并行 fixture 超时单独重跑通过。主工作区完整 `scripts/check.py` 已以 Rust 1.99.1 退出 0，具体数字见上表。
