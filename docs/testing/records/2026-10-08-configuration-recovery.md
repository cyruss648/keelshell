# 配置备份与恢复候选 — 2026-10-08

## 最新限定验证：冻结 v8

2026-10-08，根任务实际执行固定 Rust `1.98.1` 的限定检查。候选基线仍为 `a94038d`；已推送根主线现为 `7c11954`。配置候选尚未整合、提交或推送。以下结果绑定 v8 的 811 个完整输入／15,959,682 字节；本次四份状态 Markdown 更新形成独立 v9，19 份功能 Rust 与全部其他输入保持 v8 字节，不将文档新 epoch 当作重新运行的测试。

| 检查 | 实际结果 | 原始日志 SHA-256 |
| --- | --- | --- |
| GPUI 专项（6 Workspace＋2 typed completion） | 8 passed，退出 0，测试 0.89 秒 | `61e15e96f5cd702fa63b77689bc6087895e60fa653fc3a271b90f9821c007423` |
| core 集成（含本机适用的两项 Unix 条件） | 23 passed，退出 0，测试 2.10 秒 | `153f5857357f6ff85399e954b50ab14702c936339d752e136a67f9f565580dc5` |
| core 回滚故障控制 | 3 passed，退出 0，测试 0.10 秒 | `317f71e15c99a2c7623029ac8da5f189bc6b935683680966d340a75297772202` |
| 依赖 x.y 策略、最终格式 | 分别实际退出 0 | 原始命令及日志保留 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 实际退出 0 | `8d1fa31b9ea98f68f9d65d8e78d46f5f21c89fc9e1babc108da28d555175753a` |

三个测试运行均实际重新编译所需本地 crate，运行程序全文哈希、相等的前后输入、mtime 刷新、actual wait、原进程组消失及空 TMP 删除均有根读回记录。普通 App 8 项选择、669 项 filtered；不是完整 workspace 测试。全部 34 项实际通过只属于当前 macOS 受控 GPUI／隔离文件系统范围。

相同 2,986 字节失败 owner 反例 SHA-256 为 `18e291763abef12deda2e7132c6c94535f6decf1ada970eaf960ef9618d3affd`。旧 checked-event 运行实际退出 101，在既有 CheckedOption helper 的第 1054 行到达“失败后必须保留可见 owner”消息；严格 busy、外部完整原字节及无原件副作用断言此前通过。修后 v7／v8 的同一函数实际通过。早期编译失败、busy 前置失败、拒绝的旧程序复用、v5 test 宏递归、v6 UI／v7 core expect lint 失败和 v8 初次格式检查退出 1 均保留，不计为功能反例或通过。

v8 非作者包 132 payload／4,729,662 字节，封条 SHA-256 `f68388dd3c2c007da656ae1f73f6294fb247c68b69b918eed8962130b157aeb5`；根完整读回。同一反例、实际结果和全部保存源码已独立核对，无新的限定生产 P1/P2。v8 仅替换 core 测试中的 25 处 expect 为私有 Checked helper，原断言、故障注入和调用保持；生产代码与 UI 源码相等。

完整标准 workspace 门禁、同一最终输入的新 macOS 标准包、真实桌面恢复、中英明暗／最小原生窗口、主线组合及提交、Windows／Linux 界面和权限、断电耐久性、整个退出时阻塞 syscall 仍开放。MCP 的撤权／OutcomeUnknown及空句柄列表不证明实际 Join 或远端物理静止。

原始材料保留于忽略的 `work/configuration-recovery-root-validation-v1`、`work/configuration-recovery-root-correction-v8` 和 `work/configuration-recovery-independent-20261008-v8`。下文 v1／v2 的 UNRUN 等描述是封包时的历史状态。

## 历史候选 v1／v2


当前是从已发布 `a94038daae12c5f9bf01f9586b5503d862386f81` 开始的隔离 `feature/configuration-recovery` 候选，未提交、未推送。根正在使用共享编译目标完成其它已冻结输入的原生验收；本作者没有运行 Cargo、rustc、测试、Clippy、check.py 或 GUI，也没有访问共享 target。

已编写八槽单调历史、列表／创建／预览／显式恢复、当前三态和未来 schema 分类、完整原件独立保留、process-local revision 与完整资料绑定、post-commit 同步失败的 rollback／required 分支，以及生产 Workspace 专用恢复面板。候选只保存 metadata；外部 MCP 方向仍是 KeelShell 提供服务，不是 AI 使用其它 MCP。

截至候选 v2，定义 23 项 core 真实隔离文件集成测试，其中两项 Unix 条件定义、其它 21 项平台通用；另定义三项 core 单元失败控制、六项 GPUI Workspace 测试和两项 typed completion GPUI 控制。所有 34 项均为 **UNRUN**，定义数量不等于通过数量。待实际运行并按目标记录条件项。

新的非作者 v1 静态审查完整校验45个payload／1,235,053字节和810个输入／15,943,986字节，确认一项P2：已经派发恢复后预约Close，实际Err只保存在即将卸载的panel。原v1封包和审查完整保留；作者完整读回72个审查payload／2,052,053字节。v2 保留失败owner、取消旧deferred close、保存实际typed Error，分别展示stale、已确认回滚及无法确认回滚的人工修复状态。实际反例和最终非作者复核仍待执行。

新增真实文件stale-review→批准恢复→同一foreground turn关闭的GPUI反例；v1会卸载owner，v2要求实际失败owner和反馈持续可见，之后新的明确Close才关闭。另有两项生产完成处理器控制，分别输入实际类型 `ConfigRecoveryRequired` 与 `ConfigRecoveryRolledBack`，检查不发旧Close、原因保留和中英明暗反馈。后二项只验证UI错误可见性，不声称生产文件系统实际发生了同步失败。共享target释放后，应先给不变v1源码只追加同一真实stale反例并记录失败，再恢复精确v2运行同一反例和两个控制；不能把编译失败当运行反例。

定义覆盖：首次无旧快照／缺失目录无创建、最近八份内容次序、截断／损坏保持且不自动恢复、有效／缺失／损坏／未来当前配置、备份更改／同内容保存的新 revision／其它 store 身份／缺失变已有／损坏字节变化、未来备份拒绝、schema 1 缺省字段迁移、原件八份上限、owner-only／symlink／未知历史文件、真实独立 vault 错误密码与凭据绑定保持、4 MiB 限制、九份历史和序号溢出。post-commit 控制分别覆盖损坏原件实际原子回滚、缺失回滚与故意阻挡回滚时独立原件保持。GPUI 定义走实际面板点击、未批准／取消保持、明确恢复后立即关闭仍传播真实结果、外部磁盘变化拒绝、已有 remote 实体无写入和中英明暗最小视口固定 footer。

v1 已实际使用固定 Rust `1.98.1` 的直接 `rustfmt`（不是 Cargo fmt）整理18份自身 Rust 文件，实际 exit 0；v2 增加一份独立UI测试源，最终19份源的格式与diff结果由v2封包记录。这只证明格式工具处理过文件，不能证明编译成功。

共享 Cargo 目标释放后按顺序执行：

1. `cargo test -p keelshell-core --test configuration_recovery --locked -- --test-threads=4`；`cargo test -p keelshell-core --lib store::recovery::tests --locked`。
2. `cargo test -p keelshell-app --locked workspace::tests::configuration_recovery -- --test-threads=4` 和 `cargo test -p keelshell-app --locked configuration_recovery::tests -- --test-threads=4`，然后现有 storage、AI metadata、profile sync、modal／语言／更新回归。
3. 项目标准格式、依赖 x.y、严格全 workspace／all targets Clippy、完整测试及默认／2 MiB 控制器。新非作者阅读同一最终字节和真实日志，不能继承作者或父提交结论。
4. 同一最终输入构建新的标准 macOS 包；用隔离数据目录验证有效与损坏启动、明确备份／审核／取消／恢复、中英明暗／实际最小窗口、真实坏文件原件与当前配置完整读回，并确认没有 SSH／命令／传输／授权重放。

Windows/Linux 原生、Windows ACL、断电／突然终止、整个退出时阻塞 syscall、有界墙钟等待和未来 schema 真实升级／回退都没有本次证据。没有新增 vault 备份或真实 OS keychain 操作。源封条与 raw/preimage/postimage 材料保存在忽略的 `work/configuration-recovery-author-v1`；不能把格式或静态源码封包表述为功能验收。

MCP已撤权／OutcomeUnknown及空句柄列表不证明已发出远端操作物理停止或实际Join；本机恢复只不发行／重放新操作，旧未知效果必须独立核实。
