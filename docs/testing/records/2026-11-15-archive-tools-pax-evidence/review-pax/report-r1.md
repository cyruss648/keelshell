# KeelShell bounded PAX independent review — r1

结论：**REQUEST_CHANGES**。P0 **0** / P1 **0** / P2 **1** / P3 **0**。

本报告由新的非作者 reviewer 完成，审查基线为 `e97705c3d773ea6919d76441685c5064e641ca86`，候选未提交。审查覆盖 r1 生产源码，并经完整冻结核对将同一生产源码结论绑定到 r2。唯一阻塞为 P2-01：PAX 容器的完整原始 size 在依赖截断前没有验证。其余指定范围已审完，未发现其他可操作问题；不用重复计数或扩大执行来替代修复。

日期：2026-11-15，Asia/Shanghai。所有 shell 命令及启动的子进程都显式使用 cwd：

`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`

已读取 AGENTS.md、/Users/hdh/.codex/RTK.md、完整 docs/HANDOFF.md、docs/testing/SHARED_TARGET.md；使用 `/Users/hdh/.agents/skills/rust-best-practices/SKILL.md` 及相关第 1、3、4、5、8 章。只读审查源码和依赖源码；写入仅在本 review-pax 目录内的报告、日志、JSON 回执和 tmp。没有编辑或格式化源码，没有提交、push、clean、Cargo、build、Clippy 或写共享 target。

**冻结及签署对象**

| 对象 | 摘要或结果 |
| --- | --- |
| 基线与结束 HEAD | `e97705c3d773ea6919d76441685c5064e641ca86`，未推进 |
| r1 freeze SHA-256 | `9153a94c3551829a4030e90f6e3ffc801815f4fb1ad392646bf868d032d87a93` |
| r2 freeze SHA-256 | `b648c546c07a09d0ef5abf0d2f461917528c265019a7afefe583abcab6b42353` |
| r1 开始核对 | 538 输入 + 10 脚本，0 mismatch，2026-11-15T04:59:41.407497Z |
| r2 接续核对 | 538 输入 + 10 脚本，0 mismatch，2026-11-15T05:01:23.275830Z |
| 结束核对 | 538 输入 + 10 脚本与 r2 全匹配，0 mismatch，2026-11-15T05:09:52.947505Z；两个 freeze 文件本身未变 |
| r1 → r2 | 仅 tests/archive_pax.rs 的 8 处字面量：7 处 `[b'x', b'g']` → `*b"xg"`、1 处 `[b'0', b'5']` → `*b"05"`；逐字变换核对通过 |
| 生产 archive.rs SHA-256 | `8284a16f5e47cfbf1bad67ec141b17e7b6f28295078c9fc14f038a26985ba24d`，r1/r2 相同 |
| r1 archive_pax.rs SHA-256 | `dbc8f6d41afdab27805e29aa2cb3112f283439c6f9beef3f4fccbb5121d75a39` |
| r2 archive_pax.rs SHA-256 | `f7dd1aeb19caa6c6772122e0d164bc7ff473c93795a6158594772ffb309dd828` |
| 既有 archive.rs 测试 SHA-256 | `21e9c068d823cead5007f41153107c331ee1e6270e4aaa35dcde06cc20a9ff07`，r1/r2 相同 |

回执：`freeze-start.json`、`freeze-r2-start.json`、`freeze-end.json`、`r1-r2-test-literals.diff`。r1 开始的源码与测试字节另存于 tmp，以免把父修改期的未核对输入签回 r1。结束时相对 r1 唯一差异是上述已授权的 r2 测试字面量替换，没有把 r1 结束误写成全部输入无变化。

**P2-01 — 在依赖解析前验证完整原始 TAR size，防止 PAX 元数据长度截断**

绝对位置：

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:699`：`header.size()?`。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:714`：新增 PAX 分支将该结果传入 `pax_budget.read`。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:785`：8 KiB 检查仅检查已经截断的数值。
- 锁定依赖 `/Users/hdh/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/tar-0.4.46/src/header.rs:1512` 和 `:1536`：12 字节 base-256 解析无溢出/负数检查，仅保留末 8 字节。

触发：构造校验和有效的 GNU 格式 type `x` 元数据头，将 12 字节 size 字段设为十六进制 `80000001000000000000000d`，再放入 13 字节 `13 path=safe\n`、零 padding、安全的普通 fallback 空文件及两个结束块。完整 size 是 **18446744073709551629，即 2^64 + 13**；锁定依赖返回 **13**。新增分支据此通过 8 KiB、元数据累计预算及 payload/padding 检查；后续有效文件大小为 0，完整归档仅 135 压缩字节，其他路径/成员/尾部检查均可通过。负 base-256 字段 `ffffffff000000000000000d` 同样低位为 13，而完整解释是负数。

影响：超出 u64 且远超单头资源上限的原始声明被静默缩短，实际截断的归档落入新准入的 PAX 子集，违背本批“原始声明分别检查”和完整性 fail-closed 边界，并产生解码器之间的语义差异。该案例没有证明路径逃逸、任意写入或无界实际分配；返回后的实际长度、流和成员仍受既有上限约束，因此按 P2 报告。

增量归属：基线对 type `x`/`g` 始终返回 UnsupportedEntry，这个 PAX 准入路径由本批新增。依赖数值截断在既有普通头解析中也已存在，本报告不把那部分历史行为说成本批新引入的第二个发现。

证据：

- `base256-overflow-static-probe-r1.json`：字段、完整数值、低位数值、有效 header checksum、夹具摘要。
- `tmp/pax-metadata-base256-overflow-r1.tar.gz`：135 字节最小夹具，SHA-256 `029e6e6b92c608681a8e7ec56e959817b860c410968d3e6a62045af318973bd4`。
- Python `TarInfo.frombuf` 独立读回完整超大声明；补充负数数值见 `base256-additional-static-cases-r1.json`。
- `/usr/bin/tar -tf <上述绝对夹具路径>` 实际 exit **1**，输出 `Special header too large`；这是对恶意夹具的预期拒绝，不是 Rust suite 失败。原件 `base256-system-tar-list-r1.log` 与 `commands.jsonl`。
- `dependency-source-binding-r1.json`：本机 tar 0.4.46 的 header.rs 与 entry_type.rs 逐字等于缓存 `.crate` 中的文件；`.crate` SHA-256 等于 Cargo.lock 的 `3f6221d9a6003c78398e3b239969f352578258df48c8eb051caadae0015bc840`，排除仅依赖未核实本机源码的推断。

证据限制：**未把这个新恶意夹具动态送入 Rust `ArchiveBundle::decode`**。reviewer 不获准编译或修改测试，Rust 接受结论来自已核对生产源码与锁定依赖的完整控制流，而非新回归执行结果。父应补充动态回归。

修复建议：在 `header.size()` 前校验完整原始 12 字节 size；拒绝负值、超 u64、高位非零而被截断的数值及不支持/歧义编码，再把完整有效值送入各预算。将同一严格检查用于元数据头和普通头，避免原始 size 安全检查依赖有损解析。补充 type x/g、普通成员加合法 PAX size 覆盖的高位溢出及负 base-256 负例；保留真实 writer、合法小 base-256/普通 octal 及安全 fallback 的兼容回归。无需放宽路径、未知键或资源上限。

该发现已尽早在审查会话公开，并落入 `notice-p2-01-r1.md`。桌面 `send_message_to_thread` 明确拒绝向本原生祖先投递；当前会话没有暴露原生 agent 消息工具，因此不能声称父已收到跨会话消息。父可直接消费该 notice 与本报告进行集中修复。

**其余安全及兼容边界**

| 范围 | 已核对结论与绝对代码位置 |
| --- | --- |
| PAX record 长度与 UTF-8 | archive.rs:804、:816、:818、:824、:850、:898：十进制 checked 运算，usize 转换受检，切片与长度/换行校验先于索引，合法 record 必进度前移；每头先限 8 KiB。未发现 record 文本长度溢出、UTF-8 边界或 panic 通路。此项与 P2-01 的外层 TAR header 数值截断不同。 |
| 局部/全局优先级 | archive.rs:706、:715、:758、:886：局部一次消费，优先于全局，再到普通头；后续全局仅更新提供的属性。全部记录先验证，再合并或覆盖，不靠最后值掩盖先前恶意值。 |
| 删除、重复、未知键 | archive.rs:832、:846、:852：9 键白名单，全部同头重复键拒绝，空值删除拒绝；厂商/编码/linkpath/稀疏键拒绝。属于保守子集，不承诺完整 POSIX PAX。 |
| 原始/有效路径 | archive.rs:301、:420、:709、:750、:765、:929、:944：普通备用路径与有效路径分别验证 UTF-8、NUL 隐藏后缀、相对性、保留名、危险字符和目录尾斜线；ustar prefix 不因 path 覆盖被跳过。元数据容器允许前导 ./，仍拒绝 traversal。 |
| 资源与成员清单 | archive.rs:38、:362、:751、:754、:766、:784：8 KiB 单头、2 MiB + 48 KiB 元数据累计、4096 元数据头；2048 成员、32 层、4096 路径字节、48 KiB 完整清单、128 MiB 原始/有效累计载荷，目录原始和有效大小均需 0。除 P2-01 有损数值来源外，未发现预算绕过；计数器在前次已受限的状态上增长，不形成算术溢出。 |
| gzip/header/padding | archive.rs:619、:636、:645、:659、:666、:957、:981：原有 checksum、有效载荷、零 padding、两个结束块、零且对齐的尾部、读到 gzip EOF 验证 CRC/ISIZE、拒绝串接流，PAX 不绕过这些路径；既有 gzip/截断/流预算测试实际执行通过。 |
| GNU / 链接 / 稀疏 | archive.rs:686、:700、:706、:723：特殊类型先拒绝，非空 link 字段在元数据和普通成员均拒绝；GNU 长名与局部 PAX 或全局 path 的歧义链失败关闭，孤立元数据拒绝。未知 vendor 和 sparse 语义不忽略后继续提取。 |
| 元数据丢弃 | archive.rs:797、:857；tests/archive_pax.rs:153：只保存 path/size，时间与 owner 字段验证后丢弃；重新编码使用私有 mode、uid/gid/mtime 归零。 |

上述表格中未展开的绝对前缀统一为 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/`；测试前缀为同 crate 的 `tests/`。P2 发现的每个定位已在前文完整写出。

实际 writer 兼容检查：`tests/archive_pax.rs:13` 与 `:32` 的两个测试均实际执行。另用 Python 在内存独立解析嵌入字节：Python 夹具 500 字节，9 个物理头（4 局部 + 1 全局）和 4 个有效成员；macOS writer 夹具 291 字节，4 个物理头（2 局部）和 2 个有效成员。成员内容、二进制数据及元数据结构见 `fixture-audit-r1.json`。没有重新运行作者最初的夹具生成流程，故不独立认证源注释中的原始生成工具版本。

Python Unicode 限制已独立检查：用当前 Python 在内存生成 `资料.txt` 的 PAX 头，其普通备用名为 `??.txt`；archive.rs:305 与 :750 明确拒绝 `?`，tests/archive_pax.rs:457 的备用路径负例实际通过。合法 UTF-8 PAX path + 安全普通备用名的正例也已通过。不能把这两个真实 writer 夹具外推成所有 Python/macOS tar/PAX 文件均兼容；来源端 vendor/xattr/编码切换、删除、重复键、堆叠局部扩展等仍拒绝。

ADR `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/adr/0134-bounded-pax-archive-metadata.md:4` 保持实施中，:26 明确 Python `?` fallback 限制。父更新后的 ARCHIVE_TRANSFERS.md:43/:85 与 CAPABILITIES.md:15 同样说明保守子集和未验收边界。文档摘要随 freeze-end.json 保存；这些 Markdown 不冒充 Rust/Cargo 冻结输入。

**实际独立执行与计数**

仅执行指定父预构建 core binary，**没有独立重编译**。这两个 binary 是 r1 测试写法；生产代码 r1/r2 完全相同，r2 的 8 处等价字面量差分已经逐字核对。这不能代替父对 r2 的 Clippy/完整 gate。

| binary | SHA-256 | 字节数 |
| --- | --- | ---: |
| archive_pax-6f1f933af327b943 | `c4f881c481efc370da84535ed8430677d2f14949665fdabd0bfc0444e21c4299` | 4173024 |
| archive-86ed372bc283f58c | `2bdc2be801044a92818cdf23588470af4fefd48164d942de31f3619778232585` | 4400200 |

开始、实际执行前/后及结束二进制摘要一致。每个命令实际 wait 回收，无 timeout。原始 argv/cwd/退出/耗时写在 `commands.jsonl`；临时环境 TMPDIR、TMP、TEMP 均指向本 review-pax/tmp。

| 实际 argv（各行 cwd 均为本报告顶部绝对路径） | binary exit | 结果 | 耗时 |
| --- | ---: | --- | ---: |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-core/6f1f933af327b943/out/archive_pax-6f1f933af327b943 --list` | 0 | 34 个具体测试名与 r1 源码逐一相等，无缺失/多余 | 0.004s |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-core/86ed372bc283f58c/out/archive-86ed372bc283f58c --list` | 0 | 40 个具体测试名与既有源码逐一相等 | 0.003s |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-core/6f1f933af327b943/out/archive_pax-6f1f933af327b943 --test-threads=1` | 0 | 34 passed / 0 failed / 0 ignored | 0.378s |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-core/86ed372bc283f58c/out/archive-86ed372bc283f58c --test-threads=1` | 0 | 40 passed / 0 failed / 0 ignored | 10.774s |

合计 **74 passed / 0 failed / 0 ignored**；两个真实 writer 夹具已包含在 34 中，不另加计数。--list 不是通过次数。没有运行 0 测试后计为通过，没有复跑增加计数。上述四个 binary 命令各设最大 50 秒（list 25 秒），若超时则 kill 后 wait 最多 5 秒；全部实际远低于 60 秒且未触发超时。

旧 archive 日志中 `both_encoders_bound_incompressible_output_during_compression` 的预期 `ZipWriter::drop` 错误输出与 `ok` 同行。最初 reviewer Python 包装器的精确行正则漏计 1，导致**包装器 exit 1，而 binary exit 0**；保留原件，不覆盖、不重跑。按该行终态、40 个具体名与 libtest `40 passed` 汇总完成校正，见 `archive-output-parse-correction-r1.json`。这不构成产品发现或 Rust 测试失败。

额外只读检查：限定 `git diff --check e97705c3d773ea6919d76441685c5064e641ca86 -- crates/keelshell-core/src/archive.rs crates/keelshell-core/tests/archive.rs` exit 0。新未跟踪 archive_pax.rs 已单独逐行审查和摘要绑定；没有用该 diff 命令冒充其格式化验证。依赖来源检查首次因注册表无 `.cargo-checksum.json` exit 1，随后改用缓存 `.crate` 与 Cargo.lock 直接匹配；前半部分已完成的 fixture audit 保留，未扩大 Rust 测试执行。

**证据边界与后续处理**

父 full gate r1 在 8 处测试 byte_char_slices Clippy lint 停止，exit 1，历史失败保留；r2 已仅修复上述字面量。本报告收口时父完整 gate 仍由父跟踪，不签其最终成功。父提供的真实 OpenSSH 14/14、正常 GUI/MCP build exit 0 属作者证据；本 reviewer 没有重复执行，也没有计入本报告 74 项独立执行。编译与 headless/夹具测试均不代表原生 macOS/Windows/Linux 交互、跨平台或完整业务验收，Windows 安全解包等原开放边界继续保留。

当前建议：先集中修复 P2-01，增加实际 Rust 恶意头回归，冻结修复后的候选，再由父运行受影响回归/完整 gate并交独立差分复核。既有 APPROVE 的工具/UI 切片不代签本 PAX size 缺口，本报告也不重审那个切片。此刻不能给本 PAX 候选 APPROVE。

**34 个新增测试名（已与 --list 逐个核对，并全部实际通过）**

- `pax_bounds_cumulative_metadata_payloads_even_when_values_are_discarded`
- `pax_bounds_each_metadata_header_before_reading_or_allocating`
- `pax_bounds_metadata_header_count_independently_of_payload_and_entry_counts`
- `pax_directory_paths_allow_one_suffix_and_zero_size_overrides`
- `pax_effective_directory_paths_preserve_exact_manifest_byte_limit`
- `pax_effective_paths_preserve_duplicate_case_and_ancestor_conflict_checks`
- `pax_local_metadata_is_consumed_once_and_does_not_leak`
- `pax_local_overrides_global_and_later_global_updates_retain_missing_defaults`
- `pax_macos_system_tar_writer_archive_decodes_ordinary_metadata`
- `pax_metadata_and_effective_payloads_retain_padding_and_truncation_checks`
- `pax_posix_metadata_accepts_utf8_paths_and_signed_fractional_times`
- `pax_preserves_cumulative_raw_and_effective_expansion_limits`
- `pax_preserves_explicit_entry_limit_without_counting_metadata_as_members`
- `pax_preserves_raw_and_effective_declared_expansion_limits_before_reading`
- `pax_preserves_utf8_path_byte_depth_and_directory_suffix_bounds`
- `pax_python_tarfile_archive_decodes_files_directories_and_metadata`
- `pax_record_lengths_count_utf8_bytes_and_require_valid_utf8_values`
- `pax_reencoding_discards_ownership_permissions_and_timestamps`
- `pax_rejects_ambiguous_gnu_long_name_extension_chains`
- `pax_rejects_deletions_and_control_characters_in_discarded_names`
- `pax_rejects_duplicate_keys_even_when_values_match`
- `pax_rejects_empty_or_orphaned_metadata_and_stacked_local_headers`
- `pax_rejects_invalid_or_overflowing_timestamps`
- `pax_rejects_invalid_unsigned_sizes_and_owner_ids`
- `pax_rejects_malformed_record_lengths_and_framing`
- `pax_rejects_nonempty_link_fields_on_metadata_or_effective_entries`
- `pax_rejects_nonzero_directory_sizes_in_either_header`
- `pax_rejects_shadowed_malformed_global_values`
- `pax_rejects_unsafe_fallback_paths_and_hidden_ustar_prefixes`
- `pax_rejects_unsafe_metadata_container_names`
- `pax_rejects_unsafe_override_paths_even_when_globals_are_shadowed`
- `pax_rejects_unsupported_effective_entry_types`
- `pax_rejects_vendor_sparse_link_encoding_and_unknown_keywords`
- `pax_size_override_controls_block_framing_in_both_directions`
