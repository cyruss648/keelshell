# KeelShell bounded farewell 最终 r4 core 限定复核

结论：**APPROVE，仅限 core 源码及已核验可沿用的专项执行证据**。
本范围开放问题为 **P0 0 / P1 0 / P2 0 / P3 0**。
`CORE-R2-01` 保持关闭，`CORE-R1-01` 保持关闭。本报告将 r3 的 core 结论绑定
到最终 r4 冻结；工程门禁状态仍为待父绑定，不批准整批、Linux 互操作、原生平台
或完整产品。

HEAD 为 `b66ec7e564e97c3cc8261b447288377ac1ffccc2`；
最终冻结 `frozen-inputs-r4.json`，SHA-256
`9133ce6141be9498034641d25fe064b60c0904fb4ced9f8d73f52a76e2e4c58c`。
初始独立核验全部 **571 个工程输入 + 12 个脚本，583/583 匹配**，没有遗漏或
未冻结新增输入。r3→r4 仅以下四项变化，未扩大本 reviewer 的源码签署范围：

| 类型 | 变化路径 |
| --- | --- |
| app 生产输入 | `crates/keelshell-app/src/ssh_bridge.rs` |
| app 测试输入 | `crates/keelshell-app/src/ssh_bridge_tests.rs` |
| Python 脚本 | `scripts/run_linux_file_interop.py` |
| Python 测试脚本 | `scripts/test_linux_file_interop.py` |

独立从 r3、r4 两份清单重新构造 core/session/root 的完整选择集合，并核对当前
文件：**106 个 core + 123 个 session + 2 个根 Cargo 输入，共 231/231 相同**，
新增路径、删除路径及摘要差异均为 0。父
`source-equivalence-r3-r4.json` 的 231 项路径和摘要也逐项独立验证一致，不仅
采用其总数或结论。两个根输入为 `Cargo.toml`、`Cargo.lock`。见
[独立源码连续性证明](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r4/source-equivalence-r3-r4-independent.json)。

`CORE-R2-01` 仍由相同生产代码和回归关闭：

- [codec.rs:535](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/codec.rs:535)
  只剔除五个已识别的连续 CAN；此前普通尾部保留。
- [codec.rs:613](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/codec.rs:613)
  剔除已完整解析、CRC 验证成功的协议头，避免取消或拒绝帧混入普通尾部。
- [receiver.rs:137](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:137)
  对 peer Cancelled 终态取走仍持有的 farewell 尾部并保持后缀顺序。
- [receiver.rs:250](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:250)
  在 Failed 后本地 cancel 清理时同样取走尾部；`Option::take` 和终态空返回防止
  重复交还。取消、RemoteCommand、其他失败均不改写成 Finished 或缺 OO 警告。

r3 的七项父方真实红回归、八项独立绿色回归及逐行分析原件保持。六项中文尾部、
三项 parity 和两项本地取消回归继续包含在同一 102 项执行结果中。原 r2
**94/94** 与 HEAD 原 **65/65** 测试正文未被弱化；已保存的逐字节证明和全部
core 输入摘要连续性共同支持本次沿用。bounded tail、严格 OO、独立缺失结果、
未审核 offer、未确认保存以及错误/取消终态的限定结论均保持 r3 的原范围。

专项证据沿用已由本 reviewer 在 r3 独立执行的父稳定副本
`stable-tests/core-r4`，SHA-256
`8c51f02d4bf32d05d5757d310fe596c583cf7f986465ca958538098bff2cf75e`。
此处 `core-r4` 是二进制捕获标签，原始 binary receipt 绑定的是 r3 freeze；
本报告通过上述源码一致性明确补上 r4 绑定，没有改写原 receipt。
库存摘要为
`5f10332d584c90afc174aadc8747666becd94f11275281a6bfe5600c00f745b6`，
102 个唯一用例名与原执行通过集合相同。

本次重新核验了稳定二进制、父库存、原独立 receipt、stdout/stderr、CORE-R2-01
闭环证据与断言保护证据的摘要。原证据与 r3 保存的 artifacts 清单一致。
按用户明确指示，core 输入完全不变后不重复执行 102 项，也没有重新运行库存命令。

| 执行或证据 | 计数与状态 |
| --- | --- |
| r3 原独立测试执行，沿用至 r4 | 1 次；102 passed / 0 failed / 0 ignored / 0 measured / 0 filtered out，exit 0 |
| r3 原独立库存，沿用至 r4 | 1 次；102 个唯一名称，与父库存逐字节相同 |
| 本轮 r4 新增 Rust 测试执行 | 0 |
| 本轮 r4 新增库存执行 | 0 |
| core/session/root 连续性检查 | 231/231；core 106、session 123、根输入 2；零差异 |
| 最终冻结检查 | 571+12；全部 583 项匹配，结束再次核验另存原件 |
| reviewer Cargo、编译、format、Clippy、共享 target、Git mutation | 全部 0 |
| reviewer SSH/peer、完整门禁、原生桌面执行 | 全部 0 |

原执行使用父预构建稳定二进制，**没有独立重编译**。source/session 的连续性
检查不构成 session、barrier 或 app 的独立执行，也不把父全批新增用例计数另加
到 102 项。绑定详见
[retained-core-execution-binding.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r4/retained-core-execution-binding.json)；
原始独立运行见
[core-r4-independent-execution.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/core-r4-independent-execution.json)，
原闭环分析见
[r3 报告](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/review-core-r3.md)。

完整 gate-r4 将由父运行并绑定同一最终冻结的真实终态；本报告未取得或签署其成功
回执。此前 gate-r3 因 Python fixture process inventory 期限转换错误早停 exit 1，
gate-r2 停止并 reap exit -15，均保留原事实。工程条件包括最终格式、严格
Clippy、workspace 测试与依赖策略等，不能由本报告的源码批准替代。

r4 的 app bridge post-pump cleanup 修复及 Python fixture/test 修复不由本
core reviewer 判断通过，也不代替其他 reviewer 对 P2-003 的关闭结论。
若父后续改变任意冻结输入，应重新绑定并核验差分，不能直接沿用当前完整冻结身份。

Linux 严格 CRC32 Unicode 缺 Finished 的既有失败、全量真实 peer 互操作、
Linux 本地 Rust 文件后端、Windows 安全发布、macOS/Windows/Linux 原生交互、
IME/辅助技术、安装签名发行与完整产品均未由本报告验收。没有把不同轮次的原生
或互操作结果拼接成完整通过。

所有新增写入只在 `review-core-r4/`。r2 HOLD、r3 限定批准、原始 notice、失败、
冻结、源码和测试均未改写，没有创建 worktree 或新用户 chat。

交付前最终核验时间 `2026-11-15T10:55:59.937314+00:00`，全部 583/583 匹配，HEAD、freeze 和稳定二进制摘要均未变。见 [freeze-final.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r4/freeze-final.json)。
