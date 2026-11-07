# 测试记录：终端 pane 连接身份栏

- 日期：2026-11-07
- 工作树：`feature/main-visual-integration-20261106`
- 范围：`keelshell-app` pane header 与工作区布局
- 结论：当前源码定向验证与完整工程门禁通过；live SSH pane 的 IPv6 端点显示
  已统一使用带括号格式；macOS 原生包已完成主题、语言、分屏、合并、关闭面板、
  真实回环 SSH、断线和恢复场景的部分验收；当前候选仍待独立源码复核，
  900×580 紧凑窗口、Windows/Linux 原生桌面和完整辅助技术矩阵保持未完成

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
| `cargo test -p keelshell-app jump_host_picker::tests::endpoint_keeps_ipv6_account_and_port_unambiguous --locked` | 1 passed |
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

## 文档修订后的最终工程门禁

完成当前包绑定、原生结果和独立复核修订后，再次执行
`python3 scripts/check.py`，实际退出码为 `0`。本次结果为 Python 131 passed /
1 skipped，`keelshell-app` 1079 passed / 2 ignored，`keelshell-core` 197 passed，
session 汇总 434 passed / 16 ignored；workspace integration、MCP、SSH loopback、
transfer、doctest、readiness、observer、small-stack AI controller、严格 Clippy、
格式、依赖版本策略和差异检查均通过。该次运行包含完整当前 workspace 重新编译，
不是只读取旧的成功回执；app 测试阶段保留已有 linker warning，但没有失败。

## macOS 原生验收（旧人工观察包 `package-r3`）

以下人工观察绑定源码提交 `2589d3b` 的 `package-r3`。它记录了 IPv4 回环场景
的完整窗口操作；后续 `b2f8754` 只修正 live IPv6 身份显示格式，因此本节不把
旧人工观察重新归属于当前包。包绑定如下：

- 源码提交：`2589d3b62acc42a1527bbd5bcaca1d09bc32751f`
- 构建命令：`MACOSX_DEPLOYMENT_TARGET=15.0 cargo build -p keelshell-app -p keelshell-mcp --locked`
- 构建退出码：`0`
- target：`aarch64-apple-darwin`
- rustc：`rustc 1.99.1 (58cf8f9c3 2026-10-13)`
- Bundle：`work/native-cua-20261107/package-r3/KeelShell.app`
- Bundle ID：`app.keelshell.desktop`
- 隔离数据目录：`work/native-cua-20261107/data-r3`
- package manifest：`work/native-cua-20261107/package-r3/package-manifest.json`
- package manifest SHA-256：`8c164ec8fa7531173c85c657643c25013022be502dd95724fa2a5a835171aa70`
- app SHA-256：`116ed59468a6a6a0d51af23bcf697b4e5903fae4c344c09de9fca664b5616a16`
- MCP SHA-256：`036806f27b436bd1dfcbf48c22cee057cca40d2e1909bb8c699c3939cc169e09`
- 构建日志：`work/native-cua-20261107/build-2589d3b-fresh.log`
- 构建日志 SHA-256：`223b6ae8f27a3b3f350b97da05fc67c2d012d6f3111d8e680cc94e54632b86d7`
- 包内 `keelshell-app` 与 `keelshell-mcp` 的 SHA-256 与 package manifest
  的 `files`、`binary_sha256` 和 `mcp_binary_sha256` 一致；因此该包绑定到上述
  源码提交和冷构建输入。该包未安装到系统应用目录，native acceptance 由本记录
  单独观察。
- 回环端点：`hdh@127.0.0.1:22222`
- 主机指纹：`SHA256:IUwCETnmQsXnbmdoXhaFF6gbFOIkEb9cvSZHl1pt+yA`

首次使用 `SSH agent` 认证时，目标夹具明确返回 `SSH authentication rejected`；
随后在同一窗口将认证方式切换为包外的隔离夹具私钥
`work/native-cua-20261107/ssh/client_key`，空口令认证成功。私钥只用于本次
本地回环验收，没有写入 profile metadata 或测试记录。

在真实 macOS 窗口中完成并观察到：

1. 首屏中文工作区、Frost「霜白」、Graphite「曜石」、Lavender「雾紫」主题，
   中文/English 切换，以及深浅主题表面。
2. 使用真实回环 OpenSSH 连接并显示远程目录、命令审核区、文件面板、真实终端
   和底部已连接状态。
3. 横向分屏后出现两个独立 pane；再对活动 pane 纵向分屏，出现三 pane 和对应
   分隔条；合并活动 pane 后保留 SSH 会话，并在底部显示“已合并面板，会话继续
   保持 / Pane merged; sessions stay alive”。
4. 关闭一个 SSH 面板后，剩余 pane 和会话继续可用。
5. 停止当前回环 `sshd` 的会话进程后，按目标工作树相对 Bundle 路径重新绑定
   窗口，观察到断开会话仍保留，但分屏和命令执行动作被禁用，连接表单重新可用；
   这是实际断开状态，没有用 headless 结果代替。
6. 重启同一 `sshd` 夹具、关闭断开标签并重新认证后，界面恢复为已连接，真实终端
   执行 `printf NATIVE_RECOVERED` 并输出 `NATIVE_RECOVERED`。

本次验收全程绑定上述工作树相对的 `package-r3/KeelShell.app` 路径和
`data-r3` 隔离目录，没有把另一份窗口的状态混入本候选证据。窗口尝试调整为
900×580 时，macOS accessibility 返回 GPUI 无边框窗口异常 frame，截图仍为
全屏尺寸，因此该尺寸不能写成通过。当前原生证据只覆盖已列出的 macOS 场景；
Windows/Linux、IME、VoiceOver 和完整键盘矩阵仍需独立目标环境验收。

验收结束后已停止 app 和 sshd，确认 `22222` 无监听；`data-r3`、旧
`package`/`package-r2` 和回环夹具私钥/主机密钥已清理。`package-r3`、package
manifest、构建日志和本记录保留作为不可变验收证据；因此上述隔离数据目录是
运行时归属说明，不是当前仍存在的凭据或运行资源。

## 当前源码与包绑定（`package-r5`）

当前源码提交 `b2f875458e42cf374cb3d388d808e986a415c2cd` 已在共享 Cargo target
按项目包范围冷清理后重新构建。此次修复把初始连接和重连路径的 endpoint 字符串
统一改为 `jump_host_picker::endpoint`，因此 IPv6 目标显示为
`operator@[2001:db8::8]:2202`；连接 authority 和认证目标没有改变。

- target 失效命令：`cargo clean --locked --offline -p keelshell-app -p keelshell-core -p keelshell-session -p keelshell-ai -p keelshell-mcp`
- target 失效结果：退出码 `0`，`Removed 40554 files, 9.4GiB total`
- 构建命令：`MACOSX_DEPLOYMENT_TARGET=15.0 KEELSHELL_BUILD_COMMIT=b2f875458e42cf374cb3d388d808e986a415c2cd cargo build -p keelshell-app -p keelshell-mcp --locked`
- 构建退出码：`0`
- target：`aarch64-apple-darwin`
- rustc：`rustc 1.99.1 (58cf8f9c3 2026-10-13)`
- 当前 Bundle：`work/native-cua-20261107/package-r5/KeelShell.app`
- 当前 package manifest：`work/native-cua-20261107/package-r5/package-manifest.json`
- 当前 package manifest SHA-256：`31ea1613a180b9aa3f2d6a6df7f805bce017f5b4d74b13646302c3b113d725ce`
- 当前 app SHA-256：`2553a21da6e65e76d73fcea59573fb2dc99d503e622c9f8f5ee653bc3f274f50`
- 当前 MCP SHA-256：`fa49babd8bcd2c5e421de7449153054c8e023949a8b913d0221117515ed2ee41`
- 当前构建日志：`work/native-cua-20261107/build-b2f8754-fresh.log`
- 当前构建日志 SHA-256：`8b65f46cb83012d66bb0362777ea8c64e595b4dda29af5ee193e81401dd59b3a`
- `packaging/inspect_native.py --platform macos --target aarch64-apple-darwin --stage work/native-cua-20261107/package-r5`：退出码 `0`

当前包已通过短范围真实 macOS smoke check：启动了实际 `.app`，打开主题与外观
对话框，确认 Frost、Graphite、Lavender 三套主题均存在，应用 Graphite 深色变体，
切换到 English 并返回工作区；随后通过窗口关闭审核结束窗口。该 smoke check
没有重新执行 package-r3 已覆盖的真实 SSH、分屏、断开和恢复长流程，不能把旧
人工观察自动升级为 package-r5 的完整 native acceptance。当前包未安装到系统应用
目录；`package-r4` 曾因错误 provenance 环境变量生成，已在验收前删除，不属于
有效证据。

`package-r5` 是当前精确源码包；后续文档提交只记录测试和 provenance，不改变
该包的源码输入。`package-r3` 的详细 GUI 观察、`package-r5` 的当前源码绑定和
headless 工程门禁分别保持独立。

## 复核状态与限制

Avicenna 针对旧提交 `d9072ed` 的严格只读复核发现 1 个 P1 和 3 个 P2：
固定身份栏宽度压缩 pane 标签命中区域、恢复状态覆盖不足、视图写死字体字面量、
缺失档案与普通未连接状态没有区分。P1 已由 `1e63165` 修复，三个 P2 已在当前
源码中修复；该旧报告不构成当前候选的独立批准。当前提交还需要一轮固定最新
源码的严格只读复核，并要求返回 P0/P1/P2/P3、路径/行号、真实测试计数、
限制和直接 merge recommendation。

仍未覆盖：

- macOS 原生 900×580 紧凑窗口、完整键盘焦点、IME 和 VoiceOver；
- Windows/Linux 原生桌面运行；
- 真实 SSH 主机和 Podman 联调；
- 完整的多主题、多尺寸、辅助技术和跨平台原生矩阵。

Headless GPUI 结果只证明组件树、布局和局部输入合同，不能替代上述原生验收。

