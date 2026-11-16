# 2026-11-16 命令输入框双向焦点导航

本批从 `892aada58a61458fea0f130ccc912036daf2804b` 集成独立作者的三文件
补丁；作者未运行 Cargo，父在同一 checkout、同一有效 target 完成实际编译。
没有切换 Cargo 归属或清理第三方缓存。

## 工程结果

- 仅测试补丁：编译成功，`4 passed / 4 failed / 0 ignored`，退出 101；
  红例原件保留，没有把编译错误当行为红例。
- 导入生产修复后：同一八项专项 `8 passed / 0 failed / 0 ignored`，退出 0。
- app 实际库存包含全部八个新增名字，合计 1,339 名；完整门禁执行清单逐名相等。
- 正常 macOS arm64 GUI/MCP 已构建，暂存包和原生包检查通过；
  未安装或签名发行。
- 718 项工程输入冻结 SHA-256：
  `c3fa4e8b1a8a0f11f8177334fd0c9927e2f1f45d4875cb61212987fadfc6f056`。

导航、模态和编辑器键盘专项分别 `29/0/0`、`11/0/0`、`9/0/0`，使用当前
编译且已核对库存的 app 测试程序。新非作者只读源码批准，开放 P0–P3 全为 0；
其独立执行测试数为 0，父专项日志没有改称 reviewer 的独立重编译结果。

最终完整门禁实际 `exit 0 / reaped / 699.865s`：普通 Rust
`3218 passed / 0 failed / 48 ignored`，rustdoc `14/0/0`；
Python `290 run / 289 passed / 1 skipped`，归档 helper `30/30`。
格式、严格 Clippy、x.y 政策与三组 AI controller 全通过。八个新增名字实际
通过，1,339 项 app 清单逐名匹配，718 项完整工程路径和字节零漂移。
正常 GUI/MCP 在门禁后摘要保持。原报告的完整门禁 PENDING 由相同输入的
`source-review-gate-binding.json` 另行满足，原报告保持不变。

## 限定原生与未验范围

新包实际连接自有 localhost SSH/SFTP，独立指纹核对后才接受。Frost 浅色中文
默认窗口中，r3 的完整草稿已出现在命令框；首个真实 Shift+Tab 的 U+0019
事件离开命令框，七步记录逐次保持草稿字节与执行标记 absent。
随后前台变化使脚本失败，没有完成到两文件列表的路径。

五轮焦点脚本均未完整成功：r1 本地目录 Enter 后未加载；显式刷新后加载。
r2 文字中途前台切换，r3 在七步后失败，r4 快照前台断言与异常截图失败，
r5 点击前没有唯一可见窗口。旧 helper 的文字投递只在开头检查前台；
新版补每字符检查，不能反推旧轮次的每字符投递归属或把部分成功拼成完整成功。
原始异常链、失败与改进前后源全部保留。

**原生双向到达两文件列表，尤其反向 Tab，继续 HOLD。** 本批没有 raw PTY
出站审计；marker absent 不等于零网络写入。零 Write 断言限定于八项隔离 GPUI
回归。Graphite/Lavender 本批原生 NOT_RUN，真实 IME、VoiceOver、
Windows/Linux 桌面与完整主题/语言/尺寸矩阵继续开放。

父实际停止夹具并取得退出 0。三个自有根进程结束、两端口关闭、私有目录
absent。最终 r2 清理回执以 JSON roundtrip 活进程为正控，归一化内层 birth
tuple，84 项累计身份零活匹配；未正确归一化的旧清理原件不可作为该结论依据，
已保留并明确替代。没有新建用户 chat、容器或 worktree。

本轮官方依赖查询超时且进程已实际回收，没有取得全部最新的成功回执。
Rust/Cargo 实测 1.100.0，默认 stable；没有 toolchain pin 或 rust-version，
直接 registry 要求继续采用 `x.y`，没有新增依赖。

源码、测试和设计分别见
[view.rs](../../../crates/keelshell-app/src/workspace/view.rs)、
[ADR 0146](../../adr/0146-command-composer-tab-focus.md)和
[产品键盘说明](../../product/COMMAND_COMPOSER_KEYBOARD.md)。
原件将封存到[持久证据](2026-11-16-command-composer-focus-evidence/README.md)，
交付与清理按精确 Git blob 回执读取。

上一源码精确 CI `43797131194 / attempt 1 / 80647ed` 已终态：
macOS 和 Ubuntu success，Windows app `1310 pass / 8 fail / 2 ignored`。
八项均在归档审核准备的共同断言失败，底层原因未记录；原生进程身份诊断成功，
不能把成功控制里的 winerror 1816 当失败原因。详见
[CI 跟进](2026-11-16-platform-ci-followup.md)，不移签为本候选 CI。
