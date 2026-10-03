# SFTP 暂停、内容校验续传与 OpenSSH 互通

日期：2026-10-03。基线：`55862ce`。本记录与本轮功能代码一起提交，远程 CI 结果在确认后追加。设计见 [ADR 0012](../../adr/0012-resumable-sftp-transfers.md)。

## 行为与工程结构

普通文件和目录传输新增暂停、继续及取消。后台在当前分块请求得到确认后发送 `Paused`，前台在此之前只显示请求中；暂停不消耗活动预算，保留 FIFO 位置，取消可以唤醒暂停。终态独立于有界进度队列，慢消费者不能阻塞取消和后续任务。分块使用明确 SFTP 偏移及 WRITE 响应计数，缓冲直接堆分配。

显式文件续传先只读生成不可构造的审核计划，记录完整源 SHA-256，比较全部既有目标前缀，确认后在原 SSH 连接复核并追加。目录续传要求目标根已存在，先核对整棵树和全部文件，再补缺失项及空目录；额外目标、错误前缀、链接、类型和来源变化会在初始写入前拒绝。暂停恢复重新检查命名路径并重开句柄；目录完成前再次检查完整树，避免旧 inode 或较早完成文件的变化被遗漏。重连或重启后需要新计划，不自动恢复任务。

文件面板拆分 `worker.rs`、`transfer.rs`、`view.rs` 和独立真实协议夹具。目录进度明确包含逐文件复核的已有字节，不能用当前进度减去全目录已有字节来伪造新写入量。

## 自动化验证

| 项目 | 本机结果 | 保留证据（ignored work） |
|---|---|---|
| 整仓格式、严格 Clippy、x.y 依赖策略 | 通过 | `resume-full-gate-final-2.log` |
| 整仓单元/集成与文档测试 | 469 + 2 通过；4 项 OpenSSH opt-in 在此命令默认忽略 | 同上 |
| Files GPUI + 真实 SSH/SFTP | 16 通过，其中新增 8 项 | `transfer-ui-tests-final.log` |
| 目录续传专项 | 11 通过 | `directory-resume-tests-final-3.log` |
| 文件续传专项及 2 MiB 栈 | 各 9 通过 | `file-resume-tests-final.log`、`sftp-resume-small-stack.log` |
| Session 全目标（包括示例） | 119 通过；当时 3 项 OpenSSH 默认忽略，第四项随后单独增加并运行 | `sftp-resume-session-tests-final.log` |
| 打包回归 | 47 通过 | `resume-packaging.log` |
| 实际 OpenSSH 10.3 互通 | 4 通过 | `openssh-inode-regression/result.json` 与 `tests.log` |

OpenSSH 用独立回环监听、临时 Ed25519 主机/客户端密钥、固定指纹和当前本机账号公钥认证，不修改用户 SSH 配置。4 项测试覆盖双向文件续传及重连、双向目录续传、暂停/取消后新计划及同 SSH 的 exec，以及暂停后目标原子替换的拒绝。最后一项保留旧文件句柄，替换为长度、权限、修改时间相同但内容不同的新 inode；继续必须失败，新命名目标和旧句柄均不得新增写入。

`scripts/openssh_interop.py` 为 macOS/Linux 提供最多 300 秒的独立运行与清理回执，Windows 明确拒绝此脚本。临时密钥与测试文件位于上传证据目录之外；Quality 为 macOS/Linux 增加实际 OpenSSH 步骤。发布矩阵和 Windows/Linux GUI 仍需分别验收。

## 原生 macOS

采用独立 `KEELSHELL_DATA_DIR`、一次性回环 SSH/SFTP 服务及读写延迟。终端仅回显，监控 exec 拒绝属于夹具限制，不证明 Linux 主机指标。第一次构建完成 4 MiB 上传的暂停、语言切换、继续及最终 SHA-256 一致。

最终应用重新构建，二进制 SHA-256：

```text
d4d88ff8263d6457d7f768e8de582d1ce59971028357ff2af4bb46e7f31cf6d5
```

最终构建已完成重新启动后的只读续传审核：目标既有 3,145,851 B，审核前后不变；确认后暂停、切换中文和继续保持同一目标。继续后文件达到 4,194,304 B；上传及下载续传的 SHA-256 均为 `2b07811057df887086f06a67edc6ebf911de8b6741156e7a2eb1416a4b8b1b2e`。随后完成目录双向续传，2 个文件共 294,920 B、3 个目录（含根和空目录），全树路径、类型及文件摘要一致。

应用和两个测试服务已退出，回环端口关闭，GUI 临时远端树与手动 OpenSSH 临时密钥已清理。原生结果和清理状态记录于 `work/resume-native/fixture.json`。

## 发现与修复

- 真正 OpenSSH 在 subsystem 成功前发送 `WindowAdjusted`，原单次等待逻辑误判为关闭。改为继续等待明确 Success/Failure，仍由通道所有权任务保证取消和截止时间；保留 `resume-openssh/interop-1.log`、`interop-2.log`，修复后 `interop-3.log` 全部通过。
- macOS `/var` 符号链接导致新的 GPUI 夹具被安全路径规则拒绝。修复测试根的 canonical 路径，未放宽生产路径检查；中间失败记录保留。
- 独立复审发现暂停后重命名留下旧句柄仍可写，已改为重开命名路径并完整核验；真实 OpenSSH inode 回归通过。
- 第一轮整仓门禁仅因测试模块排序不符合 rustfmt 失败；格式化后完整重跑通过，原日志 `resume-full-gate-final.log` 保留。
- 互通脚本首次将 Cargo 符号链接解析成 rustup 导致 argv[0] 错误，已保留入口并实际重跑；失败回执保留。进程提前退出后脱离原进程组的后代，改为累计记录内核出生身份并逐个核验；意外 daemon 退出造成观察盲区时回执保守报告失败。真实父退出/setsid/忽略 TERM 回归通过，错误出生身份拒绝发信号。最终互通 `openssh-cleanup-ownership-final/result.json` 4 项通过，2.133 秒、25 个观察身份，进程和临时目录清理均通过。孤儿入口竞态证据为 `openssh-orphan-smoke-entry-race/result.json`。

## 证明边界

完整摘要及目标前缀保证属于显式续传；普通暂停继续沿用原有复制语义。取消无法撤销已发出的 WRITE；并发修改可能造成失败和部分输出。SFTP v3 没有可移植的原子版本比较或 NOFOLLOW，本实现不是远端文件系统事务。成功响应不证明断电后持久性。

这些结果证明受控本机 TCP、实际 OpenSSH 和 macOS 原生操作；不证明任意生产服务端、Windows/Linux 桌面、自动重连、自动任务恢复或完整产品目标已完成。本轮不创建发布标签。

## 首次 GitHub Quality 与 Windows 修复

功能提交 `4e58580` 的 [Quality 37098529404](https://github.com/cyruss648/keelshell/actions/runs/37098529404) 在 macOS 和 Ubuntu 通过：各 469 项普通测试、2 项文档测试、47 项打包回归，以及额外执行的 4 项 OpenSSH 互通。两份远程回执均确认观察到的进程和临时密钥目录清理完成，`ancestry_unverified=[]`。

Windows 的格式、严格 Clippy、160 项 app 测试均通过，普通测试累计 461 通过、2 失败，尚未进入文档测试；47 项打包测试执行，其中 1 项平台性跳过。失败仅出现在两项目录下载续传，返回 `Incorrect function`，不是编译或栈溢出。原始 job 日志与平台计数保存在 `work/resume-ci/`，不把其他平台通过或重跑成功覆盖这次失败。

根因是允许缺失父目录时逐个拼接 Path 组件并立即读元数据。Windows canonicalize 的 verbatim 绝对路径先产生单独盘符 Prefix，RootDir 尚未加入即被当成完整路径查询。修复改为遍历完整绝对祖先路径，保留存在项的目录/链接校验；新增 canonical 绝对根、缺失目录、普通文件阻挡和符号链接回归。原有失败用例保留且不降低断言；跨平台路径与普通文件祖先回归在 Windows 同样执行，静态链接回归使用 Unix API。修复提交的实际 Windows 结果以对应的 [Quality 工作流](https://github.com/cyruss648/keelshell/actions/workflows/ci.yml) 为准。上述 macOS GUI 哈希属于此路径处理修复前的构建。

修复后本机完整格式、严格 Clippy、依赖策略、472 项单元/集成测试与 2 项文档测试通过（`work/resume-ci/windows-path-full-gate.log`）；4 项真实 OpenSSH 再次通过，进程与临时目录清理通过（`windows-path-openssh/result.json`）。这些本机结果独立于后续 Windows CI 验证。

## 修复提交的远程验证

提交 `7f95c46` 的 [Quality 37099262632](https://github.com/cyruss648/keelshell/actions/runs/37099262632) 已在三个平台全部通过。macOS、Ubuntu 各通过 472 项普通测试、2 项文档测试、47 项打包回归，并单独通过 4 项 OpenSSH 互通；Windows 通过 465 项普通测试、2 项文档测试，打包回归执行 47 项（1 项平台性跳过）。Windows 原先失败的两项下载续传以及新增完整路径回归实际执行成功。

原失败流水线继续保留，成功日志、计数及互通回执位于 ignored `work/resume-ci-fixed/`。本结果对应上述精确提交，不代表此后的功能改动已验收，也不代表 Windows/Linux 原生桌面或六目标发布矩阵已经重新执行。
