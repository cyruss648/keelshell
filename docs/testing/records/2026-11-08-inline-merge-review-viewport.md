# 内联合并审核视口测试记录 — 2026-11-08

## 范围

本片验证文件工具区内联三方合并审核的固定卡片、正文独立滚动、两行固定动作区、
嵌套 wheel 命中、冲突/完整版本滚动复位和旧审核帧失败关闭。

工作树：`feature/main-visual-integration-20261106`

测试开始时的源码基线为 `ed75c9802509b54f4c57dc38f8573fe0f43aeee1`，本片包含
该基线之后尚未提交的四个源码文件变更：

- `crates/keelshell-app/src/files/merge.rs`
- `crates/keelshell-app/src/files/review_viewport_tests.rs`
- `crates/keelshell-app/src/files/transfer_tests.rs`
- `crates/keelshell-app/src/files/conflict_merge_tests.rs`

## 变更

- 内联合并卡片固定为 `220 px`，正文和动作区分离；
- 正文使用独立 `body_scroll`，由带 `review_id` 的滚轮处理器限幅并阻止外层
  `file-tools-scroll`；
- 动作区收敛为两行，展开动作移动到固定动作栏；
- 冲突切换、方案选择和完整版本切换复位正文滚动；
- 外层文件工具区、内联正文和完整版本的测试辅助函数按几何交集发送嵌套 wheel；
- stale frame 回归在替换审核已绘制但旧 listener 仍保留的窗口内验证失败关闭。

## 定向验证

所有命令均使用当前工作树、`--locked --offline` 和单线程，避免共享 Cargo target
并发写入：

```text
cargo test -p keelshell-app --locked --offline \
  stale_merge_frame_actions_cannot_mutate_a_replacement_review \
  -- --nocapture --test-threads=1
```

结果：`1 passed / 0 failed / 1114 filtered out`。

```text
cargo test -p keelshell-app --locked --offline \
  conflict_merge_tests -- --nocapture --test-threads=1
```

结果：`27 passed / 0 failed / 1088 filtered out`。

```text
cargo test -p keelshell-app --locked --offline \
  review_viewport_tests -- --nocapture --test-threads=1
```

结果：`12 passed / 0 failed / 1103 filtered out`。

```text
cargo test -p keelshell-app --locked --offline \
  editor_format_tests -- --nocapture --test-threads=1
```

结果：`13 passed / 0 failed / 1102 filtered out`。

```text
cargo fmt --all -- --check
git diff --check
```

结果：两项均退出 `0`。

## 完整工程门禁

在定向回归通过后，按共享 target 指南先执行：

```text
cargo clean --locked --offline \
  -p keelshell-app \
  -p keelshell-core \
  -p keelshell-session \
  -p keelshell-ai \
  -p keelshell-mcp
```

结果：删除 `22,860` 个文件、`23.1 GiB`。清理后的第一次真实重编在严格
workspace Clippy 阶段退出 `101`，发现了本片带入的未使用
`ConfigurationGate::new`、测试专用 `finish_window_cleanup`、可替换为
`then_some` 的写法和窗口测试未使用参数。修复后重新执行精确 Clippy 通过，
并继续完整门禁；首轮失败事实保留在本记录中。

最终执行：

```text
python3 scripts/check.py
```

结果：退出码 `0`。依赖版本策略、Python 回归、格式、严格 workspace Clippy、
workspace Rust 测试和三个 AI controller 专项均通过：

- Python：`131 run / 1 skipped`；
- `cargo test --workspace --locked -- --test-threads=4`：当前测试清单
  `2,458` 项，其中 `2,435 passed / 23 ignored / 0 failed`；
- `keelshell-app`：`1,113 passed / 2 ignored`；
- doctests：`14 passed / 0 failed`；
- readiness-error、observer-io-error、controller-small-stack：均通过；
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked
  -- -D warnings`：退出 `0`。

上述完整门禁针对本片最终源码候选执行；文档收口提交只改变文档，不改变已验证
的生产代码、测试代码和依赖锁定文件。

## 失败与修复证据

初次完整视口专项中，`inline_merge_card_keeps_review_actions_visible_while_body_scrolls`
得到 `before=0px, after=0px, max=214px`。原因有两部分：测试在外层工具区尾部把正文
标题裁出了可命中的区域；生产 `.on_scroll(stop_propagation)` 又在 GPUI 默认滚动处理
前阻止了正文自身滚动。修复后，测试先把外层工具区回滚到正文与动作区存在可见交集，
生产 listener 显式更新 `body_scroll` 并按最大偏移截断，精确重跑通过。

该失败事实保留在本记录中，没有把首次失败改写为从未发生。

## 未覆盖边界

- 连续无换行保存回归仍未补齐；
- `merge-adopt → merge-save → late newer draft/readback` 组合回归仍未补齐；
- 当前源码 macOS exact package、Windows/Linux 原生窗口、IME、VoiceOver、签名、
  公证、安装、真实 SSH/Podman 长流程和外部 AI provider 尚未由本片验证；
- 独立严格只读代码/功能复核尚未针对本片最终提交返回报告。

## 结论

内联合并审核的代码和完整工程门禁已达到可提交状态；固定源码提交、独立复核、
精确 macOS 包和原生验收仍是后续交付门槛。
