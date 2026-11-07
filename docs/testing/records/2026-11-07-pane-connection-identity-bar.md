# 测试记录：终端 pane 连接身份栏

- 日期：2026-11-07
- 工作树：`feature/main-visual-integration-20261106`
- 范围：`keelshell-app` pane header 与工作区布局
- 结论：当前源码定向验证与完整工程门禁通过；当前候选待独立源码复核，
  macOS/Windows/Linux 原生桌面验收仍未完成

## 实现摘要

每个终端 pane 都保留会话标签和比例控制，并持续显示：

- Workspace 快照中的真实 SSH 端点；
- `SSH / UTF-8` 协议与编码信息；
- 启动中、已连接、已断开和待恢复状态；
- 对应的双语可访问标签。

连接身份没有写入 `TerminalView`，也没有新增主题 ID 分支。恢复标签只从当前
保存连接生成端点，未保存的一次性连接显示 `未保存连接 / Unsaved connection`，
找不到连接档案时显示 `配置已移除 / Profile unavailable`，并使用危险语义色。
连接身份区的代码字体通过 `TypographyTokens` 获取；Frost、Graphite、Lavender
仍由 registry、Palette、组件 token、chrome token 和 typography token 组成，
视图不按主题 ID 分支。

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

## 完整工程门禁

本轮源码修复后重新执行了严格 Clippy 和完整 `python3 scripts/check.py`。实际结果：

- Python：131 passed，1 skipped；
- `keelshell-app`：1078 passed，0 failed，2 ignored；
- `keelshell-core`：197 passed；
- `keelshell-session` 单元测试：130 passed，4 ignored；
- workspace integration、MCP、SSH loopback、transfer、doctest 和 readiness/
  observer/small-stack AI controller 检查均通过；
- `cargo clippy --workspace --all-targets --locked -- -D warnings` 通过；
- `cargo fmt --all -- --check` 和 `git diff --check` 通过。

第一次完整门禁中的严格 Clippy 曾因新增测试代码的 `needless_borrow` 失败；
已修复后单独重跑 Clippy 并再次执行完整门禁。首次失败证据必须保留，不能把
最终成功描述成首轮无失败。

## 复核状态与限制

Avicenna 针对旧提交 `d9072ed` 的严格只读复核发现 1 个 P1 和 3 个 P2：
固定身份栏宽度压缩 pane 标签命中区域、恢复状态覆盖不足、视图写死字体字面量、
缺失档案与普通未连接状态没有区分。P1 已由 `1e63165` 修复，三个 P2 已在当前
源码中修复；该旧报告不构成当前候选的独立批准。当前提交还需要一轮固定最新
源码的严格只读复核，并要求返回 P0/P1/P2/P3、绝对路径/行号、真实测试计数、
限制和直接 merge recommendation。

仍未覆盖：

- macOS 原生主题切换、截图、900×580 紧凑窗口、键盘焦点、IME 和 VoiceOver；
- Windows/Linux 原生桌面运行；
- 真实 SSH 主机和 Podman 联调；
- 完整的多主题、多尺寸、辅助技术和跨平台原生矩阵。

Headless GPUI 结果只证明组件树、布局和局部输入合同，不能替代上述原生验收。

