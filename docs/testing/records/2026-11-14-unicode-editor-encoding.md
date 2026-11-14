# Unicode 编辑器编码扩展测试记录

日期：2026-11-14（Asia/Shanghai）。源码与测试实现提交：
`368a7f836f72ca06b34276e5d587a34497fc7f13`。本记录随后会随文档绑定提交更新，
但门禁的 `source_head`、源码提交和测试输入均以该实现提交为准。公司内网不可用，
本片只做本地提交，不执行 push。

## 实现范围

- core 将 `Utf8TextDocument` / `Utf8TextFormat` 扩展并命名为
  `TextDocument` / `TextFormat`，增加 `TextEncoding`、严格 UTF-16 LE/BE BOM
  解码、UTF-16 编码保存、代理项/UTF-32/奇数字节拒绝和独立大小边界。
- app 的打开、审核、保存、迟到保存、三方合并和完整读回使用观测到的
  `TextFormat`。审核正文显示解码后的 Unicode 文本；BOM-less UTF-8 开头的
  U+FEFF 保留为正文。
- 成功保存后的完整读回继续使用审核时的编码/BOM 外壳，只从读回结果观察换行
  策略；它不会再用无上下文的 `TextDocument::decode` 把 BOM-less 文件开头的
  真实 U+FEFF 重新解释为 BOM。
- UTF-16 LE/BE 差异预览先用观测到的 `TextFormat` 解码原始基线和草稿，再调用
  逻辑文本差异 API；合法 UTF-16 文件可以生成预览。
- session 编辑器写入入口重新验证支持的 Unicode 文本和编码类型；MCP 既有
  常规 UTF-8 读取预算没有扩大，session 错误文案准确描述当前入口只比较编码。
- 产品合同和设计决策见[Unicode 编码 ADR](../../adr/0126-unicode-editor-encoding-envelope.md)
  与[远程文本格式](../../product/REMOTE_TEXT_FORMAT.md)。

## 命令与结果

所有 Cargo 命令使用当前 checkout 自有的
`work/unicode-codec-20261114/full-target`、隔离 Cargo home、`--locked --offline`
和 Rust/Cargo `1.100.0`。门禁完成后没有依赖共享 target 的后续验证；当前源码
提交与门禁 `source_head` 一致。

| 命令 | 结果 |
| --- | --- |
| `cargo test -p keelshell-core --lib text_document::tests -- --test-threads=2` | `23 passed / 0 failed / 189 filtered` |
| `cargo test -p keelshell-session --test ssh_loopback editor_reviewed_ -- --test-threads=1` | `9 passed / 0 failed / 145 filtered` |
| `cargo test -p keelshell-app --locked --offline editor_format -- --test-threads=1` | `18 passed / 0 failed / 1122 filtered` |
| `cargo clippy -p keelshell-core -p keelshell-session -p keelshell-app --all-targets --locked --offline -- -D warnings` | exit `0` |
| `cargo fmt --all -- --check` | exit `0` |
| `python3 scripts/check.py`（`unicode-full-gate`） | exit `0`；Python `131 run / 1 skipped`，ordinary Rust `2462 passed / 0 failed / 23 ignored`，app `1138 passed / 2 ignored`，doctest `14 passed / 0 failed`，依赖策略、fmt、workspace 严格 Clippy 和 readiness/observer/small-stack controller 均通过；实际等待约 `697s` |

完整门禁回执中的绑定字段为：

- `source_head`: `368a7f836f72ca06b34276e5d587a34497fc7f13`
- `target`: `work/unicode-codec-20261114/full-target`
- `exit`: `0`
- `waited`: `true`
- `timed_out`: `false`

日志和手工生成的 JSON 回执分别是
[`unicode-full-gate.log`](../../../work/unicode-codec-20261114/unicode-full-gate.log)
与[`unicode-full-gate.exit.json`](../../../work/unicode-codec-20261114/unicode-full-gate.exit.json)。
app 链接阶段输出过 `__eh_frame section too large` warning，但命令实际 exit `0`。
门禁结束后的外层 zsh wrapper 使用了只读变量名 `status`，在门禁成功末段之后返回
`1`，没有改变门禁本身的结果；该事实已写入 JSON 回执，不能把 wrapper 退出码当作
测试失败。

新增 app 回归为：

- `editor_format_successful_save_preserves_bomless_leading_feff`
- `editor_format_utf16_diff_preview_uses_decoded_bodies`

## 上一轮复核问题与处理

上一轮对 `b239d78691420a30d54c914c9ad7253023b0b681` 的独立严格只读回执返回
P0 `0`、P1 `1`、P2 `2`、P3 `1`，原始回执已复制到
[`previous-b239-review.md`](../../../work/unicode-codec-20261114/review/previous-b239-review.md)。
本轮逐项处理：

- P1：成功保存读回沿用审核时的 `TextFormat`，只合并实际读回的换行策略；新增
  `editor_format_successful_save_preserves_bomless_leading_feff`。
- P2：UTF-16 LE/BE 差异预览先解码两侧正文，再调用 `diff_text`；新增
  `editor_format_utf16_diff_preview_uses_decoded_bodies`，覆盖 LE 和 BE。
- P2：修正测试记录的旧提交绑定、旧 `source_head`、旧 target 和门禁计数。
- P3：session 错误文案改为准确描述当前入口实际校验的编码合同。

当前记录状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_PENDING`。文档绑定提交后，
必须以包含本记录的 exact SHA 启动新的独立严格只读代码与功能复核，回执至少包含
P0/P1/P2/P3 计数、每个 finding 的绝对路径和行号、触发条件、影响、实际验证命令、
`APPROVE` 或 `REQUEST_CHANGES`，并保存到当前 work evidence 目录。

## 验收边界

当前记录证明 core 编解码、loopback SFTP 写入和 GPUI 文件面板中的 UTF-16 LE/BE
打开、编辑、合并、审核、备份、成功保存读回和差异预览路径；完整工程门禁也已在
实现提交 `368a7f8` 上通过。最终提交的新独立代码与功能复核仍待完成；精确新包、
macOS 当前桌面观察、Windows/Linux 原生桌面、IME/辅助技术或真实客户 SSH 验收
仍未完成，完整门禁不代签这些边界。
