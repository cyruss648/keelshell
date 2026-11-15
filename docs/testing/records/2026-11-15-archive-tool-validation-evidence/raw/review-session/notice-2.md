# P2 — POSIX 专用 Python suite 无条件进入 Windows 默认 Rust 单测

状态：OPEN / REQUEST_CHANGES。发现于冻结 r1：
`d68bec67c2cd4313383736733096f5c4ae68ccc6d786793609d31cb34aedc210`。

位置：

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation/tests.rs:323`：只有 `#[test]`，无 Unix 平台门限；模块也只有 `#![cfg(test)]`。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation/tests.rs:329`：默认运行 Python 专项，退出不为零即断言失败。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation/test_archive_validation.py:44`：实际执行 `/bin/sh`，第 47 行要求 `passed / 0`。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation/test_archive_validation.py:252`：fixture 无条件使用 `os.fork()`；同 suite 多处依赖 `os.killpg`。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation.py:293`：生产 helper 明确拒绝 `os.name != "posix"`，退出 65。

触发：Windows 客户端在原生默认 Rust 单元测试中执行这个新增测试。即便已安装 Python，测试仍要求一个明确不支持 Windows 本地运行的 helper 返回成功，并依赖 POSIX shell/fork/process-group API；不是远端 POSIX 服务端的能力限制。

影响：新增源码使 Windows 默认单测必然失败。客户端 API 本身应可从 Windows 经 SSH 检查 POSIX 服务器；仅 Linux/macOS 测试成功或 Windows 交叉编译不能覆盖这个运行回归。

证据：

- `windows-suite-source-r1.json` 保存实际源声明、Python suite 24 项且零 skip 装饰器的库存。
- 同回执记录仅将已加载 helper 的 `os.name` 设为 `"nt"` 时 `main()` 实际返回 65。此为平台入场条件的受控观测，不是 Windows 原生执行、仿真验收或 Windows 平台签署。
- 当前 suite 在 macOS 上 24/24 通过，原件 `python-author-suite-r1.log`；该结果不能消除 Windows 默认执行路径问题。

修复方向：将这个启动本地 POSIX 进程 suite 的 Rust 测试限制到 Unix，或在 Python suite 明确、逐类划分平台并跳过无法本机执行的 POSIX 用例。严格 JSON/命令构造 Rust 单元和可跨平台的加密 SSH fixture 应保留在 Windows 库存。请核对 `scripts/check.py` 如另有直接入口，也应遵守相同门限。

本复核没有运行 Cargo，也没有尝试在 macOS 冒充 Windows 原生验收。

## r2 关闭记录 — CLOSED

2026-11-15，基于最终 r2：
`523506564206cbeca415ae2bc9a82ed670cdb210705ba49f3173a84250bd4c17`，
本 P2 已关闭。上文为 r1 发现时的原始事实，保持原件。

已独立确认
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/archive_validation/tests.rs:323`
新增 `#[cfg(unix)]`，只限制启动本地 POSIX Python suite 的桥接测试。
同目录 Python suite 的两类测试分别在第 35、193 行添加
`@unittest.skipUnless(os.name == "posix", ...)`。其余 Rust 协议单元、
加密 SSH fixture 和公共 API 保持跨平台，r1→r2 输入清单显示它们未改。

本次 reviewer 在系统 Python 3.9.6 独立执行最终 suite，25/25 通过；
`python-author-suite-r2.log/.json` 的运行前后源码摘要与最终 r2 一致。
Windows 默认执行回归的关闭基于平台门限源码，**Windows 原生执行仍为 0**；
不据 Unix 产物或受控 os.name 观测签署 Windows 原生验收。
