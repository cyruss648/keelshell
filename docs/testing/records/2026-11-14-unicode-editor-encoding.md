# Unicode 编辑器编码扩展测试记录

日期：2026-11-14（Asia/Shanghai）。本轮修复提交：
`bfdc06633666a4cfa419ff2f0e47ce5f73a1ae10`，基于
`b239d78691420a30d54c914c9ad7253023b0b681`。完整门禁 r2 在修复工作树上执行，
门禁完成后不再修改 Rust、测试或 Cargo 输入；随后仅用文档 amend 固定本 SHA。
公司内网不可用，本片只做本地提交，不执行 push。

## 实现范围

- core 将 `Utf8TextDocument` / `Utf8TextFormat` 扩展并命名为
  `TextDocument` / `TextFormat`，增加 `TextEncoding`、严格 UTF-16 LE/BE BOM
  解码、UTF-16 编码保存、代理项/UTF-32/奇数字节拒绝和独立大小边界。
- app 的打开、审核、保存、迟到保存、三方合并和完整读回使用观测到的
  `TextFormat`。审核正文显示解码后的 Unicode 文本；BOM-less UTF-8 开头的
  U+FEFF 保留为正文。
- 本轮修复让成功保存后的完整读回继续使用审核时的编码/BOM 外壳，只从读回结果
  观察换行策略；它不会再用无上下文的 `TextDocument::decode` 把 BOM-less 文件
  开头的真实 U+FEFF 重新解释为 BOM。
- UTF-16 LE/BE 差异预览先用观测到的 `TextFormat` 解码原始基线和草稿，再调用
  逻辑文本差异 API；合法 UTF-16 文件可以生成预览。
- session 编辑器写入入口重新验证支持的 Unicode 文本和编码类型；MCP 既有
  常规 UTF-8 读取预算没有扩大。
- 产品合同和设计决策见[Unicode 编码 ADR](../../adr/0126-unicode-editor-encoding-envelope.md)
  与[远程文本格式](../../product/REMOTE_TEXT_FORMAT.md)。

## 命令与结果

所有 Cargo 命令使用本 checkout 自有的
`work/unicode-review-fix-20261114/target`、隔离 Cargo home、`--locked --offline`
和 Rust/Cargo `1.100.0`。未切换 checkout，因此不需要重新清理共享 target；
每次运行均使用新的日志和 exit 回执。

| 命令 | 结果 |
| --- | --- |
| 独立 codec harness（实际 core 源码，agent 回执 `standalone-final-receipt-r2.json`） | 22 passed，严格 standalone Clippy、rustdoc、fmt、diff check 通过 |
| `cargo check -p keelshell-core -p keelshell-session -p keelshell-app --locked --offline` | exit 0 |
| `cargo test -p keelshell-core --lib text_document::tests -- --test-threads=2`（接入前） | 22 passed，0 failed |
| `cargo test -p keelshell-core --lib text_document::tests -- --test-threads=2`（最终接入） | 23 passed，0 failed，189 filtered |
| `cargo test -p keelshell-session --test ssh_loopback editor_reviewed_ -- --test-threads=1` | 9 passed，0 failed，145 filtered |
| `cargo test -p keelshell-app --locked --offline editor_format -- --test-threads=1` | 18 passed，0 failed，1122 filtered |
| `cargo clippy -p keelshell-core -p keelshell-session -p keelshell-app --all-targets --locked --offline -- -D warnings` | exit 0 |
| `python3 scripts/check.py`（unicode-full-gate-r2） | exit 0，ordinary Rust `2462 passed / 0 failed / 23 ignored`，app `1138 passed / 2 ignored`，doctest `14 passed`，Python `131 run / 1 skipped`，依赖策略、fmt、workspace Clippy、readiness/observer/small-stack controller 均通过；耗时 `598.0s`。日志与回执见 [`unicode-full-gate-r2.log`](../../../work/unicode-review-fix-20261114/unicode-full-gate-r2.log) 与 [`unicode-full-gate-r2.exit.json`](../../../work/unicode-review-fix-20261114/unicode-full-gate-r2.exit.json) |
| app 定向首轮（集成修复前） | 13 passed，3 failed；失败分别暴露 UTF-8 兼容状态文案、BOM-less U+FEFF 迟到保存和 UTF-16 合并仍按 UTF-8 比较 |
| app 定向初次编译（测试辅助函数） | exit 101；两个测试 helper 借用了临时字节数组，已按 Rust 编译器诊断修复，失败日志保留 |

app 测试链接阶段仍可能输出 `__eh_frame section too large` 警告；命令实际
exit 0，未将该警告记为测试失败。

完整门禁 r2 的 `source_head` 记录为启动时的提交
`b239d78691420a30d54c914c9ad7253023b0b681`；修复源码、测试和 Cargo 输入以
工作树差异形式参与执行，具体范围和 `source_tree` 说明保存在 r2 exit JSON。
门禁完成后没有再修改这些输入；提交
`bfdc06633666a4cfa419ff2f0e47ce5f73a1ae10` 固定了同一源码和测试输入，之后只
通过文档 amend 回填本 SHA。独立复核固定该最终提交，而不是把工作树状态当作
最终源码。

## 本轮独立复核问题与关闭

上一轮针对 b239 的严格只读回执保留在
[`b239-review-receipt.md`](../../../work/unicode-review-fix-20261114/review/b239-review-receipt.md)，
结论为 P0 `0`、P1 `1`、P2 `2`、P3 `1`，`REQUEST_CHANGES`。本轮逐项处理：

- P1：成功保存读回沿用审核时的 `TextFormat`，只合并实际读回的换行策略；
  新增 `editor_format_successful_save_preserves_bomless_leading_feff`。
- P2：UTF-16 差异预览先解码两侧正文，再调用 `diff_text`；新增
  `editor_format_utf16_diff_preview_uses_decoded_bodies`，覆盖 LE 和 BE。
- P2：修正本测试记录的旧提交绑定、旧 `source_head` 描述、实际 target 和完整
  门禁计数。
- P3：session 错误文案改为准确描述当前入口实际校验的编码合同。

当前源码提交后的新独立严格只读复核尚未完成；在复核返回前，本片状态保持
`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_PENDING`。

## 验收边界

当前记录证明 core 编解码、loopback SFTP 写入和 GPUI 文件面板中的 UTF-16 LE/BE
打开、编辑、合并、审核、备份、成功保存读回和差异预览路径；完整工程门禁也已
在本轮修复工作树上通过。最终提交的新独立代码与功能复核仍待完成；精确新包、
macOS 当前桌面观察、Windows/Linux 原生桌面、IME/辅助技术或真实客户 SSH 验收
仍未完成，完整门禁不代签这些边界。
