# KeelShell core 最终 r5 同源收口

结论：**APPROVE_CORE_CONTINUITY_BINDING_ONLY**。core 范围开放发现 **P0 0 / P1 0 / P2 0 / P3 0**；`CORE-R2-01`、`CORE-R1-01` 保持关闭。仅补绑既有 core 源码及专项证据，不重新展开 core 审查。

HEAD `b66ec7e564e97c3cc8261b447288377ac1ffccc2`；最终冻结 `frozen-inputs-r5.json` SHA-256 `d8d27989768ae1827697458ee52d366d407ca8543a9eebd41baf6d8f283a2b2e`。本次重新独立核验全部 **571+12，583/583 匹配**，无未冻结新增、缺失或漂移。r4→r5 只有 app 既有背压测试文件变化；全部生产输入未变，**core 106 + session 123 + 根 Cargo 2 = 231/231 相同**。

`CORE-R2-01` 沿用的闭环位置为 [codec.rs:535](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/codec.rs:535)、[codec.rs:613](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/codec.rs:613)、[receiver.rs:137](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:137)、[receiver.rs:250](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs:250)。相同代码及回归支持取消/失败保留并仅交还一次尾部的既有结论；HEAD 原 65 项、r2 原 94 项测试断言未弱化的原证明继续有效。

沿用 reviewer 在 r3 独立执行的 **102 passed / 0 failed / 0 ignored / 0 measured / 0 filtered out，exit 0**。稳定二进制 `core-r4` SHA-256 `8c51f02d4bf32d05d5757d310fe596c583cf7f986465ca958538098bff2cf75e` 与原执行回执仍相同。它是父预构建副本，**未独立重编译**。本次 Rust 测试、库存、Cargo、format、Clippy、编译及共享 target 访问均为 **0**；没有修改源码、测试或 Git 状态。

父已实际 reap 的 app-bridge-r6 回执和日志已只读核对，日志 SHA 与回执一致：**exit 0 / 19.308s，15 passed / 0 failed / 0 ignored / 0 measured / 1223 filtered out**。此项记录为父执行证据，不计入 reviewer 的 core 102 项或本次执行次数，也不替代 app reviewer 签署。

完整 gate-r4 的历史终态仍为停止并 reap **exit -15 / 235.951s**，未签通过。最终完整工程门禁仍待父提供绑定同一 r5 冻结的真实成功终态。Linux 全量互操作、既有严格 CRC32 Unicode 缺 Finished 失败、原生平台及产品验收保持原边界。

此前 r2 HOLD、r3/r4/r5 批准、notice、失败和执行原件均未改写。此次只在 `review-core-r5/` 新增短报告及证据；逐项核验、零开放发现和父回执摘要见 [final-closeout-receipt.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/pty-farewell-20261115/review-core-r5/final-closeout-receipt.json)。本 reviewer 的最终 r5 限定复核已完成。
