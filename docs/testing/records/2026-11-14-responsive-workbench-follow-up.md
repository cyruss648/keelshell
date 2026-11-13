# 响应式工作台紧凑修复与完整门禁记录 — 2026-11-14

日期：2026-11-14（Asia/Shanghai）  
源码基线：`f5f787b8153fba2dcc563b0b258e0350ee5a2056`  
关联记录：[响应式工作台视觉切片](2026-11-13-responsive-workbench-polish.md)

## 本次修复

本片收口上一片响应式工作台在完整门禁中暴露的紧凑边界问题：

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

生产视图仍只消费现有 registry、Palette、组件和 chrome token。三套主题 Frost
「霜白」、Graphite「曜石」、Lavender「雾紫」继续共用同一布局路径；新增主题
仍只扩展主题注册表和 token，不在业务视图中增加主题 ID 分支。

## 定向验证

以下验证均在源码基线 `f5f787b`、Rust 1.100.0、锁定依赖和隔离测试数据下完成：

| 范围 | 结果 |
| --- | --- |
| `workbench_layout` | `10 passed / 0 failed` |
| 主题设计测试 | `4 passed / 0 failed` |
| 磁盘监控独立工作区回归 | `1 passed / 0 failed` |
| 紧凑连接库回归 | `1 passed / 0 failed` |
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

app 完整测试结果为 `1121 passed / 2 ignored`。core、session、MCP、AI 和工作区
集成测试均在 workspace 测试阶段通过；需要 Podman、OpenSSH、真实 Linux 采样、
Windows/Linux 原生桌面或外部 provider 的测试继续按测试合同保持 `ignored`，
没有被计入已执行通过的真实联调。

app 测试期间仍可能看到既有 macOS 链接器 warning
`__eh_frame section too large`；它没有转化为编译或测试失败。之前由 Rust 1.100
触发的 `needless_borrow` 已在本片前修复，最终严格 Clippy 没有 warning。

## 独立代码与功能复核

已启动固定源码 SHA `f5f787b8153fba2dcc563b0b258e0350ee5a2056` 的独立严格只读
复核 agent，使用独立 Cargo target。复核要求包括 P0/P1/P2/P3 严重级别、绝对
文件/行号证据、触发条件、实际命令计数、未验证边界和 merge recommendation；
复核 agent 不得编辑、格式化、提交、推送、合并或清理共享 worktree。

截至本记录写入时，复核 agent 尚未返回可审计正文，因此作者门禁不能写成独立
批准。收到报告后应在本记录和交接文档中补充结论；如果发现问题，修复后必须以
新源码 SHA 重新执行相关门禁并重新启动复核。

## 未覆盖边界

本片不代签以下交付结论：

- 当前源码绑定的 macOS 包、签名、公证、安装和升级；
- Windows/Linux 原生窗口与布局；
- IME、VoiceOver/NVDA、完整键盘辅助技术和真实像素对比度；
- 真实 SSH、Podman 主机监控和长时间外部 AI provider；
- 900×580 在真实桌面上的窗口调整、多显示器和系统缩放表现。

## 结论

`f5f787b` 的响应式紧凑修复已通过 focused 回归、Rust 1.100.0 下的完整
工程门禁和三套主题/双语受控布局检查，可以作为当前本地源码候选继续维护。
独立复核和上述原生/真实联调边界仍保持开放。公司内网暂不可用，本片只做本地
提交，不执行 push。
