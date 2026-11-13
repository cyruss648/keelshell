# 响应式工作台紧凑修复、复核修复与完整门禁记录 — 2026-11-14

日期：2026-11-14（Asia/Shanghai）
源码基线：`a33e4c3b154867e4ef542cd9ef6a1193aee31c19`
关联记录：[响应式工作台视觉切片](2026-11-13-responsive-workbench-polish.md)

## 本次修复与复核补强

本片收口上一片响应式工作台在独立复核中暴露的紧凑边界问题，并修复随后复核
发现的两个交互缺陷：

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

## 独立复核问题修复

此前针对源码与文档快照
`78091e684f95cba5faecc54571992142f5fcd766` 的严格只读复核返回 P0 `0`、P1
`1`、P2 `1`、P3 `2`：

- P1 由 `6d1f1a1` 修复。连接库旧帧的 Edit 回调按当前连接 UUID 重新解析；
  连接被删除、失效或不再可编辑时失败关闭。新增两条 stale painted Edit 回归。
- P2 由 `a33e4c3` 修复。session-tab reveal key 纳入导航 rail 宽度，连接导航
  `236px` 切换到主机监控 `260px` 后重新计算可见区域，关闭动作不会被旧布局
  结果裁切。新增 workbench 回归。
- P3 是文档证据过期：测试索引与 Rust 1.100.0 记录仍绑定旧 SHA 和旧测试
  清单，本次文档提交同步改为当前源码 SHA 与当前计数。

## 定向验证

以下验证均在源码基线 `a33e4c3`、Rust 1.100.0、锁定依赖和隔离测试数据下完成：

| 范围 | 结果 |
| --- | --- |
| `workbench_layout` | `11 passed / 0 failed / 1118 filtered out` |
| 主题设计测试 | `4 passed / 0 failed / 1125 filtered out` |
| stale painted Edit | `2 passed / 0 failed / 1127 filtered out` |
| 连接库相关过滤测试 | `84 passed / 0 failed / 1045 filtered out` |
| `session_tabs` | `6 passed / 0 failed / 1123 filtered out` |
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

app 完整测试结果为 `1127 passed / 2 ignored`，测试清单共 `1129` 项，新增测试已实际
进入二进制。core、session、MCP、AI 和工作区
集成测试均在 workspace 测试阶段通过；需要 Podman、OpenSSH、真实 Linux 采样、
Windows/Linux 原生桌面或外部 provider 的测试继续按测试合同保持 `ignored`，
没有被计入已执行通过的真实联调。

app 测试期间仍可能看到既有 macOS 链接器 warning
`__eh_frame section too large`；它没有转化为编译或测试失败。之前由 Rust 1.100
触发的 `needless_borrow` 已在本片前修复，最终严格 Clippy 没有 warning。

## 独立代码与功能复核

此前独立严格只读复核以源码 SHA `8dd5732` 为基线发现一个 P2 证据缺口：
新增回归只验证首条连接和容器边界，没有验证末条连接滚动后的动作可达、工具栏/
批量栏语义和快速连接滚动行为；该缺口已在上一轮
`9194d7da6c7fc628bc3d8f7c1155afb5c91e31f1` 中补齐。随后针对
`78091e684f95cba5faecc54571992142f5fcd766` 的复核又发现上述 P1/P2，已由
`6d1f1a1` 和 `a33e4c3` 修复，并以当前源码重新执行完整门禁。

针对文档提交
`251a8ee0398f5927685ae36f2f486e2d6d77d71d` 的新的独立严格只读代码与功能复核
已完成。复核 worktree 为
`/Users/hdh/.codex/worktrees/responsive-workbench-review-20261114/keelshell`，
Cargo target 为 `/private/tmp/keelshell-review-target-251a8ee`；复核 agent 未编辑、
格式化、提交、推送、合并或清理仓库。

复核结果为 merge recommendation `APPROVE`，P0/P1/P2/P3 均为 `0`。实际独立
回执包括 app `--list` 的 `1129 tests, 0 benchmarks`、design
`4 passed / 1125 filtered out`、workbench `11 passed / 1118 filtered out`、
stale painted Edit `2 passed / 1127 filtered out`、`workspace::tests::library::`
`31 passed / 1098 filtered out`、session-tabs `6 passed / 1123 filtered out`、
窄窗/快速连接/磁盘/工具栏矩阵各 `1 passed / 1128 filtered out`、core
workspace layout `12 passed`，以及严格 Clippy、fmt 和差异检查通过。

独立复核还记录了两个有界中止：宽 `library` 过滤器匹配 84 项但未在该复核中
形成最终摘要，app 全量测试超过五分钟未形成摘要；这两项没有被计入独立通过。
作者门禁在源码 `a33e4c3b154867e4ef542cd9ef6a1193aee31c19` 上的 app 全量
`1127 passed / 2 ignored` 仍作为单独作者回执保留。原生窗口、IME、VoiceOver/
NVDA、真实 SSH/Podman、外部 AI provider、打包签名安装升级等边界仍未验收。
当前状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。

## 未覆盖边界

本片不代签以下交付结论：

- 当前源码绑定的 macOS 包、签名、公证、安装和升级；
- Windows/Linux 原生窗口与布局；
- IME、VoiceOver/NVDA、完整键盘辅助技术和真实像素对比度；
- 真实 SSH、Podman 主机监控和长时间外部 AI provider；
- 900×580 在真实桌面上的窗口调整、多显示器和系统缩放表现；
- 真实 macOS 桌面窗口调整、多显示器和系统缩放表现。

480×580/480×900 的 headless GPUI 几何与受控交互已在本片覆盖；该结果不代签
真实桌面窗口的系统级缩放和多显示器表现。

## 结论

`a33e4c3b154867e4ef542cd9ef6a1193aee31c19` 的响应式紧凑修复与复核补强已通过
focused 回归、Rust 1.100.0 下的完整工程门禁和三套主题/双语受控布局检查，可以
作为当前本地源码候选继续维护。文档提交 `251a8ee` 的独立复核已返回
`APPROVE`，P0/P1/P2/P3 均为 `0`；本次后续文档提交只记录该复核结果，不改变
源码。上述原生/真实联调边界保持开放。公司内网暂不可用，本片只做本地提交，不
执行 push。
