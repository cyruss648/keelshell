# Windows 应用异常退出诊断 — 2026-10-08

## 当前终态修复切片 — 2026-10-08

精确 `ecab60459de5671eea0f0cbfb59359c9478b6e38` 的 [Quality 37726023963](https://github.com/cyruss648/keelshell/actions/runs/37726023963) 已结束：macOS、Linux 成功，Windows 失败。三份完整原日志已实际读回；macOS 1,812 普通 Rust／10 rustdoc／48 脚本、Linux 1,808 普通 Rust／10 rustdoc／48 脚本及各自 OpenSSH 互通通过。Windows 57 打包用例为 53 PASS／4 平台 skip，48 脚本为 46 PASS／1 failure／1 平台 skip，Rust 未到达。原 child5980 HANDLE 在 ACK 前确认属于该 Job，返回后仍 LIVE，parent5508 TERMINAL；控制器却返回 0／active0／reap／drain／errors[]。这证明旧成功判据不足，不能由新通过追溯关闭历史目录超时或 macOS 监听身份 UNKNOWN。

本切片精确导入已通过新非作者限定复核的 Windows Job v3：终止前完整绑定成员原 HANDLE，保留原直接／已验证 debuggee 对象；完整保留 ExtendedLimits 后设置活动准入上限；任意成员打开失败（包括87）保持失败。成功需要原对象结束、实际 reap 和 pipe/thread drain，以及等待前后两次 Accounting 的稳定累计关联数；活动状态保守取两个样本与 pending 原对象的最大值。所有操作共用原 Deadline，不增加宽限、不改变原结果 DWORD 或原测试断言。具体决策见 [ADR 0086](../../adr/0086-windows-job-original-process-terminal-proof.md)。本次仅 Python 验证控制器，不修改应用 Rust 清理路径。

作者封包完整未过滤 59 用例在 macOS 为 58 PASS／1 新 Windows native control UNRUN；exact sealed-v2／frozen-v3 相同末 Wait 累计数7→8模型分别返回假成功0／明确失败125。模型排序在实际 Windows 的可达性 UNKNOWN，历史原句柄失败的具体时序原因 UNPROVEN；这不是原应用 `0xc0000409` 的诊断结果。作者／根已完整读回263输入、两执行 full raw 和实际 owner wait0／reap／group absent／私有TMP删除；新非作者独立完整59用例58PASS／1Windows原生UNRUN、七项额外模型通过，无确认P1/P2；完整326证据输入及两次实际owner终态已根读回。根正式完整工程检查已实际通过，详见下段；新准确Windows仍待执行。

根 `python3 scripts/check.py` 在同831输入／16,341,107字节／相同mode上 actual0：1,812普通Rust／10rustdoc／22ignored未运行，65Python为64PASS／1新Windows原生UNRUN，格式／x.y／严格全target Clippy通过。原默认与2MiB控制器各626阶段（0至625）、10准备期及2观察IO控制通过；六对自有POSIX原父子均ABSENT、保留原退出／超时／中断及elapsed断言、reap与drain成立。owner51700实际wait0／reap／group absent，731.358秒，私有TMP为空且物理删除。完整原日志457,423字节／SHA256 `9430a4fdb615c6c5ae8eeeecb9cb6ddbbe4002c2ded894a74e146e96f7c259e1`，完整命令与终态收据在忽略的 `work/windows-job-terminal-root-import-20261008-v3`；最终非作者根复核保存在 `work/windows-job-terminal-root-independent-20261008-v3`。门禁后仅四份状态文档更新，其余827输入字节与mode保持；这次没有新UI包或Windows原生运行。

新 Windows 控制须在已有至少两个嵌套成员时验证降低上限 setter 和真实迟到 CreateProcess 拒绝；原五项 CDB 前置控制与原 a717400 的706应用测试仍 UNRUN。先在 feature 分支取得实际 Windows 证据，再决定主线整合。旧 v1／v2 拒绝、原应用异常、控制器失败与完整材料均保留。生产发布信任和 Unix 清理候选未混入本切片；MCP 始终由 KeelShell 对外提供服务。完整产品、其它平台桌面、六目标发行与安装验收继续 OPEN。

完整材料保存在忽略的 `work/windows-job-terminal-author-20261008-v3`、`work/windows-job-terminal-root-readback-20261008-v3` 与 `work/ecab604-platform-ci-root-readback-20261008-v1`。ecab 新 Mac 原日志617,170 B／SHA256 `20d2609eae84a200edb3df7dffff911387acba6c82886999e33eb8cdd6d69984`，Linux644,770 B／`6541bb6cd5292aa27dc8fc54967b00e5d48e845139fb1a6c02f58c7f70c3dd40`，Windows49,974 B／`2663c18c7da9543cef477be2510055e185194383f2679032241e22dd4ace690a`。三份日志下载 owner 的实际 wait／reap／group absent 保留，不能把早期 ANSI 拒绝的下载 actual1 改成成功。

状态：`437f896` 只导入已通过新非作者静态／离线复核的 v2 三文件诊断工具和本记录、HANDOFF、ROADMAP；应用、Rust 测试、生产路径及既有 Quality 流水线保持。首次真实 Windows 诊断已执行，但控制器回归阶段失败，未到达五项 CDB 控制或原应用测试；历史异常原因为 UNKNOWN。准确结果见下方“首次真实 Windows 诊断”。

## 原问题与准确输入

已推送 `a717400bf791bad43a293db50a02a64fb58bcfc7` 的 [Quality 37696576523](https://github.com/cyruss648/keelshell/actions/runs/37696576523) 中，macOS／Linux 成功；Windows 的 706 项应用测试在已报告 439 通过和两项 ignored 后以 `0xc0000409` 异常退出，没有 Rust FAILED／panic／完整总结。这个状态不能单凭名称归因为栈溢出、测试夹具或产品缺陷。原完整日志和失败结论保留，见[平台记录](2026-10-08-file-browser-platform-ci.md)。

本诊断固定原 a717400 源码、Rust 1.98.1 x64 MSVC、locked 构建、706 个不同测试和原四线程完整运行。原 120 秒夹具期限、栈和测试集合不变，不加名称筛选、重试或提高栈大小。Windows runner 为 windows-2025，job 总期限 90 分钟；新解析到的镜像和运行时身份须实际记录，不沿用历史镜像。

## 诊断机制与证据边界

Git、rustup、rustc、Cargo、测试列表和 CDB 均使用有界进程 owner。Windows 子进程挂起创建，先分配无 breakaway、kill-on-close 的 Job，再恢复主线程；管道读取、收尾、实际 wait 和 Job 活跃检查消耗同一绝对期限。它约束普通 CreateProcess 后代，不能宣称是安全沙箱或控制无关 broker／WMI 进程。

只使用 runner 现有 Microsoft SDK CDB／dbgeng／dbghelp，运行前检查 Microsoft Authenticode 与完整 SHA-256，没有下载器或不可信降级。原目标句柄独立取得 DWORD，CDB 退出另记；FIRST／SECOND 钩子均记录实际 `.lastevent` 并以 `gn` 继续。真实非零目标 DWORD 优先于超时和诊断解析失败，不得被 CDB exit0 或缺信息掩盖。

运行应用前必须在真正 Windows 上通过五项控制：exit0、exit7、合法 exit259、可处理的 c0000409 FIRST 及 RaiseFailFastException SECOND。这些控制与 Job/API、CDB 格式、PowerShell DWORD 传播本机均为 UNRUN；不以 macOS 离线测试模拟通过。

共享原始 CDB／目标输出只留私有目录、有大小上限且不上传；公开产物只重建数值字段、固定模块枚举及数字栈帧。未知、重复或不完整字段拒绝整份投影。nonce 不认证输出来源；只能用于受信任的准确测试源码，不能防恶意目标伪造允许的数值。禁止内存转储、环境／凭据／任意符号文本进入上传清单；不推测 OS 是否自行生成 WER。

## 已执行的限定检查

作者最终 36 项离线测试实际通过；新非作者独立重跑 36 项并增加三项真实 POSIX 输出／中断／失败控制，分工的 43 项投影／chance 控制也实际通过。真实 POSIX 进程、输出与清理和 synthetic DWORD／CDB 接口分开记录；不把前者称为 Windows 原生。v1 因准备进程所有权、chance 和共享输出上传的 P1/P2 被拒绝；v2 修正已在新独立限定审查关闭，旧封包与失败完整保留。

根按 absent preimage 导入三个精确 postimage；全部 script 回归 actual 0：42 项（原六项＋新增36项），3.851 秒，完整原日志 9,349 字节，SHA-256 `5c88f95ce6c00c6efb3d5fc0bc5846761e7c03560470b933d5ab06e327b6c750`。实际 owner wait0、已 reap、所属组消失，外层工具终态0；五对真实自有 POSIX 父进程／后代均确认 ABSENT，管道排空。并发哨兵确实出现在私有流，公开投影拒绝，同时保持受控 DWORD；这不证明真实 Windows 异常。

导入三文件 SHA-256：

| 文件 | 字节 | SHA-256 |
| --- | ---: | --- |
| `.github/workflows/windows-native-crash.yml` | 4,909 | `c0743bb1e74c551f2ee6d916e215e937f7e6328dbdd5dcc856315fb05b24c2f6` |
| `scripts/windows_native_crash.py` | 54,926 | `1bf3e6390ff95b320c94ff87c01cb938bf4f52cf8929c8fbb8717e16f902d847` |
| `scripts/test_windows_native_crash.py` | 29,011 | `e14ad43852f9a357b597a4a69478140bc717c340aaba007b8aa3e5375b1481d8` |

原配置恢复提交 `561eb9fb918a3d60955877461a68706d1791a787` 的工程／包／限定原生和独立结论保持，见[整合记录](2026-10-08-configuration-recovery-main-integration.md)；本次只增加 Python、workflow 和状态文档，不宣称重新运行同一 Rust 全量或原生界面。现有 x.y、Rust 格式、完整 Rust／Clippy 及标准包证据只按其准确输入保存；本次新 Python 回归另记。提交后最新 Quality 和手动 Windows 诊断须取得准确 SHA、完整 job 日志与允许产物后判定。

完整材料在忽略的 `work/windows-native-crash-diagnostics-author-20261008-v2`、`work/windows-native-crash-independent-20261008-v2` 与 `work/windows-native-crash-root-import-20261008-v2`。Windows 桌面、原生产触发原因、跨平台完整产品和发布安装继续开放，不发布完成产品标签。

## 新提交 Quality 的独立失败边界

配置恢复精确 `561eb9fb918a3d60955877461a68706d1791a787` 的 [Quality 37716374658](https://github.com/cyruss648/keelshell/actions/runs/37716374658) 已报告 Linux job 失败，macOS／Windows 在记录时仍运行。根取得并全文读取 Linux job 的 92,524 字节原日志，SHA-256 `71e948a4d77d50923faf53ff2b9bdc53b93884259d8ac02db61433a8bbdb54c6`：AI 库81项及HTTP20／4／5组通过，随后目录控制器 selected_actual_child_cwd 的 Codex case 到原8秒 Ask期限而返回 Timeout。实际观察为 WorkspaceReady 11,492µs、CheckingCli 5,419,654µs、CliAdmitted 5,419,658µs、Finalizing 8,001,939µs；只有 version／help／features 三个实际子进程阶段，完整 scratch 为空。应用／core恢复测试尚未到达，不将其表述为恢复功能在Linux通过或失败。超时的具体触发机制仍需源码和准确新诊断；不增期限、删测试或用本机PASS覆盖。该失败与旧 a717 Windows异常独立记录。

## 首次真实 Windows 诊断

[Windows Native Crash Diagnostics 37717377052](https://github.com/cyruss648/keelshell/actions/runs/37717377052) 运行准确 driver `437f89609f05e18f4ce3d695b0df53ee0bc763d5`，应用 source 固定为原 `a717400bf791bad43a293db50a02a64fb58bcfc7`。job `113116865613` 的完整原日志为 62,834 字节，SHA-256 `a9cb2c565dae537d6988cd493d4f37ecae4138c04a7dde399c8ca8720be285c6`，根下载 actual wait0、已 reap／所属组消失，未经过终端解释。

现有 Microsoft SDK 的三组件 inventory 验证通过，版本均为 `10.0.26100.8249`，Authenticode 为 Valid／Microsoft Corporation；这只证明本次 SDK 检查，不证明 CDB 异常控制。随后 36 项 Python 控制器回归在 2.500 秒内报告 1 failure、1 error：

- SIGINT 树控制读取空 PID 文件而触发 JSONDecodeError。源码中 `Path.write_text` 先创建文件，观察端仅凭 `exists()` 发中断；存在文件不能作为完整内容已发布的 READY。
- parent-success／detached 控制事后按 PID 查询到 child `5068` 为 LIVE，但 owner 回执已报告 direct reaped、Job active0、pipes drained。旧夹具未保留该后代的原始进程句柄，无法区分 PID 重用、终止时序或真正所有权缺陷；准确原因仍 UNKNOWN。不得用回执或 PID 单方结果覆盖另一方。

五项 CDB 前置控制、原 706 项应用测试及应用异常上下文全部未运行。只取得允许产物 `keelshell-cdb-inventory.json`，747 字节，SHA-256 `963966dc98493f54b625314821a577cb13757e49480553bb9def4f3430513541`；原 zip 460 字节，SHA-256 `0709f93a1796f9f9d2578e9a96a8705c1d817cea2affd0103493cfd5d0b5d2fe`。没有原始 CDB 输出、内存或应用上下文上传。本次 FAIL 保留在忽略的 `work/windows-native-crash-root-import-20261008-v2/native-attempt-01`。

## 精确 437 Quality 结果

[Quality 37717370699](https://github.com/cyruss648/keelshell/actions/runs/37717370699) 的三个 job 已全部结束且失败。三份完整原日志均实际下载、全文读回并保持；结果不能被此前本机 PASS 覆盖。

| 平台 / job | 原日志字节 / SHA-256 | 实际失败阶段 |
| --- | --- | --- |
| Windows / `113117074000` | 48,117 / `47d4432b1e3f9bc23a7abc12e40c4f9207d11561cc85ef39649b6b575cf7995a` | Python 42 项、1 failure／1 skip；同 parent-success 控制查询 child `1524` 为 LIVE。未运行 Rust 应用测试。 |
| macOS / `113117074053` | 157,118 / `33ba938780fc3c13c62c3bdfa1155dd5ec8943e809f81cbd617778094512a8f7` | AI 库85项及 HTTP20／4／5组通过；`local_agent_process.rs:1667` 的 Claude ProgressAsk 取消后连接拒绝断言失败。旧端口再次 connected 的准确进程身份与原因未确认。 |
| Linux / `113117074221` | 95,057 / `e851e46eb78f275d696c4df20fea5804c67241a25655bd85eaf2a4a3fff4534b` | AI 库81项及 HTTP20／4／5组通过；Codex selected_actual_child_cwd 再次在原8秒 Ask期限 Timeout。 |

Linux 第二次记录为 WorkspaceReady 10,545µs、CheckingCli 5,403,477µs、CliAdmitted 5,403,481µs、Finalizing 8,001,531µs；只记录 version／help／features PID `6054`／`6055`／`6056`，fixture executable 51,262,112 字节、scratch entry0。阶段接收时间不能直接当作发送端实际执行时长。macOS／Linux 均未到达配置恢复的 app／core 测试；完整日志与读回回执在忽略的 `work/exact-quality-437f896-20261008-v1`。保持原期限、并发、测试集合和失败证据，逐项修复后再取得新准确提交结果。

## READY 与原进程身份修正的限定本机验证

本切片只修改 `scripts/test_windows_native_crash.py` 及三份状态文档。夹具关闭完整 PID staging 文件再原子替换，父进程等待明确 ACK 后才能执行原退出／挂起路径。Windows 观察端在 ACK 前保留父子原 HANDLE、检查实际 Job membership 及 LIVE；控制器返回后先以 timeout0 观察原 HANDLE，再单独记录事后 PID 查询。原 HANDLE 的 LIVE／QUERY_FAILED 仍失败，未增加清理宽限、更新期限或假定历史 PID 重用。POSIX 仍确认真实子进程组和终态。

六项新增控制涵盖旧 exists 条件对实际空文件触发中断、新夹具拒绝真实空／部分发布、非法身份拒绝、保留句柄与重开 PID 的模型反例、非 Job 成员拒绝，以及原句柄 LIVE／查询失败拒绝。36 项原测试正文 AST 完全保持；原 5.4／8 秒绝对期限、elapsed＜3、退出0／7、超时124／中断130、Job active0、实际 wait／reap、完整 pipe drain 和无 cleanup errors 均保留。生产控制器、workflow、Rust 源码／依赖／锁文件及原706应用测试条件不变。

根完整读取作者101文件／535,454字节后按精确 preimage 导入唯一代码文件；新文件40,584字节，SHA-256 `caaa101a8183da08aee8cd401ca2afdc63653172d672ad044680e5527eca3ba2`。根全部脚本48项实际通过，0 skip、4.575秒；完整raw12,240字节，SHA-256 `1a9eec88e3b76f7c48edc2d7ae37cd48489e01128cb0797ea7adb4167fe8bcfd`。owner `4831` 实际 wait0、已 reap／group absent，私有TMP零遗留并删除，外层工具实际结束0；完整828输入在执行前后保持。

作者的macOS 42项及根48项是本机Python回归，原Windows两个FAIL均完整保留。新的非作者已完成限定源码及作者／根完整原始证据复核，独立42项实际通过；补充两个真实POSIX控制确认LIVE／QUERY_FAILED仍被终态断言拒绝，五个明确模型覆盖ACK前失败与句柄关闭。首个补充脚本因自有receipt键错误实际1，完整失败保留，纠正后实际0。新复核无P1/P2阻断，只允许本切片提交及准确Windows诊断；原HANDLE／Job原生控制、五项CDB前置控制及原706项应用测试仍未验证，不以本机PASS关闭。Linux／macOS的独立失败继续OPEN；SHA2开发成本优化和Unix后代清理终态另设切片，当前尚未导入。生产发布信任候选也尚未导入，Python signer／OpenSSL的限定验证不关闭Rust、实际签名、平台发行及安装验收。

提交前重新执行格式、x.y和diff检查，均实际0；严格 `cargo clippy --workspace --all-targets --locked -- -D warnings` 最终实际0，81.952秒，owner `91259` 已wait／reap／group absent，外层工具实际结束0。完整raw918字节，SHA-256 `bc2c713bf8cc0a59efac6c25583e5d5f0a8fdf9d1e46ad8b43e58b9cd0445aca`。第一轮Clippy收集器在54秒自身编译边界触发后，group信号返回EPERM，外层实际1且未取得Cargo实际wait；原Cargo退出保持UNKNOWN，原1,059字节日志和收尾失败保留。只调整编译收集器的绝对边界并提前保存owner身份，再取得上述完整结果，没有修改产品或测试期限。当前Rust源码及原完整工程／标准包证据保持原范围；本次未重新宣称Rust全量测试或原生界面通过。

## f3 原句柄真实终态失败与最新平台结果

修正提交 `f3c88209ae314f4eb3ae5268bcdf8fd1f7b8b8e7` 已推送并核对远端 main 相同、ahead/behind 0/0、当时工作区干净。新的 [Windows 诊断 37721774239](https://github.com/cyruss648/keelshell/actions/runs/37721774239) 通过 SDK inventory，但 42 项控制器回归于 2.766 秒出现 1 failure；五项 CDB 前置控制和原706应用测试仍 UNRUN。完整 job 61,601 字节／SHA256 `94614c2b23438df942c49f088a61f3320b53f88b4ba6867c1387dd3d810d5e7a` 已读回。parent6248 与 child9692 在 ACK 前以原 HANDLE 确认当前 Job 成员，返回后 parent TERMINAL、child LIVE；receipt 却返回0、active0/reap/drain/errors[]0，耗时0.082584秒。该次真实身份排除了 PID 重开歧义，证明旧控制器的成功终态条件不足；不将此原因追溯给旧没有原 HANDLE 的 child5068／1524，原应用异常原因仍 UNKNOWN。READY／partial／SIGINT修正已通过，其余五个树控制也通过。

唯一允许产物仍为 SDK inventory：zip463字节／`8cd9ad4e79471f2afcf0dade12faf088aa519b8ada9846085c319fcc6a19fa00`，内唯一 JSON747字节／`69ed88dcc7d672f9f366e0135d3c5f23d495cdcae262155873e1e87d35c8dbbc`，三项 Microsoft签名 Valid、版本10.0.26100.8249；artifact owner75946实际wait0/reap/group absent，外层69273已消费exit0。首次日志展示的固定标题查找 StopIteration 只发生在完整下载和 CLI wait0之后，外层57346实际1；该次 CLI PID未记录、其组收尾未确认，不回填成功owner。随后完整读回和分类明确了上述原生失败。

精确 [Quality 37721737467](https://github.com/cyruss648/keelshell/actions/runs/37721737467) 已全部结束：Linux成功，Windows／macOS失败。Windows57打包通过，脚本48项1failure／1skip，完整 raw49,639字节／`3af6534e2c56b39c4ba61709c1287a26696c175c409d8a91d507bc07863943e0`；parent2176 TERMINAL、同Job原child3960 HANDLE LIVE，返回0／active0／清理无错，elapsed0.07136秒，Rust未到达。macOS已完成workspace，small-stack Codex ProgressAsk取消后connect断言失败，具体监听身份UNKNOWN；完整 raw505,405字节／`acbbcf96d8e70808de0484b3dd4fb806ebbebcb559679c4db39d88a52bef0d51`。Linux完整 raw640,572字节／`abf67f482fc6a9018027f4dc00beaaaab3a479fc4177f39ace503fc9fb8ef489`，工程门禁及OpenSSH互通通过；原目录超时的准确原因仍UNKNOWN。两个Unix下载owner62042/62043与Windows12993均实际wait0/reap/group absent，外层96490／24673均消费exit0。

真正Windows Job终态候选现仅在忽略目录准备，拟在原期限内保留成员原HANDLE并要求其signalled，不通过删原断言或追加fixture后置grace规避；新的独立审查与实际Windows仍待完成。根另已通过仅SHA2开发优化的完整工程门禁，详见[优化记录](2026-10-08-development-sha2-build-optimization.md)，这不关闭本Windows失败或新原生产品范围。
