**Windows platform boundary：r3 最终独立差分复核**

结论为 **APPROVE，范围限定 SOURCE_CONTRACT**。
开放发现为 **P0=0、P1=0、P2=0、P3=0**，本轮没有新增阻塞项或 notice。
**Windows native = NOT_RUN，Windows 测试执行数为 0，不签 APPROVE_NATIVE。**
该结论只绑定下述 r3 九项冻结；r1 历史 HOLD、r2 历史源码批准及其原件分别保留。

输入清单为
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/platform-inputs-r3.json`，
实际 SHA-256 与父指定值一致：
`c927ff6a42813bc2824133fdb6c00121234401c1857f34a7ec55651806f0813a`。
开始和收尾均核验 **9/9 匹配、0 漂移、0 额外 platform 文件**。
清单和完整源码字节保存在 `platform-inputs-r3.original.json`、`frozen-platform/`。
baseline `a092dd5712c0a92ee4af18970fb0500fb230b760` 仍只是输入声明，没有通过 Git 认证。

本轮继续应用已读取的 AGENTS、HANDOFF、ADR 0138、STRATEGY、SHARED_TARGET，
以及 rust-best-practices / rust-engineer 的所有权、FFI 与测试指导。
源码审查和编译仅覆盖 `crates/keelshell-platform/`。未修改生产源码、运行 fmt、
使用 Git、访问父 target、重复全仓 gate，或创建子 agent/chat/worktree/container。
所有 reviewer 写入均在独占的 `review-platform/` 内。

**r2 → r3 差分与行为。**

只有三个文件变化，其余六项与 r2 完全同哈希：

| 原始绝对位置 | 本轮核对及影响 |
| --- | --- |
| [path.rs:11](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-platform/src/path.rs:11) | 新公共纯函数 `filename_alias`，函数体是原 Windows `alias` 的逐字符 uppercase → lowercase 算法直接抽取。无文件访问、native 调用或 capability 构造，结果仍为 String。 |
| [path.rs:126](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-platform/src/path.rs:126) | 新增真实宿主测试覆盖 New/new、STRASSE/straße、σ/ς，并用两个不同名字作非别名控制；本轮实际执行通过。 |
| [lib.rs:10](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-platform/src/lib.rs:10) | 公共导出不受 Windows cfg 限制，调用方可在各宿主采用同一关系。 |
| [windows/mod.rs:334](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-platform/src/windows/mod.rs:334) | 原 alias 仅委托 `crate::filename_alias`。require_absent / require_unique_name 的算法关系保持一致。 |

上述原始路径对应本次冻结字节；稳定快照位于本目录
[path.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r3/frozen-platform/crates/keelshell-platform/src/path.rs:11)、
[lib.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r3/frozen-platform/crates/keelshell-platform/src/lib.rs:10)、
[mod.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r3/frozen-platform/crates/keelshell-platform/src/windows/mod.rs:334)。
精确差分保存为 `diff-r2-r3.patch`，累计 r1 差分为 `diff-r1-r3.patch`。
累计相对 r1 共五个文件变化，另四项保持原哈希。

新 rustdoc 明确把此函数界定为便携名字排除策略，没有宣称它是 Unicode normalization
或卷的原生文件名比较算法。本轮确认的是平台新入口与原平台关系一致、公开入口及其
两组非 ASCII 回归有效。session 的 global claim、busy/quarantine、SSH controller、
Zmodem、archive 调用接入仍由对应 session 审查负责；不把本平台报告用作 WS-ZM-01
端到端关闭的独立证明，也没有审查该范围外的源码。

**原两项 P2 的 r3 状态。**

两项都继续保持 **CLOSED_SOURCE_CONTRACT**，未被 alias 抽取改变：

| 历史发现 | r3 字节证据及保留边界 |
| --- | --- |
| R1-P2-01：保留 private target 的 rename sharing 冲突 | [native.rs:132](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r3/frozen-platform/crates/keelshell-platform/src/windows/native.rs:132)、`:154` 的长期 directory READ / SHARE_ALL；[native.rs:319](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r3/frozen-platform/crates/keelshell-platform/src/windows/native.rs:319) 的 held-object 同步短期 DELETE renamer；[mod.rs:245](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r3/frozen-platform/crates/keelshell-platform/src/windows/mod.rs:245) 的调用及 RAII 生命周期均与 r2 相同。原 private 文件 access/share 分支不变。 |
| R1-P2-02：rename buffer 小于官方长度契约 | [native.rs:357](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r3/frozen-platform/crates/keelshell-platform/src/windows/native.rs:357) 的 sizeof + exact name bytes、全初始化 roundup extent 和 `:385` 实际传入长度保持 r2 修复。 |

native.rs、安全实现及两份 Windows 测试文件与 r2 同哈希。
ReOpenFile 的 ABI、flags、同步 IOSB/buffer 生命周期、held identity/private/local-volume
复查和句柄错误路径的详细结论见
[r2 报告](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/report-r2.md)。
SID/ACL 的 LocalAlloc 对齐、bounds、RAII、protected owner-only 创建 DACL 和句柄读回、
metadata 128-bit identity、逐 component no-reparse、独立目录 cursor、bounded records
的原始审查证据在
[r1 报告](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/report-r1.md)。
本次没有新增 unsafe、改变 Windows native ABI，或扩大外部 File 注入 capability 的入口。

r2 的限制继续适用：ReOpenFile 使用 NtCreateFile 来源对象的实际互操作、Windows
sharing/rename 行为、remote/reparse/drive-map 实物和竞争均没有原生执行证据；
writer/delete 负控没有断言具体 Windows 错误码，并发枚举测试没有强制多批交错。
本次批准不超出这些源码契约前提。

**实际独立验证。**

本次开始时 `r3/target` 不存在，三条命令均显式使用该绝对路径，并设置同值
CARGO_TARGET_DIR、禁用增量编译、offline / locked；没有复用 r1/r2 已删除的 target。

```text
cargo test --offline --locked -p keelshell-platform --all-targets --target-dir /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r3/target -- --list
cargo test --offline --locked -p keelshell-platform --all-targets --target-dir /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r3/target -- --test-threads=1
cargo clippy --offline --locked -p keelshell-platform --all-targets --target x86_64-pc-windows-msvc --target-dir /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r3/target -- -D warnings
```

| 原始回执 | 实际结果 |
| --- | --- |
| `host-parser-inventory.json/.txt` | exit 0，1.758443s；fresh 编译 harness，列出恰好 16 个源测试名，包含新增 alias 测试。 |
| `host-parser-tests.json/.txt` | exit 0，0.104879s；**16 passed / 0 failed / 0 ignored / 0 filtered**。 |
| `windows-cross-clippy.json/.txt` | exit 0，1.213060s；Windows x64 MSVC all-targets、`-D warnings`，无警告。 |

通用库存为 **9 path/alias + 7 records = 16**；Windows 条件库存保持
**19 platform + 27 security = 46**。共 **62** 个源测试函数经过 Windows 条件类型检查，
其中 **0** 个在 Windows 执行。库存、名字及绝对原始路径对应的行号见
`test-source-inventory.json`。没有把 Clippy 或类型检查算作原生运行。

本轮工程验证命令计数 **3 Cargo / 0 rustc，全部 exit 0**。
三轮累计 **11 条：7 Cargo + 4 rustc**，其中 r1 的两条编译负例按精确诊断码判定通过；
r1 宿主 15 项与 r3 宿主 16 项分别记录，不能据此声称有 31 个不同的通用测试。
既有四条环境观察另计，本轮未重复查询或运行整仓 gate。
所有命令都有有界超时、实际 wait/reap 以及 output SHA-256 回执。

父提供的 `platform-pe-r1.json` 已另存为 `parent-platform-pe-r1.original.json`：
其中 61 个描述符名称与保存的 **r2** 库存完全一致。
回执声称的 AMD64 PE cross-link / 字节扫描保留为父提供证据；本 reviewer 没有打开父
target、读取该 PE 或执行它。它不是 r3 的 PE 证明，也不是 native `--list` 或运行结果。
详细区分见 `pe-receipt-inspection.json`。

**冻结、历史原件与清理。**

`freeze-before.json` / `freeze-final.json` 保存九项开始和结束的精确摘要；
`compiler-artifacts.json` 保存构建产物摘要，并在删除前复制、核验 18 份 platform
dep-info / fingerprint；`cleanup.json` 记录仅删除本目录独占 target 的路径、
文件数、字节数和删除后状态：已移除 30,320,405 字节，target 确认不存在。
r1/r2 的两份历史 evidence manifest 及所列文件已重新核验，
均保持原哈希；没有改写历史报告或原件。

`result-r3.json` 保存机器可读结论及命令/测试计数，`evidence-manifest.json`
列出 r3 保留文件的最终大小和 SHA-256。
