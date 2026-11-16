# Windows 目录发布短期句柄与 Kit 0.10

## 触发与范围

基线为已推送的 `4d72e1e592bdd6194f6eadca3a6a760192b5fe82`，
本地、tracking、远端读回相等。其 Quality `43822369487 / attempt 1`
的 Windows job `133148170879` 实际 platform `63/4/0 / 1.03s`，
67 名逐项匹配，三项新游标回归全部通过。四项余下失败均为目录发布前
`reopen-directory-rename / PermissionDenied / raw_os_error=Some(5) /
nt_status=None`，Windows 原完整 gate `NOT_RUN`。
原日志与前批交付事实见[游标记录](2026-11-16-windows-directory-cursor.md)；
不将固定阶段与 OS 错误表示为 `ReOpenFile` 全部拒绝原因已证明。

本批先调整 `keelshell-platform` 的 Windows 同步 opener 和目录源短期
DELETE lease，新增句柄绑定回归。最终门禁前另集成独立作者准备的
Kit 0.10 依赖与应用语言方向候选，统一做组合验证。
审核、未知写入隔离、协议、凭据、长期目录 capability 与普通文件的
访问/分享规则保持。

## 候选机制

将普通 opener 的原访问与分享计算移到薄包装，原参数交给私有
`open_with_access`。目录发布通过实际保留目录句柄和空 NT 相对名，
请求 `FILE_GENERIC_READ | DELETE`、`FILE_SHARE_READ | FILE_SHARE_WRITE`、
`FILE_OPEN`；复用原 `NtCreateFile` 同步、reparse 拒绝、实际本地卷、
对象类型、所有权与确认状态检查。长期目录仍只读/share-all，
普通文件创建仍只分享 READ；短期 lease 不请求普通写权限。

重开前后完整 metadata 与私有 DACL 验证保持，不按路径重建来源，不设置
替换已有目标，不采用复制句柄或失败回退。native 失败继续返回原 typed
`io::Error`，test-only 阶段输出保留，正常应用无新增日志。
机制见 [ADR 0138](../../adr/0138-windows-handle-local-files.md)。

三项新增 Windows 回归分别覆盖取得 lease 前的路径改名替换、取得 lease 后
用同一短期句柄两次发布，以及私有目录/普通文件、四类长期句柄的实际权限和
lease 前/期间/释放后的分享行为。原 22 项 Windows 测试正文逐字节保留。

首轮 `scoped-gate-r1` 实际 `1/reaped/2.964s`：fmt、x.y 和本机 platform
`18/0/0` 已过，Windows cross-Clippy 因新测试五处 `expect_err` 触发既有
`expect_used = deny` 失败。父只将这五处错误取值改为显式
`err().ok_or_else(...)?`，原错误消息、kind/raw code 断言及原测试正文保持，
没有 lint allow、skip 或生产修订；作者的 r1 冻结和失败回执均保持。

最终 r2 完整非文档工程输入仍为 719 项，SHA-256：
`d0707bf3c8971f4d1116fe9b9fc39b1d153d3471639764db0cebe0feb645f4b2`。
`scoped-gate-r2` 实际 `0/reaped/2.890s`：fmt、x.y、本机 platform `18/0/0`、
Windows `x86_64-pc-windows-msvc` 全 target 严格 Clippy `-D warnings` 通过。
新非作者已对 r2 静态批准，P0–P3 均为 0，独立 Cargo、测试及 Windows 执行
均为 0。r2 报告保持原字节，其相同完整输入 gate 的待定条件不冒充被后续
组合输入满足；组合 r3 使用新的增量复核与自己的最终门禁。
真实 Windows 成功尚未取得。
本批活动目录为 `work/windows-directory-rename-integration-20261116/`。

## 依赖与后续验证

本轮官方实时查询实际 `exit 1 / reaped / 17.668s`，覆盖 37 包和 113 项
声明/使用；36 包符合最新要求，`gpui-kit` 已出现稳定 `0.10.0`，
当前声明 `0.9`、锁定 `0.9.1` 的三项检查失败。
这次失败是已确认的新版本差异，不是请求失败 UNKNOWN；
前批 `06:28:17Z` 的全部通过结果保持其原时点，不作为当前全部最新的证明。
独立作者在候选目录完成了迁移准备，没有运行 Cargo 或修改工程。

## Kit 0.10 组合输入

父随后集成 `Cargo.toml` 与 `i18n.rs` 候选，由 Cargo 实际解析 lock，
命令 `0/reaped/5.607s`。Kit、Base、Component、Macros 和 Assets 更新为
`0.10.0`；九个可选 Fast 包从 `0.1.15` 更新为 `0.1.16`。
直接要求为 x.y，不添加项目 Rust toolchain pin 或 `rust-version`。
新增语言入口同步布局方向，机制见 [ADR 0147](../../adr/0147-kit-locale-direction.md)。

独立作者静态核对 1,831 个 assets 原字节一致、现有 118 个主题颜色字段仍在、
默认 gpui-pre 与应用用到的组件/测试接口保持；这不是编译或原生结果。
父真实编译新 Input 键盘回归 `kit-locale-green-r1` 为 `1/0/0`，
command `0/reaped/58.383s`。仅删除方向同步一行后的真实重新编译
`kit-locale-red-r1` 为 `0/1/0`、`101/reaped/10.382s`，
实际得到 `abX` 而非 `aXb`。精确恢复全部工程字节并再次重新编译，
`kit-locale-green-r2` 为 `1/0/0`、`0/reaped/10.454s`。
原始编译另有 test executable 的 `__eh_frame` 超出 compact unwind 编码范围
链接器 warning，未把该非零 warning 数写成无警告或原生验收。

最终组合 r3 有五个工程改动文件，完整非文档输入仍为 719 项，SHA-256：
`83e4927b62b7e86592c485f9cb65e46d2d96f0f39a52adb9c2278efe018b4593`。
两个 Windows 文件与已审 r2 逐字节相等。真实编译后的 app 库存为 1,342 项，
新增方向例只有一名；新非作者基于五文件不可变快照做 r2→r3 增量复核。
该 r3 静态复核已批准，P0–P3 均为 0、独立执行 0；
相同输入最终成功门禁的条件未满足，首轮实际失败如下。

升级后的实时检查 `dependencies-live-r2` 实际 `1/reaped/51.877s`，
33 包核实符合最新要求，uuid、zip、winresource、windows-sys 四包请求失败
UNKNOWN，完整报告已形成。Kit 0.10 的三处直接声明/使用通过。
原 r1 的新版本差异失败保持，不把本轮 UNKNOWN 改写为全部最新。

格式、严格 Clippy、完整工程门禁、Windows 实际预检、八项 app 归档、
三平台原生桌面和发行均使用实际回执；计划或交叉检查不能代签。

## 组合门禁发现的渲染栈问题

`full-gate-r1` 绑定 r3，实际 `1/reaped/107.954s`；Python、格式和严格 Clippy
已过，workspace app 在
`ai_settings::request_options::draft_tests::duplicate_draft_edits_prune_models_cancel_and_discard_late_callback`
栈溢出中止。后续 target、rustdoc 和 controller 未全部执行，不能签完整 Rust 计数。
原日志 171,602 字节，SHA-256：
`39fe68b0f0b8d738ef1f3edb0c7532efae7a43af186350abb27b86d1bf02e98a`。
同一 workspace 二进制的精确重放也发生 SIGABRT，Python 包装退出映射为 250，
不改写为 Cargo 101。一次有界 LLDB 调查实际 controller 124、timed_out=true，
85.058s，直接及子进程全部回收；停止重试长调试会话。

三份本轮自有 crash report 的故障线程均显示 AnyElement 按钮分配、
`request_vault_controls`、`request_options_view`、`AiSettingsPanel::render` 链。
只保存故障线程与原报告 SHA，不公开系统标识或其他线程。
父将 profile list 和 form 构建拆成不内联的 `AnyElement` helper，保留 render
开头的 pending key 清理和 inference editor 装载，以及原节点、处理器和禁用条件。
重新用 workspace feature 组合编译，`kit-stack-fixed-r1` 实际 `1/0/0`，
`0/reaped/10.920s`；AI settings 全模块 `79/0/2 ignored`，
`0/reaped/6.453s`。r4 全输入 719 项 SHA-256：
`6ac314882cc707b7622a4b95082e3264b5e3e9359109d48cfcaf4b12c9f87755`。

`full-gate-r2` 绑定 r4，实际 `1/reaped/58.955s`；在
`files::browser_keyboard_tests::browser_keyboard_delete_repeats_only_review_and_cancel_keeps_remote_bytes`
再次栈溢出。原日志 195,309 字节，SHA-256：
`ddf78d2aee15ceab3b54ef5eb43baaf742a0910fad96a00930ae78d794cab739`。
该自有报告的故障线程指向 `persisted_recovery_card`、`parallel_queue_card` 与
`FilesPanel::render`，没有证据是无限递归。

r5 将文件内嵌编辑器、比较/镜像/逐项结果和文件工具整体构建拆为不内联 helper。
比较结果继续为原位置的多个 sibling，未加新的布局包裹；原渲染树、按钮顺序、
审核、焦点和操作回调保留。`kit-files-stack-fixed-r1` 重新编译原精确失败例，
实际 `1/0/0`，`0/reaped/10.860s`。两次修订均未提高线程栈、skip、放宽断言或
改变远端执行策略。既有完整测试作为真实行为回归，不添加只镜像函数拆分的测试。

当前七文件组合 r5、719 项完整工程输入 SHA-256：
`f2374487a954c6bc2be418461d8199e3c77ae1cf6d54f13b49103cf53ece4af4`。
真实 app 库存再次逐名列出 1,342 项，新非作者对 AI 与文件视图修订分别复核；
`full-gate-r3` 终态如下；前两轮失败和每份签署的原范围保持。

## 最新依赖与环境读回

`dependencies-live-r3` 实际 `0/reaped/17.158s`，官方检查 37 包、113 处声明/使用
全部通过；原新版本差异和请求 UNKNOWN 保留各自原件。没有为重复取得成功
而改变依赖判定。本机重新读取 Rust `1.100.0`、Cargo `1.100.0`，默认
`stable-aarch64-apple-darwin`；工程没有 toolchain pin 或 `rust-version`。
新 Kit 的测试二进制不代表新正常应用、包、安装或原生桌面已验收。

## 隧道管理栈帧与最终 r6

r5 的 `full-gate-r3` 实际 `1/reaped/389.376s`，在
`workspace::tunnel_rule_tests::review_regressions::conflict_blocks_new_workspace_panels_until_authoritative_reload`
栈溢出。原日志 314,727 字节，SHA-256：
`dee7588c5b8b667f9e4f5bc0ffa975d432122e95d4bf0caefaf5755f0641bda9`。
该次 Files 全模块实际 `344/0/0`，独立 reviewer 逐名核对库存相等，但 app 全部
执行被中止，不能补签其余计数。r4 AI 与 r5 Files 的新非作者静态范围各
P0–P3 全 0，组合交付各 HOLD，原报告及未满足的相同输入条件保持，均已关闭。

只保存本轮自有 crash 的故障线程与原 SHA；调用链显示规则编辑器按钮构建与
`TunnelsPanel::render`。r6 将完整普通 runtime 分支移到不内联的私有
`runtime_view`，先执行 review/editor 优先分支，正常显示时才调用该 helper。
原 runtime 子树原样保留，无新的布局层级、网络、文件写入或权限处理。
`kit-tunnels-stack-fixed-r1` 真实 workspace 重新编译原失败例实际 `1/0/0`，
`0/reaped/23.745s`；`kit-tunnels-workspace-r1` 相关工作区模块 `19/0/0`，
`0/reaped/3.739s`。保留正常线程栈和原断言。

当前 r6 有八个工程改动文件，完整 719 项 SHA-256：
`f0f64a6629b2441d63eef1e801189eaef69dd848aea0039eef4d09ff143b5164`。
前 r5 七文件全部精确保持；实际重新编译的 app 库存仍为 1,342 名。
新非作者隧道增量/组合静态复核独立记录，不改写此前各冻结的 HOLD。

## 最终完整门禁

`full-gate-r4` 绑定 r6 的 719 项输入，实际 `0/reaped/687.799s`，未超时或中断，
直接及所有子进程均已回收，管道排空。原日志 645,378 字节，SHA-256：
`386d76544c5e71390b2ea58d75e0390fec322b218dcd9d7e8bdfe9ebad6e8b96`。

- 普通 Rust：`3223 passed / 0 failed / 48 ignored`；rustdoc：`14/0/0`。
- app：`1340/0/2 ignored`，1,342 个执行名与 r6 实际编译库存逐项相等。
  新语言方向及三个原栈失败用例均逐名为 `ok`。
- session `server_archive`：`40/0/0`，全部 40 名与保全的原库存逐项相等。
- 本机 platform：`18/0/0`，两项原 typed-error/日志净化回归保持通过。
- Python：290 run、289 passed、1 skipped、0 failed。
- fmt、严格 workspace all-target Clippy `-D warnings`、直接依赖 x.y 政策和
  readiness、observer、small-stack 三组 AI controller 均通过。
- 完整工程路径集合与全部文件字节均相等，719 项冻结无漂移。

此前三次完整门禁失败各自保留，不能用本次 r6 成功补签它们。
Windows 预期 70 项与八项旧 app 归档失败的名称清单均只是待验收库存，
尚未作为本批 Windows 实际结果。正常 app/MCP debug 字节只作观察，
未称新 Kit 正常程序、原生包、桌面、安装或发行已通过。

## 最终复核与持久原件

新非作者对 r5→r6 隧道增量及保持原字节的 r2/r3/r4/r5 范围做新的 r6 组合
静态批准，P0/P1/P2/P3 均为 0。普通 runtime body 19,494 字节、管理/早返回
2,179 字节保持，逆向拼接可逐字节还原原文件；719 项输入、最终日志/回执和
逐名测试结果已由 reviewer 核对。独立 Cargo、Rust、GUI、Windows、容器及
共享 target 访问均为 0，不把父执行改写为独立执行。
旧输入的批准条件、r4/r5 组合 HOLD 和三次失败保持。所有本批 agents 已关闭。

[持久证据](2026-11-16-windows-directory-rename-evidence/README.md)保留 237 份成员，
其中 232 份原件、5 份派生输入映射/快照清单，未压缩 8,904,728 字节；
压缩包 1,763,368 字节，SHA-256：
`41701965edfeb7c3fa6a630a9085d5e83cf889f00ca0bb2728f6411ea347a0e0`。
前批 70 份已归档副本不重复，补存其后交付与三平台精确 CI；Kit 作者按 DELIVERY
保留当前 26 份文件。第三方 HTML/完整 crate payload 不重新分发。

封存预检首轮实际 exit2：验证器只识别 pending required-list，误拒最终 reviewer
明确已满足的条件对象。父增加相同冻结、原 gate 日志/回执/summary SHA 的核对，
未改源码或评审结论；第二轮预检、实际封存及 current 绑定均实际 0/reaped。
两轮预检原件保留于包内，工具字段适配不算工程测试失败或重新签署。
index、精确源码树绑定和后续推送使用各自真实交付回执。
