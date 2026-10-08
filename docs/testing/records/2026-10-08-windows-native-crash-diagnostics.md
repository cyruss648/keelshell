# Windows 应用异常退出诊断 — 2026-10-08

状态：本次提交只导入已通过新非作者静态／离线复核的 v2 三文件诊断工具和本记录、HANDOFF、ROADMAP；应用、Rust 测试、生产路径及既有 Quality 流水线保持。实际 Windows 诊断由单独手动流水线执行，当前仍未取得原生结果；历史异常原因为 UNKNOWN。

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
