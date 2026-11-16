# Windows 文件系统原生阶段诊断

## 范围与当前结论

本批基线为 `64ce892428ad67ea70c1377c21f6d361db36497d`，包含四个工程文件：
平台库模块、私有诊断模块、Windows native 调用点及 Windows-only CI 预检。
该改动提供失败阶段，不更改文件访问、审核、发布或隔离语义。
本机没有实际执行 Windows 程序。推送后的真实 Windows 预检已定位共同失败到
目录游标的 `NtCreateFile`；归档 app 的恢复仍须修复后单独验证，见下节。

完整门禁 `full-gate-r1` 实际为 `exit 0 / reaped / 750.798s`：
Rust `3222 passed / 0 failed / 48 ignored`，rustdoc `14/0`，
Python `290 run / 289 passed / 1 skipped`。格式、严格 workspace Clippy、
x.y 政策及三组 AI controller 均通过。app 为 `1339/0/2 ignored`，
1,341 个实际执行/忽略名与上批编译库存逐项一致；session `server_archive`
的 40 个名逐项通过。platform 本机为 `18/0/0`，包含两项新诊断行为测试。

719 项完整非文档工程输入的路径集合和字节均无漂移，集合 SHA-256 为：

```text
774b46416b9c1ef424d89d7d3cb49799563d1c00237d1e52bcd4598677dd8f29
```

## 真实旧 CI

源码 `76b8253acd9bfa62180563f482e851064bc1ea34` 的 Quality
`43806549649 / attempt 1` 已 `completed/failure`：

| 平台 | Job | app 实际结果 | Job 终态 |
| --- | --- | --- | --- |
| Windows | `133098539513` | `1320/8/2 ignored`，1167.79s | failure |
| macOS | `133098539273` | `1339/0/2 ignored`，686.71s | success |
| Ubuntu | `133098539119` | `1340/0/2 ignored`，767.51s | success |

八项 Windows 失败均在审核准备返回 `io/other-redacted`，
显示后缀为 `Some(123)`，`atomic_writes_started=0`，控件存在且未被禁用。
该显示后缀不能证明原始 typed OS error、NTSTATUS 或具体 syscall。
五项上传和三项解包没有到达原有正文安全断言；没有删除或放宽它们。

完整 640 份原件、三平台原始日志和调查报告已无损保存到
[旧 CI 证据](2026-11-16-windows-archive-ci-evidence/README.md)。
当前文件校验全部通过，18 份来源文件与精确源码 Git blob 一致。
完成调查的 agent 已关闭，重复临时目录在逐项比较后回收。

另一个精确源码 `bc9ebd6cf4f213b9330c04380f866ead9133fb0a` 的 Quality
`43809777642 / attempt 1` 也已 `completed/failure`：
macOS job `133108602697`、Ubuntu job `133108603084` 成功，
Windows job `133108603091` 失败。三平台原始日志另存本批原件，
不能将前一个 run 的计数移签至此 run。

## 实现与验证

`native_diagnostics` 使用固定操作枚举，仅在平台库的单元测试构建中输出
操作标签、`io::ErrorKind`、`raw_os_error` 和实际取得的 NTSTATUS。
正常 library 构建不输出这些诊断。它原样返回传入的 `io::Error`，
保留 kind、原始系统码及自定义 payload；不包装错误，也不格式化其
Display、Debug 或 source，因此不记录路径、SID、peer 内容或凭据。

NT 调用仍使用原有 NT→DOS 转换，Win32 错误在失败分支立即读取。
FFI 参数、访问权限、对象和句柄生命周期、目录枚举及发布行为均保持原语义。
`entries()` 的 `open_child(parent, ".", true)` 没有修改；该调用仍只是
后续需真实 Windows 阶段证据检验的调查对象。

| 编号 | 命令/范围 | 实际结果 |
| --- | --- | --- |
| `native-diagnostics-focused-r1` | 平台诊断两项 host 测试 | `2/0/0`，exit 0 |
| `windows-cross-clippy-r1` | platform 全 target，`x86_64-pc-windows-msvc`，`-D warnings` | exit 0 |
| `full-gate-r1` | `python3 scripts/check.py` | exit 0、已回收，完整计数见上 |

第一项使用不可格式化的错误 payload 验证无泄露和 payload 保持；
第二项验证 typed OS error 保持。数字 `4321` 和 `0xe1234567`
是明确 synthetic 数据，不能作为 Windows 故障码或根因。
Windows 交叉 Clippy 不是 Windows 原生执行。

CI 只在原完整 Rust quality gate 前加入 Windows-only、单线程的 platform lib
预检。预检失败则 job 失败，后续完整 gate 为 `NOT_RUN`；
预检成功后照常执行原 gate。macOS/Linux 前置保持，原完整 gate 的定义和
命令保留；没有 continue-on-error、自动重试、扩时或削减断言。
原作者 r1 的强制继续条件未采用，原候选和报告逐字节保留，父修订 r2 另记。

## 工具链、依赖与边界

本机实际 Rust/Cargo 为 `1.100.0`，活动工具链是环境默认 stable。
项目没有工具链 pin 或 `rust-version`；本批未改变 manifest 或 Cargo.lock。
实时官方 registry 检查覆盖 113 项声明/使用、37 个包，19 包最新版本已核实，
18 包元数据请求失败为 `UNKNOWN`，整体 exit 1；不称全部依赖已确认最新。
x.y 格式政策已独立通过。

本机未新增正常应用构建、打包、安装或桌面操作。
门禁后观察到的 GUI/MCP 字节分别为
`af2a88364650b90b5dfab4ca8a4655a8430febc4971f575c7a7fd0fde19eb9a3`、
`3e9270c84f3aad14f331b06cb3f15e77f2d2a40602dcd61e90dd39b5b939ed4b`，
只作为现存字节观察，不能签署当前 Windows 修复、三平台原生 GUI、
IME/辅助技术、严格 Zmodem peer、完整产品或发行验收。

## 独立复核与交付

新非作者独立静态复核为 APPROVE，P0–P3 均 0；22/22 项静态检查通过，
起止各核对 719 项输入无漂移。reviewer 的 Cargo、测试二进制及 UI 执行均为 0。
原报告不改写；后续补充件读回父完整门禁的终态和日志摘要，并区分父回执与
独立执行。补充件同时说明官方文档抓取无有效正文，不能算在线核验。
reviewer 已关闭。原报告 SHA-256 为
`5d62a526cae184859b12b66d5083d40d6cc5fcba3aee82b7895dd48da7d4fc84`。

本批原件和冻结的完整绑定入口为
[诊断证据](2026-11-16-windows-native-diagnostics-evidence/README.md)。
源码及首批证据已在 `878f13b4cb58a45dfe96f32fc1c389ac7799366d` 提交推送到
`feature/editor-unicode-codec-20261114`，本地、tracking、实际远端 SHA 读回一致，
719 项工程 blob 与精确提交绑定零 mismatch。
旧临时 CI 已终态，无需取消；自有临时分支
`feature/archive-deadline-clock-20261116` 已删除，本地、tracking、远端均 absent。
前批 11 份交付回执、23,386 字节已在精确源码提交中归档后回收。

## 推送后的真实 Windows 预检

精确源码 `878f13b` 的 Quality `43815115588 / attempt 1` 中，
Windows job `133125296042` 于 `2026-11-16T06:14:52Z` 完成并失败。
platform lib 为 `48 passed / 16 failed / 0 ignored / 0.50s`；
后续完整 Rust quality gate 为 `skipped / NOT_RUN`，不能填为成功。

16 项失败的捕获正文均包含成对原生诊断：

```text
native-files operation=nt-create-file kind=InvalidFilename raw_os_error=Some(123) nt_status=0xc0000033
native-files operation=open-directory-cursor kind=InvalidFilename raw_os_error=Some(123) nt_status=None
```

前者来自实际失败的 NT 调用，后者是同一错误通过外层游标阶段的观察，
不是两个独立故障。该证据把共同平台失败点定位到独立目录枚举游标打开；
对应候选源码仍是 `open_child(parent, ".", true)`。修复候选另行开发，
本记录不把定位提升为 Windows 归档已修复，八项 app 回归在本 run 为 NOT_RUN。
macOS、Ubuntu 此时仍运行完整 gate，后续终态另记。

原日志 72,320 字节，SHA-256 为
`955d86d6b266c69108844757391dd8766e89837be5ebea936b66ed3da07ccf6b`，
含精确源码身份。首个日志获取因 gh 的转义序列输出保护退出 1；
明确允许原样保存后获取成功，未重跑 job。
派生摘要首版因贪婪正则误取一组而失败，错误保留，修订版逐项核对 16 组。
两项数据获取/摘要失败不覆盖原始日志，也不更改 CI 失败事实。
