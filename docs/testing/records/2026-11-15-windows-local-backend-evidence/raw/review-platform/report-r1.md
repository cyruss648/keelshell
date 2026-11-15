**Windows platform boundary：新的非作者独立复核，r1 历史冻结**

结论为 **HOLD**。开放项为 **P0=0、P1=0、P2=2、P3=0**。两个 P2 均属于
publication 的 Windows API 契约与测试覆盖问题；其余已检查的 unsafe 所有权、
同步调用、内存生命周期及访问控制源码边界没有发现新的阻塞项。
**Windows native = NOT_RUN；本报告不签 APPROVE_NATIVE。**

审查对象仅为 `crates/keelshell-platform/`。原始冻结为
`work/windows-local-backend-20261115/platform-inputs-r1.json`，9 项，清单 SHA-256 为
`c10ab180854098fbe636f7c57805b7e10ad4c9498043918f06ea655d4bb34954`。
清单所记 baseline `a092dd5712c0a92ee4af18970fb0500fb230b760` 仅作为输入声明保存；
本轮没有运行 Git 来重新认证该提交。

开始时 9/9 当前文件与冻结逐字节摘要匹配，platform 没有未列出的文件。
验证实际编译这些输入，并使用全新、独占的本 review 目录下 target。
父随后明确接受 notice，授权开始修改平台并将在后续提供 r2。因此本结论只签历史 r1；
末次活动源码与 r1 的差异另存 `freeze-final.json`：3 项发生父授权的 r2 改动
（`windows/mod.rs`、`windows/native.rs`、`windows/tests.rs`），其余 6 项相同。
这些后续修改不纳入本轮批准；r2 另有独立差分报告。
原 9 项已从开始时的编号快照恢复为原始字节，再逐项对原清单校验；
`frozen-platform-r1-binding.json` 记录 9/9 匹配，实际源码保存在 `frozen-platform-r1/`。

读过当前 AGENTS、`docs/HANDOFF.md`、ADR 0138、STRATEGY、SHARED_TARGET；
应用了 rust-best-practices 与 rust-engineer 的所有权、FFI、错误处理和测试指导。
没有修改生产源码、运行 fmt、使用 Git、运行整仓 gate、使用父 target、
创建子 agent/chat/worktree/container，或修改文档主文件。

**R1-P2-01：私有目标目录的共享模式与 DELETE access 均不兼容 rename 目标 open。**

原始绝对位置：
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-platform/src/windows/native.rs:131`
（创建 access），同文件 `:153`（仅 share READ），以及
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-platform/src/windows/mod.rs:263`
（`as_directory()` 复制原文件对象）。r1 快照可从
[native.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/frozen-platform-r1/crates/keelshell-platform/src/windows/native.rs:131)
及 [mod.rs](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/frozen-platform-r1/crates/keelshell-platform/src/windows/mod.rs:263)
读回。

触发是通过 `create_directory()` 保留私有目标目录，再 `as_directory()`，
最后将另一个保留的 private source 发布到该目标目录。目标的创建文件对象仍带
WRITE/DELETE access 且只 share READ；rename 的内核目标 open 需要写目录。
此外微软 MS-FSA 产品行为注 185 明确规定：截至 Windows 11 /
Windows Server 2022 的对应目标 open 只 share READ|WRITE，已有 DELETE-access
目标句柄会导致 sharing violation。因此仅增加目标创建句柄的 share WRITE，
甚至仅调整其 ShareAccess，均不能解决已有 DELETE access 的反向冲突。

影响是该合法 safe API 组合不能依照所支持 Windows 版本的官方契约完成发布。
不是覆盖写或内存越界结论。普通父目录通过 SHARE_ALL、无 DELETE 的只读 open
取得，现有成功发布测试只覆盖那条路径。缺少保留私有目标目录的确切正控。
`probes/public_composition_control.rs` 已以 Windows target 独立类型检查该公开组合；
这证明 API 接受组合，不是 Windows 行为执行。

证据为 [notice-01](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/notice-01-private-rename-target-sharing.md)
及 [notice-03 补充](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/notice-03-target-delete-access-official-confirmation.md)；
notice-03 不是第三个 P2。微软原始材料见
[MS-FSA rename](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/sources/ms-fsa-rename.txt:78)
与 [产品行为注 185](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/sources/ms-fsa-product-behavior.txt:1387)。
URL、原始 HTML 摘要与提取文本均已保留。
这是官方产品行为支持的源码契约结论；没有实际观察 Windows NTSTATUS。

**R1-P2-02：rename 传入的缓冲长度未满足官方最小长度，短名甚至小于结构本体。**

原始绝对位置：
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-platform/src/windows/native.rs:320`
（offset + filename bytes），同文件 `:348`（把该值传给系统调用）。
[r1 快照](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/frozen-platform-r1/crates/keelshell-platform/src/windows/native.rs:320)。

官方 FILE_RENAME_INFORMATION 文档要求信息缓冲至少为结构大小加文件名字节数。
r1 以 FileName 的 offset 计算传入长度。Windows x64 编译期断言独立确认
offset=20、sizeof=24，因此单个 UTF-16 单元的合法名字 `"x"` 会传入 22，
低于 24 字节结构本体，也低于文档要求的 26。底层 allocation 虽向机器字取整，
调用却没有把该完整长度传出。

触发是合法的一单元目标名；所有名字的当前长度也都低于上述文档公式。
影响是 publication 未满足 native API 的明确缓冲契约。没有把它表述为实际
STATUS_INFO_LENGTH_MISMATCH 或内存越界；当前实际 allocation 足以容纳结构，
Windows 运行结果仍为 NOT_RUN。

证据为 [notice-02](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/notice-02-rename-buffer-contract.md)、
[官方长度要求](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/sources/file-rename-information.md:179)
及 [Windows 编译期布局原件](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/windows-local-backend-20261115/review-platform/probes/rename-buffer-layout.json)。
需要满足文档长度并传递完整初始化后的实际 buffer extent，补一单元及最大 UTF-16
名字的 file/directory 发布正控。现有 `"published"` 正控没有覆盖这一边界。

**其余重点的检查结果。以下“通过”只指已检查的源码前提，不是原生验收。**

下表行号均对应保存的 r1；路径相对上述仓库根，仅用于缩短矩阵。

| 范围 | 证据位置 | 结论与边界 |
| --- | --- | --- |
| 同步 NT IOSB / buffer 生命周期 | `windows/native.rs:93-185,188-215,274-315,318-356` | Native opener 一律设置 SYNCHRONOUS_IO_NONALERT，GENERIC_READ 含 SYNCHRONIZE。名称、OBJECT_ATTRIBUTES、IOSB、descriptor、query/rename buffer 都活到同步返回。没有以 STATUS_PENDING 后立即释放 buffer 的异步入口。依据微软 ZwCreateFile 同步文件对象契约；仍依赖操作系统遵守该契约。 |
| 外部 File 无法注入同步 capability | `windows/mod.rs:99-133,175-195,251-267` | 构造器和字段私有；`as_file()` 只给共享借用；as_directory 只复制内部已有同步对象。外部 crate 的构造器负例实际产生 E0624，字段注入负例产生 E0451；同环境公开 API 正控成功，排除因依赖或 target 配错造成假负例。 |
| 外部 File 的 metadata 查询 | `windows/native.rs:218-271`；`windows/security.rs:309-373` | 使用同步 Win32 查询，没有把外部 File 交给借用栈 IOSB 的 NT helper，也没有从 metadata 构造 capability。 |
| LocalAlloc / SID / ACL / RAII | `windows/security.rs:73-115,163-293,309-373,428-529` | 固定 allocation 对齐并拥有稳定地址；descriptor 与 owner/ACL 同时保活。token 用 OwnedHandle，分配用 LocalFree RAII；两阶段 TokenUser 有容量上限和返回长度检查。SID 全长、ACL extent、活动 ACE header/size/cursor 均检查，原生结构用 unaligned copy，观察不包含指针、ACL free space 或 SACL。成功 Win32 readback 的有效结构及组件对齐仍是 FFI 前提，不接受任意外来指针。未发现 unsafe impl Send/Sync。 |
| 实际 held volume 拒绝 remote | `windows/native.rs:181-184,188-215` | 每个实际 open 后检查 FILE_FS_DEVICE_INFORMATION，要求磁盘且拒绝 REMOTE_DEVICE/VSMB；GetDriveType 只是前置筛选。public metadata 本身不授予本地来源 capability。没有 Windows remote-volume 实测。 |
| 单 component RootDirectory / no-reparse | `path.rs:22-52,70-102`；`windows/mod.rs:108-121,145-158`；`windows/native.rs:112-127,159-165` | 路径和单名称先验证；后代相对 held parent 打开，DONT_REPARSE + OPEN_REPARSE_POINT，metadata 再拒绝 reparse attribute。root 保留 DOS drive mapping 的既定语义。没有 Windows junction/symlink/drive-map 竞争实测。 |
| 创建时 protected owner-only DACL / 句柄读回 | `windows/mod.rs:175-184`；`windows/security.rs:73-115,140-143,376-398` | 直接附加当前进程 TokenUser owner 和单一 full-control ACE；目录 OI/CI、文件无继承标志，设置 DACL_PROTECTED。首次内容写入前通过实际句柄检查 kind/owner/protection/exact ACE，失败不删除可疑结果。 |
| 完整 metadata 与安全观察 | `windows/mod.rs:14-89`；`windows/native.rs:237-271`；`windows/security.rs:401-424` | 完整 128-bit ID + volume、link、delete pending、attributes、creation/write/change times、owner/group/DACL 观察。普通文件必须单 link；目录 stamp 排除兄弟变动时间；access time 不参与自身读取失效。公共 Debug 不打印 security bytes。 |
| 独立枚举 cursor / bounded records | `windows/native.rs:274-315`；`directory_records.rs:5-45` | 新 open 文件对象持有 cursor，不以 DuplicateHandle 扫描；64 KiB buffer，返回长度先验证，再逐 record 有界 UTF-16 解码，总 names 上限 16,384。纯 parser 的 7 项测试实际通过；`.` 重开、实际 cursor 行为与 Windows record ABI 尚未原生执行。 |
| retained source / no-replace | `windows/mod.rs:237-242`；`windows/native.rs:318-356` | 对 held source 发 NT rename、目标为 held directory 加单名称、ReplaceIfExists=false；公开方法还检查 absent/alias 与源 private security。发布后的完整身份/bytes/父链/旧名缺席仍由上层完成。两个上述 P2 阻止给 publication 完整源码批准。 |
| 读取 sharing 与私有文件 sole-writer | `windows/native.rs:131-165`；`windows/tests.rs:80-95,99-121,243-254` | 普通只读 open 使用 SHARE_ALL，因此与原 private source 的读共享及其已有 WRITE/DELETE access 在静态 share 矩阵上兼容；private file 的原 share READ 保持拒绝第二 writer/delete opener。已有 reopen 负控只测 RW+DELETE 组合，不分别隔离 write 和 delete。没有以类型检查冒充这组行为运行通过。 |

**测试是否真实有意义。**

宿主 15 项是 8 项路径与 7 项目录记录 parser 的真实执行，包含 UTF-16 单元上限、
DOS aliases、路径逃逸、记录截断/offset 越界、坏 UTF-16、跨批 names budget 等。
先从新编译的 harness 列出全部具体名字，再实际运行一次，15 passed /
0 failed / 0 ignored。没有使用父预构建二进制。

Windows 条件库存为 15 项平台测试 + 27 项 security 测试，共 42，名字和行号逐项保存。
本轮 strict Windows x64 MSVC all-targets Clippy 实际重新检查了相应模块；
只是 typecheck，**42 项 Windows 条件测试的运行计数为 0**。
Windows 下还会包括宿主可运行的 15 个纯测试，不能把这些不同平台的数量相加冒充运行结果。

Windows 测试内容确实有真实 API fixture：创建时权限、在 retained READ_CONTROL
句柄上修改真实 DACL 并验证拒绝、句柄在路径替换后仍绑定原对象、直接绕过 absent
预检查调用 no-replace syscall、完整文件/目录发布正控以及超过单 query buffer 的枚举。
其他明确标作 snapshot/encoding 的测试变异内存值，不把它们描述为真实 owner/group
权限更改。它们不是只检查 mock 返回值，但现在均未运行。

并发枚举用例只有四个名称，且未用 barrier 强制两个多批扫描交错；它不能单独
证明 cursor 隔离。单独 1100-name 用例确实覆盖多批扫描。reparse 实物、held-volume
remote/drive-map 竞争、保留私有目标目录发布、短/最大名发布及 write/delete
分别拒绝仍是原生矩阵中的开放项；不是额外 P3，也不是 Windows 通过证据。

**独立执行与命令计数。**

工程验证共 **7 条底层命令：3 Cargo + 4 rustc**。2 个编译负例的 rustc
exit 1 是预期结果，必须出现精确错误码；其他五条工程命令 exit 0。
所有调用有实际 wait/reap 和有界超时，原始 argv/output/exit/timing 已保留。
环境观察另有 4 条命令，不计入 7 条工程验证。
文档取回/搜索、冻结与资源管理另计，不叠加为测试通过数。

| 命令 / 原件 | 实际结果 |
| --- | --- |
| `host-parser-inventory.json/.txt`：Cargo test `-p keelshell-platform --all-targets --offline --locked -- --list` | exit 0；新编译，15 个具体名字。 |
| `host-parser-tests.json/.txt`：相同 package、同一独占 target，`--test-threads=1` | exit 0；15/15。 |
| `windows-cross-clippy.json/.txt`：Cargo Clippy `-p keelshell-platform --all-targets --target x86_64-pc-windows-msvc --offline --locked -- -D warnings` | exit 0；无警告；不链接/不运行 Windows binary。 |
| `probes/results.json`：外部公开 API 正控 | rustc exit 0；Windows metadata-only 类型检查。 |
| 同上：外部 File 调私有 directory constructor | rustc exit 1，精确 E0624。 |
| 同上：外部 File 直接注入 Directory/PrivateEntry 字段 | rustc exit 1，精确 E0451，两处字段构造；一个命令。 |
| `probes/rename-buffer-layout.json` | rustc exit 0；Windows x64 编译期布局与长度断言。 |

工具链实际为 Rust/Cargo 1.100.0、Clippy 0.1.100，host
`aarch64-apple-darwin`；预装 Windows std target 被复用，没有安装工具链。
每条 Cargo 都显式指定本 review 的绝对 `--target-dir`，并设置同值
CARGO_TARGET_DIR、禁用增量编译、offline/locked。没有借用父缓存产物。
没有重复全仓门禁、运行 fmt 或假定本次通过等于批次集成 gate。

**父的新设计及后续边界。**

父已接受两个 P2，明确撤回仅增 share WRITE 的方案，开始将 private directory
创建 capability 与 publication renamer 分开，并修正缓冲长度、添加回归。
本报告没有审查或批准这些后续字节。尤其 ReOpenFile 的实际对象身份、独立
同步模式、DELETE 临时权限、与目标共享的兼容性、错误返回和 handle RAII，
应在 r2 冻结后独立差分复核；现有 r1 的 HOLD 不被未来方案或类型检查回填。

在本报告中，unsafe 边界等未发现阻塞的项目可作为限定的源码审查结果供父集成参考，
但整个 r1 platform publication 仍为 HOLD。Windows 原生文件系统行为、三平台
桌面、上层 session/归档/Zmodem 集成以及业务验收都不在本轮批准范围。

**证据与清理。**

完整命令回执、42 项条件库存、9 项 r1 原始字节、微软 primary sources、编译负例、
布局断言、18 份 compiler dep-info/fingerprint、构建产物摘要以及末次 live 差异记录
均保存在此 review 目录。`cleanup.json` 记录在先保存并核验 compiler evidence 后，
仅删除本 review 的独占 target 与已被逐字节恢复快照替代的编号副本；
其中 target 的 30,096,259 字节已删除。父 target 和其他任务目录不在删除集合内。
`evidence-manifest.json` 给出 r1 保留文件的最终大小和 SHA-256，不包含后续 `r2/`。
