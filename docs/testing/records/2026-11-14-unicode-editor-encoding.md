# Unicode 编辑器编码扩展测试记录

日期：2026-11-14（Asia/Shanghai）。最终源码提交：
`9c43f5506d0ea4acc6abd70bb54602cdab6a1c22`。完整门禁在提交前的同一冻结源码
输入上执行，随后只做了本记录的提交状态修订；公司内网不可用，本片只做本地
提交，不执行 push。

## 实现范围

- core 将 `Utf8TextDocument` / `Utf8TextFormat` 扩展并命名为
  `TextDocument` / `TextFormat`，增加 `TextEncoding`、严格 UTF-16 LE/BE BOM
  解码、UTF-16 编码保存、代理项/UTF-32/奇数字节拒绝和独立大小边界。
- app 的打开、审核、保存、迟到保存、三方合并和完整读回使用观测到的
  `TextFormat`。审核正文显示解码后的 Unicode 文本；BOM-less UTF-8 开头的
  U+FEFF 保留为正文。
- session 编辑器写入入口重新验证支持的 Unicode 文本和编码类型；MCP 既有
  常规 UTF-8 读取预算没有扩大。
- 产品合同和设计决策见[Unicode 编码 ADR](../../adr/0126-unicode-editor-encoding-envelope.md)
  与[远程文本格式](../../product/REMOTE_TEXT_FORMAT.md)。

## 命令与结果

所有 Cargo 命令使用本 checkout 自有的
`work/dependency-refresh-20261114/target-r2`、隔离 Cargo home、`--locked
--offline` 和 Rust/Cargo `1.100.0`。未切换 checkout，因此不需要重新清理共享
target；每次运行均使用新的日志和 exit 回执。

| 命令 | 结果 |
| --- | --- |
| 独立 codec harness（实际 core 源码，agent 回执 `standalone-final-receipt-r2.json`） | 22 passed，严格 standalone Clippy、rustdoc、fmt、diff check 通过 |
| `cargo check -p keelshell-core -p keelshell-session -p keelshell-app --locked --offline` | exit 0 |
| `cargo test -p keelshell-core --lib text_document::tests -- --test-threads=2`（接入前） | 22 passed，0 failed |
| `cargo test -p keelshell-core --lib text_document::tests -- --test-threads=2`（最终接入） | 23 passed，0 failed，189 filtered |
| `cargo test -p keelshell-session --test ssh_loopback editor_reviewed_ -- --test-threads=1` | 9 passed，0 failed，145 filtered |
| `cargo test -p keelshell-app --locked --offline editor_format -- --test-threads=1` | 16 passed，0 failed，1122 filtered |
| `cargo clippy -p keelshell-core -p keelshell-session -p keelshell-app --all-targets --locked --offline -- -D warnings` | exit 0 |
| `python3 scripts/check.py` | exit 0，ordinary Rust `2460 passed / 0 failed / 23 ignored`，app `1136 passed / 2 ignored`，doctest `14 passed`，Python `131 run / 1 skipped`，依赖策略、fmt、workspace Clippy、readiness/observer/small-stack controller 均通过；耗时 679.355 秒 |
| app 定向首轮（集成修复前） | 13 passed，3 failed；失败分别暴露 UTF-8 兼容状态文案、BOM-less U+FEFF 迟到保存和 UTF-16 合并仍按 UTF-8 比较 |
| app 定向初次编译（测试辅助函数） | exit 101；两个测试 helper 借用了临时字节数组，已按 Rust 编译器诊断修复，失败日志保留 |

app 测试链接阶段仍可能输出 `__eh_frame section too large` 警告；命令实际
exit 0，未将该警告记为测试失败。

完整门禁回执的 `source_head` 仍记录为门禁启动时的父提交
`c3a9632a4c95b22cc606f6490cf4d49f68df532c`；门禁完成后没有修改任何 Rust、
测试或 Cargo 输入，最终提交 `9c43f5506d0ea4acc6abd70bb54602cdab6a1c22`
仅收录该冻结源码及文档。后续独立复核固定最终提交 SHA。

## 验收边界

当前记录证明 core 编解码、loopback SFTP 写入和 GPUI 文件面板中的 UTF-16 LE/BE
打开、编辑、合并、审核、备份和完整读回路径；完整工程门禁也已在本片源码上
通过。独立新 agent 代码与功能复核、精确新包、macOS 当前桌面观察、
Windows/Linux 原生桌面、IME/辅助技术或真实客户 SSH 验收仍未完成；完整门禁不
代签这些边界。
