# 诊断、探测、更新与历史清理验收

日期：2026-10-03。对应提交：`2ee0693c02f1ceca4796d5ccc500ff48a487bd6a`。

## 本地验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test -p keelshell-ai --all-targets --locked`：通过，包含诊断计划、模型发现和 HTTP 约束测试。
- `cargo test -p keelshell-session --all-targets --locked`：通过，包含监听 socket 解析和远程 TCP 探测测试。
- `cargo test -p keelshell-app --all-targets --locked`：通过，包含助手诊断计划、监控探测和更新面板单元/GPUI 测试。
- `python scripts/check.py`：通过，依赖策略、格式、严格 Clippy 和工作区测试均通过。
- `python -m unittest discover -s packaging -p 'test_*.py' -v`：通过。
- `python scripts/changelog.py check --version 0.1.0` 与 `py_compile`：通过。
- `git diff --check` 及当前工作树名称扫描：通过。

## 远端验证

[Quality 运行 37125780395](https://github.com/cyruss648/keelshell/actions/runs/37125780395) 对提交 `2ee0693c02f1ceca4796d5ccc500ff48a487bd6a` 成功：

- Ubuntu 24.04：Rust 质量、打包回归、Linux OpenSSH 互通和清理步骤通过。
- macOS 26：Rust 质量、打包回归、macOS OpenSSH 互通和清理步骤通过。
- Windows 2025：Rust 质量、打包回归和清理步骤通过；Unix OpenSSH 步骤按平台跳过。

## 历史与仓库状态

- 公开仓库为 [cyruss648/keelshell](https://github.com/cyruss648/keelshell)，远端 `main` 与本地提交一致。
- 早期公开提交中的旧产品别名已从可达历史移除；当前树、提交说明、发布说明和 GitHub 仓库描述均通过名称扫描。
- 本阶段通过 `git push --force-with-lease` 更新远端主线；未创建版本标签，因此没有触发 Release 发布。

## 证明边界

更新面板的测试覆盖发布元数据约束、平台资产匹配、SHA-256 校验、临时目录暂存和失败状态；没有执行真实 GitHub 下载，也没有覆盖运行中安装。签名、公证、安装权限、运行中替换和回滚仍需独立的三平台原生安装器验收。Quality 矩阵证明构建与受控互通，不等于任意生产 SSH 主机或完整桌面体验已验收。
