# ADR 0122：编辑器格式策略与迟到读回组合回归

- 日期：2026-11-08
- 状态：组合回归与完整工程门禁已通过；源码已固定为
  `9134cfaa771057a0f246ead47479be3706248415`；首次独立复核的 P2 文档状态问题已
  修订，第二次独立复核为 `APPROVE`（P0/P1/P2/P3 均为 0），精确包待完成
- 范围：无换行文本连续保存、合并采用后的保存、较新草稿与成功读回的时序
- 关联：[远程文本格式](../product/REMOTE_TEXT_FORMAT.md)、
  [远程文本合并](../product/TEXT_CONFLICT_MERGE.md)、
  [测试记录](../testing/records/2026-11-08-editor-format-merge-combinations.md)、
  [当前交接](../HANDOFF.md)、[开发路线](../ROADMAP.md)

## 背景

编辑器格式策略和三方合并保存各自已经有回归，但跨操作的错误通常发生在状态
边界：无换行文件第一次插入换行后，连续保存空内容或再次无换行内容可能丢失已知
插入策略；合并采用后进入保存审核时，编辑器又可能在发布或读回期间产生较新草稿。
只测试单个操作无法证明这些状态会一起保持。

## 决策

### 1. 无换行文件在首次保存后固定 LF 插入策略

打开时观察到没有换行的文件使用 `LF` 作为新插入换行的策略。成功保存后，即使
远端内容变成空文件或再次变成无换行内容，编辑器仍保留 `LF` 策略。后续编辑只
转换新插入的换行，不增加 BOM，也不把内容改成 CRLF。

测试通过真实 GPUI 编辑器在末尾按 Enter 插入首个换行，再连续确认带换行、空内容、
无换行内容和后续带换行内容的保存；每一步都检查审核字节、读回字节、格式 token
和备份原始字节。

### 2. 合并采用、保存审核和编辑器草稿保持独立身份

采用合并草稿只更新本地编辑缓冲区和当前远端观察，不写远端。保存审核捕获合并
版本作为审核基线；发布期间编辑器可以继续产生较新草稿。成功读回只更新成功
发布内容对应的编辑基线，不能覆盖较新草稿。下一次保存必须重新审核，并使用
成功读回的新基线和原有格式策略生成出站字节。

测试使用受控原子写入停顿让较新草稿在真实发布路径中产生，并在停顿释放后等待
完整目标读回；它同时断言远端内容仍是已审核合并结果、编辑器仍是较新草稿、基线
已经切换，以及下一次保存的 reviewed/content 字节正确。

### 3. 组合测试不修改生产授权边界

两个回归只增加测试，不改变保存、备份、原子替换、完整读回、失败隔离或人工确认
规则。生产代码仍要求 Adopt 与 Save 分离，AI 或 patch 不能自动批准；测试夹具只
提供受控 loopback SFTP、写入停顿和远端字节观察。

## 取舍与限制

本片选择真实 GPUI、loopback SSH/SFTP 和已有测试夹具，以验证跨层状态传递；它不
把 headless 结果扩展成 Windows/Linux 原生窗口、IME、VoiceOver、真实客户 SSH
长流程或外部 AI provider 的验收。原子写入停顿验证了编辑器对迟到结果的状态处理，
不能证明恶意远端在最后元数据复核与 rename 之间不存在竞争。

## 验证

定向命令：

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

专项重跑结果：

- `editor_format_tests`：`14 passed / 0 failed / 1103 filtered out`；
- `conflict_merge_tests`：`29 passed / 0 failed / 1088 filtered out`；
- `rustfmt --edition 2024 --check` 与 `git diff --check`：退出 `0`。

完整工程门禁第二轮 `python3 scripts/check.py` 退出 `0`：Python
`131 run / 1 skipped`，workspace Rust `2,437 passed / 23 ignored`，app
`1,115 passed / 2 ignored`，14 项 doctest、依赖策略、严格 workspace Clippy 和
readiness/observer/small-stack AI controller 均通过。第一轮门禁在格式阶段因新增
断言尚未格式化而退出 `1`，该失败事实保留在测试记录中。

源码提交已经完成。首次独立只读复核指出的状态文案问题已在本次文档修订中关闭，
第二次独立复核返回 `APPROVE`；精确新包和原生桌面验收仍未完成，这些证据完成前
不写成发布或跨平台原生完成。
