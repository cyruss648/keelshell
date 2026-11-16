**独立非作者证据／功能复核：APPROVE（限定签署）。P0=0、P1=0、P2=0、P3=0。**

复核日期为 2026-11-16，工程为 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。我未参与本批实现；本次只补查封存、日志解析和记录的验收边界，不重复 Turing 的两文件静态源码评审。建议父收取本报告并关闭本复核；93 成员封存和 r2 派生摘要可作为所述固定历史范围的证据。本意见不签署新 Windows CI、父后续 Python 工装批次、GUI 或产品完成。

**固定输入与只读边界。** 签署源码为 `95813a76f0ec9b59fd043cd982e33cdc01dd69f8`。初始 HEAD／tracking／远端分支均直接读回该完整 SHA，初始工作区干净。先读 AGENTS.md、RTK.md、HANDOFF.md、SHARED_TARGET.md。C09 使用完整 `--source`，后续 C11–C16 从 `git show 95813a76f0ec9b59fd043cd982e33cdc01dd69f8:…` 在内存读取归档／文档；不将并行工作副本作为固定签署依据。

父后续通知 docs-only 补充、新 Windows 原日志和 Python 工装修订。本报告明确排除这些后续时点；C16 观察到父的 docs、delivery 目录及 `scripts/test_windows_native_crash.py` 变化，没有把它们当作工程冻结失配或回滚对象。下文的“新候选执行 0／待精确 CI”只描述 95813a7 封存／文档时点，不主张最新 CI 仍未执行。未查询、等待、取消或重跑 run 43873806902／job 133321520130。

**封存与宿主计数。** `verify.py --source 95813a76f0ec9b59fd043cd982e33cdc01dd69f8` 实际退出 0，session 88490 已回收；输出为 719 工程输入、93 成员、0 mismatch、tests_replayed=0。这验证精确 Git 输入，不要求父正在修改的工作副本继续匹配旧冻结。归档只在内存打开，未解包落盘。

| 项目 | 独立复核结果 | 固定原件／行号 |
| --- | --- | --- |
| 归档 | 93 个唯一普通非 sparse 成员；压缩 1,173,225 字节，原文 6,422,868 字节；所有成员大小／SHA-256 与 manifest 一致 | `docs/testing/records/2026-11-16-windows-zmodem-test-contract-evidence/manifest.json`；`originals.tar.gz` |
| 归档 SHA-256 | `4a69f8a7772d25a617ff0826143746844b64016068e020e43aa2ede46b70e18a` | 固定 Git blob 与 manifest；封存内 tools/verify.py 与 Git 验证器逐字节相同 |
| 工程聚合 | 719 项；`96753dee615c9cd7711a48ad769ec5b14f49c45c581b0bb67323bb51ba508766` | `batch/inputs-freeze-r1.json`；C09 逐项 Git 输入核验 |
| 宿主完整门禁 | observed_exit=controller_exit=0，direct_reaped=true、timed_out=false，723.520796458 秒；645,815 字节日志摘要匹配 | `batch/full-gate-r1.json`；原日志 SHA-256 `bb7a8f08dc723259b190283e95154648a33f9707ba1d2d3b98a9d906282a74d2` |
| 普通 Rust | 92 个普通 libtest 套件，声明数／唯一结果名／摘要逐套件相等；总计 3225 passed／0 failed／48 ignored | `batch/full-gate-r1.log`，workspace 起点第 20 行；每套件独立核算 |
| rustdoc | 单独合计 14 passed／0 failed／0 ignored | 同一日志 Doc-tests 区段；不计入普通 Rust |
| app | 1343 个唯一编译库存名与 1343 个唯一执行名逐项相等；1341 passed／0 failed／2 ignored | `batch/app-inventory-r1.log`；gate 第 1003、1005、2354 行 |
| Python | 主 discovery 290 run／1 skipped；archive helper 独立 30 tests／OK | gate 第 3–8、3831–3834 行；两种运行上下文不相加 |
| 工程检查／controller | 原门禁记录 dependency policy passed、fmt、严格 Clippy；readiness／observer IO／small-stack 三组单列，不算普通测试 | gate 第 1、13、16、4675、4777、4785 行及原完整 gate 成功回执 |

相对 b7d6e92 的 engineering name-only diff 只有 `.github/workflows/ci.yml` 和 `crates/keelshell-session/src/zmodem/local_windows/tests.rs`。这里核对输入绑定与既有报告范围，不重新评价这两文件源码。43 块不变、45 名映射及静态安全推理的署名仍属于 Turing；其原报告第 3、103–109 行如实保留 APPROVE、12 次静态命令的 11 个 exit 0／1 个 exit 1 和独立 Rust／Windows／GUI 执行 0。

**旧 01f6f6b 两平台日志与 r1／r2。** 三平台原日志均直接含 `01f6f6bd8272303fe049bc7b52976afefdb44c37` 的 checkout／fetch 读回；macOS 第 112 行、Ubuntu 第 111 行、Windows 第 98 行可定位。封存终态 JSON 绑定 run 43867112056；macOS job 133298403996、Ubuntu job 133298403683 的 quality gate／OpenSSH step 均 success。上述属于旧源码运行事实。

| 原日志成员 | archive | PAX | app | ZIP 新用例／原安全用例 |
| --- | --- | --- | --- | --- |
| `batch/prior-macos-job-r1.log` | 41/0/0，15.70s；第 3141–3186 行 | 40/0/0，0.34s；第 3188–3232 行 | 1341/0/2，1343 个唯一结果 | 新例第 3171 行 ok；原重编码安全例第 3159 行 ok |
| `batch/prior-ubuntu-job-r1.log` | 41/0/0，20.26s；第 3340–3385 行 | 40/0/0，0.28s；第 3387–3431 行 | 1342/0/2，1344 个唯一结果 | 新例第 3370 行 ok；原重编码安全例第 3358 行 ok |

macOS 原日志第 3188 行和 Ubuntu 第 3387 行确实在 `ZipWriter::drop: … ArchiveTooLarge` stderr 文本后，同一行带 ANSI 的 `Running tests/archive_pax.rs`。r1 只接受行首头，两个平台的 archive 条目都恰有 81 条结果，其顺序与 r2 的 archive 41 条 + PAX 40 条完全相同；r1 的 archive summary 与 PAX summary 完全相同。原 archive 41 项摘要在各自前两行真实存在，故错误属于派生归属，不能据 r1 改写测试实际结果。

独立解析按 `running N tests` 开始一轮执行，头部进入等待队列，直到当前轮摘要结束才释放当前归属；头在行中也能识别。两个平台各 92 个普通套件均满足 declared = 唯一结果名数 = 摘要总数，每个结果名、outcome 与 r2 逐项相等，mismatch 为 0。原 macOS workspace 有 13 个、Ubuntu 有 3 个普通后继头先于前一轮 summary 到达；例如 macOS 第 1457 行头先于第 1461 行摘要、Ubuntu 第 3578 行头先于第 3581 行摘要，独立解析仍保持原轮归属。

`local_agent_process.rs`、`local_agent_directory_process.rs` 两个自定义 harness=false controller 单列；不把它们的阶段事件或退出作为普通测试数量。rustdoc 单独记账。宿主同样只计 92 个普通套件。r1／r2 普通范围本来不同，未把 r1 的 101 个混合条目错误称为 101 个普通套件。未执行封存解析器的写文件入口、未重放任何测试。

macOS 原日志 SHA-256 为 `fd4e4675c123338719f7e929c704574e97819bd5fc05576cc10c0b4a0a75ac42`；Ubuntu 为 `6c818ccd176b2327655e45b17957402fe7c3226e370714359c320c70b04f8139`。两个 r2 报告的 raw_bytes／raw_sha256 与原日志吻合，r1 错误原件仍被 manifest 封存。

**旧 Windows 与新候选、GUI 的边界。** 原 Windows job 133298403995 的 `batch/prior-source-windows-job-r1.log` 为 172,738 字节，SHA-256 `86d46a6833c820a4d438f92bacba2b3fbef8a6700d2daf04bb31d4893e520218`。抓取命令 receipt 退出 0 是日志获取成功，旧测试结果仍为失败；没有把两者混同。

| 固定旧 Windows 范围 | 直接从原日志核得 |
| --- | --- |
| filesystem | 70/0/0；第 474、546 行 |
| core archive | 41 名编译库存／41 个唯一执行名逐项相同；41/0/0；库存第 683 行、执行第 687 行、摘要第 730 行；新 ZIP 例第 718 行与原安全例第 706 行均 ok |
| session library | 332 名编译库存与 332 个唯一执行名逐项相同；327 passed／2 failed／3 ignored；库存第 1233 行、执行第 1237 行、摘要第 1584 行 |
| Zmodem 子集 | 45 名，其中 41 passed／2 failed／2 ignored；原空父目录替换拒绝用例逐名 ok |
| 两项失败 | 原两个旧全名；第 1573–1577 行各有 typed `Os { code: 5, kind: PermissionDenied }`。原日志没有独立 syscall／源码行定位，不能仅据裸错误补签某个内核分支 |
| 旧完整 Windows gate | 封存终态中 Rust quality gate 为 skipped，记录 NOT_RUN 正确；不以宿主或旧 app／codec 通过扩签 |

固定 Git 文档的对应证据为：

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/testing/records/2026-11-16-windows-zmodem-test-contract.md:15`、第 79–81、88–98 行：旧 332／327/2/3、封存时新两例未验、r2 纠正及产品开放范围分开记录。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/testing/records/2026-11-16-portable-zip-metadata.md:94`：终态补充明确旧 Windows codec 41 通过后 session 失败，PAX 40 与 archive 41 分开。较早“执行中／待日志”段落属于前时点，后面的精确终态补充接续它。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/HANDOFF.md:32`、第 66–74、96–100 行：旧 CI、宿主门禁、封存时新 Windows 断言待验及 GUI／截图 0 分开；旧 8c 正常包不代签当前源码。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/ROADMAP.md:27`、第 47–51、70–76 行：旧失败保留，新断言与完整 gate／原生桌面继续开放，FILE-05／M4 和 strict OO 不宣称完成。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/product/ZMODEM.md:54`：旧源码 45 项 41/2/2、新两例封存时待精确 CI、Windows 桌面／真实文件选择／端到端仍开放。第 3–12 行继续保留 strict direct PTY HOLD/PARTIAL。

因此，在固定签署时点未发现把旧 Windows 成绩转签为新断言、把 327/2/3 写成通过、把 GUI 未验写成原生通过的情形。对“后端已接入／入口启用”仅核对其与开放验收的区分，不重新实测或签署功能实现。

**发现与推荐。** P0=0、P1=0、P2=0、P3=0，无本范围必须修复项。r1 的已知解析错误已由保留原件的 r2 纠正，不重复登记为未修复缺陷。接受封存真实性、宿主计数、旧两平台 archive/PAX 解析及固定文档边界；后续 Windows CI／Python 失败与 GUI 由父按独立批次处理。本报告不代签新 Windows run，也不形成发布或完整产品验收建议。

**实际命令与退出。** 按顶层 exec_command 计数，C01–C16 是 16 次只读命令（14 个 exit 0，2 个 exit 2）；C09 的一次 write_stdin 只收取已有 session 终态，不另算 shell 命令。下面 C17 只写本目录两文件，C18 只回读两文件并将实际读回结果加入 receipt；18 次顶层调用最终为 16 个 exit 0、2 个 exit 2。内嵌 Git／正则断言不是 Rust 测试数。完整用途、工具输出索引、实际退出见 receipt.json；stdin Python 以实际入口列出，用途明确标注而不伪造为仓库脚本。

| ID | 实际命令／stdin 工作 | 退出 |
| --- | --- | --- |
| C01 | `pwd` | 0 |
| C02 | `cat AGENTS.md /Users/hdh/.codex/RTK.md docs/HANDOFF.md docs/testing/SHARED_TARGET.md` | 0 |
| C03 | `rg -n 'KeelShell&#124;keelshell&#124;严格只读&#124;independent&#124;read.only' /Users/hdh/.codex/memories/MEMORY.md` | 0 |
| C04 | `git status --short --untracked-files=normal ; git rev-parse HEAD refs/remotes/origin/feature/editor-unicode-codec-20261114 ; git log -1 --format=fuller ; git ls-remote --exit-code origin refs/heads/feature/editor-unicode-codec-20261114` | 0 |
| C05 | `rg --files -g AGENTS.md -g '*zmodem*' -g '*ZMODEM*' -g '*portable-zip*' -g '*windows-archive-root*' docs work/windows-zmodem-evidence-review-20261116` | 2 |
| C06 | `sed -n '413,470p' /Users/hdh/.codex/memories/MEMORY.md ; rg --files -g AGENTS.md -g '!work/**' -g '!target/**' docs .agents` | 2 |
| C07 | `cat docs/testing/records/2026-11-16-windows-zmodem-test-contract-evidence/README.md docs/testing/records/2026-11-16-windows-zmodem-test-contract-evidence/manifest.json docs/testing/records/2026-11-16-windows-zmodem-test-contract-evidence/verify.py` | 0 |
| C08 | `cat docs/testing/records/2026-11-16-windows-zmodem-test-contract.md docs/testing/records/2026-11-16-portable-zip-metadata.md docs/product/ZMODEM.md docs/ROADMAP.md` | 0 |
| C09 | `PYTHONDONTWRITEBYTECODE=1 python3 -B docs/testing/records/2026-11-16-windows-zmodem-test-contract-evidence/verify.py --source 95813a76f0ec9b59fd043cd982e33cdc01dd69f8` | 0 |
| C10 | `PYTHONDONTWRITEBYTECODE=1 python3 -B -（stdin：stdin: 内存读取归档字段、原解析器/门禁验证器、修订说明与历史终态；未调用其写文件入口）` | 0 |
| C11 | `PYTHONDONTWRITEBYTECODE=1 python3 -B -（stdin：stdin: 从固定 Git 归档独立解析 host/macOS/Ubuntu 原日志，逐套件比较 r2，核对 host app 库存与结果）` | 0 |
| C12 | `PYTHONDONTWRITEBYTECODE=1 python3 -B -（stdin：stdin: 独立核对 93 唯一成员/总字节/哈希；r1 结果恰为 archive+PAX，定位下一头早于上一摘要）` | 0 |
| C13 | `PYTHONDONTWRITEBYTECODE=1 python3 -B -（stdin：stdin: 原 Windows 41/332 编译库存与执行逐项对齐；45 Zmodem 子集、两项 OS5、旧 gate skipped）` | 0 |
| C14 | `PYTHONDONTWRITEBYTECODE=1 python3 -B -（stdin：stdin: 固定 Git 文档行号/哈希及 engineering name-only diff；能力台账邻接检索未扩展为完整能力评审）` | 0 |
| C15 | `PYTHONDONTWRITEBYTECODE=1 python3 -B -（stdin：stdin: 固定文档关键段、host controller/Python 原日志、Turing 原报告的签署范围/命令事实）` | 0 |
| C16 | `PYTHONDONTWRITEBYTECODE=1 python3 -B -（stdin：stdin: 三平台原日志均直接含旧完整源码 SHA；当前 status 仅观察父的并行改动，不要求工作副本冻结）` | 0 |
| C17 | `PYTHONDONTWRITEBYTECODE=1 python3 -B -`（仅创建本 REPORT.md 与 receipt.json） | 0 |
| C18 | `PYTHONDONTWRITEBYTECODE=1 python3 -B -`（回读 JSON／报告 SHA 与两文件边界，记录实际结果） | 0 |

C05 exit 2 是允许写的报告目录当时不存在；C06 的 sed 成功而 `.agents` 路径不存在导致 rg／整体 exit 2。两项检索诊断保持原退出，不伪装所有命令成功，也不是测试失败。检索输出过长处按需改成固定 Git 小范围读取；未通过 Cargo 或清理补救。

**独立执行与未验范围。** 本复核 Rust 编译／测试（含预构建）、Cargo／fmt／Clippy／build、Python unittest、scripts/check.py、Windows／GUI、CI 查询／等待／取消／重跑均为 0。独立实际执行的是只读封存验证、Git 查询与日志解析；父和旧 CI 的测试数均明确标为历史证据核对。未访问共享 target 执行任何操作；未改生产代码／docs／脚本，未提交／推送／清理／创建 worktree、分支、容器或 chat。唯一写入为本目录 REPORT.md 和 receipt.json。父收取后可立即关闭本复核。
