# Windows session 非作者复核：Zmodem r1

审查结论：**HOLD / REQUEST_CHANGES**。本范围新增开放发现为 **P0=0、P1=0、P2=1、P3=0**。Windows **Native NOT_RUN**。这是冻结 Zmodem 源码及接入的只读结论，不签署父级 crosscheck、完整工程门禁、Windows 原生文件操作、桌面交互或独立 rz/sz 互通。

记录时间：2026-11-15T12:57:27.794992+00:00。主机 Darwin arm64。先完成 Zmodem；archive 尚未审查，等待父追加冻结后另编号。

## 输入与边界

- cwd：`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。
- HEAD：`a092dd5712c0a92ee4af18970fb0500fb230b760`；分支 `feature/editor-unicode-codec-20261114`。直接读取 `.git` 指针、HEAD 与 ref 元数据，**未运行 Git 命令**。HEAD 是基线；当前候选是未提交文件，不能称为该 SHA 的已提交实现。
- 冻结清单：`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/zmodem-inputs-r1.json`。
- 冻结清单 SHA-256：`8fb6ea161f2ce4d4ba0d882901ff867b9cf44f1cfcb19138af6d088a9ed01075`。
- 6 项入口、收口前均逐项匹配；12 项必要依赖上下文的前后 SHA-256 相同。依赖上下文不属于这份 6 项冻结，其实际摘要另见 static/readback JSON。
- 已读 AGENTS（含父目录规则及 RTK）、HANDOFF 当前批次、ADR0138、STRATEGY、SHARED_TARGET；已使用 `rust-best-practices`，重点为所有权、typed Result、状态保留和真实行为测试。
- 仅在本 `review-session-windows/` 新增报告与日志。Cargo、rustc、格式化、Git 命令、父 target 读写均为 **0**；没有运行或复制预构建测试，没有创建 agent/chat/worktree/container，没有修改源码或主文档。
- 平台 reviewer 的两项 P2（private target directory sharing、rename buffer minimum extent）是用户已给定的独立前置 HOLD，不重复计为本报告发现，也没有替其签署修复或运行结果。

| 冻结项 | SHA-256 |
| --- | --- |
| `crates/keelshell-session/src/zmodem.rs` | `b61d348d414995e328ea6844f38d71408dd3251d2b726babe2fcb824abf15bff` |
| `crates/keelshell-session/src/zmodem/local_windows.rs` | `68ebb6fc05cf61c7b3fd8a64138038c981c03a469d99f7a9c6ca7456ba0f842f` |
| `crates/keelshell-session/src/zmodem/local_windows/tests.rs` | `125a148b0ea76d8483411c30e9daa08566534c5152f193e39f62ddd2320d48e3` |
| `crates/keelshell-session/Cargo.toml` | `61e707d92be30616f67f5171034d99c70168edb55167313bcb5d67b31b4eac3f` |
| `crates/keelshell-app/src/terminal_zmodem.rs` | `1db9316e87b83db1ed04ab1468c59a69a7d469c88ed76e6edb1faf0b9d973c54` |
| `crates/keelshell-session/tests/zmodem.rs` | `67a196d6a429258f13e137919d2dece286704746ff6a74a20a5e1b450c1ed8de` |

## 开放发现：WS-ZM-01 / P2 — 别名可避开 busy 与 quarantine 的路径匹配

**位置：**

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem.rs:1026`：Windows 接入继续调用共享 `ZmodemLocalReservation::acquire`，之后才 `receiver.accept()`。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/zmodem_local.rs:19`：目标和 staging 都经 `local_claim` 建立 process-wide claim。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/queue.rs:151`：已有祖先 canonicalize 后，canonical components 与 missing basename 均只 `to_lowercase()`，关键调用在 153、159 行。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/queue.rs:99`：冲突依赖 components 的相等与 prefix 相等。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem/local_windows.rs:93`：本地命名空间的 portable alias 却先 uppercase 再 lowercase。平台 `require_absent/require_unique_name` 使用相同的更强关系；本地测试 102 行已明确将 `straße.bin` / `STRASSE.BIN` 视为冲突。

**触发：**同一已有目录中的两个目标均缺席时，连接 A 确认 `STRASSE.bin`，在目标尚未发布期间，连接 B 准备并确认 `straße.bin`。A 的目标 claim 末项为 `strasse.bin`，B 为 `straße.bin`；不同随机 staging 名也不冲突，`whole_side` 在目标缺席时为 false。只要没有其他限额或冲突，B 可以被 admit。A 的 claim 后续若变成 Unknown，而目标依然缺席，同样的别名仍可避开 quarantine 匹配。`σ.bin` / `ς.bin` 也属于相同类别。

**影响：**第二个请求可继续接受数据、创建 staging，绕过了本后端自己定义的目标别名保留边界。原 Unknown 记录仍存在；原子 no-replace 和最终枚举可能继续拒绝发布。因此本发现不声称已经覆盖旧文件、清除旧隔离、成功完成 Windows exploit 或真实运行复现。

**证据性质：**对生产函数和实际共享 registry 比较关系的静态调用链分析。共享 `queue.rs` 是已有依赖；本次启用 Windows 的接入路径让它成为必须复核的边界。不是平台那两项 P2 的重复范围。当前 1150 行的跨连接 integration 用例使用相同拼写，未覆盖此别名。

**修复与复核建议：**process-wide admission 与 quarantine 对所有本地 components 使用与该后端一致、保守的 alias 关系，同时保留 canonical ancestor、现有 Unknown 原件及不确定状态。补充真实 production reservation 的跨连接测试：第一种拼写占用时，另一种拼写在 accept 前收到 DestinationBusy；真实 mutation 后保留未知 reservation 时，别名收到 DestinationQuarantined。父形成新冻结后做差分复核；不以 Python 镜像命名空间或计数测试补签 Windows 行为。

阻塞 notice 已先行新建：`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-session-windows/notice-zmodem-r1-p2-01-unicode-reservation-alias.md`。本报告不将“文件已落盘”表示为父已读取或已确认接收。

## 其余要求的源码观察

| 要求 | 具体源码观察与边界 |
| --- | --- |
| 完整祖先与 identity/ACL/bytes | `local_windows.rs:29` 的 Source 保留真实 File；`:47` 的 Parents 保留 drive root 和全部 Directory。`:125` 逐组件打开并保存 stamps，`:141` 同时比较重新解析链与保留句柄。`:159` 在长度预算内读完整内容、检查读前后 FileInfo、重新打开名字绑定 identity；FileInfo 包含完整 volume/file ID、link/attributes、created/modified/changed 与 owner/group/DACL observation。未发现本层仅凭路径或长度/时间替代内容核验。 |
| source confirm 整批 recheck | `local_windows.rs:263` 消耗一次 plan，每个来源重新核验父链、完整 stamp、review 字段、bytes 与旧保留句柄；全部成功且最终取消检查通过后，`:290` 才构造 SendFile。Controller `zmodem.rs:967` 只有确认成功才启动 Sender；任一来源失败不返回部分批次。这是逐项完成后整批释放，不是外部文件系统的原子快照保证。 |
| 路径与预算 | `local_windows.rs:100` 在打开前使用 WindowsPath/parser 与协议 basename 验证；`:167` 在读取前限制单文件和剩余批次预算，增长越界被拒绝；`:230` 限制 32 文件、64 MiB/文件、128 MiB/批次，用 checked_add 总计，并拒绝 portable 名称别名和重复 identity。路径 parser 的 UNC/设备/ADS/DOS/控制字符/尾随空格点/UTF-16 component/4096 UTF-8 path 字节限制只作为依赖上下文核对。共享 reservation 的别名不一致仍见 WS-ZM-01。 |
| retained stage 与 private/no-replace | `local_windows.rs:342` 首先设置 mutated，再尝试创建。生产 create 带 private DACL 且首次写入前读回；`:471` 分块写、flush/sync_all、完整内容与 private metadata 回读。`:375` 使用原 PrivateEntry 发布；命名空间观察句柄 `named` 明确在 rename 前 drop，`:405` 调用 retained-handle no-replace。没有临时关闭写句柄再依名字认领另一个对象的流程。平台 syscall 的独立发现仍不在本报告签署内。 |
| 发布后读回 | `local_windows.rs:412` 校验新目标 identity/size 与保留创建对象一致，再检查 private DACL、逐字节读回、前后元数据、父链、唯一别名、旧 staging 缺席、最终命名对象与保留对象及取消状态。只有返回 Saved 后 `zmodem.rs:1124` 才产生 FileSaved，之后才 acknowledge_saved。错误不会从这个路径产生保存回执。 |
| mutation/取消/丢弃 wait | `zmodem.rs:1201` 在 spawn 前创建 PublicationTicket 并登记 hold，ticket 进入真实 worker。`:1274` StopOnDrop 标记取消；RunningOnDrop 只在 worker 终止时清 running；`:1381` mutated result 被 drop 时按未知完成。`:1313` unresolved 单调保留，后续成功或 unmutated drop 不清旧 unknown。Windows 后端没有删除或自动重试调用。路径 alias 绕过是 claim 匹配问题，不是这里把 mutated 清成 false。 |
| UI cfg/成功语义 | `terminal_zmodem.rs:630` 明确启用 Windows；`:558` 使用 USERPROFILE 和绝对路径缺省。只处理 FileSaved 的分支登记 verified_saves；Finished、缺 farewell、Cancelled、Failed 不制造保存回执。`:580` 风险确认只处理窗口关闭，检查 worker/epoch，并不清 backend quarantine。UI/bridge 仍需要原生验收。 |
| integration cfg | 17 个正向 cfg 和 1 个反向 fallback cfg 均包含 Windows；平台依赖位于 Cargo.toml 的 cfg(windows)。这是源码条件检查，不是 Windows 编译或动态库存验收。 |

以上具体源码的完整绝对路径以表格前的冻结项为准；关键接受/拒绝路径的绝对行号同时保存在 notice 与静态检查记录中。

## 测试 hook 是否调用真实 native 对象

`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem/local_windows/tests.rs:1` 限定 test + windows 并 `use super::*`。`:25` 的 staged helper 只串联实际 `Destination::revalidate`、`create_staging` 和 `write_staging`，返回实际 Directory、PrivateEntry、FileInfo；生产模块没有 test-only 替代后端。取消后 staging/target、identity replacement、hard link、no-replace、读回和目录枚举断言落在真实 tempfile 与平台 API 对象上。源码检查未见镜像 Windows 文件系统实现。

这些是**将来在 Windows 执行时会走真实对象的测试定义**，本次没有执行，不能据此声称 native 通过。两个 symlink privilege 用例显式 ignored。

`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem/tests.rs:63` 的 PublicationTicket helper 刻意使用 `ticket: None`，验证的只是生产 PublicationHold 的内存计数与 sticky unknown；它没有真实 reservation 或 native 文件，不能作 Windows quarantine/cancel 成功证据。相邻 worker barrier 测试走实际 spawn_blocking/Drop，但未进行 Windows 文件发布。当前冻结 integration 文件的 source inventory 为 26 项，其中 25 项为 Windows 源码候选，1 项 Unsupported fallback 在 Windows 排除；不能当成 compiled inventory。

`tests/support/zmodem_fixture.rs` 使用真实 loopback SSH 身份，codec wire 使用项目自己的实现；SFTP Namespace 仅提供 reservation 所需只读响应，不是 Windows local backend，也不证明独立 peer 或操作系统 SSH 服务互通。本次未运行此 fixture。

## close-time timestamp 与分享模式

源码重点核对：`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/zmodem/local_windows.rs:471` 在写后 capture stamp，保持原 writable PrivateEntry 存活到发布及完整读回结束；不把写前 modified/changed 当写后固定值。`:427` 明确允许 rename 改时间，使用重开的目标观察与仍存活的创建对象重新匹配。最终 Saved 只包含 path/bytes/hash，不返回 pre-close timestamp。

微软当前官方文档说明 last-write 完整更新时间依赖写句柄关闭；因此不要求最终关闭后的时间戳等于写句柄仍开着时的 stamp。这里没有发现必然跨写句柄关闭去做旧完整 stamp 相等比较的本层缺陷。不同句柄实时观察值的一致性、延迟时间戳和 filesystem 差异仍必须 Windows 原生验证，不能仅从 `sync_all` 推断。

分享模式的静态核对：regular staging 的原句柄请求 read/write/delete、只 share read；每个 `open_file` 观察请求 read、share all。因此按 documented 的双向兼容关系，观察 handle 的 access/share 与保留写句柄一致；`:402` 还在 rename 前释放名字观察句柄。session 没有调用需要另开 writable handle 的 `open_private_entry`。此结论是 access mask 与调用链审查，**不是实际 NT open/rename 成功回执**。private target directory sharing 的另一个已知平台问题继续由其 reviewer 处理。

官方 API 观察通过只读 HTTPS GET 保存于 `zmodem-r1-official-api-observations.json` 和 `zmodem-r1-official-share-observations.json`：File Times、WriteFile、NtCreateFile、CreateFileW 共 4 份 HTTP 200；首次错误 DDI 路径的 HTTP 404 也保留。没有在线结果或宿主 parser 被计为 Windows 运行。

## 命令结果、原件与建议

详见 `zmodem-r1-command-results.log`、入口 intake JSON、静态检查 r1/r2 JSON 和收口前 readback JSON。只读源码检查最终为 **22 MATCH / 0 MISMATCH**，含 6 项摘要检查；不是 22 个 Rust 测试。首版审查脚本把正向 cfg 次数误填为 18，得到 21 MATCH / 1 MISMATCH；实际为 17 正向与 1 反向。r2 改成枚举并核验每个相关 cfg 均含 Windows，保留 r1，不改源码、不复跑门禁。

| 静态测试定义库存 | 默认候选 | 显式 ignored/排除 | 本次实际执行 |
| --- | ---: | ---: | ---: |
| Windows local tests 45 项 | 43 | 2 privilege ignored | 0 |
| integration 文件 26 项 | Windows 候选 25 | 1 other-platform fallback excluded | 0 |

Rust 单元/集成、rustdoc、Windows native、GPUI、独立 rz/sz peer、本批全门禁均 **NOT_RUN by this reviewer**。父正在进行的 x64 MSVC crosscheck 未查看、未代签，唯一 target 未触碰。历史 HOLD、失败、修复前原件与平台两项 P2 不变。archive 模块及其 UI 接入尚未复核，等待追加冻结。

**建议：修复 WS-ZM-01、形成新冻结并差分复核以前，保持 session 集成 HOLD。** 平台前置发现、父工程门禁与真实 Windows 原生验证各自按实际证据收口；本报告不以源码观察解除任何原生/互通验收限制。
