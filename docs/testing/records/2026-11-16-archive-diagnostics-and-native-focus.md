# 归档准备诊断与命令框原生焦点补验 — 2026-11-16

基线 `f4299487c132180e484008326c5b52682b3a753f`。
本批唯一工程变更为 `crates/keelshell-app/src/files/archive_tests.rs`，
没有修改生产归档实现。Windows 八项旧失败的根因仍为 **UNKNOWN**；
本批目的是让下一次真实运行给出可定位、脱敏的失败状态。

## 诊断实现与复核

从已封存的诊断草稿恢复补丁后，把 pending 分类缩为 None、PlanArchive、
TransferArchive、Other，避免与无关操作耦合。缺审断言继续失败，
`track_caller` 指向具体测试调用处。失败记录只有固定状态分类、布尔值、
计数、按钮状态和受限 `u32` display 错误数字，不输出路径、peer text、
审核正文、凭据或操作 payload。display 数字不等于保留的原生 syscall 错误。

原十项测试正文、完整审核、独立确认、无覆盖和 UNKNOWN 断言字节保持。
新增两项测试覆盖九组准确分类/脱敏输出。没有 skip、重试或延长等待。
新非作者源码复核 **APPROVE，P0/P1/P2/P3 全 0**；独立直接执行父预构建
程序的归档专项 `12/0/0`，未独立重新编译或执行 Windows。

## 本机统一门禁

718 项完整工程输入冻结 SHA-256：
`e967a7619be613cca5c3fc103f4a6d3c8ad2ad746caacc32386cf169820dd1ff`。
最终 `python3 scripts/check.py` 实际 **exit 0 / reaped / 704.319s**：

| 项目 | 实际结果 |
| --- | --- |
| 普通 Rust | 3,220 passed / 0 failed / 48 ignored |
| rustdoc | 14 passed / 0 failed |
| 工程 Python | 290 run / 289 passed / 1 skipped |
| app 库存 | 1,341 个具体名，与门禁执行逐名相同 |
| 新增默认用例 | 两项具体名均在库存及成功执行中 |
| fmt / 严格 Clippy / x.y / 三组 controller | 全通过 |
| 完整工程输入 | 路径集合、字节均零漂移 |

归档专项实际 `12/0/0`。首次命令误加 `--lib`，Cargo 返回 101：
app 没有 library target，未执行测试；修正为 binary target 后通过。
格式预检首次失败，运行格式整理后最终完整门禁通过；原失败日志保留。
这些失败不属于 Rust 行为回归失败。

## 原生焦点补验

通过 CUA 操作隔离 macOS arm64 窗口及自有 localhost OpenSSH/SFTP。
先与独立生成的服务器公钥指纹核对，再显式接受和认证。
暂存包复用上批正常 GUI/MCP 二进制，字节摘要保持，未重新编译正常程序；
本批只改 cfg(test) 模块。包 manifest 中当前 HEAD/dirty 为暂存时观察，
不能当作正常二进制的新编译身份。打包与原生静态检查均 exit 0。

Frost 浅色中文界面，两侧各含一个 `child.txt`，每个方向先真实点击命令框：

| 方向 | 远程列表根获得可见焦点框 | 本地列表根获得可见焦点框 |
| --- | --- | --- |
| Shift+Tab | 第 26 步 | 第 32 步 |
| Tab | 第 35 步 | 第 29 步 |

完整 67 步逐项保留全部 149 字节草稿。另有初始 38 项列表的 100 步反向观察，
只到远程列表根，不计为完整双列表场景。167 份导航 AX 原件均用完整值
精确字节相等复核，不依赖执行时的 suffix 检查。四张原始 PNG 显示对应列表
的 accent 焦点框；CUA AX focus summary 一直报告窗口根，因此不代签
AX focused-child。初始 CUA typeText 漏掉中文，随后 AX setValue 装入准确草稿，
不是实际 IME/中文键盘输入验收。本地目录通过显式刷新加载，不签 Enter 加载。

最终执行标记 absent；36 份 seed、welcome 和 nested child 字节保持。
这不代表 raw PTY 零出站审计。三根进程 stop/reap、两个端口关闭、
临时私有目录 absent；JSON birth 身份归一化及活进程往返正控确认
76 项累计身份零活匹配。其他主题、尺寸、平台、VoiceOver 和完整产品仍开放。
新的非作者原生证据复核 **APPROVE，P0/P1/P2/P3 全 0**，亲自查看四张
列表根与两张起始 PNG，独立核对 167 份完整 AX 值；执行测试和 UI 重放均为 0。
该 reviewer 与源码 reviewer 均已关闭。

## 工具链、依赖与精确 CI

本机实际 `rustc --version`、`cargo --version` 均为 **1.100.0**。
项目没有 toolchain pin 或 `rust-version`，使用环境提供的默认 stable；
没有新增依赖或改动锁文件。37 个直接 registry 包、38 处显式要求均为 x.y；
16 包实时确认与官方最新稳定版一致，21 包和官方 Rust stable 元数据因超时
保持 UNKNOWN，不能称全部最新版已核实。并行核对员已关闭。

上一源码 `9d0f886acda4e48058004a28f85ef1fa3c510d4c` 的 Quality
`43801985537 / attempt 1`：Ubuntu job `133084142104` success；
macOS job `133084142077` failure，`server_archive` target `39 passed / 1 failed`，
失败名为 `server_archive_dispatch_and_readback_share_one_fixed_execution_deadline`，
仅有 `Error: Elapsed(())`，不能确定具体失败 await。Windows job
`133084141812` 在 04:41:45 UTC 终态 failure，app
`1318 passed / 8 failed / 2 ignored / 1131.09s`；仍为同八项完整审核准备失败，
旧 helper 没有底层错误码。run 现为 completed/failure。原始与去 ANSI 日志保留，
没有重跑或取消该 run，没有把本批本机成功移签给远端。
macOS 测试调查另作下一片，不改写本批冻结和成功回执。

本批持久原件、复核与交付状态从
[证据入口](2026-11-16-archive-diagnostics-and-native-focus-evidence/README.md)读取。
