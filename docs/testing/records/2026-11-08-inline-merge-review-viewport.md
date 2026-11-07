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

## 精确 macOS 包与桌面短验

源码提交已固定为：

```text
e5891d1c8101c00944979ec2761718d376ae60d5
```

按共享 target 指南再次失效五个项目包后，执行：

```text
MACOSX_DEPLOYMENT_TARGET=15.0 \
  cargo build -p keelshell-app -p keelshell-mcp --release --locked

KEELSHELL_BUILD_COMMIT=e5891d1c8101c00944979ec2761718d376ae60d5 \
  python3 packaging/package.py macos \
  --binary target/release/keelshell-app \
  --mcp-binary target/release/keelshell-mcp \
  --target aarch64-apple-darwin \
  --output work/native-cua-20261107/package-r11

python3 packaging/inspect_native.py \
  --platform macos \
  --target aarch64-apple-darwin \
  --stage work/native-cua-20261107/package-r11
```

三个命令均退出 `0`。包路径为
`work/native-cua-20261107/package-r11/KeelShell.app`，manifest 的
`build.commit` 精确绑定上述源码 SHA，目标为 `aarch64-apple-darwin`，
GUI 与 MCP 均为 arm64 Mach-O，实际 `LC_BUILD_VERSION minos=15.0`，
Info.plist 的最低 macOS 版本为 `15.0`。`installed=false`、
`signed_by_packaging_script=false`，本次没有安装、签名或发布。

摘要如下：

- manifest SHA-256：
  `54ed0e165a27c760f4672673bbde16d3572efaff815b50b6b7c629927b49f8d6`；
- GUI SHA-256：
  `c6afd91a3cdb219a3974c92b90f0d2310d70c16a45ce1efc068bffe8e22cc139`；
- MCP SHA-256：
  `94eaf83f8bbb23ae4db7cc190c9c06332ddbc19b4bf9246dcdf35f43b3645d8f`；
- manifest 列出的 `5` 个包文件逐一重算哈希并通过。

使用独立 `KEELSHELL_DATA_DIR` 启动 exact package 后，已观察到默认工作台和连接
库空态、Themes and appearance 对话框、Frost「霜白」、Graphite「曜石」、
Lavender「雾紫」三套主题切换、英文到中文切换和中文工作台空态。进程与隔离
数据均已回收。该短验没有代签 inline merge、真实 SSH/SFTP、Podman、900×580
原生调整、IME、VoiceOver、Windows/Linux 窗口、签名、公证或安装升级。

## 独立只读复核状态

源码提交后新启动了独立复核 agent，但两次尝试都没有返回可审计报告：

- 第一次 agent 在读取旧工具会话时连续重复同一个 `write_stdin` 调用，随后被
  系统停止；
- 第二次使用不继承旧会话的隔离 agent，等待和中断请求后仍未输出审查正文，随后
  被关闭。

因此本片状态明确为 `NON_AUTHOR_REVIEW_UNAVAILABLE`。没有 P0/P1/P2/P3 计数，
没有 `APPROVE` 或 `REQUEST CHANGES` 的非作者结论；作者的定向回归、完整门禁和
macOS 短验不能替代独立复核。后续新一轮代码变更仍需重新启动并取得可审计的独立
报告后，才能把合并建议写成批准。

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
