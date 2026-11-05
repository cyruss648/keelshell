# 单窗启动恢复 — 2026-11-05

## 输入与范围

- 独立 worktree：`workspace-startup-restore-20261105/keelshell`。
- 本地分支：`feature/workspace-startup-restore-20261105`。
- 基线：`dcf7438a13fff737997ee5e8061044c94d4ba53a`；下文按作者冻结与父组合后续记录，
  不可变提交/推送以Git和本轮交付回执核对，历史状态不覆盖后续交付。
- 决定：[ADR 0104](../../adr/0104-single-window-startup-restore.md)。
- 产品范围：[布局合同](../../product/WORKSPACE_LAYOUT.md)。
- 仅恢复保存的活动窗口内容；保存数据没有 OS 外框尺寸/坐标，因此几何恢复限于
  已有分屏拓扑与比例。其他保存窗口的记录保留，不创建额外 OS 窗口。
- HANDOFF、ROADMAP、README 由父整合，本轮不修改；没有新增依赖或锁文件变化。

## 源码和测试

生产 `Workspace::new` 安装保存快照，使用真实面板树与会话栏显示关闭输入的占位实体。
状态为可用未连接、未保存占位、缺失/回收连接；绘制和动作时都重新解析当前 profile。
显式连接使用正常逐跳认证和主机密钥审核，结果绑定占位实体与稳定布局 UUID，
原位替换不占新增容量、不改选其他实时会话或迁移其工具、草稿、审核授权。
读取/选择/关闭/保存占位没有建立传输的路径。

新增 26 项测试源码，另更新 2 项已有测试：

| 测试模块 | 新增数 | 关注点 |
| --- | ---: | --- |
| `workspace::pane_sessions::tests` | 9 | 恢复拓扑/身份/顺序/选择、多个空叶、空活动叶、未知叶、容量/无效元数据/重复临时 key 拒绝 |
| `terminal::inactive_tests` | 2 | 没有传输/生命周期/轮询任务、命令/粘贴输入拒绝 |
| `workspace::tests::startup_restore` | 7 | 从 StateStore 加载到生产 Workspace、真实 GPUI 几何、双语三主题、关闭保存、当前 profile/删除回收、紧凑选择、保存冲突 |
| `workspace::tests::routes::startup_restore` | 8 | loopback SSH、逐跳新认证/未知密钥审核、动作时 profile 更新、认证中删除/路线变化、其他实时会话与审核目标、满容量原位替换、取消后关闭 |

已有 `runtime_keys_need_neither_debug_nor_default` 扩展恢复适配，
`explicit_save_round_trips_only_layout_references_and_reopen_creates_no_sessions_or_grants`
改为断言真实惰性恢复后的稳定身份、逐叶选择及零运行授权。
新路线测试的 peer 记录 Shell 输入；继承夹具拒绝 exec/SFTP 请求但不记录这些请求的正文，
不能以该字节断言代签所有后台网络操作。

## 本轮检查与父排期

初始约束为不运行 Cargo；实现和并行子 agent 只运行 rustfmt / 静态差异检查。
随后父明确接管并回收 `exec54030`，告知其四组首次专项通过并释放共享 target。
这些父任务结果不代签本轮输入。本轮获独占槽位后仅安排下列定向检查：

统一命令前缀为 `cargo test -p keelshell-app --locked --offline`；
环境 `CARGO_BUILD_JOBS=2`，`CARGO_TARGET_DIR` 使用父指定共享目录。
过滤项后使用 `-- --test-threads=4`，exact 回归另加 `--exact`。
父随后明确授权本机隔离 loopback 集成测试，固定预算 240 秒；没有启动前述
threads=1 计划，不记录为实际运行。本轮不连客户/公网、不自行启动外部服务，
不跑完整门禁、GUI、Podman 或网络工具。新 GPUI 状态夹具关闭自动更新检查。

冻结输入、精确命令、预算、完整输出、数值退出码和 reap 结果分别写入
本 worktree 忽略目录 `work/workspace-startup-restore-20261105/targeted/`。
工程绑定包含 Cargo 文件、全部 crates 源码/测试、assets、scripts、`.cargo` 与 `.github`；
本记录只追加实际结果，不改变工程输入。15 个修改的 Rust 文件已使用
`rustfmt --edition 2024 --config skip_children=true` 格式化，
`git diff --check` 退出 0；不等同于完整格式门禁或原生验收。

### 工程输入绑定

三个快照均包含 547 个工程文件，HEAD 和分支如上，没有提交或锁文件漂移：

| 冻结版本 | 工程输入 SHA-256 | 变化 |
| --- | --- | --- |
| r1 | `a9995279cedfe4a3ef6fa282ec31cd4a404cd236579dcb8e1095781bef0d32df` | 首次冻结 |
| r2 | `0bcf4ab07a41c1d17b81a3600b2b0cdb99d7835c60de20bb46cf7c71f4eefc45` | 仅给 `terminal::inactive_tests` 补 `use ::core::prelude::v1::test;`，解决通配导入导致的宏递归；未提高 recursion limit |
| r3 | `c815f9efada03ace120898511cb4dcc6a381e778f7ffb3b1407885f1f3a2ab07` | 仅调整紧凑测试窗口 900→360px，并断言真实 viewport 和紧凑条件；生产源码未变 |

绑定文件为 `targeted/frozen-inputs-r{1,2,3}.json`。每次运行开始前拒绝
不匹配输入，结束后重新校验全部工程输入；已完成各条均
`inputs_unchanged=true`。r2 已通过测试不因无关夹具更改重跑；
启动组 6 个成功用例的代码在 r3 保持原样，失败的紧凑用例单独精确重跑。

### 实际命令与结果

命令均在本worktree顺序执行。下例以`KEELSHELL_SHARED_TARGET`表示父指定的
共享target，实际展开值保存在本地执行回执，不把开发机绝对路径写入仓库：

```sh
CARGO_BUILD_JOBS=2 \
CARGO_TARGET_DIR="$KEELSHELL_SHARED_TARGET" \
CARGO_NET_OFFLINE=true \
cargo test -p keelshell-app --locked --offline FILTER -- --test-threads=4
```

下表逐条给出实际 `FILTER`；标有 exact 的条目在末尾实际追加 `--exact`。
预算由本地 runner 的 `Popen.wait(timeout=...)` 约束，含 Cargo 编译和测试。
超时先终止该进程组，5 秒未退出再强制终止并 wait；本轮没有触发超时。

| 运行/输入 | 实际 FILTER | 预算/实际秒 | 输出 | Cargo exit | 外层 exec |
| --- | --- | ---: | --- | ---: | --- |
| panes-r1 / r1 | `workspace::pane_sessions::tests::` | 480 / 9.475 | 编译阶段 `recursion limit reached while expanding #[test]`；没有执行测试 | 101 | 25378，已 wait/reap，exit 101 |
| panes-r2 / r2 | `workspace::pane_sessions::tests::` | 480 / 24.974 | 43 passed，0 failed，918 filtered | 0 | 53450，已 wait/reap，exit 0 |
| inactive-r2 / r2 | `terminal::inactive_tests::` | 180 / 0.543 | 2 passed，0 failed，959 filtered | 0 | 同步 exec 完成，exit 0 |
| startup-r2 / r2 | `workspace::tests::startup_restore::` | 240 / 1.962 | 6 passed，1 failed，954 filtered | 101 | 12614，已 wait/reap，exit 101 |
| compact-r3 / r3 | `workspace::tests::startup_restore::restored_compact_view_preserves_geometry_and_selects_tabs_without_ssh_focus`，exact | 240 / 10.624 | 1 passed，0 failed，960 filtered | 0 | 6696，已 wait/reap，exit 0 |
| reopen-r3 / r3 | `workspace::tests::pane_layout::explicit_save_round_trips_only_layout_references_and_reopen_creates_no_sessions_or_grants`，exact | 180 / 0.829 | 1 passed，0 failed，960 filtered | 0 | 同步 exec 完成，exit 0 |
| routes-r3 / r3 | `workspace::tests::routes::startup_restore::` | 240 / 1.999 | 8 passed，0 failed，953 filtered；仅本机隔离 loopback | 0 | 84795，已 wait/reap，exit 0 |

全部 Cargo 子进程均有数值退出码、`reaped=true`，
`process_group_exists_after_reap=false` 和 `timed_out=false`；
外层长执行会话另经工具 wait 收割，没有把发出命令视为完成。
完整原始输出和回执为 `targeted/{运行名}.log` / `targeted/{运行名}.json`，
首轮失败证据保留。编译成功后 linker 有一条 `__eh_frame section too large`
警告；本轮没有调整链接参数或以此声称完成完整门禁。

启动组唯一失败是紧凑夹具前提错误：三叶树最小宽度 400px，
900px 窗口仍能容纳，因而找不到紧凑提示。修正为 360px 后，
真实 canvas 进入紧凑模式，保留节点/比例、会话切换和惰性焦点断言通过。
新测试模块均已由真实 app test target 编译；没有以 preview 代替生产恢复。

按最终结果合并为 61 个不同测试通过：领域 43、惰性终端 2、生产启动 7、
已有 reopen 1、loopback 路线 8。累计执行 62 次测试，含一次修复前失败；
r1 编译失败没有运行测试。该合并没有把 r2 的全组失败描述成全组通过，
也没有重跑其他已通过项。本轮没有完整工作区门禁或图形桌面验收。

路线组实际测试名：

```text
workspace::tests::routes::startup_restore::startup_renders_only_the_saved_active_window_without_ssh_or_runtime_authority
workspace::tests::routes::startup_restore::restored_connect_requires_fresh_per_hop_trust_and_replaces_only_its_saved_slot
workspace::tests::routes::startup_restore::restored_connect_resolves_the_profile_saved_after_startup_at_action_time
workspace::tests::routes::startup_restore::restored_authentication_discards_late_success_when_profile_is_trashed
workspace::tests::routes::startup_restore::restored_authentication_discards_late_success_when_upstream_profile_changes
workspace::tests::routes::startup_restore::restored_connect_preserves_another_live_session_tools_and_reviewed_command_target
workspace::tests::routes::startup_restore::restored_connect_replaces_a_slot_at_both_pane_and_workspace_capacity
workspace::tests::routes::startup_restore::cancelled_restored_authentication_cannot_recreate_a_closed_placeholder
```

生产启动组实际测试名：

```text
workspace::tests::startup_restore::startup_restores_persisted_active_window_geometry_tab_identity_order_and_selection
workspace::tests::startup_restore::startup_empty_active_leaf_does_not_select_or_focus_a_sibling_and_all_empty_topology_survives
workspace::tests::startup_restore::restored_states_and_actions_are_bilingual_in_all_three_semantic_themes
workspace::tests::startup_restore::restored_connect_uses_current_profile_and_deleted_or_trashed_references_stay_inert
workspace::tests::startup_restore::restored_close_and_save_keep_other_window_and_remaining_references_without_connecting
workspace::tests::startup_restore::restored_compact_view_preserves_geometry_and_selects_tabs_without_ssh_focus
workspace::tests::startup_restore::restored_layout_save_conflict_preserves_placeholder_identity_and_external_profile_change
```

所有执行结束后工程输入继续匹配 r3，源码/测试冻结；共享 target 已释放给父，
本任务不再运行 Cargo。源码子 agent 85930、路线测试子 agent 63352 和只读设计
review 子 agent 46696 均已实际 wait/reap，数值退出码均为 0。
冻结候选的新非作者只读审查已由父收取，见下方组合跟进；
本记录不代替该独立报告。

## 审查与剩余边界

并行只读设计审查针对基线，指出空活动叶、容量、未挂载终端焦点、
异步安装身份和保存未解析引用等风险，修订已纳入源码。
该报告只作设计输入，不作为冻结源码的独立批准。
新非作者`startup-source-r1`已批准冻结r3源码与61项专项，
开放P0/P1/P2/P3为0；没有签后续正式文档、完整门禁或原生验收。

## 父组合跟进

18个冻结路径已导入父分支，17个与来源逐字节一致；
terminal保留本轮输入测试注册，并同时包含恢复的惰性实体实现与测试。
来源25份执行证据已逐摘要复制到
`work/input-and-layout-readiness-20261105/source-startup-restore/targeted/`，
绑定见同目录上级的`startup-restore-integration-r1.json`。

最终520项工程输入首轮full-gate-r1于13:47:30–13:47:48 UTC停止在严格Clippy，
退出1并回收；Python131run/1skip和格式检查已通过。三项失败只涉及测试辅助：
两处`expect_used`与一处无用类型转换。父保持断言作三行最小清理，
新非作者在`startup-source-r1`附录核对后限定批准；原失败证据保留。
full-gate-r2于14:01:24–14:12:28 UTC实际通过，执行会话88486已wait/reap、exit0；
固定1500秒、共享target/2jobs和四测试线程保持。最终520项输入前后一致，
2,221 ordinary Rust/23ignored、app963/2ignored、14rustdoc、Python131run/1skip，
完整格式、严格Clippy、依赖策略和附加控制器通过。来源专项不代签完整组合。
build-r1于15:12:27–15:12:57 UTC完成app与MCP构建，exec47106已wait/reap、exit0，
与门禁及当前520输入完全相同；linker的`__eh_frame`警告原样保留。
新macOS开发包完成结构检查及五成员摘要核对，41个assets另记，
程序SHA-256为`99cd7f6955735b18686df30626d68688c8de1f2fb3b1ceba2ae6329f3b49b9ae`。
包未安装、签名或发布。

## 新包原生保存与重启

run`60c040c7da1a4cadbab99a1a2d78f520`以同一私有目录前台监督两次启动，
总TTL固定600秒、末30秒预留清理，没有重置预算。准备r1的悬空symlink清理P2有
修前失败证据；r2用`lexists`与原目录身份核验，14项纯单测通过，
新的非作者限定批准后才启动本次应用。
种子纯合成：一个可用profile、一个未保存引用、一个缺失引用，
活动窗口在第二个元数据位置，另一个窗口作为保持对照。没有凭据或历史授权。

全部GUI操作使用CUA，保存六对2880×1866原始AX/JPEG：

| 捕获 | 实际观察 |
| --- | --- |
| 01 startup | 水平两叶、62%/38%；左叶可用未连接，右叶未保存占位 |
| 02 missing | 右叶明确选择缺失标签，只提供管理/关闭动作 |
| 03 ratio | 右叶“缩小”使左侧目标比例从62%变67% |
| 04 language | 切中文后保持三态和缺失标签选择 |
| 05 saved | 点击保存，界面底部显示已保存到本机 |
| 06 restart | 正常退出、显式restart后，中文、67%、缺失选择和两叶均保持 |

两轮应用分别exit0/reaped，父前台exec22504已实际wait/reap、exit0。
两次退出后的raw state摘要完全相同；原state只留摘要及提取结果，
随后随private删除，没有事后重新读取原文件。父从合成seed仅施加上述
比例、活动叶和选中标签变化，枚举节点/面板序列后得到唯一完整布局，
其SHA精确匹配两次保存摘要。因此布局节点、稳定UUID、引用、顺序、选择和6700bps
均有摘要绑定；语言由第二次原生AX/图像确认。未选窗口内容与窗口顺序也保持。

`result.json`确认两次启动完成、直接子进程回收、观察到的后代停止、私有目录删除及
原TTL内清理；父额外读回三PID均不存在、private路径`lexists=false`。
过程原件在本轮`restore-native-preparation/runs/`，冻结摘要与父实际wait回执见
`native/restore-verification-r1.json`、`restore-native-preparation/parent-close-readback-r1.json`。
新的非作者`restore-native-r1`逐项核对23份run原件、6对AX/JPEG，
独立枚举12个布局候选，唯一匹配预期布局；结论为
`APPROVE_SCOPED_RESTORE_EVIDENCE / PARTIAL_OBSERVED`，开放P0/P1/P2/P3=0。
新的`final-combined-r1`复核21个改动Rust文件、完整520输入、门禁计数、
构建/五包成员/41assets及原件归属，结论为
`APPROVE_SCOPED_COMBINED_SOURCE_AND_EVIDENCE`，开放P0/P1/P2/P3=0。
两者都不签父编辑中的正式文档。文档r1另发现三项历史/当前状态P3，
父仅同步四份文档，原REQUEST_CHANGES报告保留；最终文字以`final-docs-r2`限定复核为准。
源码、文档和不可变提交/推送读回分别绑定，不能将旧dcf7438的CI代签本轮。

本次只验离线视图恢复，未点击Connect，没有SSH/AI/隧道/任务的网络审计，
不能把无操作当作零网络证据或真实认证验收。霜白浅色覆盖限定中英文；
没有OS外框恢复、完整主题尺寸矩阵、VoiceOver/IME、Windows/Linux桌面或多窗口原生结论。
源码、定向测试、开发包和真实原生接受度分别记账，不继承历史批准。
