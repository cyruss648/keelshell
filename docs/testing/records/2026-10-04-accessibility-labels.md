# GPUI 可访问名称 — 2026-10-04

本轮为远程工作区和连接管理器补充显式 accessibility label，保持可见文案和布局不变：

- 一次性快速连接的主机、端口、用户名和可选私钥输入，以及连接库搜索输入，使用双语字段名作为原生输入名称。
- 顶部新建 SSH 会话、连接管理器关闭、会话标签关闭使用双语操作名称；会话标签名称包含“关闭 SSH 会话”。
- 连接库收藏、新建文件夹和文件夹管理等符号按钮使用包含目标名称的双语操作名称。
- 终端搜索输入、上一项、下一项和关闭搜索使用双语名称，符号不会成为唯一提示。

`crates/keelshell-app/src/workspace/tests.rs` 的 GPUI 回归在真实渲染窗口中读取 `ElementSnapshot::label()`，验证首次中文界面和切换英文后的快速连接输入、新会话按钮名称，并恢复中文状态。该测试只验证 GPUI accessibility tree 中的名称和状态，不声称已完成 macOS VoiceOver、Windows Narrator、Linux AT-SPI 或小窗口原生桌面验收。

验证状态：目标文件已通过 `rustfmt` 和 `git diff --check`。修复共享文件面板的组合语法后，`cargo test -p keelshell-app --locked` 已通过（267 项），其中包含首次启动中英文 accessibility label 的真实 GPUI `ElementSnapshot` 断言，以及终端搜索输入/导航/关闭按钮的回归。该测试只验证 GPUI accessibility tree 中的名称和状态，不声称已完成 macOS VoiceOver、Windows Narrator、Linux AT-SPI 或小窗口原生桌面验收。
