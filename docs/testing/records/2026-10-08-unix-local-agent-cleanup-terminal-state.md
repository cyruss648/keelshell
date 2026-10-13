# Unix 清理动作所有权和终态 v4 — 2026-10-08

## 有界未知观察：根实际完整通过 — 2026-10-09

新非作者静态复核准入四路径精确导入。EPERM只保留未知并沿原三秒零信号观察，实际ESRCH仍须在原期限内且原leader已回收；其它未知errno立即拒绝，持续EPERM及迟到合成ESRCH失败，动作许可、取消／重入／Drop、Windows和全部原controller保持。原“EPERM一次即失败”的断言域显式修改，完整34,991字节前像与旧失败保留；新增四控制，原12变为16，不将契约变化冒充等价重写。

根锁定库编译owner60053实际wait0，23.961925秒；全部16项Unix控制owner60678实际wait0，9.737468秒，16PASS／1自托管ignored。原完整 `scripts/check.py` owner66008实际wait0，903.893174秒。三个owner已reap、所属组缺席，无外层timeout／观察错误，三个私有TMP为空且实际删除；835输入／16,438,546字节及mode在各阶段前后相同。专项与完整运行各40合法group记录＝19leader_reaped＋21严格complete，无fallback；各21个独立根逐一实际ENOENT，不能以外层TMP推断。

完整检查通过65Python＝64PASS／1Windows原生skip、x.y／格式／全workspace all-targets locked严格Clippy；普通Rust1,828PASS、rustdoc10PASS、23ignored未运行。rustdoc包含1个no_run仅编译。原default626连续0..625和end、small-stack626连续和end完成；readiness另外61条default记录单独归属，不能混成687。10准备期控制和2观察IO控制保持原结果。原流中的受控panic、block未来兼容及大型unwind链接警告保留，不能写成零告警。

完整stdout431,153字节／SHA256 `35a540984d06d8f7c963cd5d9aa621807f58b6a82d0df2ec4f3f11a2bce682ac`；stderr377,414字节／`363c8354324907652f0a8ce7e45aa308cbd69ba014d6c0c621cb234bf982f1e3`。材料在忽略的 `work/unix-bounded-unknown-policy-root-20261008-v1/`，新的非作者原流复核单独绑定于 `work/unix-bounded-full-gate-independent-20261009-v1/`。本次收尾仅HANDOFF／ROADMAP／ADR／本记录四文档，执行输入与文档后继分别存映射。

本机自有探针实际present→同一未reap原Child的zombie／EPERM→原wait0／ESRCH，正常非零signal为零；源码经独立修复期限P2后的准入，实际ownerwait0且工程输入保持。它只证明本机机制，不能归因旧v7／v9，不证明所有非直接后代、主动detach、blockingworker或原生UI。功能提交／推送须完成最终文档后像与证据复核；准确平台CI、完整MCP／CLI自身身份、桌面与发行验收保持各自未完成边界。

## 有界EPERM未知观察候选：源码准备，Rust行为UNRUN

根隔离机制探针实际owner wait0／reaped／所属组缺席，0.126414秒，835工程输入不变；同一自有Child的native PID／group在live为present、确认未reap zombie后为EPERM，原Child实际wait0后signal-zero为ESRCH。其正常路径非零signal为0，scratch为空且删除。该探针仅证明本机退出过渡机制，不证明旧v7／v9原因，也没有修改生产契约。忽略证据在`work/unix-zombie-zero-observation-root-20261008-v1/`，旧v7／v9失败继续保持。

新的候选明确改变“signal-zero EPERM首次立即Failed”的契约，将其保留为同一原三秒内的未知观察；仅实际wait及期限内native ESRCH建立成功。持续EPERM／迟到ESRCH仍CleanupFailed，EIO／EINVAL／ECHILD仍首次拒绝；取消／重入／Drop不重置信号许可或deadline。原有EPERM恰好一次观察测试的完整前像保留，候选显式移除该errno的立即失败断言域并改名，其它三个errno和断言保持；原owner错误控制及其所有失败断言保持，只是持续EPERM沿原三秒到期。

新增四项有意义的控制：

1. 瞬时注入EPERM返回Pending，原leader已实际reap／成员仍活跃；取消后原deadline重入，成员经自有nonce IPC退出，仅真实signal-zero ESRCH准入Complete，累计动作许可一次。
2. 持续注入EPERM经历取消／重入，保存原三秒deadline，期限到达后typed失败并锁定；成员仍活跃，后续重入与实际Drop不得增加动作许可。
3. EPERM后仅在deadline之后返回的合成ESRCH必须被拒绝；观察期间真实组仍present。此控制只验证迟到结果准入，不宣称真实OS组缺席。
4. EPERM观察取消保持Observing及未完成，真实成员仍活跃；限定IPC收尾后实际OwnedChild Drop动作许可计数仍一次。

当前十六项Unix行为控制、全部Rust锁定编译、严格Clippy、原默认／2MiB各626和完整workspace／doc门禁均UNRUN。原controller源码、四线程、全部原请求／准备／外部期限与断言、Windows分支、依赖及toolchain保持。候选只存于忽略的`work/unix-bounded-unknown-policy-author-20261008-v1/`，先经过新的非作者静态审查；本节不是导入、提交、推送、实际Rust通过或产品完成声明。真实leader已reap的非child zombie归属边界和各平台原生验收仍OPEN。

## v9 临时诊断：原完整检查实际失败

v9-v3候选经过新非作者源码复核后，仅临时用于一次原完整 `python3 scripts/check.py` 诊断；同步诊断I/O可能扰动调度，没有进入生产功能提交。执行前834输入／16,416,622字节，所有原测试、原四线程、原三秒及控制期限保持。owner13713实际wait1／reaped／所属组缺席，189.323468秒，无外层timeout或收尾错误。65 Python为64PASS／1Windows原生skip，x.y／格式／严格全target Clippy通过；AI库98项实际96PASS／1FAIL／1ignored，十二Unix控制实际11PASS／1FAIL。

唯一失败为 `long_child_tmpdir_uses_short_private_root_for_member_and_held_leader`：saved group19198的 `group_observation_errno` 为EPERM、deadline_expired=false，之后 `group_absence_failed_latched`，原unwrap返回CleanupFailed。不能把其它独立注入控制的errno归入该身份。原default626、其它workspace／rustdoc、readiness10、observer IO2、small-stack626均未到达。本次不是原v7 ClaudeCode继承管道场景原因的证明，旧原因保持UNKNOWN。

完整stdout13,929字节／SHA256 `cf93785d1e16e15132c0d97e11a762b8051ecb592646830d1a73323af84fdb16`；stderr11,694字节／`4f20bc65573980644c14b9225cac5725ebb32141cec938b5442e9842e8970139`。原流实际读回，group control共31条合法记录：15leader_reaped、15严格complete、1非成功fallback，16个独立短根。15个complete根逐一实际缺席；失败根 `/tmp/ks-ug-lr5vpL` 保留，fallback报告leader_reaped／group_absent为true且cleanup_claimed_success=false，不能改写原失败。原外层TMP为空并保留，不能以此推断每个夹具终态。

finally精确恢复process.rs为60,718字节／`977e2c323b9bd198644a08f2b85a076b13ae1e655a36a6a60218cb7de2a17055`、mode0644，以及全部834输入原路径／字节／mode。恢复后的locked integration no-run owner19372实际wait0／reaped／所属组缺席，23.069421秒；stderr271字节／`8bad220ec6c0cd85c63e21b3b616a7ed3cdf58038413fc70b9ba41ddb6188288`，TMP为空且实际删除。此编译通过不证明完整门禁通过。材料在忽略的 `work/unix-local-agent-cleanup-diagnostic-root-20261008-v9/full-experiment-v3/`，提交／推送仍不准入，后续独立原因分析保留所有失败证据。

本次状态收尾只更新HANDOFF／ROADMAP／ADR／本记录四份文档；本地磁盘清理另外有自己的记录及声明文档后像，不倒填为本次原执行输入。生产清理、信号次数、错误拒绝和Windows路径没有新增变化。

## v8 临时 debug 观察实验：未重现旧失败

原v7完整工程仍失败，准确分支和原因UNKNOWN，功能提交／推送不准入。新的非作者完整核对33作者payload及最小源差量，只准入临时owned调试实验；source-review包49payload／3,394,588 B，manifest SHA256 `a894b2a93624cac47a06eb74fcfe227e02d019c311ad1ac2f393433081dbe7f6`。实验前基线834输入／16,412,549 B，唯一process.rs从60,718 B临时变为60,834 B，四个hook选入test/debug、配套discard及注释；其余833输入／mode不变，临时834／16,412,665 B。没有增加signal／wait／observe／retry或改原期限、控制器、断言。同步stderr输出可能阻塞、写错误panic或扰动调度；它不是生产诊断或修复。

原locked integration no-run owner14177 actualwait0／reap／group absent，14.292222584秒，stderr271 B／`2ed8092da86d8ca396cc69dd68e8f2172b8f3f345336b7488e95c620023e023c`。原默认 `cargo test -p keelshell-ai --test local_agent_process --locked -- --test-threads=4` owner14495 actualwait0／reap／group absent，14.105641125秒；stdout103 B／`f8a99c811001f2d4972fa46891a44f10bab2f59e6df79d5498f53a636e4fdbf0`，stderr115,609 B／`ebb10c7fb3614aa670e8bd2b7fdd5127318641512d6242d8fb040d5b3337eac5`。root会话78851实际exit0已消费。全流解析626连续阶段0..625、default模式及最终controller end，0失败hook、0不合法记录。保持原test-threads参数；该harness=false控制器自建Tokio runtime有2workers，不宣称四case并发。两个owner无outertimeout／收尾错误，所属TMP为空且实际删除；所属owner组与TMP不能代替每个未记录fixture身份的终态。四项owner-local env覆盖为TMPDIR／CARGO_BUILD_JOBS=2／CARGO_TERM_COLOR=never／RUST_TEST_NOCAPTURE=1。

实验后唯一process.rs精确恢复60,718 B／`977e2c323b9bd198644a08f2b85a076b13ae1e655a36a6a60218cb7de2a17055`，整834基线字节／mode／set返回16,412,549 B。随后恢复源码的同原locked no-run owner17987 actualwait0／reap／group absent，2.501265542秒，stderr270 B／`0c0fb48ddb7250c1a35702442717bf0f3be923361b5747f360ef65fcebcdc917`；root会话84558 actualexit0已消费，TMP实际删除。这个子目录只有no-run，通用结束banner不算第二次默认626运行。完整原流与所有输入已逐条读回，临时debug选型未留在产品源码。

本次没有重现旧CleanupFailed，也没有失败hook可定位旧分支；通过不能证明同步日志无调度影响或旧问题已修复。恢复后的完整workspace／doc／readiness／observer IO／small-stack没有在本轮重跑，原v7未到达范围继续保留。材料在 `work/unix-local-agent-cleanup-diagnostic-root-20261008-v8`，新实际运行独立复核另行封存。收尾仍只改四份状态文档，其外830执行输入与v7原执行图相等；当前map单列，不将文档收尾倒填为实验执行输入。完整对外MCP、供应商登录、UI、其它平台桌面与发布目标继续OPEN。

## 最终 v7 实际失败与提交边界

原完整 check owner58397 actual wait1／reap／所属组缺席，131.519784 秒，无外层timeout／cleanup errors，TMP为空且保留；stdout21,502 B／`7b6e0392edb9420f211698d718beda5b67eb14254f965fd2a4de1bcde50015ab`，stderr87,096 B／`8d365dfcc1a8b5e2f5c1b1400f57f326cf9c611604b55f207bcb642eb408d068`，原完整双流已读回。65 Python=64 PASS／1 Windows skip，x.y／fmt／严格Clippy通过。普通 Rust 已通过126／ignored5；custom目录20场景另算。AI unit 12控制在workspace重跑通过，32合法control JSON／17严格complete／17个独立根实际ENOENT。以上只是已到达范围。

原 default 本地进程 controller 到408连续阶段0..407，在 ClaudeCode progress_ask case8、`leader-exits-descendant-inherits-pipes` 返回 typed CleanupFailed，原local_agent_process.rs:1635 unwrap失败；真实单次端口断言未到达，准确 wait／观察／errno 分支仍UNKNOWN，cfg(test)诊断未在该普通库构建启用。原626剩余、app/core/session后续、doc、readiness10、IO2、小栈626均UNREACHED。outer owner组缺席与17 Unix夹具根删除，不证明该未记录 PID/PGID 的原case身份终态；不推定其存活或泄漏。

当前功能提交／推送不准入，失败不覆盖v4/v5或v6记录。root只收尾HANDOFF／ROADMAP／ADR／本记录四文档，其余830执行输入（含既有文档）保持准确v7执行字节。新的非作者最终完整组合／原始失败证据复核已封存V1与数量精度V2，拒绝功能提交。后继v8临时诊断实际结果及精确恢复见本文最新段落，不作为修复；原三秒、所有信号／等待／观察次数及控制期限保持。其它平台、MCP、UI与发布仍各自独立开放。

## v6 原控制通过、完整门禁失败与 v7

v6 编译 owner41142 actual wait0／reap／组缺席，2.586363 秒；stderr255 B／`10fc902ae27e5dd7d70bfdfb619b69a13bed75abc9dd2f15a06b5b71b5156fb2`。原十二控制 owner41297 wait0／reap／组缺席，5.971428 秒，12 PASS／0 FAIL／1 ignored；stdout1,728 B／`1c96f3a3d09ffbe1ddc3513c80666e9ea42d742a831cd01da601212e4537d98e`，stderr8,084 B／`73673c15a23342c9cb5b505ad4c7a32e95e16e0448ca258961247d01d391cdba`。32 合法 control JSON／17 严格 complete／17 根逐个 ENOENT、两个 owner TMP 空且实际删除。12 条失败诊断属于通过的预期拒绝控制；日志缺直接 test-name→PGID 标记，不能把其注入 errno 倒填为旧 held 失败原因。旧 v4／v5 原因仍 UNKNOWN。

之后完整 check owner45283 actual wait1／reap／组缺席，141.885378 秒；stdout4,282 B／`b887d7f2b082c44f5f24f8a19a3c651f65a9fb14e9258b658986c0c6d96a60d9`，stderr1,436 B／`80a68bee6ff384b19adccac6a5feb1a92f2951fde260e802644e089f5b64a0e4`。65 Python 为64 PASS／1 Windows skip，x.y、格式通过；Clippy 唯一错误为 fixture line424 单字节数组 byte_char_slices，actual command101。完整 Rust、doc、默认／小栈／readiness／IO 未到达，没有本次新 fixture roots，socket receipt 不合格不能解释为另一个运行中的 cleanup 失败。外层 TMP 实际空且保留，失败材料保持。

v7 只把 release_leader helper 的 &[b'Q'] 改成 b"Q"，实际传输同一个 Q 字节；不降低 lint、不改十二测试正文、nonce/member 协议、动作次数或原期限。新组合完整 check 当前 PENDING，材料在 `work/unix-local-agent-cleanup-root-gates-20261008-v7`，同样要求完整输入 bytes/hash/mode 前后不变与每个私有根 strict complete/实际 ENOENT。新源码/工程最终非作者复核及准确后继 CI 仍待完成。下文 UNRUN/PENDING 保留各自历史时点。

## v5 根实际结果与 v6 诊断

v5 编译 owner 4872 wait 0／reap／组缺席，23.114435 秒，stderr 256 B／`a976f22aeeb08d21513ca20b83677d1b0220eb2c94e96aba93ddfcbc03cdd7f7`，TMP 空且删除。原控制 owner 5800 wait 101／reap／组缺席，7.631463 秒，11 PASS／1 FAIL／1 ignored，85 filtered；stdout 1,861 B／`7ec553ed66aa19a38aa9b735488e28deaf0bb321e4beb4343a6db8fa3f75700f`，stderr 7,165 B／`0fef56104d9ca9b2abd2a5b2bfc51965ef88196b4896c05af91cd2f3cc1aec36`。完整 check UNREACHED，不能声明本轮 Clippy／完整工程通过。

独立解析 32 合法 JSON：15 leader_reaped／16 严格 complete／1 非成功 fallback；17 根实际 16 ENOENT／1 保留 `/tmp/ks-ug-2STxo9`，inode 127661550／device 16777232。两私有 stderr 均 0 B，无 setter EINVAL 或 helper panic；准确生产失败分支 UNKNOWN。fallback 原 leader reap、PGID 5925 缺席、cleanup_claimed_success=false。新非作者根终验 ENGINEERING_BEHAVIOR_FAILED_NOT_ADMISSIBLE，MANIFEST `b0cee677718ec9110eb8e8d4d62aa9622d0d04e3af81d1fad6639c628f3f317e`，61 payload 已全文读回；v4 原失败保持。

v6 只增四个 cfg(test) 失败后观察，记录 wrapper wait timeout／raw errno／native ID、group errno 与期限边界。无额外 wait／signal／observe／重试或成功；独立逆变换精确恢复 59,167 B 原源码，十二正文与 v5 fixture 34,994 B 保持。post process.rs 60,718 B／`977e2c323b9bd198644a08f2b85a076b13ae1e655a36a6a60218cb7de2a17055`；新非作者 MANIFEST `eb477516283a4bc3d0bba10ba66b34fdc7bcc126c05aa53997a9b0e03c135a83` 完整读回。仅准入锁定编译与原控制，当前执行 PENDING。失败 I/O 交付及更早未覆盖分支仍有边界，缺少诊断不证明原因。第一次隔离 rustfmt 中间候选变化未观察，按 UNKNOWN 修正；末次格式 0 只为格式解析。

新材料在 `work/unix-local-agent-cleanup-diagnostic-root-20261008-v6`。下文 PENDING／UNRUN 保留各次执行前历史，以本段及后续实际结果为准。

## 根 v4 失败与 v5 重验准备

v4 锁定库编译 owner 30357 实际 wait 0／reap／所属组缺席，26.623 秒；raw 345 B／SHA256 `5a629ee0a177df8e6eddd3395f672da51eb604483488a5e5f210e68cc93bbbef`。十二控制 owner 31044 实际 wait 101／reap／组缺席，7.489 秒，11 PASS／1 FAIL／1 ignored；raw 9,088 B／`2b514d97a67b5aed5675bfb858cad3b581d6f11ded7b0ada99707f98b1359621`。失败 `cancelled_actual_leader_wait_reentry_never_signals_again` 到达取消／重入及保存期限断言，最后 cleanup 返回 CleanupFailed；具体生产错误／到期分支 UNKNOWN。完整门禁未到达，不能按成功计数。

合并流有 31 条合法 JSON、1 条不合法；15 条 phase=complete 中 1 条必需键被并发文字污染，所以仅 14 条严格合格。17 个命名私有短根逐个 lstat：16 ENOENT，1 失败保留 `/tmp/ks-ug-vtKjzI`，另保留失败外层 TMP。held leader stderr 367 B／`ccfd744331aec91d91b9530631512e616de27c7ea09816e2485c9307d7be5037`，实际 accepted stream 配置 100 ms read timeout 返回 OS 22 EINVAL 后夹具 panic。fallback leader 已 reap／原组缺席且 cleanup_claimed_success=false；这不改写原失败。新非作者 v4 根终验为 ENGINEERING_BEHAVIOR_FAILED_NOT_ADMISSIBLE。另 1,024 次独立 accepted／peer setter 探针零错误只界定标准库边界，不能证明原孤立失败原因或候选成功。

v5 新作者与新非作者完整源码复核仅准入根编译／原十二控制／完整检查，无确认 P1/P2，不能准入提交或原生产品验收。唯一测试源码变化是 held leader 17 字节读取 helper：明确 nonblocking、同一个 min(now+100 ms,parent 10 s) 绝对期限、offset 完整读取、保留原 2 ms 有界轮询，不在短读／Interrupted／WouldBlock 重置期限，EOF／其它错误／配置失败仍拒绝。成员 timeout、nonce ACK、所有十二测试正文、原生产三秒、Cargo 锁图和 Windows 路径保持。post 34,994 B／SHA256 `fffd702891a4f7cbb5b4374687a2656075283198b50b1c2a09ffbcfc904f9d43`；新非作者 MANIFEST `f2ddd139ef4b060948462b489ad3bc75169d2be2f9a279bdea7610585ae0dd6a` 已全文件读回。

新的根材料在 `work/unix-local-agent-cleanup-root-import-20261008-v5`；编译／行为／完整门禁当前 PENDING，stdout 与 stderr 分开保留，不能覆盖 v4 原始材料。每阶段输入前后完整 bytes／hash／mode 不变；正常收尾必须逐一检查 17 个 `/tmp/ks-ug-*` 根的完整终态记录与实际 ENOENT，不能以外层 TMP 为空推断。

以下“根组合导入”及作者 UNRUN 是 v4 导入／封包历史，后续结果以本段及新增实际结果为准。

## 根组合导入

精确父提交为 `af9465c9350dc1cd8e2ad4feb4d5693f2882ac91`，当前独立分支 `feature/unix-local-agent-cleanup-terminal-state`。新的非作者v4静态复核无确认P1/P2，仅准入根编译与门禁。六路径前后像完整核对，Windows脚本／Rust分支、原完整控制器及期限保持；ADR仅补齐ESRCH／NotFound／EPERM首次错误枚举。根锁定编译、严格Clippy、十二控制、完整工程及精确新CI待实际完成。当前根执行材料在忽略的 `work/unix-local-agent-cleanup-root-import-20261008-v4`。

## 作者准备历史范围

以下保留作者封包时点的UNRUN状态；后续根实际结果以本页前段为准。

状态：仅新 ignored 候选；Rust 编译、Clippy、行为、根整合、独立复核和 CI 均 **UNRUN**。

## 精确范围

生产前像 process.rs 51,888 B／SHA256 `73c13c5db0a5912e3e1110e977c5fdec60587b6529f78b9478d2d1cd08816257`；AI manifest 972 B／`08188ec2916ef786cf2ad68c4635852cd2b807424ab57449cd12c6a29fa3080e`。六个候选路径为 process.rs、独立 Unix tests、AI manifest、Cargo.lock、ADR 0085 与本记录。保留 spawn 实际 PGID 捕获与 owner 先建立；nix 0.31 保留原 fs/poll/user 并补 process/signal/socket，Cargo.lock 只增加 nix0.31.3→既有 memoffset0.9.1 依赖边（14 B），所有锁包版本／checksum／其它边不变。精确前后 hash 存作者封包，实际 --locked 解析 UNRUN。

v1 非作者正式报告判定 P2：post-wait 观察错误／取消令 cleanup 和 Drop 重新向可能复用的数字组号发 SIGKILL，同时重入重置三秒期限。v2 分离信号动作许可、Waiting／Observing 阶段、Failed 和 Complete，等待与观察共享首次保存的原三秒期限。错误不重试动作，取消可沿原期限恢复，失败永远继续 typed CleanupFailed。首次 EPERM 仅允许同期限 reap+只读观察，不按旧数字号再次发信号。v2 非作者完整生产链静态复核没有附加 P1/P2，但指出五个 unsafe fixture block 与继承的 forbid lint 冲突；v3 用 nix safe socket/UnixAddr/connect、OwnedFd→UnixStream、安全 fcntl CLOEXEC 和 nonblocking 消除该源码冲突，不降低 lint，仍须新独立复核与实际编译。生产 process.rs 与 v2/v3 完全相同，不能继承为行为通过。v3 新非作者报告指出正规 check 的嵌套 TMPDIR 生成 121 B socket 路径，超过 macOS 103 B；单独标准库边界探针仅证实长度限制，未执行本候选。v4 夹具自身在通用 /tmp 创建随机 0700 短根并校验完整端点，正规检查不依赖人为缩短 TMPDIR。

## 控制源码

六项原候选控制的目的保留，资源回收改为不再发信号，均未继承执行结果：实际 leader 回收后普通同组后代仍活跃、最后成员退出、活跃组 helper 到期、观察 errno、过期拒绝、真实 OwnedChild SIGKILL 与真实 native wrapper 缺失拒绝。

既有五项真实 owner 控制保持，全部 UNRUN：

1. post-wait helper 实际 Pending 后取消，重新进入仍 Pending，核对保存 deadline 完全相同；实际自有成员退出后完成，signal attempt 总数一。
2. 真实 held leader 的 wrapper wait 实际 Pending 后取消；重入继续等待同一 deadline，nonce IPC 释放 leader/member 后实际 wait/reap 和 ESRCH 完成，无第二次动作。
3. 首次 signal EIO／EPERM 和观察 EPERM/EIO/EINVAL/ECHILD 注入，完整 owner 返回 CleanupFailed；外层 cleanup 再入仍为失败，成员活跃，实际 Drop 不补发动作。
4. post-wait 取消后的 actual OwnedChild Drop 不对仍被数字查询为 present 的身份补发动作；此存在注入只代表复用后可见的信息，不强迫 OS 重用编号或接触无关进程。
5. 自有活跃组在 owner 原三秒期限到期，真实 CleanupFailed；重入没有新期限、没有新动作，错误状态不冒充 cleaned。

cfg(test) 的 per-owner 计数只记录进入 signal callback／Drop 的动作尝试；没有全局注入、共享环境变更或模型调用。无动作 signal callback 与 errno 注入只验证生产控制流；实际权限错误、复用、原生信号与跨平台时序不能由这些注入替代。

另追加第 12 项隔离长父目录控制：只在子进程环境传入超过 103 B 的 TMPDIR，普通 leader/member 与 held leader/member 在绑定前断言实际继承该值；现有身份 ACK、真实 owner cleanup、native id None、ESRCH、独立短根删除与长父根删除均需自己的断言。原 11 个控制正文不改，不调用进程全局 set_var，执行仍 UNRUN。

夹具 exact ignored 入口为自有测试进程：/tmp 中 0700 私有随机短根、0600 stderr、原子身份文件、nonce 限定 Unix socket ACK。正常终态实际 leader id None 与 signal-zero ESRCH 后关闭根；负向控制保持 owner 失败状态，独立 fixture 协议回收资源，不伪造清理成功。leader/member 有硬生命期，控制器八秒，Drop fallback 三秒且不重新向原 PGID 发信号。init 回收若未确认，保留失败 scratch 并报告 UNKNOWN。短根位于 check 外层 TMPDIR 之外，完整日志单独记录其路径与删除／保留状态；外层 TMP 空不能证明该根已回收。

## 必要门禁

固定 Rust 1.98.1 rustfmt 仅格式解析，实际退出与完整 diff/pre/post hashes 存作者封包。未运行 Cargo、rustc 编译、项目夹具、微型实验、GitHub、GUI、客户 SSH 或用户 CLI；不改旧 seal、root tracked 或 shared target。完整消费正式 v3 的 102 文件复核封包和全部 finding；未把 rustfmt 当作 forbid lint 或 --locked 编译验证。

原 local_agent_process.rs 81,603 B／SHA256 `d328c51d32a4f05ab57de070c39bfa3209bcf636b53e246486ed2fc8c25e6261` 保持：默认与 2 MiB 各 626 控制、四线程、所有原期限和单次连接断言不变。Windows 函数正文与 observer/Drop 编译分支保持，不能据此继承 Windows native PASS。

根需先新非作者静态复核，再组合源码执行严格编译／Clippy、12 项新 Rust 控制（原 11 项保留）及原完整工程检查，绑定精确 macOS/Linux CI。旧 macOS raw 157,118 B／SHA256 `33ba938780fc3c13c62c3bdfa1155dd5ec8943e809f81cbd617778094512a8f7` 仍保留，历史原 listener 身份与原因 UNKNOWN。本候选不宣称修复历史失败、不消除首次数字 PGID 竞争或恶意逃离组，也不更改原 fixture 身份／端口发布。
