# 2026-11-15 直接依赖官方复查与 `nix 0.32` 更新

## 结论

本次复查发现 `keelshell-ai` 的 Unix 目标直接依赖 `nix = "0.31"` 已落后于
官方稳定的 `0.32.0`。Manifest 已调整为 `nix = "0.32"`，继续满足项目要求的
`x.y` 直接版本约束；Cargo.lock 同时保留 `nix 0.31.3`，因为
`process-wrap 10.0.1` 的传递约束仍要求 `nix ^0.31.1`。直接依赖检查随后重新
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

首次失败和最终通过的 JSON、日志及退出信息保留在被 Git 忽略的 `work/` 下，
文件前缀为 `dependency-refresh-follow-up-20261115-`。`cargo generate-lockfile`
同时刷新了其余传递依赖：移除 45 个旧版本 package record、新增 49 个版本 record；
这不是只更新 `nix` 的最小锁文件差分。原始锁文件继续保存在父提交中。

随后在同一工作树使用独立 target 执行
`CARGO_TARGET_DIR=work/full-gate-dependency-refresh-20261115/target python3 scripts/check.py`，
已实际等待退出 `0`。Python `131 run / 1 skipped`；普通 Rust
`2537 passed / 0 failed / 23 ignored`，其中 app `1174 passed / 0 failed / 2 ignored`。
依赖 `x.y` 策略、fmt、workspace all-targets 严格 Clippy、
readiness/observer/small-stack controller 均通过。原始日志和退出回执分别为
`work/full-gate-dependency-refresh-20261115/logs/full-gate.log` 与
`work/full-gate-dependency-refresh-20261115/logs/full-gate.exit`。
这次门禁绑定依赖刷新后的基线 `59c59a8` 的 Rust/依赖输入，不覆盖后续功能改动。
Windows/Linux 编译、原生桌面和发布验收仍未执行。
