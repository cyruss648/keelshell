# 会话、连接库与 AI 助手界面增强测试记录 — 2026-11-07

- 工作树：`feature/main-visual-integration-20261106`
- 范围：连接库范围反馈、会话条导航、AI 工作流表面
- 当前结论：定向 GPUI 回归和完整工程门禁通过；独立只读复核与原生桌面验收仍开放

## 实现摘要

- 连接库标题显示当前范围、总量、搜索命中、批量选择和搜索条件摘要；
  空库、空收藏、空最近连接、空回收站、空文件夹和无搜索结果分别表达；
- 会话条显示会话数量、双语 AX 语义和单会话标题；上一个/下一个按钮支持
  macOS `Command-Option-Left/Right` 以及 Windows/Linux `Control-Alt-Left/Right`；
- AI 助手增加六阶段工作流轨道、上下文摘要、请求审核摘要、回复审核标题和
  Agent 安全边界提示；普通回复到达后自动显露回复审核区域；
- 知识保存动作固定在助手底部审核栏，长预览不会把保存按钮推到正文滚动区域之外；
- Agent 停止并编辑问题时，停止状态完成布局后按最新问题矩形调整正文滚动，
  小窗口下不会把问题输入框留在历史滚动位置之外；
- 三套内置主题 Frost「霜白」、Graphite「曜石」、Lavender「雾紫」继续由
  registry、Palette、组件形状、Chrome token 和 Typography token 提供，视图
  没有按主题 ID 分支。

## 定向验证

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all` | passed |
| `cargo check -p keelshell-app --locked` | passed |
| `cargo test -p keelshell-app --locked library_ -- --test-threads=1` | 26 passed |
| `cargo test -p keelshell-app --locked assistant_ -- --test-threads=1` | 6 passed |
| `cargo test -p keelshell-app --locked keyboard_session_navigation_keeps_the_active_target_and_exposes_count -- --test-threads=1` | 1 passed |
| `cargo test -p keelshell-app --locked session_tabs -- --test-threads=1` | 5 passed |
| `cargo test -p keelshell-app --locked mcp_long_multiple_proposals_scroll_to_review_and_execute_in_both_languages -- --nocapture --test-threads=1` | 1 passed |
| `cargo test -p keelshell-app --locked reviewed_agent_stop_and_edit_question_is_reachable_in_small_bilingual_themes -- --nocapture --test-threads=1` | 1 passed |
| `cargo test -p keelshell-app --locked workspace_capture_review_and_save_round_trips_confirmed_knowledge -- --nocapture --test-threads=1` | 1 passed |
| `python3 scripts/check.py` | exit 0 |
| `git diff --check` | passed |

AI 工作流首轮定向测试曾在回复测试中因新增内容把回复区推到滚动视口之外而失败；
实现补充回复锚点并在收到普通回复后安排下一帧滚动，精确重跑为 2 passed。失败
事实保留在本记录中，不回写成从未发生。

本轮三项可达性回归的中间失败也保留如下：

- 长 MCP 提案在最终源码精确重跑为 1 passed；
- Agent 编辑问题第一次和一次基于 `window.defer` 的修补仍在
  `900×580` 下得到 `question.top() = -7.5px`，说明事件回调使用了旧布局；
- 最终改为在新状态 render 的 `on_prepaint` 中按问题和正文 viewport 的实际几何
  修正 `ScrollHandle` 偏移，精确重跑为 1 passed；
- 知识保存长预览回归最终为 1 passed，保存按钮位于固定 footer；
- 失败输出包含 GPUI 链接器的 `__eh_frame section too large` warning，但没有导致
  测试失败。

## 安全和行为边界

本轮只增加 UI 状态派生、滚动和键盘可达性。连接、SSH transport、凭据、命令
审核、Agent 操作审核、知识保存闸门和外部 AI provider 协议没有被放宽。快捷键
只切换已有会话，不创建连接、不发送命令、不覆盖草稿。

## 精确提交复核与当前 macOS 包

源码片已提交并推送到 `feature/main-visual-integration-20261106`：

- reviewed commit / HEAD：`dd6c88b0710bb56ca785eeeb83ee0835a1b77ede`；
- parent：`8402a93c0304b8f0766e8f2f4deb79b985c59ff6`；
- 本地、tracking、远端 SHA 一致，`git status --porcelain=v1` 为空。

新的独立严格只读复核固定在上述 exact tip，未编辑、reset、checkout、提交、推送
或清理工作树，结论如下：

- P0：0；P1：0；P2：0；P3：0；
- merge recommendation：`APPROVE`，同时保留本文列出的原生和外部集成边界；
- `git diff --check dd6c88b^ dd6c88b`：通过；
- `cargo test -p keelshell-app --locked keyboard_session_navigation_keeps_the_active_target_and_exposes_count -- --test-threads=1`：1 passed，0 failed，1085 filtered；
- `cargo test -p keelshell-app --locked workspace_capture_review_and_save_round_trips_confirmed_knowledge -- --test-threads=1`：1 passed，0 failed，1085 filtered。

上述两项测试出现既有 `ld: __eh_frame section too large` warning，但命令均退出
0。复核 agent 没有重新执行完整 `python3 scripts/check.py`，完整门禁退出 0 的计数
来自本片作者回执；共享 target 失效也没有在只读复核中执行。

为关闭共享 target 归属风险，本片随后执行：

```text
cargo clean --locked --offline -p keelshell-app -p keelshell-core -p keelshell-session -p keelshell-ai -p keelshell-mcp
MACOSX_DEPLOYMENT_TARGET=15.0 KEELSHELL_BUILD_COMMIT=dd6c88b0710bb56ca785eeeb83ee0835a1b77ede cargo build -p keelshell-app -p keelshell-mcp --release --locked
KEELSHELL_BUILD_COMMIT=dd6c88b0710bb56ca785eeeb83ee0835a1b77ede python3 packaging/package.py macos --binary target/release/keelshell-app --mcp-binary target/release/keelshell-mcp --target aarch64-apple-darwin --output work/native-cua-20261107/package-r6
python3 packaging/inspect_native.py --platform macos --target aarch64-apple-darwin --stage work/native-cua-20261107/package-r6
```

四步最终均退出 0；首次没有传入 `KEELSHELL_BUILD_COMMIT` 的 `package-r6` 临时
目录读回 `build.commit=null`，已删除且不作为证据。最终 `package-r6` manifest
绑定 exact `dd6c88b...`，`installed=false`，GUI SHA-256 为
`42d546065cfd8e056b96de597cb564e650f03a8c6ab4f6117f76f1c1791786f5`，MCP SHA-256
为 `94eaf83f8bbb23ae4db7cc190c9c06332ddbc19b4bf9246dcdf35f43b3645d8f`，目标为
`aarch64-apple-darwin`，最低 macOS 版本为 15.0。包结构检查只证明 bundle、架构、
依赖和 manifest 自洽，不证明签名、公证、安装或完整 GUI 验收。

解锁后从 `package-r6/KeelShell.app` 启动并完成短范围 smoke：连接库空态显示当前
范围、数量和新建连接入口；主题对话框依次将 Frost、Graphite、Lavender 标为
当前并可应用，之后恢复 Graphite；AI 助手侧栏显示六阶段轨道、明确捕获提示、
问题输入和预览动作，AX 树可读到相应按钮和文本。测试结束后 package-r5/r6 的
KeelShell 进程均已关闭。该 smoke 没有运行真实 SSH、完整分屏/恢复长流程、IME、
VoiceOver 或外部 AI provider。

## 尚未完成

完整 `python3 scripts/check.py` 已退出 0，包含 Python 131 run / 1 skipped、格式、
严格 workspace Clippy、workspace Rust tests、doctests、readiness/observer/
small-stack AI controllers 和依赖策略。当前 exact-tip 独立代码/功能复核、macOS
当前源码包的长流程、Windows/Linux 原生窗口、IME、VoiceOver、真实 Podman/SSH
联调和外部供应商请求仍需独立证据。Headless GPUI 结果不能代替这些边界。
