# 测试记录：终端 pane 连接身份栏

- 日期：2026-11-07
- 工作树：`feature/main-visual-integration-20261106`
- 范围：`keelshell-app` pane header 与工作区布局
- 结论：当前源码定向验证与完整工程门禁通过；route/split 夹具稳定性已在完整
  app 并行负载下复核；当前候选待独立源码复核，macOS/Windows/Linux 原生桌面
  验收仍未完成

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
| `cargo test -p keelshell-app workspace::tests::pane_layout --locked` | 18 passed |
| `cargo test -p keelshell-app workspace::tests::routes::reconnect --locked` | 8 passed |
| `cargo fmt --all` | passed |
| `git diff --check` | passed |

上述检查均使用真实 GPUI Kit 工作区测试和隔离 fixture；fixture 没有产生 SSH
写入。首次实现迭代中曾分别暴露 fixture 端点预期错误和协议 AX 标签包含状态的
断言错误，均已按实际渲染合同修正后重新通过，失败过程没有被当作通过结果隐藏。

## 完整工程门禁

本轮源码修复后重新执行了严格 Clippy 和完整 `python3 scripts/check.py`。实际结果：

- Python：131 passed，1 skipped；
- `keelshell-app`：1079 passed，0 failed，2 ignored；单线程与最终并行重跑均为此计数；
- `keelshell-core`：197 passed；
- `keelshell-session`：434 passed，16 ignored；4 线程重跑与最终 workspace 门禁均通过；
- workspace integration、MCP、SSH loopback、transfer、doctest 和 readiness/
  observer/small-stack AI controller 检查均通过；
- `cargo clippy --workspace --all-targets --locked -- -D warnings` 通过；
- `cargo fmt --all -- --check` 和 `git diff --check` 通过。

第一次完整门禁中的严格 Clippy 曾因新增测试代码的 `needless_borrow` 失败；
已修复后单独重跑 Clippy 并再次执行完整门禁。首次失败证据必须保留，不能把
最终成功描述成首轮无失败。

## Route/split 夹具稳定性

本轮完整 app 并行负载下首次执行 route/split 回归时，连接仍在正常建立：
gateway 已认证并打开跳板通道，target 已认证且 SSH 会话仍存活，
`connect_route` 仍处于进行中。原来的 10 秒 UI 就绪等待先于生产 route
的 15 秒 SSH 操作期限结束，导致测试夹具把健康连接误判为尚未就绪。

修复仅扩大测试夹具的有界等待：`reconnect.rs` 使用 20 秒等待 route 建立、
split 后新 shell 和恢复后的原 pane；route server session 使用 120 秒上限，
覆盖完整的 repeated split/close 循环。生产 reconnect、SSH 生命周期、命令审核和
AI 权限逻辑没有修改。恢复套件最终 8/8 通过，app 完整并行重跑为
1079 passed / 2 ignored，用时约 198.66 秒。

session 完整测试曾有一次 traceroute 夹具抖动：129 passed、1 failed、4 ignored，
失败用例为 `traceroute_hop_limit`，预期 hop-limit 诊断但在系统负载下得到
`timeout`。该用例精确重跑、4 线程重跑和最终 workspace 4 线程门禁均通过；
原始失败事实保留，不能回写成从未发生。

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

