**APPROVE — r3 限定证据核验附录，仅签署 linux-final-r2**

签署时间：2026-11-15T13:28:29.055306+00:00。开放 **P0/P1/P2/P3 = 0/0/0/0**。
本附录在原 r2 runner 源码/Python APPROVE 之后，仅核验父线程新完成的
Linux 原生本地文件后端与 codec 原件。未重新编译、执行 Rust/Python 测试或重跑 Linux，
未等待全 gate；不签 Windows、桌面或完整产品验收。

精确输入：验证快照 `f945c4c780010067aee664bf7ccbf6caace4eea5`，tree `b9cebaeda6d35f028fd84fc0324272d4b209a47f`，
原工程基线 `a092dd5712c0a92ee4af18970fb0500fb230b760`。
[combined-inputs-r2.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/combined-inputs-r2.json) 为 **588 + 14**，SHA-256
`1fb9b4b343d25634fed4bc6cf9ec15d12ac7049562fd83f977960d9d0b78e1c9`；
[validation-snapshot-r2.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/validation-snapshot-r2.json) 绑定同一冻结。
这是仅用于不可变 Git archive 验证的快照，不冒称为正式功能提交。

Runner SHA-256 `b134be0b11b7b79b5c88baddf363a25bad3b5f4583265e1ca26ff8a35a4a55b0`；Python 测试源码 SHA-256
`660a7b8fc6347364eb6dd0be340d421ff207c708cc1da3f64ac8ad964bbef8af`。三份实际执行的 host-runner-inputs 均与
receipt、combined 冻结及原 r2 APPROVE 一致，起止读回无变化。
原独立 39 + 8 = **47** 项 Python 结果维持其原轮次，未在本轮重跑或累加。

**源码与归档绑定**

[source-manifest.json:592](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/linux-final-r2/source-manifest.json:592) 的 588 个完整路径及
SHA-256 与组合冻结精确相等；逐项根据归档字节重算 Git blob OID，**588/588** 匹配
f945c4c7 的 Git tree。归档全局 comment 也是该精确提交，没有复制活动工作树。
14 个脚本的完整路径集合、内容摘要与 Git blob 也匹配冻结。
审查起止两次读回，当前 588 源码及 14 脚本没有工程输入漂移；文档不在该冻结集合。

- source.tar SHA-256：`3159ac90e2f2509850eafdff824136ba89fdd6d1615b57bf986d93ee1710e7e0`。
- 源码 canonical manifest SHA-256：`22e95c2f8c57c416093cff1af18221a5eb14ab93cbc5cc445f5fb20e56a10419`。
- Linux 实际运行后的原始 sha256sum 日志，逐项覆盖同一 588 文件并完全相等。
- receipt SHA-256：`ba373feff61c2685a4ceddcfec6acc45e5a4879d0f0a8f6f6242c4fdb6fbb3f5`；cleanup SHA-256：`9021710f824da9fea98a13dd56fc9197fec2373bc6ab93df50d1153898ddfc30`。

**实际结果、库存与 ELF**

最终 [receipt.json:2382](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/linux-final-r2/receipt.json:2382) 为 PASS，实际 **59.797s / 900s**。
父明确报告外层 **exit 0 / reaped**；本 reviewer 没有接管或 wait 父外层进程，
独立核验的是最终 receipt、全部 46 个命令终态及 stdout/stderr 原件。
45 个步骤 exit 0；最后 `podman container exists <full-id>` 的预期 exit 1、stdout/stderr 为空。
所有步骤均非超时/中断，owned_processes_stopped=true，预算与实际耗时保留。

| package / target | 选集 | passed/failed/ignored | 原始实际结果 |
| --- | --- | --- | --- |
| keelshell-session/keelshell_session | `zmodem::` | 25/0/0 | [021-test-keelshell-session-keelshell_session.stdout.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/linux-final-r2/021-test-keelshell-session-keelshell_session.stdout.log:29) |
| keelshell-session/keelshell_session | `sftp::archive_transfer::local::tests::` | 1/0/0 | [022-test-keelshell-session-keelshell_session-archive-local.stdout.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/linux-final-r2/022-test-keelshell-session-keelshell_session-archive-local.stdout.log:5) |
| keelshell-session/zmodem | `完整 target` | 26/0/0 | [026-test-keelshell-session-zmodem.stdout.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/linux-final-r2/026-test-keelshell-session-zmodem.stdout.log:30) |
| keelshell-session/archive_transfer | `完整 target` | 18/0/0 | [030-test-keelshell-session-archive_transfer.stdout.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/linux-final-r2/030-test-keelshell-session-archive_transfer.stdout.log:22) |
| keelshell-core/zmodem | `完整 target` | 102/0/0 | [034-test-keelshell-core-zmodem.stdout.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/linux-final-r2/034-test-keelshell-core-zmodem.stdout.log:106) |
| keelshell-core/archive | `完整 target` | 40/0/0 | [038-test-keelshell-core-archive.stdout.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/linux-final-r2/038-test-keelshell-core-archive.stdout.log:44) |
| keelshell-core/archive_pax | `完整 target` | 40/0/0 | [042-test-keelshell-core-archive_pax.stdout.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/linux-final-r2/042-test-keelshell-core-archive_pax.stdout.log:44) |

**合计 252 passed / 0 failed / 0 ignored**。
逐项核对 raw `--list`、selected、required、实际 `test ... ok` 与汇总，完整名称集合一致；
以 package/target/name 去重，没有将两个 lib 选集或旧轮次重复计数。
旧 251 的全部名称仍存在，新增 selected 名为
`zmodem_controller_unicode_alias_is_busy_before_accepting_any_receive_data`；
本次 252 独立来自 r2 原始日志，不借用旧 251 或 r1 的 252。

实际 Cargo JSON 给出 6 个 test executable；六份保留 binary 原件及原始
sha256sum/readelf stdout 的目标路径、摘要、ELF 字段、revision 全部一致，
库存共 **418** 个名称，执行选集为其中 252 个。
六个均 ELF64、小端、AArch64，与原始 VM arm64、kernel aarch64、
compiler host aarch64-unknown-linux-gnu 一致，Rust/Cargo 均为 1.98.1。
库存、摘要、ELF、测试命令直接指向对应 Cargo 编译产物的 `/linux-target/debug/deps/...` 路径；
实际构建与测试为 UID/GID 1000、locked/offline、容器私有 target/Cargo home。
没有宿主 target 挂载、MSRV bypass、依赖下载或网络。
六份完整 binary 摘要见 [audit-r3.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-linux/r3/final-r2/audit-r3.json)。

边界：新增共享 mutation_worker 的下列 **3 项**已经出现在 session lib 的 192 名库存中，
但不在此 runner 的七组选集中，**本次 252 不声称它们已执行通过**：

- `sftp::archive_transfer::mutation_worker::tests::cancelled_publication_handoff_reports_uncertain_before_worker_result_is_returned`
- `sftp::archive_transfer::mutation_worker::tests::cancelled_stage_handoff_reports_uncertain_before_worker_result_is_returned`
- `sftp::archive_transfer::mutation_worker::tests::observed_mutation_result_acknowledges_a_stopped_worker`

这些测试的宿主回归、WS-AR-01 修复源码及全 gate 由父及相应非作者另行签署。
容器已删除，reviewer 没有可执行二进制字节可以重新 hash；这里核对的是同次运行保留的
Cargo/sha256sum/readelf/库存/执行链，不冒称为独立重建或独立 Rust 执行。

**公开 registry 载荷与回收**

registry.tar 在原位读取核验，没有复制、解包或归档 vendor payload。
Cargo.lock 的 978 个公开 registry package 与 manifest 精确一致；949 个实际缓存 crate
逐字节 checksum 匹配 lock，855 个 sparse entry 匹配公开名称白名单与其摘要，
另一个文件是固定公开 crates.io config，共 1805 个普通文件，没有额外配置、credential、
链接或非白名单成员。29 个未缓存 archive、0 个缺失 index 显式保留；实际选集离线编译通过，
不把这 29 个缺失依赖冒称为已缓存或已覆盖的完整 workspace 验收。
Bundle SHA-256 `4d22edf25fc5268f59c7d6d0b5a92d7cf48e3c16f6f79b05798ee32e7ef0378c`。
父可按既定计划删除临时 registry.tar，保留 registry-manifest.json 与本派生核验即可。

运行 owner `65231814245d4a98ab4e98e796cdc0a2`，full container ID
`38a13b7ad2e2b9dc2c3b60a74b93d1b00d224b0f9ab6d29093d575585d8a077d`。四次原始 inspect 均满足完整 ID、唯一名称和两个 owner label，
末次身份核验紧接按 full ID 删除，随后读取 absent。
[cleanup.json:2](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/linux-final-r2/cleanup.json:2) 与主 receipt 一致：container_removed、
owned_processes_stopped、container_filesystem_and_target_removed 均为 true。
本 reviewer 另做一次只读 full-ID exists，**exit 1 / 空 stdout/stderr**；
对原 46 个命令 PID 的 ps 读回也为 **exit 1 / 空 stdout/stderr**，均不存在。
没有删除 image、volume、VM 或其他任务资源。

**本轮命令、历史与资源边界**

唯一实际证据核验命令为：

`python3 -B /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-linux/r3/verify_final_r2.py`

其 [command receipt](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-linux/r3/final-r2/audit-r3.command.json) 记录实际
**exit 0 / 0.513s / waited_and_reaped=true**，stderr 为空。
内部仅执行两个只读 Git 查询、一个只读 Podman exists 和一个 ps 查询，均已 wait/reap；
9373 个核验断言不是测试用例数。本轮实际 Rust 测试 **0**、Python 测试 **0**。
未运行 Cargo/rustc、父 target、容器测试、旧 probe 或全 gate。

原 r1 HOLD、notice-001/002 与原 35 文件保持原字节；r1/r2 review 合计 **140** 个文件
完整路径与摘要不变。r2 native 的 **110** 份非 registry payload 原件在本轮起止不变。
[notice-003](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-linux/r3/notice-003-current-tree-drift.md) 与首轮 r1 核验中止原件保留：
其 exit 1 来自父继续修复 WS-AR-01 后当前树偏离旧冻结的附加断言，
不是 Linux 运行失败，也不重新打开 runner P2。
`linux-final-r1` 的 252 和更旧 native-r3 的 251 各自保留原提交、runner 与时间，
没有改签为本附录的 f945c4c7。

本轮持久写集仅 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-linux/r3`。没有新建 agent/chat/container/worktree 或 Cargo target；
只读等待已结束，核验子进程已取回终态，无自有活动会话或待回收运行资源。
旧 review-linux/tmp 与 review-linux/r2/tmp 仍为可回收空目录，未删除；
本轮报告、原始日志和派生核验均作为证据保留。无需等待全 gate，可结束本 reviewer。
