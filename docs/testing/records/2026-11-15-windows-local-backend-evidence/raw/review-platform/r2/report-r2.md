**Windows platform boundary：r2 独立差分复核**

结论为 **APPROVE（仅 SOURCE_CONTRACT）**，开放发现 **P0=0、P1=0、P2=0、P3=0**。
r1 的两个 P2 在本次冻结字节中关闭；r1 历史结论继续为 HOLD，不回写成通过。
**Windows native = NOT_RUN**。本结论不签 APPROVE_NATIVE，也不代表 Windows 测试执行通过。

审查及编译范围仍仅为 `crates/keelshell-platform/`，沿用本轮已读取的
AGENTS、HANDOFF、ADR 0138、STRATEGY、SHARED_TARGET，以及
rust-best-practices / rust-engineer 的所有权、FFI、错误处理与测试指导。
没有源码编辑、fmt、Git、父 target、全仓 gate、子 agent/chat/worktree/container。

冻结输入为
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/platform-inputs-r2.json`，
SHA-256 `567a74e6bfccdb8b9bc846e30ed613ccd1051fc90caffa5e6a35e3a525143c81`。
9/9 文件在开始时和收尾时匹配，完整字节已保存于本目录 `frozen-platform/`。
相对 r1 仅 `windows/mod.rs`、`windows/native.rs`、`windows/tests.rs` 改动；
其余 6 项保持原哈希。精确差分为 `diff-r1-r2.patch`。
baseline 字符串沿用输入声明，没有以 Git 认证。

**R1-P2-01：关闭于源码契约层。**

原始修复位置为
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-platform/src/windows/native.rs:132`
（directory 不再请求 WRITE/DELETE）、同文件 `:154`（directory share ALL）、
`:319`（held-object renamer），以及
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-platform/src/windows/mod.rs:245`
（短期 lease 的调用及生命周期）。
稳定快照见
[native.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/frozen-platform/crates/keelshell-platform/src/windows/native.rs:319)
与
[mod.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/frozen-platform/crates/keelshell-platform/src/windows/mod.rs:245)。

长期目录句柄使用 `FILE_GENERIC_READ`，不是 generic 位本身，但已展开为等价文件读权限，
包含 SYNCHRONIZE，不含 WRITE_DATA、APPEND_DATA 或 DELETE。创建和重开 private directory
都得到此读 capability，share READ|WRITE|DELETE。创建时 owner-only protected DACL
的构建、传入系统调用及句柄读回路径没有改动。

微软 MS-FSA 产品行为注 185 明确说明，在 Windows 11 / Server 2022 及更早版本，
rename 内核目标目录 open 不 share DELETE。r2 的长期目标句柄没有 DELETE，
并允许所需写访问，消除了原有两个方向的 sharing 冲突。该官文证据仍保存在
[r1 note 185 原件](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/sources/ms-fsa-product-behavior.txt:1387)；
本轮没有把它升级成 Windows 实测结论。

目录 source 通过 `ReOpenFile` 获得临时 `FILE_GENERIC_READ|DELETE`、share READ|WRITE
句柄。它以原 held handle 为输入，调用中保活原句柄，不重建路径，也不替换原 capability。
对失败的 INVALID_HANDLE_VALUE/null 不接管，对成功句柄仅 `File::from_raw_handle` 一次；
其后 local-device、完整 `FileInfo` equality 或 private security 检查的任何错误都由 RAII
关闭新句柄。完整复查包含 128-bit identity、volume、kind、links、attributes、
creation/write/change times 及 security observation；metadata 继续拒绝 reparse、
delete pending 和不合要求的文件链接。临时 DELETE lease 活过 NtSetInformationFile 返回，
随后随局部 Option 关闭。

| 组合 | 已检查的双向 sharing 关系 |
| --- | --- |
| 长期目录 READ / share ALL 与内核目标 WRITE / share READ|WRITE | 旧句柄允许新 WRITE；新 open 允许旧 READ；目标上没有旧 DELETE。 |
| 长期 source 目录 READ / share ALL 与临时 READ|DELETE / share READ|WRITE | 原句柄允许新 DELETE；临时句柄允许原 READ。 |
| 原 private file READ|WRITE|DELETE / share READ 与独立 reader / share ALL | 两侧兼容，允许内容读回。 |
| 原 private file 与独立 WRITE 或 DELETE opener | 新 access 被原 share READ 拒绝。文件创建分支的 access/share 未改。 |

这些是对代码与官方 sharing 规则的静态核对，不是执行所得的 Windows NTSTATUS。
上层仍负责名字/祖先链、完整发布结果与 mutation reservation；长期目录 share ALL
本身不提供整个准备阶段的 namespace 排他性。

**ReOpenFile flags、ABI 与同步生命周期。**

实际采用 windows-sys 0.61.2 已生成的 kernel32 `system` ABI，参数依次为 HANDLE、
三个 32-bit 值，返回 HANDLE，没有自行声明 FFI。绑定路径、哈希、签名和实际常量见
[binding-evidence.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/binding-evidence.json)。
本轮 fresh Windows x64 MSVC strict all-targets Clippy 已重新检查这些调用。

传入 flags 为 `0x02200000`，只有 BACKUP_SEMANTICS 与 OPEN_REPARSE_POINT。
二者都列于官方 ReOpenFile 可接受 flags；前者允许目录句柄，后者不进行普通 reparse
处理。没有 FILE_ATTRIBUTE_*、OVERLAPPED 或 DELETE_ON_CLOSE。ReOpenFile 文档
把 OVERLAPPED 定义为异步操作开关，CreateFile 文档明确说明该开关决定同步/异步模式。
本次返回句柄按非 OVERLAPPED 同步文件对象使用，FILE_GENERIC_READ 含 SYNCHRONIZE；
local_device 的栈 IOSB/buffer 与最终 rename IOSB/buffer 均活到对应同步调用返回。
没有将任意外部 File 接入这条 NT helper 路径。

官方原件见
[ReOpenFile 参数与 sharing](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/sources/reopenfile.md:65)、
[允许目录 flag](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/sources/reopenfile.md:152)、
[reparse / OVERLAPPED](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/sources/reopenfile.md:233)、
[同步/异步说明](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/sources/createfilew.md:1014)。
URL 与取回摘要见 `sources/fetch-reopen.json`。

文档对 hOriginalFile 的措辞使用 CreateFile；这里来源是内部同步 NtCreateFile
的文件对象句柄。对该互操作路径以及 flags 的实际内核归一化没有 Windows 运行证据。
这项限制与所有 publication 正控一样保留在 native NOT_RUN 范围；本轮批准的是已检查的
源码、ABI 与上述同步/共享契约前提，不声明所有文件系统或驱动组合已经得到验证。

**R1-P2-02：关闭于源码契约层。**

原始修复位置为
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-platform/src/windows/native.rs:357`
（sizeof 加 exact name bytes）、`:360`（全初始化 roundup allocation）、`:385`
（传完整 extent）。稳定
[r2 快照](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/frozen-platform/crates/keelshell-platform/src/windows/native.rs:353)。

required 现在按 `sizeof(FILE_RENAME_INFORMATION) + name_units*2` checked-add；
机器字对齐的 `vec![0usize; …]` 提供完整初始化 allocation，传入长度为全部存储字节数。
FileNameLength 仍只报告准确名字节数，ReplaceIfExists=false，RootDirectory 为实际 held
目标，UTF-16 tail 在足够且对齐的存储中复制。公开入口先限制名字为 1..255 个 UTF-16
单元，因此长度乘法及 u32 转换在该边界内有界。

r1 已有独立 Windows x64 编译期断言：sizeof=24、FileName offset=20；相同 ABI 依赖
未变。本次 1 单元名字 required=26、实际传 32；255 单元 required=534、实际传 536，
均覆盖文档最小值并只传已初始化存储。没有重新运行相同布局编译器探针充数。
原官方长度契约见
[FILE_RENAME_INFORMATION](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/sources/file-rename-information.md:179)。

**新增和修改测试的有效性。**

| r2 Windows 测试 / 绝对原始位置 | 检查内容与判断 |
| --- | --- |
| [private target composition，:174](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/frozen-platform/crates/keelshell-platform/src/windows/tests.rs:174) | 精确保留 private target、as_directory、发布 file 与 directory；核验两者 identity、target private DACL 和旧名缺席，实际覆盖原 P2 组合。 |
| [file 名字边界，:228](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/frozen-platform/crates/keelshell-platform/src/windows/tests.rs:228)、[directory 名字边界，:233](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/frozen-platform/crates/keelshell-platform/src/windows/tests.rs:233) | 分别对 file / directory 发布 `"x"` 与 `127×🦀 + x`，后者准确为 255 UTF-16 单元；核验 identity、唯一拼写及旧名缺席。 |
| [文件 sharing，:238](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/frozen-platform/crates/keelshell-platform/src/windows/tests.rs:238) | 同一已存在、可读对象，先读回内容，再分别 WRITE open、DELETE-only open 与 remove_file 拒绝；DACL 读回仍通过。 |
| [raw directory no-replace，:154](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/frozen-platform/crates/keelshell-platform/src/windows/tests.rs:154)（原有测试修改） | raw directory no-replace 使用真实 DELETE lease；拒绝覆盖后在同一 lease 上成功发布到空名字并验证 identity，排除“所有 rename 都因缺权限失败”的假负控。 |

所有上述原始位置的完整绝对路径为
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-platform/src/windows/tests.rs`；
稳定快照为
[tests.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/frozen-platform/crates/keelshell-platform/src/windows/tests.rs:154)。

还核对了当前实际 Rust 1.100.0 commit 对应的标准库源码：
`OpenOptions::access_mode(DELETE)` 在 `get_access_mode` 中直接返回自定义 access，
不会因为未设置 read/write 而在 Rust 层报 InvalidInput；默认 disposition 是 OPEN_EXISTING，
默认 share ALL。这条负控会进入真正的 Windows open。原件为
[当前标准库实现](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/sources/rust-openoptions-access-mode.md:265)。
测试中的 `.is_err()` 没有断言精确 Windows 错误码，因此仍不把它描述为已观测到
ERROR_SHARING_VIOLATION；原生执行也仍未发生。

纯路径/目录记录 parser 与安全实现字节没有变化，沿用 r1 的已检查结果。
r1 中关于 remote/reparse/drive-map 实物、并发枚举未强制多批交错的原生覆盖限制继续存在；
没有因此新增或隐藏 P3。

**独立验证、冻结与资源。**

r2 新增工程验证只有 **1 条 Cargo 命令**，exit 0，实测 wall time **3.939433s**：

```text
cargo clippy --offline --locked -p keelshell-platform --all-targets --target x86_64-pc-windows-msvc --target-dir /Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/r2/target -- -D warnings
```

该 target 开始时不存在，是全新独占输出目录。完整命令、时间、exit 和原始输出见
`windows-cross-clippy.json/.txt`；不是父的 r3 回执，也不是 r1 已删除 target 的缓存结果。
本轮额外有 1 条有界文档取回 wrapper，3 个 primary 文件请求，exit 0，
不计工程命令或测试数。编译 dep-info/fingerprint 与产物摘要在删除前另行保存。

条件库存为 19 platform Windows + 27 security Windows = **46**，另有 **15 通用**
（8 path + 7 parser），合计 **61** 个测试函数经过 Windows 条件类型检查；
**Windows 测试实际执行数为 0**。库存与行号见 `test-source-inventory.json`。
宿主 15 passed 是 r1 对同哈希通用输入的实际执行，本轮没有重复执行并冒充新增通过。

合并两轮工程命令总数为 **8：4 Cargo + 4 rustc**；
其中 r1 两条 rustc 是诊断码符合预期的编译负例。
环境观察 4 条另计，原生行为执行始终为 NOT_RUN。

`freeze-final.json` 记录结束时 9/9 与 r2 冻结相同、无额外 platform 文件；
`cleanup.json` 记录保存并验证 12 份 compiler evidence 后仅删除本目录的独占 target，
移除 12,835,449 字节产物；
`evidence-manifest.json` 列出本 r2 目录保留文件摘要。
历史 r1 的原字节、两个 P2、三个 notice、命令和清理证据在上一级单独保留。
