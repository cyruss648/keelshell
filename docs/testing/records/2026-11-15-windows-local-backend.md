# Windows 本地文件边界与 Linux 本地专项 — 2026-11-15

当前状态：`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_APPROVED / WINDOWS_NATIVE_NOT_RUN`。
基线：`a092dd5712c0a92ee4af18970fb0500fb230b760`。
本批只本地提交，不 push；完整产品、FILE-05/M4 和发行保持开放。

## 交付与最终输入

新增 `keelshell-platform`，将 Windows ABI 限定在同步对象句柄、逐组件访问、
实际本地卷检查、完整 file ID、创建时 protected owner-only DACL、有界目录枚举
及无覆盖发布的安全边界内。Windows Zmodem 来源/保存、归档来源/私有 staging/
发布、UI capability 及条件测试已接入。阻塞工作在 worker，既有 crate 继续
禁止 unsafe。设计见 [ADR 0138](../../adr/0138-windows-handle-local-files.md)。

最终 [r2 冻结](2026-11-15-windows-local-backend-evidence/raw/combined-inputs-r2.json)
为 `588` 个 Cargo/crates 文件及 `14` 个脚本，SHA-256：
`1fb9b4b343d25634fed4bc6cf9ec15d12ac7049562fd83f977960d9d0b78e1c9`。
完整路径集合及门禁后摘要一致。Linux 验证专用快照为
`f945c4c780010067aee664bf7ccbf6caace4eea5`，tree
`b9cebaeda6d35f028fd84fc0324272d4b209a47f`；
独立临时 index 未移动工作分支或正式 index，不冒称为正式功能提交。

## 实际验证

| 范围 | 实际结果 | 边界 |
| --- | --- | --- |
| 最终 full-gate-r2 | exit 0 / 632.963s / wait/reap | 当前 r2 冻结 |
| 普通 workspace Rust | 2958 passed / 0 failed / 35 ignored | 忽略项不算通过 |
| rustdoc | 14 passed / 0 failed | 与普通测试分别计数 |
| Python discovery | 187 run / 186 passed / 1 skipped | 包含新 runner 39 项 |
| 格式、严格 all-target Clippy、x.y 策略 | 全部通过 | 不等于在线最新版本核验 |
| AI readiness/observer/small-stack 控制器 | 全部通过 | 受控进程，无供应商 CLI 或模型调用 |
| Windows platform+session strict cross Clippy r3 | exit 0 / 5.138s / wait/reap | x64 MSVC all-targets |
| Windows 四个测试 target cross-link | exit 0 / 17.507s / wait/reap | AMD64 PE32+，没有执行 |
| 最终 PE 字节检查 | 143 个选定新描述符全部匹配 | 不是 native --list 或运行库存 |
| 正常 macOS GUI/MCP 构建 | exit 0 / 8.416s / wait/reap | 不签实际窗口或安装 |
| 最终 Linux arm64 后端/codec | 252 passed / 0 failed / 0 ignored，59.797s | 精确 r2 快照 |
| Windows 原生文件系统/GUI | NOT_RUN，实际执行 0 | 不能由其他结果补签 |

23 项新增默认 Rust 均逐名通过：platform 通用 16、queue alias 2、真实 SSH
controller 1、共享 worker 3、app 映射 1，已包含在 2958 中。
Windows 条件源定义另有 platform 46、Zmodem 45、archive 29；后两组分别有
2 和 1 项 privilege ignored，均未在 Windows 执行。完整 suite 与名字见
[机器摘要](2026-11-15-windows-local-backend-evidence/raw/gate-summary-r2.json)。
完整 app 库存 1239、session 宿主 lib 库存 194，不只检查总数。
既有 app `__eh_frame section too large` 链接警告保留，命令仍实际退出 0。
唯一父 target 始终属于同一 checkout，没有切换源码归属。

Linux 使用只读 Git 归档的 588 文件、原生 Linux arm64 Rust/Cargo 1.98.1、
UID/GID 1000、全新容器 target/Cargo home、locked/offline、network none。
六份 ELF64 小端 AArch64 与 VM/kernel/compiler 匹配，完整库存 418 名，
七组选集 252 名。归档、Git blob、冻结及执行后摘要一致。949 个公开 crate
checksum、855 个 sparse entry 和固定公开配置均核验；vendor payload 不入仓库。
共享 worker 的 3 项只在 Linux 库存中，不在该选集，未计入 252。
旧基线 251 项、r1 快照 252 项及其漂移 notice 不改签最终 r2。
最终容器和全部 46 个命令 PID 已核验回收；不是 Linux 桌面、x64 或独立 peer 验收。

## 独立复核与真实修复

平台 r3 SOURCE_CONTRACT **APPROVE**，开放 P0/P1/P2/P3 = `0/0/0/0`。
reviewer 用全新独占 target 实际通过 16 项通用测试和 Windows strict cross
Clippy，保存证据后回收 target。Linux runner r2 源码/Python **APPROVE**、
开放 `0/0/0/0`，独立 39 项仓库测试与 8 项故障注入通过；父相同 39 项不累加。
Linux r3 附录 **APPROVE** 仅核验最终运行原件、来源、库存和资源，不重跑测试。
Session r2 最终差分 **APPROVE**、开放 `0/0/0/0`；
非作者实际通过六项预构建专项，完整库存与 SHA 一致，没有独立重编译。
三个复核范围共关闭六项 P2，无开放发现。

| 复核问题 | 修复与真实证据 |
| --- | --- |
| 平台 private target directory sharing | 长期只保留读 capability；源发布从 held 对象取得短期 ReOpenFile DELETE lease，重新核验本地卷、身份和 DACL。普通文件独占其他 writer/delete 的契约保持。 |
| 平台 rename 缓冲长度 | 满足完整固定结构加准确名称字节，传入初始化实际长度；短/最大名称文件与目录增加 Windows 正控定义。 |
| runner ignored 误签 PASS | 每个 selected 必须实际 ok；必需及额外选定 ignored 的负控均拒绝。 |
| runner finish/signal 异常丢回执 | 先保存 FINALIZING 与 primary error；持久化失败不跳过清理，恢复 handlers，失败不能残留 PASS。 |
| WS-ZM-01 Unicode reservation 关系不一致 | 共用 filename_alias；两项 queue 与一项真实 SSH controller 红/绿回归，旧 unknown 不因新拼写/会话解除。 |
| WS-AR-01 worker 结果交接提前清 pending | 仅 async owner 实际接收 join 结果后确认；两个真实 SSH scope、实际临时 staging/publication 和精确 barrier 回归先失败再通过，另有正控与 app 未知 close-hold 回归。 |

Windows syscall 测试定义没有原生执行证据。session reviewer 的宿主专项使用
父提供、核对 SHA 和完整库存的稳定预构建副本，不是独立重编译。
原 HOLD、notice、红回归和签署时边界全部保留。

## 保留失败、环境与依赖

full-gate-r1 因 WS-AR-01 实际 TERM/wait/reap，exit -15，不算成功。
Windows 组合 Clippy r1 的共享 AtomicBool 导入错误、app 专项不存在的 --lib
和缺 test imports、Unicode 测试自己打开 SFTP 后仍错要求零 channel 的失败均保留。
修正后专项及最终 gate 通过，没有放宽生产行为；controller 在打开 SFTP 前严格
检查无远端权限，之后精确检查一个 channel/subsystem、零命令。

SDK 首次缺 windows.h 及后续 intrinsic include 顺序失败保留；
最终任务内 r2 wrapper 未修改全局 Cargo 配置、依赖源码或 SDK header。
37/37 SDK/CRT payload 匹配实际 Microsoft 产品清单 SHA；频道记录的清单摘要与
实际内容不一致，未验证 Microsoft 签名，不是发行供应链批准。来源、清单、
版本和完整校验记录保留，临时 vendor payload 与重复解压不入仓库。

本机实际 Rust/Cargo 1.100.0，默认 stable，无项目 toolchain pin 或新 MSRV。
Linux fixture 的缓存镜像版本是运行事实，不是最新工具链声明。
直接 registry 要求均为 x.y，保留 Cargo.lock；新 windows-sys 为 0.61 / 锁定 0.61.2。
两轮全量在线依赖检查均 60 秒超时、TERM/wait/reap，没有最新版本报告，
不能声明所有依赖已核验最新。

## 收口和剩余范围

持久原件从[证据入口](2026-11-15-windows-local-backend-evidence/README.md)读取。
1301 份原件已逐字/无损压缩核验，
[index/blob 读回](2026-11-15-windows-local-backend-evidence/index-input-binding.json)
确认同一 602 项工程输入与全部原件一致；提交后继续绑定精确功能提交。
纯文档/绑定变化不触发不变工程的重复门禁。
三个 reviewer 已关闭，六个自有容器重新确认 absent；SDK、registry payload、
稳定测试副本及批次目录等 `2,609,954,825` 字节临时内容已回收。
[清理回执](2026-11-15-windows-local-backend-evidence/cleanup-final.json)保留完整范围，
唯一有效 target、其他任务资源和所有失败/未知证据保持。

下一步为真实 Windows 平台/session 测试（privilege opt-in 单列），以及文件选择、
归档、取消和关闭保护的原生桌面流程。三平台完整桌面、IME/辅助技术、
strict peer HOLD/PARTIAL、FILE-05/M4、完整产品与发行安装保持开放。
