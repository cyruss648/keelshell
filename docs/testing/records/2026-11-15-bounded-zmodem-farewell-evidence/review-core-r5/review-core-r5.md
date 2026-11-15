# KeelShell bounded farewell r5 core 连续性补绑

结论：**APPROVE_CORE_CONTINUITY_BINDING_ONLY**。仅将既有 core 源码与专项证据批准绑定到 r5；本轮未重新展开 core 审查。已批准且摘要未变的 core 范围开放问题保持 **P0 0 / P1 0 / P2 0 / P3 0**，`CORE-R2-01`、`CORE-R1-01` 保持关闭。

HEAD：`b66ec7e564e97c3cc8261b447288377ac1ffccc2`。
冻结：`frozen-inputs-r5.json`，SHA-256：
`d8d27989768ae1827697458ee52d366d407ca8543a9eebd41baf6d8f283a2b2e`。
独立逐项核验 **571 个工程输入 + 12 个脚本，583/583 匹配**，没有未冻结新增输入、缺失输入或摘要漂移。交付前完整复核另存 [freeze-final.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r5/freeze-final.json)。

r4→r5 唯一变化为 [ssh_bridge_tests.rs:354](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/ssh_bridge_tests.rs:354)，摘要从 `8825c13cea74f348259c57b19179e59d0366147da751c12bc84ea453679e4655` 变为 `47771ef2e6d6e14a7afe3698281ee5e9334e4a230578e6dce003e6437d178c01`。其余 **570 个工程输入及全部 12 个脚本相同**，所有生产输入未变。core 106、session 123、根 Cargo 输入 2，共 **231/231** 与 r4 完全相同，无新增、删除或摘要差异。路径及逐项摘要见 [r5-continuity-receipt.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r5/r5-continuity-receipt.json)。

`CORE-R2-01` 沿用原闭环判断：已识别 CAN 与已验证协议头的剔除见 [codec.rs:535](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/codec.rs:535)、[codec.rs:613](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/codec.rs:613)；peer Cancelled 与 Failed 后 cancel 的一次性尾部取回见 [receiver.rs:137](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:137)、[receiver.rs:250](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:250)。这些输入及回归均未改变；HEAD 原 65 项、r2 原 94 项断言未弱化的既有证明继续有效。

沿用本 reviewer 在 r3 已独立执行的 **102 passed / 0 failed / 0 ignored / 0 measured / 0 filtered out，exit 0**，见 [原独立执行回执](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r3/core-r4-independent-execution.json)。父预构建稳定副本 `stable-tests/core-r4` 的 SHA-256 仍为 `8c51f02d4bf32d05d5757d310fe596c583cf7f986465ca958538098bff2cf75e`；该名称是捕获标签，原 binary receipt 绑定 r3，经 r4、r5 源码连续性补绑。**没有独立重编译；本轮新增测试执行 0，新增库存执行 0。** Cargo、编译、format、Clippy、共享 target 访问、Git mutation、源码或测试修改、新 worktree、新用户 chat 均为 0。

据父通报，完整 gate-r4 因 P2-004 测试契约不匹配已停止并实际 reap，**exit -15 / 235.951s**，未签通过。最终工程条件仍待父提供绑定同一 r5 冻结的完整门禁真实终态。本报告不补签 app 测试断言修正、P2-003/P2-004、Python fixture、Linux 全量互操作、原生 macOS/Windows/Linux 或完整产品验收；既有 Linux 严格 CRC32 Unicode 缺 Finished 失败仍保留。

本轮写入仅在 `review-core-r5/`。r2 HOLD、r3/r4 限定批准、原 notice、失败证据及原执行回执全部保留；[r4 批准原件](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r4/review-core-r4.md) 未改写。任何后续冻结输入变化均需另行核验和补绑。
