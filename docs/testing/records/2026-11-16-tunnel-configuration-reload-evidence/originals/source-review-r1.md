KeelShell 隧道面板配置 Reload：独立非作者只读源码复核 R1

日期：2026-11-16。结论：**限定 APPROVE**。本批相对 base 的 production 与 Cargo.lock 中，**P0 = 0、P1 = 0、P2 = 0、P3 = 0**；未发现有源码证据支持、需要修复的新增缺陷。没有缺陷复现条目，不以推测或历史问题补足问题清单。

建议通过本批静态源码合并复核。最终工程合并与交付仍应结合父任务正在进行的精确输入全量 gate；本报告不签署编译、专项执行、原生桌面或三平台运行验收，也不覆盖后续另一个新功能。

**身份、范围与只读边界**

本复核者不是该功能作者。本次没有实现或修改 production、测试、依赖、Git 状态或 target。已读 `/Users/hdh/.codex/RTK.md`、适用 AGENTS、`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/HANDOFF.md`、`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/adr/0151-tunnel-configuration-reload.md`，并应用 rust-best-practices / rust-engineer 的所有权、异步边界、错误处理和文档规范。技能中的执行型门禁服从用户本次明确禁止 Cargo / fmt / build / test 的约束。

- cwd：`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。
- base 与复核结束时 HEAD：`c81f718ccc3ee5d6a8cce6b7988b1cbcab8ea694`。本批是工作区变更；两个新 production 模块尚未跟踪，已经作为完整新增文件复核，未遗漏在普通 Git diff 之外。
- 评审对象仅为下面六个 production 源文件的新增/变更，以及 Cargo.lock。未变更调用方仅用于确认现有状态采用、gate、归档与 worker 生命周期合同，不对它们重开历史审计。测试、README、路线及其他文档不列为缺陷评审对象。
- 用户补充的 runtime_view 根节点 `.id("tunnels-panel").test_support()` 已纳入最终 diff。测试宏 scope、unused import 修复和父任务测试执行不属于本次 production 复核的验证成绩。
- 本复核唯一文件写入是本报告；没有 Cargo、rustfmt、Clippy、build、测试二进制执行，没有创建 chat / worktree / 分支 / 容器，也没有清理或访问 target 输出。

**最终输入绑定**

父任务冻结文件：`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/tunnel-configuration-reload-20261116/inputs-freeze-r2.json`；文件 SHA-256 为 `217a3e8960fc9a720fb9194cb76837ea75d3abbe1424da9a04dd28ca78436129`。其声明 base 与本报告相同，输入数 721，排除 docs / .hallmark，聚合 `442393e019c51a19b8fe49d5040813e44a06b5a77cc3a7eb4c9b162701a979f2`。本次独立核对其中全部 7 个评审输入的字节数与 SHA-256，**7/7 一致**；未扩大为重算其余 714 项或全 721 输入聚合。

以下为最终完整文件摘要，含用户补充的观测 ID；它们比仅记录 HEAD 更能界定未提交的评审对象。

| 绝对路径 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock` | 261885 | `c3c24d03d05a6a04513f837b648bf9ec903a0250df4a811ce8f94b484dab8535` |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels.rs` | 79237 | `c296c64f0b67551f9ea0a5b4c2375001d140149bb79c9fd923b0b935c314210b` |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/reload.rs` | 3824 | `22a5728a331a5c6a863ba0975b198f094ef5f02e7b14710cb5c0676b1fbc3941` |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/rules.rs` | 70731 | `6d41d39c7fad20cbb764738babae593d232d963ff7d847a638295a0376e453ab` |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace.rs` | 149284 | `3091591cbffba684d3bcbfb4823565c01553edab51d88bfc6f3d0228785f584d` |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/tunnel_reload.rs` | 2902 | `023eb78717ea4a7b45a0eb79ebaa3d9493e09687a19d4cc8714568dc27098e3a` |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs` | 16108 | `0660a0e93148f80518b7a7cb77eb264583b712c30c9a83c94a47ef243e84ee12` |

评审输入清单 SHA-256：`c7add6690fdd4a1e407ee55c106a832c7f3129e887962380a25536d5da5c7a68`。算法：按相对路径字典序排列，每项为 `sha256 + "  " + relative_path + "\n"`，UTF-8 连接后计算 SHA-256。

初次读取的 tunnels.rs 为 `f76fe48f0de139cef2d211e2e73e9750d46a661aa80a2e43432827d0fe697e16`；之后用户明确补充的两行观测注册已逐行复核，最终采用上表 `c296c64f…`。其余六项摘要未漂移。父任务的测试修复没有被误认作本复核者的写入。

**Production 审查证据与静态触发推演**

以下均为静态追踪，未实际执行场景。位置使用最终工作区的绝对路径和一基行号。

1. 显式入口与旧绘制防护。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/reload.rs:14-52`：按钮捕获绘制时 epoch，回调重新经过 `accept_rule_action`；`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/rules.rs:311-317` 同时检查 suspended / busy / epoch / mutation / pending。触发推演：旧按钮在状态轮换后回调或重复点击，均不产生第二个有效请求。准入成功后只撤销审核并生成 operation、原 connection_id、新 epoch；不写配置、不启 SSH。

2. 精确 pending 与 live panel / connection 绑定。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/reload.rs:55-100` 要求完整 pending event 相等、epoch 和连接相等；拒绝只能清除同一 event，不能清掉替代请求。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/tunnel_reload.rs:19-68` 在接收与完成两个阶段均要求 `self.panels` 中的原 Entity 和 terminal→connection 映射。触发推演：仅持有订阅、archived panel、保留的 worker owner、替换 Entity、改变绑定或 pending identity，均不能授权权威采用。

3. 生命周期撤销。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels.rs:396-416` 与 `:593-606` 在替换状态或 suspend 时清 pending 并轮换 epoch；`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace.rs:1203-1221` 关闭 tab 时移除新订阅，原 worker 的保留仍沿现有路径。对未变更的 reconnect suspend/archive 路径进行了调用关系确认；没有将 archived 面板纳入新的 reload 权限集合。

4. 准入失败不遗留 busy / pending。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:298-328` 拒绝 saving / state_reloading / closing / active_modal / connecting / connect_route / 配置维护或 gate 不可用；先取得 reload lease，再设置 state_reloading 与 panel busy。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/tunnel_reload.rs:36-46` 在失败时清除精确匹配 pending。触发推演：其他窗口持有写租约，或另一个模态/连接操作未结束，不分派新的 StateStore 读取，也不保留假 busy。

5. 后台 I/O 与前台采用边界。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:321-363` 将 `store.load()` 单独放在 background executor；ReloadLease 由前台 continuation 持有，后台读取结束不释放。进入前台且确认 origin 有效后才 finish，并在同一个无 await 的前台闭包内完整采用状态。核对现有 gate 准入调用均在 Workspace 前台路径；未发现另一窗口可以在后台读完、前台尚未处理期间取得写租约的新增窗口。这里没有声称 lease 在完整采用函数返回后才释放：finish 位于采用之前，但二者之间没有异步让出。

6. 迟到完成与 uncertainty。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:332-349` 先检查 closing 和精确 origin；过期时不采用成功结果，而是 drop 未完成 lease、标记 configuration_invalidated 和规则存储阻断并通知共享状态。未变更的 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/app_windows.rs:352-360` 证实该 Drop 保留 shared uncertainty 并轮换 gate epoch。触发推演：读取期间原面板 suspend / archive / replace / 改绑，即使后台读成功，也不能解开替代面板的写屏障；不存在自动重试或不确定写入重放。

7. 现有与新面板 busy。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:325-333` 设置和退出读取 busy；`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace.rs:3174-3178` 同步 live / archived 面板，`:3453-3457` 为读取中新开的面板继承 saving / state_reloading 与存储阻断。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/rules.rs:588-604` 同步真实 InputState disabled，新增 pending 也进入规则编辑、命名启动、恢复动作与运行候选持久化的检查。

8. 只有权威读取采用解阻断，失败保留原错误。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:354-379` 成功调用完整 `accept_reloaded_knowledge_state`；失败标记 invalidated / store blocked，错误原件保存在 Workspace 的 `Message::detail(..., error)`，精确 pending 结束后允许显式重试。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/reload.rs:64-99` 的 finish/reject 自身均不清 store blocked。未变更的 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/knowledge_library.rs:146-174` 证实采用完整 AppState，并处理 AI ephemeral credentials、知识审核、语言/主题、重连、MCP、Agent、隧道和 snippet 来源刷新。

9. 同内容撤销审核、草稿及运行 owner 保留。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels.rs:381-420` 中 authoritative 会绕过 unchanged 的早退，清除旧选择/审核，但保留 RuleEditor 原实体与原 snapshot；`:503-590` 的命名启动不能越过 reload pending。旧草稿的保存审核仍由 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/rules.rs:436-456` 对原 state / connection 二次校验，用户需取消后重新编辑。reload 请求与采用路径没有改写 session、runtime_owner、rows 或已捕获端点，也不调用 start / stop / reconnect。不能把这些静态事实扩大为已经验证真实监听器保持。

10. sibling invalidation 使用 store blocked。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/workspace/window_configuration.rs:113-118` 从永久 busy 改为 `mark_tunnel_rule_store_uncertain`；`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels/rules.rs:607-617` 保留草稿并使旧审核失效。Reload 的 `accept_rule_action` 有意不因 store blocked 而拒绝，因此没有“必须 reload 才解锁，但 reload 自身被禁用”的环路；没有凭普通 refresh 清掉 uncertain。

11. 用户补充的观测 ID。`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/tunnels.rs:1716-1725` 新增根 ID / test_support，与管理视图分支 `:1752-1758` 互斥。已读 `/Users/hdh/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/gpui-base-0.10.0/src/observe.rs:20-35`：非 test-support 返回原元素，启用时只包观测器、不增加布局节点；该两行未加入 SSH、I/O、规则授权或可见样式操作。没有原生视觉验收声明。

上述检查未发现新增 panic / unwrap、unsafe、私有 Git 依赖、凭据持久化、遥测或 UI 线程 StateStore I/O；此结论只针对所评新增/变更代码。

**Cargo.lock 单独复核**

相对 base，解析后 package 数为 **984 → 984**。只替换四个包版本，无第五个新增/删除包版本；10 个未换版本包的 Windows/getrandom 发生 11 条依赖边重绑，另有 digest 的 zeroize 边和 rsa 的 crypto-common / hmac 边。Cargo.toml 未发生本批变更，直接 russh workspace 约束仍为 `0.65`。

| 包 | base → 当前 | 当前锁文件绝对位置 |
| --- | --- | --- |
| russh | 0.65.1 → 0.65.2 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:7112-7180` |
| rsa | 0.10.0-rc.18 → 0.10.0 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:7091-7109` |
| pkcs1 | 0.8.0-rc.4 → 0.8.0 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:6188-6195` |
| ssh-key | 0.7.0-rc.11 → 0.7.0-rc.12 | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:8100-8123` |

依赖边逐项对照已缓存发布包 Cargo.toml；缓存根为 `/Users/hdh/.cargo/registry/src/rsproxy.cn-e3de039b2554c837`。下表范围是发布包原声明，不是本复核者推测的兼容范围。

| 依赖方与绝对锁位置 | 新绑定 | 发布包声明范围 |
| --- | --- | --- |
| dirs-sys 0.5.0，`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:1813` | windows-sys 0.59.0 | `>= 0.59.0` |
| errno 0.3.15，`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:2101` | windows-sys 0.52.0 | `>=0.52, <0.62` |
| gpu-allocator 0.28.0，`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:2700` | windows 0.57.0 | `>=0.53, <=0.62` |
| iana-time-zone 0.1.65，`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:3999` | windows-core 0.57.0 | `>=0.56, <=0.62` |
| quinn-udp 0.5.16，`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:6612` | windows-sys 0.52.0 | `>=0.52, <=0.61` |
| rustix 1.1.6，`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:7348` | windows-sys 0.52.0 | `>=0.52, <0.62` |
| rustls-platform-verifier 0.7.1，`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:7426` | windows-sys 0.52.0 | `>=0.52.0, <0.62.0` |
| tempfile 3.27.0，`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:8390` | getrandom 0.3.4 | `>=0.3.0, <0.5` |
| tempfile 3.27.0，`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:8393` | windows-sys 0.52.0 | `>=0.52, <0.62` |
| winapi-util 0.1.11，`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:9645` | windows-sys 0.48.0 | `>=0.48.0, <=0.61.*` |
| winreg 0.56.0，`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/Cargo.lock:10270` | windows-sys 0.59.0 | `>=0.59, <=0.61` |

- 新 russh 0.65.2 的发布 manifest 要求 `ssh-key =0.7.0-rc.12`，rsa `0.10` 与 pkcs1 `0.8`；新 ssh-key 的 rsa 要求也是 `0.10`。四版本及直接 `0.65` 约束没有发现不匹配。
- rsa 新增 `crypto-common 0.2.2`、`hmac 0.13.0` 与其发布依赖一致；rsa 启用 hmac/zeroize，而 hmac 0.13.0 的 zeroize feature 为 digest/zeroize，因此 digest 0.11.3 新增 zeroize 边有可追溯来源。
- 对发生变化的 **15 个最终 package 条目**，逐一读取已有 `.crate` 归档、不解包到磁盘：**15/15 归档 SHA-256 等于 Cargo.lock checksum，15/15 解压内存中的 Cargo.toml 等于 src 缓存 manifest**；缓存 registry index 的相同版本 checksum 再次 **15/15 一致**。这是本地静态一致性，未刷新 registry、未承诺当前 yank/安全公告状态。
- 全锁 **3,530 条依赖边**按 name / version / source 唯一解析，缺失或歧义 **0**；没有残留指向被替换版本的悬空边。
- Windows 多版本并存以及选中较旧的 windows-sys / getrandom，本身不足以证明错误。未发现这里存在违反声明、错误 checksum 或悬空绑定的可证实缺陷。核对 gpui-pre / gpui-fast platform 的当前 manifest，Windows 原生路径走对应 windows 包，gpui_wgpu 直接边限定 Linux；没有据此把锁中同时存在不同 windows 版本猜成 KeelShell Windows 类型冲突。未下载的 wgpu-hal / Windows 目标 crate 源码不构成本报告的平台编译证据。

**实际执行与限制**

实际操作为 Git 只读 `rev-parse / show / diff / ls-files`，文本 `rg / cat / nl / sed`，以及 Python 内存中的 TOML / JSON / tar / SHA-256 解析与比较。涉及索引列表的 Python Git 查询显式设置 GIT_OPTIONAL_LOCKS=0；其余 Git 命令也仅执行读取，没有执行任何 Git mutation。

- Cargo / fmt / Clippy / build / tests / 预编译 binary / native GUI 执行次数：**各 0**。
- 本复核执行测试数：**0**；passed / failed / ignored：**不适用，未运行**。不存在本次编译 test inventory 或跨平台回执。
- 父任务提供“21 新增已编译、7 Workspace 专项通过、修 root ID 后纳入 63 专项通过”的信息，均仅作为任务上下文；本复核没有独立运行或审计这些执行回执。另一个 reviewer 的 binary 执行也不计入本报告成绩。
- 未验证 rustc / Cargo resolver / 全 feature 编译、完整 gate、Windows/macOS/Linux 行为、真实 SSH peer、活动 listener 保持、三主题/尺寸/双语视觉、键盘/IME/辅助技术、包/签名/公证/发行。
- 检索中曾出现 zsh 未展开 `crates/keelshell-app/src/windows*`、不存在 `workspace/actions.rs` 及未缓存的目标 crate 源路径等只读诊断；后续按实际文件与缓存 index 校正，没有将检索失败记作编译失败，也没有因此修改源码、下载依赖或扩大执行范围。长文本工具输出出现过截断，关键 diff、新模块、所引 Rust 章节与证据位置已分段重读。

**最终结论：限定 APPROVE；P0/P1/P2/P3 = 0/0/0/0。** 建议父任务继续原定最终一次全量 gate；本报告针对上表精确 production/lock 输入，后续这些输入发生变化时需要重新界定复核范围。
