# 2026-11-15 审核式客户端归档传输

状态：`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`，仅适用于本批冻结的
客户端归档源码与工程验证。原生与平台范围见下文。
日期采用 Asia/Shanghai；机器回执保留 UTC。
本轮仅本地提交，不 push。

## 范围与源码

以 `dfbc95904ec841fae64701181de4802959c5fb34` 为基线，新增客户端 ZIP32/tar.gz
编解码、本地多选打包上传、远端归档下载解包、完整成员审核及失效绑定。
实现按 core、session、app 的独立写集并行；测试代码随实现编写，
完整验证集中在集成后。core 的隔离 worktree 已合入并受管归档，
两个实现 agent 已关闭，当前主树为唯一 Cargo owner。

最终冻结为证据目录的 `frozen-inputs-r3.json`，包含 532 个 Rust/Cargo 文件及
10 个 gate 脚本，SHA-256 为
`2b768eaeeb9a57498cdcc881ce142cd778227711aaed405d70c82b15082c71c2`。
作者完整门禁结束后、正常程序构建后均逐项核对零 mismatch。独立 reviewer
在自己的执行前后也核对同一输入。r1/r2 冻结和全部失败原件保留。
设计见 [ADR 0132](../../adr/0132-reviewed-client-archive-transfer.md)，
用户流程见[归档指南](../../product/ARCHIVE_TRANSFERS.md)。

## 最终集中门禁

主线程在同一有效 target 上执行并实际等待回收：

```sh
CARGO_TARGET_DIR=work/recovery-batch-integration-20261115/target \
CARGO_BUILD_JOBS=4 CARGO_NET_OFFLINE=true python3 -B scripts/check.py
```

`full-gate-r3` 退出 `0`，耗时 `588.762s`。完整日志解压后的 SHA-256 为
`7f030127fd915aefa976e0df801a61df8e2e40aa8187d94310b63add4c526731`。
逐 suite 汇总和具体通过名称保存在
[validation-summary-r3.json](2026-11-15-reviewed-archive-transfer-evidence/validation-summary-r3.json)。

| 最终检查 | 实际结果 |
| --- | --- |
| app 普通 Rust | 1193 passed / 0 failed / 2 ignored |
| core 普通 Rust | 684 passed / 0 failed / 0 ignored |
| AI 普通 Rust | 194 passed / 0 failed / 5 ignored |
| session 普通 Rust | 453 passed / 0 failed / 18 ignored |
| MCP 普通 Rust | 77 passed / 0 failed / 0 ignored |
| 普通 Rust 合计 | 2601 passed / 0 failed / 25 ignored |
| workspace rustdoc | 14 passed / 0 failed |
| Python | 131 run：130 passed / 1 skipped |
| 依赖 `x.y` 策略、fmt、workspace all-targets strict Clippy | 全部通过 |
| readiness / observer / small-stack AI controllers | 三组分别通过，不重复计入普通 Rust |

app 实际清单为 1195 项，包含全部 9 个归档回归；9 个具体名均在完整门禁中
实际为 `ok`。core 归档集成 `40/40`，session 归档集成 `18/18`，后者包含
12 项归档场景和 6 项复用协议夹具；另有 no-replace 原语单元回归。
rustdoc、独立定向复跑和单独执行的 ignored OpenSSH 用例不重复累计。

本机 Rust/Cargo 实际为 `1.100.0`。项目不固定 toolchain，也没有将本机版本写成
`rust-version` / MSRV。直接 registry 要求继续为 `x.y`，Cargo.lock 保留 patch。
已知链接 warning `__eh_frame section too large` 没有改变任何成功退出码。

## 开发过程与保留的失败

| 检查 | 实际结果 |
| --- | --- |
| core 归档测试 r1 | 40 passed / 0 failed，16.447s；含往返、边界与恶意输入 |
| app 首次编译 | 因字体辅助 trait 不在作用域失败，已改为既有 FontWeight API |
| app 归档 r1 | 6 passed / 2 failed；Kit 程序赋值不发 Change 及测试未刷新画面，驱动已修正 |
| app 归档 r2 | 测试导入了同名 InputEvent trait，已改为明确的组件事件路径 |
| app 归档 r3 | 8 passed / 0 failed，12.655s |
| session 归档 r1 | 16 passed / 0 failed，18.440s；10 项归档场景及 6 项复用协议夹具回归 |
| 完整 gate r1 | 13.081s 后于严格 Clippy 失败；3 项测试代码 lint 已修复 |
| 完整 gate r2 | exit 0，631.394s；普通 2600/0/25 ignored，rustdoc 14/0；其两项 P2 仍需修复，不作为最终候选验收 |
| 生命周期回归：修复前 | 5 passed / 4 failed，14.824s；真实路径复现关闭保护与旧隔离目标丢失 |
| 生命周期回归：修复后 | 9 passed / 0 failed，14.061s |
| 最终完整 gate r3 | exit 0，588.762s；结果和冻结绑定见上表 |
| 官方在线依赖检查 | 75.015s 有界期限超时，exit -15，无完整版本回执；不能确认所有依赖最新或代签漏洞审计 |

app 检查实际走 GPUI 窗口/context 和受控 SSH/SFTP，覆盖完整成员、二进制、
空目录、128 项审核、取消、源变化、目标竞争、草稿/选择/会话失效、
三主题明暗与双语。它们是原生组件控制器检查，不是桌面人工验收。
session 后续收口还加入总期限、staging 后源变化、单独打包以及 OS no-replace
原语回归；最终完整门禁已实际执行，不把早期 16 项改签为最终结果。

## 独立复核与修复闭环

新的非作者 agent 完成 r2 冻结候选的只读代码与功能复核，P0/P1/P2/P3 为
`0/0/2/0`，结论为 `REQUEST_CHANGES`：

- 新归档成功后残留 `isolation_target`，使窗口关闭检查持续 Busy。
- Unix 分支以外无需可变的目录 builder 在 Windows 触发严格 `unused_mut`。

归档开始时保存之前的隔离目标，worker 创建失败或已知终态恢复原目标。
未知结果、Cleanup 和 IsolationUnknown 继续保留新目标的关闭保护；已成功的新
归档不能清掉更早的未决目标。真实 GPUI 回归先红后绿：成功打包上传、安全提取
和未写入前源拒绝释放自身关闭保护；实际 pending WRITE 取消留下未知结果后，
另一归档完整成功仍保留旧目标、关闭保护和 quarantine。本次修复不宣称解决
其他既有普通 mutation 的整个 isolation 生命周期。

目录 builder 的 mutable shadow 限于 Unix 分支，Windows 保持 immutable。
独立 reviewer 确认修复了原 `unused_mut` 原因，但没有运行 Windows 编译器；
源码修复不等于 Windows strict gate 已通过。

[首轮原始报告](2026-11-15-reviewed-archive-transfer-evidence/independent-review-r1.md)
和[最终差分原始报告](2026-11-15-reviewed-archive-transfer-evidence/independent-review-r3.md)
均按原字节保留。最终报告对四文件差分完成有界复核，关闭原两项 P2，
P0/P1/P2/P3 为 `0/0/0/0`。它独立核对测试二进制 SHA-256 和 1195 项清单，
独立执行 9 项归档回归，结果 `9 passed / 0 failed / 0 ignored`。
执行使用作者预构建二进制，reviewer 没有独立重编译，也没有再次跑整仓门禁；
这一限制在原报告中明确披露。

原报告时点仍保留“最终 r3 gate 待完成”的工程条件，没有改写为事后批准。
父线程随后取得相同冻结输入的最终 gate exit `0` 并逐项核对，
满足报告中剩余的工程条件；原生、平台和依赖新鲜度边界没有因此解除。
reviewer 已关闭，不为纯文档再创建一轮复核。

## 真实协议与正常程序

当前 r3 源码通过现有隔离 localhost OpenSSH runner：`13/13`，外层 exit `0`，
耗时 `7.915s`。其中两项新增用例为两种格式完整打包/提取往返，以及审核后被
外部创建的远端目标保留。完整测试名和临时 pinned sshd 回执见
[openssh/result.json](2026-11-15-reviewed-archive-transfer-evidence/openssh/result.json)。
进程身份检查确认自有进程停止，临时目录已删除；父线程再次读回目录不存在。
这是实际 OpenSSH 协议互通，不是原生桌面或生产服务器验收。

最终源码的 GUI 和 MCP 正常程序构建 exit `0`，耗时 `11.316s`，
二者均为 macOS arm64 Mach-O。GUI SHA-256 为
`90ab21c8412ed9ea29bc877f5e73ffcb8ec7b05aa5d4d781e83890b4627534c7`，
MCP 为 `cf89dbc2b7fb77958d16c225d7d94c536970aca4c3f7675e544696a8a5811c88`。
见 [构建回执](2026-11-15-reviewed-archive-transfer-evidence/logs/normal-build-r3.json)
和[正常程序摘要](2026-11-15-reviewed-archive-transfer-evidence/normal-build-artifacts-r3.json)。
本批没有把测试二进制、正常构建或历史窗口短验当作新归档的桌面验收。

## 仍开放的验收

macOS/Linux 的本地提取依赖原子 no-replace 目录发布；Windows 的安全发布与
权限后端尚待实现，界面明确停用提取，打包上传可用。本机 macOS 结果不代签
Linux 或 Windows。服务器端打包/能力探测、Zmodem、中继、原生三平台、
完整 IME/辅助技术、签名、公证和安装发行仍开放。

应用资源预留保护合作写入者。SFTP v3 不提供对外部服务器进程的原子 no-follow/
内容版本比较；本地子路径观察也不消除所有恶意同用户祖先替换竞争。
未知结果、未完整验证的 staging 保留隔离，不自动清理或重放。
Podman 本批只读确认 VM 已停止，没有创建容器；不继承上一批镜像拉取失败为
本批容器执行结果。在线依赖最新版本检查未完成。

## 证据与清理

原始日志、三轮输入冻结、两轮独立报告、库存、完整门禁、真实 OpenSSH、
正常程序摘要和清理回执归档到
[持久证据目录](2026-11-15-reviewed-archive-transfer-evidence/)。
gzip 日志解压后逐字节相同，归档清单同时记录原文及压缩文件摘要；
失败、被拒绝的 r2 和原始条件性 HOLD 均保留。

两个实现 agent 和独立 reviewer 均已关闭，已合入的 core worktree 已受管归档。
自有测试临时目录、runner 和重复工作日志在归档校验后清理；唯一有效主 target
保留供下一批使用。既有其他任务 WIP 与受保护的旧 review worktree 保留，
不绕过 Codex 的活动任务保护。清理和源码提交绑定由证据目录单独记录。
