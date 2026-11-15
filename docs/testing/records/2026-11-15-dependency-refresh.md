# 2026-11-15 直接依赖官方复查与 `nix 0.32` 更新

## 结论

本次复查发现 `keelshell-ai` 的 Unix 目标直接依赖 `nix = "0.31"` 已落后于
官方稳定的 `0.32.0`。Manifest 已调整为 `nix = "0.32"`，继续满足项目要求的
`x.y` 直接版本约束；Cargo.lock 同时保留 `nix 0.31.3`，因为
`process-wrap 10.0.0` 的传递约束仍要求 `nix ^0.31.1`。直接依赖检查随后重新
通过，未修改 Rust 工具链 pin 或 `rust-version`。

## 实际验证

工作树为
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`，
执行环境为 macOS arm64，`rustc 1.100.0`、`cargo 1.100.0`。

| 命令 | 结果 |
|---|---|
| `python3 -B scripts/check_dependencies.py --timeout 30 --jobs 4 --json-report work/dependency-refresh-follow-up-20261115-02.json` | 失败，报告明确指出 `nix` 最新要求为 `0.32`，旧锁定为 `0.31.3` |
| 修改 `crates/keelshell-ai/Cargo.toml` 的 `nix` requirement 为 `0.32` | 完成 |
| `cargo generate-lockfile` | 通过；产生 `nix 0.31.3` 与 `nix 0.32.0` 两条满足不同 owner 约束的锁定边 |
| `python3 -B scripts/check_dependencies.py --timeout 30 --jobs 4 --json-report work/dependency-refresh-follow-up-20261115-03.json` | 通过，`103` 项 Rust 声明/使用 |
| `CARGO_TARGET_DIR=work/dependency-refresh-follow-up-20261115/target cargo test -p keelshell-ai --locked --lib` | `129 passed / 0 failed / 1 ignored` |

首次失败和最终通过的 JSON、日志及退出信息保留在被 Git 忽略的
`work/dependency-refresh-follow-up-20261115-*` 目录中。由于本次只验证受影响的
AI crate，workspace 全量 gate、Windows/Linux 编译、原生桌面和发布验收仍由后续
统一门禁覆盖。
