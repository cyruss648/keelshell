# KeelShell bounded PAX independent delta review — r3 repair / r4 freeze

结论：**APPROVE**。当前未关闭发现：P0 **0** / P1 **0** / P2 **0** / P3 **0**。原 **P2-01 CLOSED**：完整原始 TAR size 已在预算与覆盖之前校验，新增回归已实际执行通过。本结论签署 r3 修复内容，经结束核对绑定最终 **r4** 冻结；原 r1 的 REQUEST_CHANGES、恶意夹具与失败证据保持原样。

日期：2026-11-15，Asia/Shanghai。非作者独立、严格只读 delta review；范围为 P2-01 修复、六项新增测试及原有安全/兼容边界不退化。所有 shell 命令及启动子进程均显式 cwd：

`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`

沿用已读 AGENTS.md、/Users/hdh/.codex/RTK.md、docs/HANDOFF.md、docs/testing/SHARED_TARGET.md，并应用 `/Users/hdh/.agents/skills/rust-best-practices/SKILL.md` 的类型化错误、整数边界、切片与有意义回归审查要求。写入仅限本 review-pax 目录内报告、隔离日志、JSON 回执及 tmp；未编辑、格式化、提交、push、clean 源码，未运行 Cargo、build、Clippy，也未写共享 target。本轮没有重查 UI 或重复全仓门禁。

**冻结与证据绑定**

| 对象 | 独立核对结果 |
| --- | --- |
| 基线与结束 HEAD | `e97705c3d773ea6919d76441685c5064e641ca86`，未推进，候选未提交 |
| r3 freeze SHA-256 | `e90687d5fb0f2932ec7f4d754180980a798c85cc7e05d42d708b63385e710ec8` |
| r4 freeze SHA-256 | `96bb5b3bded14516e3167d8281adee35901711cbca6ba9f41f91215afa179aef` |
| r3 开始核对 | 538 输入 + 10 脚本，0 mismatch；`2026-11-15T05:18:00.505852+00:00` |
| r4 接续核对 | 538 输入 + 10 脚本，0 mismatch；`2026-11-15T05:19:32.723948+00:00` |
| 最终 r4 结束核对 | 538 输入 + 10 脚本，0 mismatch；`2026-11-15T05:23:14.396155+00:00`；r3/r4 freeze 文件本身未变 |
| r2 → r3 | 仅 `crates/keelshell-core/src/archive.rs` 与 `crates/keelshell-core/tests/archive_pax.rs` 变化 |
| r3 → r4 | 仅新增测试内两处等价 NUL 转义；生产源码逐字相同 |
| r3/r4 生产 archive.rs SHA-256 | `786773b3839adaf01cb52f08e071e0d9ba8c70db97cfb4fdce4befbfdcd64d77` |
| r3 archive_pax.rs SHA-256 | `4e8e6011f9e51bd82cc1f40bf4ea8ece2d2e9ff1ccebd127a2459a332b4ae335` |
| r4 archive_pax.rs SHA-256 | `7ae59ddfdc1b03709edbe3394bd893c76f44c2ae1cac46e67860d68606d20e96` |
| 既有 tests/archive.rs SHA-256 | `21e9c068d823cead5007f41153107c331ee1e6270e4aaa35dcde06cc20a9ff07`，未变 |
| 原 r1 报告 SHA-256 | `352ddd7dbc3aacff4c4f2a0eff73c3403cde958f7cece165b9383906c041b248`，未改写 |

逐字核对 r3 → r4 只有以下两处，位置分别为 [archive_pax.rs:346](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/archive_pax.rs:346)、[archive_pax.rs:352](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/archive_pax.rs:352)：

```text
*b"0003\00000007"  -> *b"0003\x000000007"
*b"\000000000003" -> *b"\x0000000000003"
```

上面展示的是 Rust 源码中的原始转义写法；`\0` 与 `\x00` 均是一个 NUL，后续数字均为普通字节，两个数组仍为相同的 12 字节。恰好两次替换即可逐字得到整个 r4 测试文件，其他内容不变；不是改变测试恶意输入以绕过回归。r1 → r2 的 8 处 byte_char_slices 修正已在原报告绑定；本轮 r3 binary 已包含 r2 写法，仅与 r4 存在这两处 NUL 表达差异。

冻结、快照与差分证据：[freeze-r3-start.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/freeze-r3-start.json)、[freeze-r4-start.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/freeze-r4-start.json)、[freeze-r4-end.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/freeze-r4-end.json)、[r2-r3-production.diff](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/r2-r3-production.diff)、[r2-r3-tests.diff](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/r2-r3-tests.diff)、[r3-r4-test-escapes.diff](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/r3-r4-test-escapes.diff)。结束回执也重新核对原 r1 报告、回执、notice、静态探针、恶意夹具五件关键证据，SHA 全部保持；没有改写原始 r1 日志。

**P2-01 关闭依据：触发、影响与修复**

原触发为 checksum 有效的 type-x PAX 头，12 字节原始 size 为 `80000001000000000000000d`，完整值是 `2^64 + 13`；依赖低位截断后当成 13，使超大声明通过 PAX 元数据预算。另一触发是无效普通头 size 被合法 PAX size 覆盖。影响为错误接受完整数值无效或超限的归档，破坏原始声明完整性与 fail-closed 边界；原发现没有证明任意写入或无界实际分配。

父已补上实际 Rust 红回归：`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/logs/pax-size-reproduction-r1.log`，exit **101**，**0 passed / 2 failed / 34 filtered**，两处均显示旧解码器返回 `Ok`。日志 SHA-256 `9f1afc4f6721dc8a1b3f861522e6c0d97b2d070f010443986c103d4e13effa8d` 与其 JSON 回执相符。这是已核对的作者执行证据，不计为 reviewer 的独立执行。

| 绝对源码定位 | 已核对的修复不变量及影响 |
| --- | --- |
| [archive.rs:699](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:699) | 将唯一可执行的 `header.size()?` 替换成 `tar_size(&block)?`。普通文件、目录、GNU 长名、PAX 局部/全局头共同经过此入口；此处早于元数据 payload 读取、原始累计预算与有效 size 覆盖。特殊/不支持的头仍先拒绝。 |
| [archive.rs:957](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:957) | 参数固定为 512 字节块，读取完整 `[124..136]`；固定切片无越界通路，返回 `Result<u64, ArchiveError>`，无 panic 或静默默认值。 |
| [archive.rs:960](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:960) | base-256 高 4 字节必须恰为 `[0x80, 0, 0, 0]`，拒绝负值、超 u64 高位和其他不支持编码；随后恰好折叠 8 个低位字节，完整保留 `0..=u64::MAX`，不会再截断高位。 |
| [archive.rs:972](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:972) | 八进制仅接受前导 ASCII space、至少一个 `0..7` 数字、随后仅 NUL/space；进入尾部后再次出现数字即拒绝。拒绝 NUL 后隐藏内容、正负号、tab/newline、空字段及前导 NUL。八进制累计采用 checked 乘加并返回 Corrupt。 |
| [archive.rs:714](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:714)、[archive.rs:754](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:754)、[archive.rs:759](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:759)、[archive.rs:766](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:766) | 单头/累计元数据预算、原始累计 size、有效 size 与 manifest 的既有预算继续执行；PAX 覆盖不能掩盖无效原始编码或原始超限。目录与 GNU 长名各自的零载荷/小载荷限制也继续作用。 |

本次生产差分只有替换入口及新增 `tar_size`；未发现第二个未覆盖的可执行 `header.size()` 调用。完整数值有效而超预算仍交由既有预算拒绝，不能用合法编码绕过资源约束。推荐修复已经落实，不再有阻塞项；当前无需额外源码修改。

**六项新增回归与原边界保持**

| 新增具体测试名与绝对位置 | 核对覆盖 |
| --- | --- |
| `pax_rejects_full_width_base256_sizes_in_every_admitted_header_kind` — [archive_pax.rs:184](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/archive_pax.rs:184) | x/g/普通/目录/GNU 长名五种头，多个非零高位及负编码；校验和修正后断言严格 size 编码错误。 |
| `pax_size_cannot_hide_a_full_width_invalid_ordinary_size` — [archive_pax.rs:230](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/archive_pax.rs:230) | 局部和全局 PAX size 覆盖都不能隐藏普通头的超宽/负原始声明。 |
| `pax_accepts_small_nonnegative_base256_sizes_for_all_admitted_headers` — [archive_pax.rs:251](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/archive_pax.rs:251) | 五种允许头的合法小 base-256 正例，以及 0/1/511/512/513 字节块边界，与原八进制解码内容相等。 |
| `pax_base256_sizes_preserve_full_u64_and_existing_budget_limits` — [archive_pax.rs:297](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/archive_pax.rs:297) | `u64::MAX`、普通原始展开超限、元数据 8193 字节超限；即使 PAX 覆盖为 0 也不能隐藏原始展开预算。 |
| `pax_accepts_unambiguous_octal_size_padding` — [archive_pax.rs:323](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/archive_pax.rs:323) | NUL 尾、满宽数字、前导/尾部 space、NUL 后空格等普通 writer 编码继续可读。 |
| `pax_rejects_hidden_or_malformed_octal_sizes_even_with_overrides` — [archive_pax.rs:344](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/archive_pax.rs:344) | NUL 后隐藏数字、非法后缀、正负号、控制空白、前导 NUL、无数字字段；合法 PAX 覆盖也不能消除错误。 |

与 r2 对比，测试清单 **34 → 40**，恰新增上述六项，删除 **0**。移除新增块后，原 34 项测试、辅助代码、嵌入的 Python/macOS tar 实际 writer 字节逐字不变；r4 两处变化只在新增块内。证据：[delta-audit-r3.json](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/delta-audit-r3.json)。

两个真实 writer 测试及原有 record 长度/UTF-8、局部/全局 precedence、删除与重复拒绝、原始/有效路径、manifest、累计预算、gzip/header/padding、GNU 混合扩展、链接/稀疏/未知 vendor 拒绝的测试均包含在实际 40 项 PAX 执行内。旧 archive 的 40 项也实际重跑，涵盖原 TAR/ZIP、gzip 尾部与串接流、完整性和资源限制。生产相应逻辑未因这次差分改变，未发现原边界退化。

批准范围继续是有界 PAX 子集。普通 fallback 仍需满足可移植路径要求，Python Unicode `?` fallback 继续明确拒绝；两个实际 writer 夹具不能外推成所有 Python/macOS tar 或全部 PAX 兼容。删除、重复键、未知 vendor/xattr/编码/link/sparse 及歧义 GNU 混合扩展仍拒绝。本轮未重新生成来源端夹具，也未扩大跨平台或产品验收承诺。

**独立预构建执行、命令与计数**

本轮只执行父提供为 **r3** 的两个 core 预构建 binary，**没有独立重编译**。`--list` 先于正式执行，40 个具体名分别与源码清单完全匹配，无缺失/多余；随后各完整执行一次。源码哈希、库存与测试结果能绑定本轮观察，但不能代替从源码独立重建。最终 r4 与执行 binary 的 r3 测试源码仅有上述已逐字核对的两处等价 NUL 写法。

| binary | 执行前/后及最终 SHA-256 | 字节数 |
| --- | --- | ---: |
| archive_pax-6f1f933af327b943 | `650f6852fb207b01136b89d822592a000b940536edf9775cdd5c6ab8ef917d67` | 4223248 |
| archive-86ed372bc283f58c | `aa30b8f14c5c611966b2e7fd7dcb902d2d0a04d3f79e2ffcf6c7c806081b2733` | 4401400 |

下列每行的 cwd 均为本报告顶部绝对主树路径；原始完整 argv、时间、exit、库存、SHA 见 [commands-r3.jsonl](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/commands-r3.jsonl)。

| 实际 argv | exit | 库存或执行结果 | 耗时 |
| --- | ---: | --- | ---: |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-core/6f1f933af327b943/out/archive_pax-6f1f933af327b943 --list` | 0 | 40 个具体测试名，全部匹配源码 | 0.004s |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-core/86ed372bc283f58c/out/archive-86ed372bc283f58c --list` | 0 | 40 个具体测试名，全部匹配源码 | 0.004s |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-core/6f1f933af327b943/out/archive_pax-6f1f933af327b943 --test-threads=1` | 0 | 40 passed / 0 failed / 0 ignored | 0.406s |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-core/86ed372bc283f58c/out/archive-86ed372bc283f58c --test-threads=1` | 0 | 40 passed / 0 failed / 0 ignored | 11.141s |

合计本轮 **80 passed / 0 failed / 0 ignored**；六项新增及两个真实 writer 已包含在 40 中，不重复相加，`--list` 不算通过次数。原 r1 的 74 项执行是历史证据，不并入本轮计数。所有四个 binary 命令 exit 0，均实际 wait 回收，未触发 timeout；list 上限 25 秒、执行上限 50 秒，超时处理为 kill 后最多 5 秒 wait，满足每命令 ≤60 秒。TMPDIR/TMP/TEMP 均指向本 review-pax/tmp，未写共享 target。

原始日志：[pax-list-r3.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/pax-list-r3.log)、[archive-list-r3.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/archive-list-r3.log)、[pax-execute-r3.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/pax-execute-r3.log)、[archive-execute-r3.log](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/archive-execute-r3.log)。旧 archive 的 ZipWriter::drop 输出属于既有压缩上限测试的预期 stderr，实际用例终态和 libtest 汇总均为通过；本轮包装器与 binary 均 exit 0。未为统计扩大或重复执行。

**作者门禁与未验证范围**

作者红回归、专项修复日志以及 full-gate-r3 失败日志均已与 JSON 回执的 SHA 核对，记录在 delta-audit-r3.json；作者结果不混入独立执行计数。full-gate-r3 因新增测试两处 `octal_escapes` Clippy lint exit **1**，历史失败保留；r4 只修正上述等价 NUL 表达。父正在运行最终 full-gate-r4，其最终通过与完整工程门禁仍由父提供，本报告不代签。

父通报 r3 real OpenSSH **14/14**、正常 GUI/MCP build exit **0**；reviewer 未重复执行。这些构建/集成结果与本轮预构建 core 执行都不构成 macOS/Windows/Linux 原生交互、跨平台解包或完整业务验收。UI reviewer 的 APPROVE 属另一切片，本报告仅关闭 PAX size 的独立复核任务。

最终建议：本次 size 修复及最终 r4 有界 PAX 候选在所审范围 **APPROVE**，P2-01 已关闭；由父继续完成 full-gate-r4 与整体集成收口。原 [report-r1.md](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/archive-tools-pax-20261115/review-pax/report-r1.md) 继续保留 REQUEST_CHANGES 及其当时证据，不追溯改写成通过。
