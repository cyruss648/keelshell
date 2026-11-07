# 编辑器格式与合并迟到读回组合回归记录 — 2026-11-08

## 范围

本片补齐两个跨操作回归：

- 从远端无换行文件开始，首次插入换行、连续保存空内容与无换行内容后，仍保留
  LF 插入策略，后续编辑不误加 BOM 或 CRLF；
- 读取远端并生成合并审核、采用合并草稿、进入完整保存审核后，在真实发布路径
  中产生较新编辑器草稿，完成目标读回，再验证下一次保存使用新基线和正确格式。

工作树：`feature/main-visual-integration-20261106`

本片开始时源码与文档基线为：

```text
1d0898025bcd714550de8d702d5c8026242844bf
```

本片修改：

```text
crates/keelshell-app/src/files/editor_format_tests.rs
crates/keelshell-app/src/files/conflict_merge_tests.rs
docs/product/REMOTE_TEXT_FORMAT.md
docs/product/TEXT_CONFLICT_MERGE.md
docs/adr/0122-editor-format-and-late-readback-regressions.md
docs/testing/records/2026-11-08-editor-format-merge-combinations.md
```

## 实现

新增 GPUI/SFTP 回归：

```text
independent_editor_format_no_newline_keeps_lf_policy_across_empty_and_plain_saves
merge_adopt_save_keeps_late_newer_draft_and_rebases_next_save
```

第一个用例使用真实编辑器按键在 `seed` 末尾插入首个 LF，依次确认 `seed\n`、空
内容、`plain` 和 `later\nline`。每次保存都检查 reviewed/content 字节、读回内容、
`TextLineEnding::Lf`、无 BOM，并最终读取四份原始备份。

第二个用例使用 40,000 字节级别的真实文本让 SFTP 原子写入跨越多个写入块，使用
已有 `hold_atomic_writes_after_first` 在发布期间停顿。它在停顿时修改编辑器，释放
写入后检查远端仍为已审核合并结果，编辑器保留较新草稿，`editing` 和
`editor_snapshot` 已切换到成功读回的合并版本；下一次保存的 reviewed/content 字节
和最终基线也逐字节核对。

生产代码没有修改。保存、备份、原子替换、完整读回和审核授权仍由现有生产路径
执行，测试只使用隔离 Harness 与 loopback SFTP。

## 定向验证

```text
rustfmt --edition 2024 \
  crates/keelshell-app/src/files/editor_format_tests.rs \
  crates/keelshell-app/src/files/conflict_merge_tests.rs
git diff --check
```

结果：两项退出 `0`。

```text
cargo test -p keelshell-app --locked --offline \
  independent_editor_format_no_newline_keeps_lf_policy_across_empty_and_plain_saves \
  -- --nocapture --test-threads=1
```

结果：`1 passed / 0 failed / 1116 filtered out`。

```text
cargo test -p keelshell-app --locked --offline \
  merge_adopt_save_keeps_late_newer_draft_and_rebases_next_save \
  -- --nocapture --test-threads=1
```

结果：`1 passed / 0 failed / 1116 filtered out`。

```text
cargo test -p keelshell-app --locked --offline \
  editor_format_tests -- --nocapture --test-threads=1
```

结果：`14 passed / 0 failed / 1103 filtered out`。

```text
cargo test -p keelshell-app --locked --offline \
  conflict_merge_tests -- --nocapture --test-threads=1
```

结果：`29 passed / 0 failed / 1088 filtered out`。

最终测试清单确认两个新增测试都出现在
`files::transfer_tests::conflict_merge_tests` 路径下。链接器仍报告历史性的
`__eh_frame section too large` warning；它没有改变测试退出码。

## 完整工程门禁

第一轮 `python3 scripts/check.py` 已完成 Python 阶段
`131 run / 1 skipped`，随后在 workspace 格式阶段退出 `1`。原因是新增断言尚未
按 `cargo fmt` 展开；这不是测试失败。格式化后重新启动同一完整门禁，第二轮退出
`0`，结果为：

- Python：`131 run / 1 skipped`；
- workspace Rust：`2,437 passed / 23 ignored`；
- `keelshell-app`：`1,115 passed / 2 ignored`；
- 14 项 doctest 通过；
- 依赖版本策略、严格 workspace Clippy、readiness/observer/small-stack AI
  controller 全部通过；
- `cargo fmt --all -- --check` 与 `git diff --check` 均退出 `0`。

完整门禁继续使用当前工作树的共享 target 归属和隔离测试数据；可选的 Podman、
OpenSSH、真实 Linux 采样以及原生桌面边界仍按既有规则保持未代签。

## 失败与修复证据

首次把第二个切片接入主工作树后，定向编译因测试闭包误把外层
`&mut TestAppContext` 传给 `Entity::read(&App)`，在
`crates/keelshell-app/src/files/conflict_merge_tests.rs:170` 得到 `E0308`。
修正闭包使用 GPUI 测试上下文后，重新格式化并重跑通过。并行切片还曾覆盖第一个
测试文件的未提交内容；主工作树依据已返回的差分重新应用该测试，再通过清单和
定向回归确认它确实被编译执行。

上述失败事实保留在本记录中，没有用最终成功覆盖首次失败。

## 独立严格只读复核

首次复核固定基线为 `9134cfaa771057a0f246ead47479be3706248415`，复核工作树保持
干净，没有编辑、提交、推送或清理。复核结论为 `REQUEST CHANGES`：

- P0：`0`；
- P1：`0`；
- P2：`1`，文档仍把已提交源码写成“待提交/未提交候选”；
- P3：`0`。

复核指出的文档位置为：

- `docs/HANDOFF.md` 当前片状态与组合回归段落；
- `docs/ROADMAP.md` 当前片下一步；
- `docs/adr/0122-editor-format-and-late-readback-regressions.md` 状态行。

本次文档修订已写入固定源码 SHA `9134cfaa771057a0f246ead47479be3706248415`
并明确首次 `REQUEST CHANGES` 结果；下一次独立复核需确认该 P2 已关闭。复核实际
运行两个新增用例，均为 `1 passed / 0 failed / 1116 filtered out`，并执行
`cargo test ... -- --list`、`cargo fmt --all -- --check` 和
`git diff --check 9134cfaa^ 9134cfaa`，均确认通过。完整门禁计数未由该复核重复，
仍以本记录前面的作者门禁回执为界。

第二次独立严格只读复核固定基线为文档修订后的
`bcc5990f286159a98cdca4156583e3e2148ff534`。复核确认工作树干净、父提交仅包含文档
修改，首次 P2 已关闭，P0/P1/P2/P3 均为 `0`，结论为 `APPROVE`。它重新执行两个
新增用例，各为 `1 passed / 0 failed / 1116 filtered out`，并通过格式检查和差异
检查；没有把作者完整门禁计数作为独立结果。代码和文档复核均未覆盖 Windows/Linux
原生窗口、IME、VoiceOver、真实客户 SSH 长流程、Podman 联调、外部 AI provider、
精确 macOS 包或安装发布。

## 尚未完成

- 新源码精确 macOS 包和有限桌面短验；
- Windows/Linux 原生窗口、IME、VoiceOver、真实 SSH/Podman 长流程和外部 AI provider。

本记录的 GPUI/loopback SFTP 结果不代签上述平台、发布和外部服务边界。
