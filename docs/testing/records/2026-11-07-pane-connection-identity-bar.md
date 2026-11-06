# 测试记录：终端 pane 连接身份栏

- 日期：2026-11-07
- 工作树：`feature/main-visual-integration-20261106`
- 范围：`keelshell-app` pane header 与工作区布局
- 结论：定向验证通过；完整工程门禁、独立源码复核和原生桌面验收待完成

## 实现摘要

每个终端 pane 都保留会话标签和比例控制，并持续显示：

- Workspace 快照中的真实 SSH 端点；
- `SSH / UTF-8` 协议与编码信息；
- 启动中、已连接、已断开和待恢复状态；
- 对应的双语可访问标签。

连接身份没有写入 `TerminalView`，也没有新增主题 ID 分支。恢复标签只从当前
保存连接生成端点，找不到连接时显示缺失状态。

## 定向命令

| 命令 | 结果 |
| --- | --- |
| `cargo test -p keelshell-app every_terminal_pane_keeps_a_bilingual_connection_identity_bar --locked` | 1 passed |
| `cargo test -p keelshell-app workbench_regions_fit_all_registered_themes_without_changing_drafts --locked` | 1 passed |
| `cargo test -p keelshell-app long_session_titles_keep_close_visible_in_both_languages_and_window_sizes --locked` | 1 passed |
| `cargo test -p keelshell-app minimum_window_keeps_bilingual_appearance_and_workspace_actions_visible --locked` | 1 passed |
| `cargo test -p keelshell-app workspace::tests::pane_layout --locked` | 16 passed |
| `cargo fmt --all` | passed |
| `git diff --check` | passed |

上述检查均使用真实 GPUI Kit 工作区测试和隔离 fixture；fixture 没有产生 SSH
写入。首次实现迭代中曾分别暴露 fixture 端点预期错误和协议 AX 标签包含状态的
断言错误，均已按实际渲染合同修正后重新通过，失败过程没有被当作通过结果隐藏。

## 尚未覆盖

- `python3 scripts/check.py` 的本轮完整 fresh 结果；
- 新提交的独立只读代码评审；
- macOS 原生主题切换、截图、窗口尺寸、IME、键盘和 VoiceOver；
- Windows/Linux 原生桌面运行；
- 真实 SSH 主机和 Podman 联调。

Headless GPUI 结果只证明组件树、布局和局部输入合同，不能替代上述原生验收。

