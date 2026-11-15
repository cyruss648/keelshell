**KeelShell core/session Zmodem 独立只读评审（报告 r2，最终绑定 freeze r3）**

评审结论：**源码 APPROVE；可以作为 candidate 合入，但必须完成当前批次要求的门禁。direct PTY 握手互操作验收 HOLD / PARTIAL，产品和原生验收未完成。** 截止 2026-11-15T09:13:31.243564+00:00，父报告 full-gate-r2 已 exit 1/reap，full-gate-r3 顺序运行中；本报告不签完整门禁成功。

最终 r3 上新增未关闭代码发现：**P0=0、P1=0、P2=0、P3=0**。r1 独立发现的 publication hold 竞态为 **P2=1**，已在两个固定 r2 文件上完成源码复核与实际回归，状态 **CLOSED_IN_R2 / R3 再确认**。保留原 notice，不覆盖历史证据。互操作失败是当前可复现的验收缺口；此评审没有把它归因到某一源码缺陷或据此捏造新的 P2。

**冻结与权限**

唯一 cwd：`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。基线 HEAD `721bad2157da8d3a0bc11a1eab5ac59b6616aa9c`，r2 时点通过只读 HEAD/ref 文件核对相同，没有执行 Git。r1 清单 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/frozen-inputs-r1.json` SHA256 `0fb6bc9d7c1a845583c56c803796c92ccc73b0c590c9f3b29bcd83f292ca63d5`，开始时 570 inputs + 12 scripts 全部匹配。r2 清单 SHA256 `5119d1724074afb5332148f5002b4614767f6ec2837acd6af62a596b0d8ebfb1` 在实际 session 专项时独立核对 582/582 匹配，r1→r2 六个 Rust 文件。最终绑定 r3 清单 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/frozen-inputs-r3.json`，SHA256 **`14b353ecbf84d9684b20dd7bafa0ffc090644b452fe199e4d32f1c2745e8e882`**，截止再次重算 **582/582 匹配，0 缺失、0 不符**；r2→r3 仅 `crates/keelshell-session/src/zmodem/local/tests.rs` 首部新增 `#![cfg(test)]` 和一个空行。去掉这两行后，其 SHA 精确恢复 r2；生产输入相对 r2 完全不变，core 和 12 scripts 相对 r1 也未变。r1→r3 共七个 Rust 文件变化，具体 SHA 见 [freeze-check-r3.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/review-core-session-r2/freeze-check-r3.json)。

零源码编辑、零 fmt、零 Cargo、零 Git 命令、零共享 target 变更、零无关清理。只写自己的报告/日志和这些测试的临时数据。执行稳定 binary 是使用父构建副本，**不是独立重编译**。副本的 SHA、完整库存、运行前后字节及当前冻结由评审独立核对，生产构建来源仍依赖父提供的 metadata/说明。

**已读范围**

先读 AGENTS.md、`/Users/hdh/.codex/RTK.md`、HANDOFF 前 75 行、ADR 0136 和产品 ZMODEM。全部阅读以下 14 个主范围文件，最终 r3 版本合计 **7,662 行**；r1 主范围为 7,476 行，之后重新读 session 修复、诊断变化和完整 r3 local/tests.rs。

| 绝对路径 | 最终行数 | 范围 |
|---|---:|---|
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem.rs` | 417 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/codec.rs` | 580 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/detector.rs` | 119 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/receiver.rs` | 510 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/zmodem/sender.rs` | 458 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/zmodem.rs` | 1268 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem.rs` | 1242 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem/local.rs` | 673 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem/local/tests.rs` | 492 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem/tests.rs` | 179 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/zmodem_local.rs` | 55 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/zmodem.rs` | 957 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/zmodem_openssh.rs` | 462 | 全文件；变化处再次复核 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/support/zmodem_fixture.rs` | 250 | 全文件；变化处再次复核 |

另读 core/session 导出、完整 session events/Cargo.toml、SFTP reservation 与 registry 的相关范围、SSH 连接身份/有序 shell reader/lifecycle/write 路径。Cargo.lock 整体 TOML 结构已解析，新增 ACL、rustix 和 session resolved dependency 条目人工核查。ACL helper 缓存源码全读，并将锁定 `.crate` SHA 与五个提取成员逐字节匹配；证据见旧 [dependency-check.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/review-core-session-r1/dependency-check.json)。曾有一次只读 `.cargo-checksum.json` 缺失读取失败，随后用实际归档校验解决；没有据该失败读取签验收。

为核对 finding 影响与保存回执语义，还定点读了 app bridge、terminal safety 和 receipt 代码/测试；**不构成完整 app 评审**。每个文件的实际范围、SHA 和版本说明见 [read-scope.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/review-core-session-r2/read-scope.json)，不把只搜索到名字的其他文件算作完整阅读。

**P2 原因与关闭依据**

原位置是 r1 SHA `1c1ce173d51a0f4d40dad10357b5f17e53dda1973a97639f9ddffa37bd1b36d6` 的 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem.rs:1149`，即 `unresolved.load()` 后单独 `pending.store()`，同类写入也存在于旧 drop 路径。

可达触发序列：旧 publication 的 await 已超时/被丢弃；worker 清除 running，但返回值中的 mutated ticket 尚未 disposition；新 publication 成功，在 known() 中读取 unresolved=false；旧 ticket drop 设置 unresolved=true 并保留 backend quarantine；新 known() 再写入先前读取的 false。结果是后端仍隔离，但公开 mutation_pending handle 可以持续错误地为 false。原证据是源码交错证明，未在旧实现上独立构建竞态测试。影响限定为 hold/unknown API 契约违反；未证明绕过 backend quarantine，也未证明原生关闭绕过。

修复源码 SHA `034ebc6dfa3a2c512ee5a0106f9554c80a23b836235dda60c4fec8b5496c4f6d`，测试 SHA `aa0d3b1baf5ff639e4928128d7910b3eb1cc70521e8ced18dc8f2090bb9dbcdd`，均与最终 r3 冻结和父声明一致，运行前后未改变。当前 [/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem.rs:1128) 的 PublicationHold 用同一个 Mutex 管理 active ticket 数及 sticky unresolved，1177 行在同一临界区发布 pending；未观察结果在 ticket disposition 前一直计数。1209 行 take 保证 disposition 幂等；1215 行 backend disposition 保持在 hold 锁外；计数溢出/下溢、poison 都保守保留 unknown。两种完成先后顺序均不能再把 active/unknown 投影为 false。

新增五项回归实际通过：旧未观察结果与新成功、unknown 后成功/未变更 drop、poison、晚到 unknown 对新成功的竞争、晚到 unknown 对未变更 drop 的竞争。其中竞争测试在持有同一状态锁时使两个 disposition 等待，再释放锁，并用有界等待确认最终 active=0、unresolved=true、pending=true。测试用无 backend claim 的真实 PublicationTicket disposition 验证计数；实际 backend/连接/文件排他另由 18 项集成测试覆盖。详见 [publication-hold-closure.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/review-core-session-r2/publication-hold-closure.json)；原 [notice](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/review-core-session-r1/notice-01-publication-hold-race.md) 原样保留。

**实际执行的命令与数量**

共 **6 次 binary 调用：3 次 `--list`，3 次测试执行**。精确核对完整库存 **65 + 185 + 18 = 268 项**，实际执行 **65 + 27 + 18 = 110 项，110 passed、0 failed、0 ignored**；session lib 其他 158 项被筛除，未代表其已执行。三个测试运行均设置外部 60 秒上限；两个 session 运行将 TMPDIR 显式放在本报告目录内。

| binary / 实际测试命令 | SHA256 | 结果 |
|---|---|---|
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/stable-tests/core-zmodem-r1 --test-threads=1` | `48978f2b923ece687861b070a88bc3f42153213445cccd07787aadec1162554b` | 65/65，exit 0，无忽略 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/stable-tests/session-lib-r2 zmodem:: --test-threads=1` | `6c44842fd3a9f5a90ad1137737f2b4cdf95219c46ef202aef42d2ce227889611` | 27/27，exit 0，无忽略 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/stable-tests/session-zmodem-r2 --test-threads=1` | `7c0a2eaaad3ea3d5c0e1a769c364b2cc93ddabce273a2200f5978780e2640a26` | 18/18，exit 0，无忽略 |

**r3 测试输入边界：** session 的 27+18 项执行基于 r2 稳定副本。后来添加到 `local/tests.rs` 的两行是 remote_only 架构 guard 要求的 test-only 标记；外层本就仅在 cfg(test) 下引入该模块。此差分未改生产代码、断言或库存逻辑，已全文件复读并用 SHA 证明只有该前缀，但本评审没有重新构建或声称执行来自 r3 新测试字节的 binary。已执行结果的字节来源仍如实标为 r2；core 输入一直未变。详见 [post-freeze-test-cfg-delta.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/review-core-session-r2/post-freeze-test-cfg-delta.json)。

每个 binary 另执行一次 `--list`，与父相应 inventory 文件和 metadata 的测试名逐项等序匹配。core 来自父 core-r3，core 全输入 r1→r2 不变，因此没有无意义重跑或重编译。session 新副本来自父 session-hold-r1 / session-integration-r4，27 项含五项新回归。实际 argv、环境、时间、计数、前后 SHA、日志都在 [report.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/review-core-session-r2/report.json)、[session-test-results.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/review-core-session-r2/session-test-results.json) 和旧 [core-zmodem-tests.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/review-core-session-r1/core-zmodem-tests.json)。只读核查使用 `rg`、带行号读取及 Python hashlib/json/tomllib/tarfile；没有执行全仓测试。

这些测试与源码复核覆盖 CRC16/32 和独立 wire vectors、分片与控制转义、ZCOMMAND 拒绝、声明大小/文件数/累计流量/包数/无进展重试、ZFIN/OO 与空文件/空批次边界、审核撤销/连接替换、保留 fd/祖先身份、实际 macOS ACL、no-replace、unknown 与 quarantine。未据此宣称穷尽所有并发时序或真实 peer 组合。

**互操作原件与当前阻塞**

下表全部为**父执行、评审读原件并核对计数**，没有计入评审自己的 110 项。

| 本批证据 | 结果 | 结论 |
|---|---:|---|
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/real-pty-r5` | 7/7 | 旧输入/旧时点的本机实际 SSH raw PTY；保留历史，不能签当前候选 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/linux-cycle-r2` | 5/7 | Debian lrzsz direct PTY：empty、CRC32 下载最终握手失败 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/linux-cycle-r3` | 4/7 | Debian lrzsz direct PTY：三项下载缺 OO，FileSaved 完整而 controller=Receiving |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/linux-pipe-diagnostic-r1` | 7/7 | 相同 stable binary-r2、未改 Debian sz，经临时 pipes relay；仅为诊断对照 |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/real-pty-r6` | 6/7 | r2 本机 actual raw PTY；r3 生产输入相同；macOS lrzsz 0.13.1 CRC32 下载也缺 OO |

r6 harness exit **1**、内部测试 exit **101**，实际 peer exit **0**；失败项 `real_sz_sends_binary_unicode_content_over_crc32` 的 FileSaved 为 **262,156 字节**，SHA256 `41fb26520c4a149eb60ab75a50a51d8c72228549629e2a32d1fe7a74cc10715c`。最后记录的输入 `2a2a184230383030303030303030303232640d8a` 是有效 ZFIN，controller 仍 Receiving，peer 日志有 Transfer complete。诊断只记录最近六个 Data chunk 各前 96 字节；最终 FIN chunk 长 20 字节，被完整记录，但这不等于完整 wire capture。原件 [tests.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/real-pty-r6/tests.log)、[result.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/real-pty-r6/result.json) 均已独立读过。

pipe shim 只将 peer stdin/stdout 的 pipes 与实际 SSH raw PTY 间转发，不改 sz 可执行文件；嵌入 peer wrapper SHA `eec1246db6b2095db23cb57ba84cb2224d29599f71e44fbba18a575b8e95f959` 与日志一致。对该诊断 result 的 **59 项归档文件**逐项重算大小和 SHA，0 不符，cleanup 三项均 true。它支持继续调查 TTY 路径/退出尾字节，**不证明 tcflush 根因，不替代 direct PTY**。r6 进一步说明问题不能限定为 Debian。较早 r1-r4 失败证据不清理，r2/r3/r6 的失败也不被成功对照抵消。详细 hash 和各运行结果见 [interop-boundaries.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/zmodem-integration-20261115/review-core-session-r2/interop-boundaries.json)。

源码确认：session 只在 no-replace 发布、身份/ACL 和完整回读都成功后发 FileSaved，随后 acknowledge_saved；receiver 在 Finishing 收到连续 OO 才发 Finished。实际 SSH reader 先有序投递 Data，最终才投递 Exited。代码没有用退出码 0、FileSaved 或 peer 文字补签 Finished。UI 的独立 verified_saves 列表保留最近 32 条完整回执，失败反馈不删除该列表；仅为会话状态内保留，非重启持久化，亦无本评审的 GPUI/原生运行签收。

提出的 ReceiveClosedWithoutFarewell 属于后续设计，当前冻结没有该结果，也没有用它放宽测试或关闭互操作缺口。若后续实现，应另审精确 PTY/连接代次/transfer 身份、当前 Finishing、已保存范围、实际有序 Exited0、ZFIN 成功写入证据，以及不释放输入/旧 unknown/quarantine 的拒绝路径；本次 APPROVE 不覆盖该未来实现。

**合入建议与未验收边界**

源码层面推荐将这份最终 r3 core/session 候选作为 **candidate** 合入：唯一确认的 P2 已通过单锁状态修复并独立执行回归；其余范围未发现可证实的新增 P0-P3。实际提交/合入仍应按工程契约完成本批 final gate。full-gate-r2 已 exit 1/reap，唯一失败为 remote_only 架构 guard 未识别外置 local/tests.rs 的 test-only 归属，父补内层 cfg 标记而未改 guard。父报告 follow-up remote-only-r1 5/5 与 format-r8 通过，均非本评审独立执行；normal build 后 full-gate-r3 顺序运行中。full-gate-r1 曾被 SIGTERM/reap（exit -15、360 秒），亦不能记为通过；此前 Clippy 结果不代签最终完整门禁。

direct PTY 握手验收继续 **OPEN / PARTIAL（HOLD）**，包括当前本机 0.13.1 和 Debian 0.12.21rc 证据。候选状态不能写成真实 lrzsz 全兼容或功能已完成。单文件完整保存与 batch Finished 独立报告，产品目标继续跟踪。

macOS：本评审确实运行所列 local/backend/ACL 专项，但未执行原生 GPUI/窗口、IME、辅助技术、安装包或发行验收。Linux：cfg 和文件后端源码已审；父的远端 Debian peer、17 项 archive ACL 和独立 reference 检查不等于 Linux 桌面或 Linux 本地 Zmodem 发布后端的独立验收。Windows：安全本地后端仍显式 Unsupported，本评审未进行 Windows 构建、运行或原生验收。上传的 peer 仍控制远端目录和覆盖策略，协议完成不能证明远端落盘路径、无覆盖或保存后完整内容。

