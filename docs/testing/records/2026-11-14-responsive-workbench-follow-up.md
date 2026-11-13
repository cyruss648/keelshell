# 响应式工作台紧凑修复与完整门禁记录 — 2026-11-14

日期：2026-11-14（Asia/Shanghai）
源码基线：`9194d7da6c7fc628bc3d8f7c1155afb5c91e31f1`
关联记录：[响应式工作台视觉切片](2026-11-13-responsive-workbench-polish.md)

## 本次修复与复核补强

本片收口上一片响应式工作台在独立复核中暴露的紧凑边界问题：

- AI 助手列继续使用窗口宽度的 34% 和 `360px` 上限；窗口宽度低于
  `720px` 时最低宽度改为 `200px`，否则保持 `240px` 最低宽度。900×580
  目标仍使用 240px，480px 空工作区打开助手时为快速连接表单保留可用空间。
- 主机监控的磁盘设备列表由 `max_h(144px)` 调整为固定 `160px`，使导航
  header 增加后的最后一行设备仍能通过列表滚动完整到达。
- workspace layout 集成测试改用 `iter_mut().take(...)`，消除 Rust 1.100
  下新增的 Clippy 诊断。
- 移除未使用的 workspace `anyhow` 依赖声明。
- 首次中文工作区测试改为断言生产按钮的完整无障碍标签
  “切换语言：中文或英文”。
- 480px 窄窗口中的快速连接表单限制在连接管理器内部滚动，避免英文长文案把
  连接列表压缩到不可操作的高度。
- 窄宽度连接库工具栏使用图标优先布局；每个动作仍保留完整双语
  `accessibility_label` 和 tooltip，加密同步增加 Lock 图标。
- 批量操作栏在窄宽度使用紧凑间距和“批量 / Batch”短文案，同时保留 Group 语义；
  连接列表滚动容器和文件夹树增加测试句柄与最小高度约束。
- 连接库回归使用 12 条连接记录，验证首条和末条记录、滚动后的全部动作 bounds，
  并实际点击末条编辑动作；
- 工具栏和批量操作栏增加中英文 Group role、AX label、按钮 bounds 及批量 Move
  动作回归；
- 快速连接增加 480×900 英文滚动回归，确认滚动后 Connect 动作仍在快速连接区域
  内且不会改变下方连接库 sibling bounds；
- 480×580 低高度窗口将快速连接区域收紧为 `120px`，内容顶部对齐并在自身区域
  内滚动；连接库继续保留约 `48px` 的正向列表视口。480×580 和 480×900 均挂载
  一个真实保存连接，验证首条连接可见、列表使用 `Role::List`、滚动后保存连接和
  快速连接的 Connect 动作可达，以及快速连接滚动不会改变连接库 sibling bounds。

生产视图仍只消费现有 registry、Palette、组件和 chrome token。三套主题 Frost
「霜白」、Graphite「曜石」、Lavender「雾紫」继续共用同一布局路径；新增主题
仍只扩展主题注册表和 token，不在业务视图中增加主题 ID 分支。

## 定向验证

以下验证均在源码基线 `9194d7d`、Rust 1.100.0、锁定依赖和隔离测试数据下完成：

| 范围 | 结果 |
| --- | --- |
| `workbench_layout` | `10 passed / 0 failed` |
| 主题设计测试 | `4 passed / 0 failed` |
| 磁盘监控独立工作区回归 | `1 passed / 0 failed` |
| 紧凑快速连接滚动回归 | `1 passed / 0 failed` |
| 紧凑工具栏/批量栏语义回归 | `1 passed / 0 failed` |
| 窄窗主题/语言/高度矩阵 | `1 passed / 0 failed` |
| 连接库专项 | `29 passed / 0 failed` |
| 首次中文工作区回归 | `1 passed / 0 failed` |
| `cargo test -p keelshell-core --test workspace_layout` | `12 passed / 0 failed` |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy -p keelshell-app --locked --offline --all-targets -- -D warnings` | 通过，无 warning |
| `git diff --check` | 通过 |

本片还保留了之前的 focused 几何断言：900×580 下 AI 列保持
`240..=360px`，活动终端保持至少 `200px × 80px`，连接导航与主机监控分别
保持 `236px` 和 `260px` rail 宽度，磁盘列表末行可滚动并可点击。

## 完整工程门禁

执行命令：

```text
python3 scripts/check.py
```

结果：退出码 `0`。实际阶段均完成：

- 直接 registry 依赖版本策略；
- Python 回归：`131 run / 1 skipped`；
- `cargo fmt --all -- --check`；
- workspace 全 targets 严格 Clippy；
- `cargo test --workspace --locked -- --test-threads=4`；
- `keelshell-ai` readiness-error、observer-io-error 和 small-stack controller
  三组专项；
- 14 项 rustdoc/doctest。

app 完整测试结果为 `1124 passed / 2 ignored`。core、session、MCP、AI 和工作区
集成测试均在 workspace 测试阶段通过；需要 Podman、OpenSSH、真实 Linux 采样、
Windows/Linux 原生桌面或外部 provider 的测试继续按测试合同保持 `ignored`，
没有被计入已执行通过的真实联调。

app 测试期间仍可能看到既有 macOS 链接器 warning
`__eh_frame section too large`；它没有转化为编译或测试失败。之前由 Rust 1.100
触发的 `needless_borrow` 已在本片前修复，最终严格 Clippy 没有 warning。

## 独立代码与功能复核

此前独立严格只读复核以源码 SHA `8dd5732` 为基线发现一个 P2 证据缺口：
新增回归只验证首条连接和容器边界，没有验证末条连接滚动后的动作可达、工具栏/
批量栏语义和快速连接滚动行为。本片在
`9194d7da6c7fc628bc3d8f7c1155afb5c91e31f1` 中补齐上述证据，并以最终源码重新
执行完整门禁。

针对包含本片最终文档快照的新提交，独立严格只读代码与功能复核尚待启动。复核
必须使用独立 Cargo target，报告 P0/P1/P2/P3 计数、绝对路径和行号、实际命令及
测试计数、未验证边界和 merge recommendation；复核 agent 不得编辑、格式化、提交、
推送、合并或清理共享 worktree。第一次独立复核尝试因无法形成可审计正文被终止，
不能计入批准。

## 未覆盖边界

本片不代签以下交付结论：

- 当前源码绑定的 macOS 包、签名、公证、安装和升级；
- Windows/Linux 原生窗口与布局；
- IME、VoiceOver/NVDA、完整键盘辅助技术和真实像素对比度；
- 真实 SSH、Podman 主机监控和长时间外部 AI provider；
- 900×580 在真实桌面上的窗口调整、多显示器和系统缩放表现；
- 480×580/480×900 的 headless GPUI 几何与受控交互已经覆盖，但真实 macOS
  桌面窗口调整、多显示器和系统缩放仍未验收。

## 结论

`9194d7da6c7fc628bc3d8f7c1155afb5c91e31f1` 的响应式紧凑修复与复核补强已通过
focused 回归、Rust 1.100.0 下的完整工程门禁和三套主题/双语受控布局检查，可以
作为当前本地源码候选继续维护。独立复核和上述原生/真实联调边界仍保持开放。公司
内网暂不可用，本片只做本地提交，不执行 push。
