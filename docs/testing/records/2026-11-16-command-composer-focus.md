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

## 交付与接续

源码、文档及 205 份无损原件已在
`9d0f886acda4e48058004a28f85ef1fa3c510d4c` 提交并推送当前功能分支，
本地、tracking 与远端 SHA 实际相等。首次 push 返回 HTTP 408，随后只读查询
确认远端仍为旧 `892aada`；使用单命令 HTTP/1.1 与有界 postBuffer 的普通
推送成功，`exit 0 / reaped / 10.287s`，没有修改仓库 HTTP 配置或强推。
首次以 `HEAD` 调 verifier 被精确 SHA 参数校验拒绝，未生成验证输出；
改用完整 40 位 SHA 后源码提交绑定成功，没有放宽 verifier。

源码与实际原件在 Git index、精确源码提交均零 mismatch。新非作者原生证据
复核仅批准上述限定片段和 r2 清理，开放 P0–P3 为 0，亲自查看两张 PNG，
独立执行测试 0；最终 99 项评审输入集合 SHA-256 为
`47bfda9a865b1aed46cfd73a514cc99cf825da9f6ebd655d5a702c7216bcba9f`。
源码 reviewer、原生 reviewer 与旧 CI observer 都已关闭，未创建用户 chat。

字节核对后回收 213 个临时文件、逻辑 `230,504,949` 字节；
原 `work/composer-focus-integration-20261116` 已 absent。正常 GUI/MCP、
唯一有效 target、其他任务 worktree/分支保持。旧 CI 工作目录的完整 84 文件
副本已在本目录原件中封存；原目录因后续 Windows 归档调查仍需读取而暂留，
调查结束再回收。交付命令原件以 `delivery-source-*` 单列，不伪装成源码
提交前已存在的回执。

精确新源码 Quality `43801985537 / attempt 1` 已实际启动，首个快照为
in_progress，未取消旧 run。活动跟进目录为 `work/command-focus-ci-20261116/`；
没有提前签署新 CI、反向 Tab 全路径或完整产品。

后续文档 `7df6020c0bd302cd16197120e906c3bce627d44e` 已推送并三方匹配；
精确源码/文档两树再次绑定 718 输入和 205 原件，零 mismatch。
docs push 查询为 0 个 workflow run，源码 CI 的三个 job 首次读回仍 in_progress。
这些快照按精确 SHA 分开保存，不代签当前或未来终态。

Windows 调查作者已完成报告、诊断候选和静态文本检查后关闭，根因保持 UNKNOWN。
其[草稿原件](2026-11-16-windows-archive-diagnosis-draft/README.md)不计入本批
编译或测试；尚未集成。随后确认不再依赖旧目录，回收旧 CI 84 个重复文件、
`1,265,813` 字节；25 份草稿原件核对 gzip 与 Git index 后也回收临时副本、
`550,312` 字节。三个临时目录现在均 absent，共回收 322 个文件、逻辑
`232,321,074` 字节。所有本批 agent 已关闭，没有新建用户 chat/worktree，
活动新 CI 目录仍保留。
