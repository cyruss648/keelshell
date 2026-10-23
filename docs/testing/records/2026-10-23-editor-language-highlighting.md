# 远程编辑器语言识别与语法高亮记录 — 2026-10-23

## 实现范围

FILE-04 第一阶段把主远程草稿输入从 `TextareaState` 迁移到 GPUI Kit 0.7 的 `EditorState`。打开文件或加载新的审核草稿时，仅根据远端路径最后一个文件名组件选择语言：Rust、Python、JSON/JSONC、TOML、YAML/YML 和 Bash/Shell。未知、歧义、无扩展名和当前未启用 grammar 的 SQL 回退为 `text`。

编辑器自有字面查找／替换控件保持唯一搜索入口；GPUI 内置搜索、折叠、自动闭合和智能缩进关闭。补丁编辑器与冲突合并手工输入仍使用 `TextareaState`。语言识别不嗅探内容，不执行路径，不改变草稿字节。

## 工程变更

- `gpui-kit` 0.7 启用窄范围 Tree-sitter grammar：Bash、JSON（基础 tree-sitter 功能）、Python、Rust、TOML 和 YAML。
- 未启用 SQL grammar，以控制首阶段语言范围。当前已启用 grammar 集合会把 `cc` 解析到旧兼容锁定线；扩展 SQL 或其它语言需单独评估依赖和原生构建影响。SQL 文件继续以纯文本打开。
- 语言识别函数和大小写、隐藏 shell 配置、未知路径、歧义路径测试位于 `crates/keelshell-app/src/files/editor_language.rs`。

## 实际验证

```text
cargo fmt --all                         PASS
cargo check -p keelshell-app --offline  PASS (0 errors; existing block 0.1.6 future-incompatibility warning)
cargo test -p keelshell-app files::editor_language --locked
  PASS (2 passed, 754 filtered out)
```

随后在当前工作树实际运行完整 `python3 scripts/check.py`，退出码为 0。门禁包含 67 个 Python 脚本用例（1 个 Windows 原生用例按既有条件跳过）、依赖版本策略、全 workspace 格式、严格 Clippy、workspace 单元／集成／文档测试，以及三个有界本地 agent 进程控制场景。应用测试结果为 755 passed、2 ignored；其余 workspace crate 与集成测试均通过，OpenSSH、Podman 网络诊断和真实桌面场景按既有条件保持 ignored 或未运行。

这轮门禁第一次重跑时在既有 `keelshell-session` 网络诊断夹具的并发清理断言失败：测试用临时端口的 `ConnectionRefused` 反推自身监听器已关闭，并行测试可能复用该端口，造成归属误判。单线程专项通过后，夹具改为由监听器所有者发布显式 drop receipt；修复后的并发专项及完整门禁通过。该测试修复不改变生产网络诊断协议。

这轮门禁重新覆盖了查找替换、冲突合并、审核保存、同目录备份、拖放受控流程和远端完整读回回归。仍未覆盖提交级 CI、语法颜色像素、IME、VoiceOver、最小窗口或 Windows/Linux 原生桌面；完整工程门禁不等于这些平台验收。

## 安全与产品边界

语言高亮是编辑器显示能力。它不启用 LSP、代码执行、远程命令、自动修复或 AI 自动写回；保存仍由完整文本审核和显式确认控制。SQL grammar、编码转换、换行风格保持及原生辅助技术继续保持独立开放项。
