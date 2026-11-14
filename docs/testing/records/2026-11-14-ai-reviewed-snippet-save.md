# AI 审核命令保存为片段测试记录 — 2026-11-14

产品实现基线：exact HEAD
`b89445604b4ce44fc5a80ff26605f6e6b9039d57`。

作者 worktree：
`/Users/hdh/.codex/worktrees/ai-command-snippet-save-20261114/keelshell`

主体/修复提交：
`9b831103aae5563a5450561988d668d2e77a19dd`

作者专用 Cargo target：
`work/ai-command-snippet-save-20261114/target`

作者全量 Cargo target：
`work/ai-command-snippet-save-20261114/full-target`

日志目录：
`work/ai-command-snippet-save-20261114/logs/`

## 结论

作者工程门禁在主体/修复提交上实际完成并通过。当前作者状态为
`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_PENDING`；独立严格只读复核尚未完成，因此
这里的 P0/P1/P2/P3 只是作者自查，不是独立批准。

## 功能回归

以下命令均在作者专用 target 上使用当前本机 Rust/Cargo `1.100.0`、`--locked`
和 `--offline`；测试链接阶段出现已知 macOS linker warning：
`__eh_frame section too large (max 16MB) to encode dwarf unwind offsets in compact unwind table`。
warning 没有改变任何命令的 exit code。

| 命令 | 实际结果 |
| --- | --- |
| `CARGO_TARGET_DIR=work/ai-command-snippet-save-20261114/target cargo test -p keelshell-app --bin keelshell-app --tests ai_snippet_save --locked --offline -- --nocapture --test-threads=1` | `6 passed / 0 failed / 1149 filtered; exit 0` |
| `CARGO_TARGET_DIR=work/ai-command-snippet-save-20261114/target cargo test -p keelshell-app --bin keelshell-app --tests snippet_editor --locked --offline -- --nocapture --test-threads=1` | `11 passed / 0 failed / 1144 filtered; exit 0` |
| `CARGO_TARGET_DIR=work/ai-command-snippet-save-20261114/target cargo test -p keelshell-app --bin keelshell-app --tests command_review_target --locked --offline -- --nocapture --test-threads=1` | `6 passed / 0 failed / 1149 filtered; exit 0` |
| `CARGO_TARGET_DIR=work/ai-command-snippet-save-20261114/target cargo test -p keelshell-core --locked --offline -- --nocapture` | `634 passed / 0 failed / 0 ignored; exit 0` |

覆盖内容包括：

- suggestion 和 diagnostic step 必须先通过 `CommandProposal::review` 才发出保存事件；
- event 携带命令、target、session identity 和一次性 review ID，重复消费失败；
- 打开草稿时重新检查活动 SSH tab、terminal/session、target、configuration gate
  和 modal scope；
- 名称、说明、标签可编辑，审核正文 disabled 且不能直接保存修改；
- 二次确认保存、显式 re-review、取消不执行命令；
- target 切换和 terminal 退休后保存 fail closed；
- StateStore revision conflict 保留完整草稿和错误；
- 成功保存后可进入 snippet search 与 command suggestion；
- 双语、Frost/Graphite/Lavender、light/dark、fixed footer、bounded scroll
  和 accessibility/test labels；
- 保存路径没有向测试 SSH panes 写入任何命令。

## 全量工程门禁

命令：

```text
CARGO_TARGET_DIR=work/ai-command-snippet-save-20261114/full-target python3 scripts/check.py
```

实际 exit code：`0`。完整日志：
`work/ai-command-snippet-save-20261114/logs/full-gate-9b83110.log`。
exit 回执：
`work/ai-command-snippet-save-20261114/logs/full-gate-9b83110.exit.txt`。

实际计数：

- Python：`131 run / 1 skipped`；
- workspace 测试：`2499 passed / 0 failed / 23 ignored`；
- `keelshell-app`：`1153 passed / 0 failed / 2 ignored`；
- `keelshell-core`：`627 passed / 0 failed / 0 ignored`；
- `keelshell-ai`：`194 passed / 0 failed / 5 ignored`；
- `keelshell-session`：`448 passed / 0 failed / 16 ignored`；
- `keelshell-mcp`：`77 passed / 0 failed / 0 ignored`；
- dependency version policy：passed；
- `cargo fmt --all --check`：passed；
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：passed；
- readiness、observer、small-stack 三组 AI controller：passed。

脚本 `scripts/check.py` 本身不执行 doctest。单独命令

```text
CARGO_TARGET_DIR=work/ai-command-snippet-save-20261114/full-target cargo test --workspace --doc --locked --offline
```

实际结果为 `14 passed / 0 failed`，其中 `2` 项 compile-fail doctest 通过。

## 作者自查

| 级别 | 数量 | 说明 |
| --- | ---: | --- |
| P0 | 0 | 没有发现自动执行、凭据写入或绕过审核的阻断问题 |
| P1 | 0 | 没有发现 stale target/session/configuration/modal 仍能写入的问题 |
| P2 | 0 | 定向回归和源码检查未发现当前切片内的中等级别问题 |
| P3 | 0 | 当前切片未登记低等级缺陷 |

独立复核必须重新给出自己的计数和证据。

## 独立严格只读复核

状态：`PENDING`。复核必须固定最终文档 SHA，在独立源码 worktree、独立 Cargo
target 和独立日志目录中运行。复核范围包括：

- P0/P1/P2/P3；
- 每个 finding 的绝对路径、行号、触发条件和影响；
- 一次性 review event、双重 target/session admission、locked command invariant；
- StateStore conflict/failed save 的完整草稿保留；
- 普通片段持久化不含 session/host/credential/AI profile/review token；
- 双语、三主题、fixed footer/scroll/accessibility；
- 保存和取消没有执行副作用；
- 实际测试命令、计数、限制和 `APPROVE`/`REQUEST_CHANGES`。

## 未验证边界

本记录没有把以下内容写成通过：

- macOS 真实窗口级 GPUI 视觉、IME、VoiceOver 和完整辅助技术；
- Windows/Linux 原生窗口、输入法和辅助技术；
- 真实客户 SSH、主机密钥变化现场和外部 AI provider；
- Podman 容器联调、跨进程 StateStore、断电耐久性；
- 签名、公证、安装升级、发布制品和 CI runner；
- 完整业务验收。

没有 push。父工作树和旧
`/Users/hdh/.codex/worktrees/ai-command-snippets-20261113` WIP 未修改。
