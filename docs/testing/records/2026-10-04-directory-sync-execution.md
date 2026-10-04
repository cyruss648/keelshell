# 双向目录内容校验与合并执行

日期：2026-10-04

## 实现范围

`files/sync.rs`、`files/worker.rs` 与文件面板接通从本地/SFTP 常规文件生成内容摘要、双向审核计划和显式确认后的目录合并。目标独有项保留，取消审核不写入；执行前重建整棵两侧快照，每项再次核对源和目标的类型、大小、SHA-256。文件使用同目录临时文件原子替换并读回摘要；目录按父到子顺序建立。

`SftpSession::inspect_entry` 与 `read_regular` 验证父链、命名目标和打开 handle；读取严格受字节界限约束，拒绝类型不明/符号链接/特殊文件和读取期间可观察到的元数据变化。资源清理复用既有 owned channel 机制。

## 已通过的行为回归

五项真实 GPUI + TCP SSH/SFTP 测试覆盖：

1. 同大小不同内容被识别，预览及取消审核保持远端原文件；确认后上传新文件和替换已有文件，同时保留远端独有项。
2. 审核后源文件或目标内容变化在第一项写入前拒绝，远端原始/外部编辑结果不被覆盖。
3. 下载已有文件与嵌套新目录，保留本地独有内容；480px 窗口中英文校验/审核按钮实际可见并可点击。
4. 缺失原子 rename 扩展失败关闭，原目标未截断，未开始临时文件写入。
5. 远端静态链接、大小写路径冲突和挂起后的旧审核拒绝，未发起原子写入。

普通单测覆盖本地 64 MiB 限制、取消读取、静态叶/祖先链接拒绝、256 MiB 双侧/整数溢出边界及 Windows drive/device/stream/上标数字保留名。SFTP 单测覆盖绝对规范路径、明确类型/大小和变化检测。

## 失败与修复

首次 GPUI 回归发现 `compare-directories` 在1100px窗口不可见。原因是此前视图链把传输工具栏意外嵌在文件修改工具栏的末尾，导致整个传输栏被移到右边。已将两栏恢复为独立纵向行；五项回归通过，并核对480px新同步动作。

新增普通测试首次误导入 GPUI `test` 宏造成递归扩展，已改为窄导入；严格 Clippy 首次指出测试中的 unwrap/expect，已采用带上下文的测试断言处理。这些为开发中间失败，不表示最终门禁失败。

## 集成门禁与边界

本轮五项专项全部通过；随后增加了“收起比较”撤销待审核与按新建目录/新建文件/替换文件标注审核动作的回归，再次通过五项专项。最终整仓门禁、提交与三平台 CI 结果见下方追加记录。

本记录使用有界、隔离的内存文件系统协议夹具，无生产主机。GPUI 测试证明实际控件事件和字节结果；macOS 原生和独立 OpenSSH 的额外证据见下方，其范围仍不等同 Windows/Linux 原生窗口或生产主机验收。整个目录非事务；失败/取消可能保留已完成项。SFTP v3 与本地路径的外部并发替换、硬墙 filesystem 超时、目标平台名称规范化、权限/ACL/掉电持久性不在已证明范围。详见 [ADR 0032](../../adr/0032-reviewed-directory-merge-execution.md)。


## macOS 原生与独立 OpenSSH 验证

使用隔离数据目录、临时文件树和仅回显命令的回环 SSH/SFTP 服务，在真实 macOS GPUI 窗口完成内容预览、取消审核、再次审核并确认上传。取消前后远端原目标及独有文件保持不变；确认后内容摘要与本地源一致，目标独有项保留。随后重新比较并生成反向计划，在保留计划的情况下切换英文，通过固定审核按钮确认下载；远端全部8个常规文件与本地对应文件的 SHA-256 一致，包含嵌套路径和带引号的文件名。

这一轮已操作构建的 SHA-256 为 `1505e20aa617a08471c6b1b776deaef083877e6d4f9894de286574f19655a297`；证据保留在 ignored `work/sync-native-20261004/native-checks.json`、控制器回执和日志。该构建早于最后“收起比较”/动作类别标签及连接布局改动，这些新增行为由后续 GPUI 回归验证；最终重建的 UI 独立复审记录须单独引用。控制器显式终止本轮应用（exit -15），SSH fixture 正常退出（exit 0），已核对自有进程退出、端口关闭和远端临时根删除。没有覆盖已安装应用，也没有连接生产主机。

`python3 scripts/openssh_interop.py --output work/openssh-20261004-checked-content --timeout 300` 在13.561秒内通过全部7项真实系统 OpenSSH 互操作。新增读取测试验证常规/空文件、明确类型与大小、完整内容、低一字节的严格限额失败、目录读取拒绝和缺失叶返回；其余既有 exec、补全和传输回归一并通过。回执确认自有进程退出及临时密钥目录删除。它证明新读取接口与 OpenSSH 协议互通；完整目录合并仍是在受控 SFTP fixture 上验证。


## Final integrated local gate

`python3 scripts/check.py` passed on the integrated frozen source: dependency version policy, formatting, strict workspace/all-targets Clippy, and 853 Rust unit/integration/documentation tests (848 ordinary tests plus 5 doctests), with zero failures. The seven opt-in OpenSSH tests were ignored in the ordinary gate and executed separately; all seven passed. Packaging regression tests passed 47/47. The gate log is preserved at ignored `work/gate-20261004-integrated-final.log`; earlier failed logs remain preserved. These results do not certify Windows/Linux native GUI interaction or paid AI providers. Remote CI for the new commits is recorded separately.
