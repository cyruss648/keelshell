# 编辑器键盘与原生文本流程 — 2026-11-16

基线为本地 `92c8ddaab84d19a1044a5803d73a0495d0b89354`。
本批按集中实现、专项修复、非作者复核、最终完整门禁和本地提交推进，不 push。
当前状态为 `FINAL_GATE_PASS / SOURCE_APPROVED / NATIVE_SCOPED_APPROVED`。
完整工程门禁与限定原生片段分别签署，不扩大为完整桌面验收。

父负责隔离 macOS 原生文件/编码流程与集成；独立作者在受管 worktree 实现
限定焦点的编辑器保存审核与查找快捷键，不运行 Cargo。唯一 target 仍为
`work/recovery-batch-integration-20261115/target`，归属未切换。

保存快捷键只创建原有审核，重复按键不能确认。查找快捷键使搜索框可见并聚焦。
终端、其他输入、旧连接、隐藏输入、忙碌和待审状态不能获得新的写入权限。
三主题继续共用 registry、Palette 和 Kit 组件。

原生流程使用暂存应用包、独立 `KEELSHELL_DATA_DIR` 与 disposable localhost
OpenSSH 服务，不覆盖安装、不使用用户主机。实际检查 UTF-16/GBK 的打开、
键盘输入、审核取消和确认、完整字节与备份读回；范围限定为下述明确接续片段。
Windows/Linux 桌面、IME、完整辅助技术、发行和完整产品目标保持开放。

## 实际集成与复核

新增 9 个 `files::editor_keyboard_tests::*` 用例与 2 个
`files::transfer_tests::conflict_merge_tests::editor_format_tests::*review_uses_final_characters*`
用例，均在实际 1289 项 app 库存中逐名核对。键盘专项 `9/0/0`、编码红绿最终
`2/0/0`、文件模块集中专项 `300/0/0` 通过；正常 arm64 GUI/MCP 构建和暂存包检查
通过。没有切换唯一 Cargo target 的来源。

首次编码红例也暴露测试对无换行基线的 CRLF 插入策略预期错误；修正 fixture 后，
第二轮有效红例分别证明 LE 漏报、BE 将非换行字符误报 LF。生产修复只用已无损
回解的最终正文判断字符，完整出站字节、内容、保存和备份合同保持。
键盘编译首轮宏导入递归、第二轮测试 helper lifetime，以及首次旧帧场景失败均
保留。旧帧夹具需先进入键盘 modality，否则 GPUI 在首次 KeyDown 前重绘，测试
没有使用旧监听器。修后保留全部安全断言，最终 9 项通过。

新的非作者对 715 项 r2 输入核对前后无漂移，独立预构建执行键盘 9、编码 2 项
全通过，没有独立重新编译。其 P2 notice 已关闭，开放 P0–P3 均为 0，限定批准
保留签署时完整门禁待定条件；父另以最终成功回执和无漂移的 Rust 输入满足该条件，
不修改原报告。作者与该 reviewer 已关闭，作者受管 worktree 已归档，
实际路径和 Git worktree 记录 absent。Rust 独立执行是父预构建程序，没有独立重编译。

## 完整门禁与历史 fixture 修复

首轮 `full-gate-r1` 实际 `exit 1 / reaped / 11.221s`。工程 Python
`284 run / 1 failure / 1 skipped`，后续格式、Clippy 与 Rust 阶段未执行。
唯一失败是零库存真实子进程用例写死历史 source SHA，先被生产 source guard 拒绝。
只在该测试真正读取当前 HEAD，仍经过生产二次来源校验，并补上 cleanup 进程
列表非空；生产 runner 未改，原五个断言保持。

父专项 `23/23` 与新非作者独立 `23/23` 通过；其实际零库存子进程 stdout、
wait/reap 与 resources=0 已封存，窄审批准、开放 P0–P3 均为 0，reviewer 已关闭。
相对 r2 冻结只变这一 Python 文件，其余 714 项保持。
最终 r2 为 715 项，集合 SHA-256
`a17d4a12b9187755a59af96f99afa5f0998498b0de1a980de0bb9c13f2916c3c`；
新完整门禁 r2 实际 `exit 0 / reaped / 698.706619833s / timed_out=false`。
普通 Rust `3168 passed / 0 failed / 48 ignored`、rustdoc `14/0/0`；
app 为 `1287/0/2 ignored`。工程 Python `284 run / 283 passed / 1 skipped`，
session helper Python `25/25`。格式、严格 Clippy、x.y 依赖政策和三组 AI controller
均通过，11 个新增默认 Rust 名逐项匹配。`post-gate-inputs-r2.json` 重新枚举完整
715 项路径及摘要，与最终冻结完全相等；没有因文档更新而重跑不变工程输入。

## 实际 macOS 与保留边界

`candidate-frost-r2` 在隔离正常包中，经明确接续操作完成 UTF-16 LE 和 GBK：
真实正文/Enter、Cmd+F 聚焦、Cmd+S 展开完整审核、重复不确认、取消保持原字节，
以及各一次明确确认后的完整目标和唯一原始备份读回。UTF-16 LE 最终
`44 bytes / d00d6cecdd4be69db66ef0cd4022f46bebcb39c512503c3637e4a11f71b6f90a`，
GBK 最终
`21 bytes / fb1032d55ab1c8a8faaffd21f01ade30000b91353d46df8e7eb8430665efa56a`。
审核显示真实末尾 LF，GBK 中文与英文审核均留有实际 PNG/AX。

不能把早期脚本改签成功：观察器曾忽略同时存在的 AXValue；浮层需实际指针打开；
编码选项的真实角色为 AXCheckBox；首个合成组合键和紧接 AX 的事件有未被观察的
情况，原因未全部确认。各次失败、旧 observer 和单次显式接续原件保持。
`candidate-graphite-r1` 没有观察到合成清空后的空草稿，未到达新的快捷键，
保持 `HOLD`；其三个种子原件未变、备份为 0。本批 Lavender 原生执行为 0。
不以 GPUI 三主题回归补签这些桌面边界。

四个自有会话均已停止，12 个受管根进程清理回执为真；181 条 Darwin birth
身份再次读回无活匹配，八次端口关闭，四个私有目录 absent。没有创建容器或用户
chat，不操作其他任务资源。新的原生非作者实际解析 41 份 AX、直接查看 24 张
PNG、核对 45 份事件回执，独立重建两份读回的原始/最终完整字节和摘要，批准
Frost 已观察片段；本轮没有重新运行原生动作。其 276 份保护原件零漂移，715 项
冻结相等。完整脚本、完整 native finish、IME、辅助技术、Windows/Linux 和完整
产品不在批准范围。保存状态在两张 PNG 可见，但 AX 未含对应文本；UTF-16 补发
保存没有单列 event，均保留限制。报告与配套 JSON 保持原样，reviewer 已关闭。
四个本批作者/复核 agent 均已释放，作者 worktree 未创建独立 feature 分支。

正常 app 为 `198,960,496 bytes`，SHA-256
`ec0a4c3d538a9997c528495207d966bc58ebeb4633baebe61b3bb97b64dcbf1e`；
MCP 为 `14,548,464 bytes`，SHA-256
`0e5efdcbe2069fb4ea3c809a99419c557d11ba0ba18e7ba8e5fc6ea0e9bde816`。
暂存包共 6 个文件逐项核对；Mach-O minos 为 11.0，Info.plist 最低系统为 15.0，
分别记录，不推断旧系统兼容。没有签名、安装、发布或可复现构建证明。

## 封存、提交与清理

无损原件及三个未修改 PNG 从
[持久证据入口](2026-11-16-editor-keyboard-native-evidence/README.md)读取。
278 份原件共 `42,326,696` 字节已逐字节 gzip 封存并验证解压摘要；15 个派生包、
编译 helper 和缓存文件另列原路径、完整字节数与摘要，不重复保存在 Git。
最终门禁、三份原报告、失败与 HOLD 都保留。
本地源码提交为 `546bc8577e53b61bc50ebabea2e26d5057af834f`，没有 push。
[index 绑定](2026-11-16-editor-keyboard-native-evidence/index-input-binding.json)、
[源码提交绑定](2026-11-16-editor-keyboard-native-evidence/commit-input-binding.json)及
[清理后绑定](2026-11-16-editor-keyboard-native-evidence/post-cleanup-commit-binding.json)
核对 715 项完整工程路径/字节、278 份原件及三个预览与精确 Git blob，零 mismatch。
[清理回执](2026-11-16-editor-keyboard-native-evidence/cleanup-final.json)记录回收本批
293 个临时文件、逻辑 `471,461,892` 字节；删除前每项原件及 15 份派生文件均与
封存摘要匹配。正常 app/MCP 在删除后重新计算摘要一致，唯一有效 target、其他任务
资源保留。四个 agent 已释放，作者受管 worktree 和临时目录均 absent。
后续仅补文档与回执，不重跑工程输入未变的门禁。完整产品目标保持 active。

Rust/Cargo 实测 `1.100.0`，默认 stable；无项目 toolchain pin、rust-version
或新依赖。官方最新依赖检查在 45 秒超时，SIGTERM 后实际 reap；
不能声称全部依赖已确认最新。只本地提交，不 push。
