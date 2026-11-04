# NET-01 命名规则管理候选记录

日期：2026-11-04，Asia/Shanghai。

最新状态：两轮独立修复已精确导入组合树，最终 Curie 复核未发现 P0/P1/P2；父任务专项与完整本机工程检查通过，见末段及[组合记录](2026-11-04-native-themes.md)。提交推送以 Git 及交接记录为准；NET 原生桌面验收仍开放。

下方初始作者记录基于 `4cd1809b2e1b02ddfcae0c2cdef36a060a9affd7`，原分支为 `feature/net-01-rule-management`。其当时的未提交/未运行状态保留为历史，不覆盖末段结果。

## 接续与写入范围

接管时已有未提交的 `tunnels.rs` 和 `workspace.rs` 修改，共 855 行新增、17 行删除。先读取已有 diff、AGENTS、HANDOFF 和两个 Rust skills，再接续；没有 reset 或丢弃原表单/保存集成。接管时会话列表未见其他绑定本副本的编辑任务，目标文件没有打开句柄，连续文件摘要一致；这一检查不是对其他后台任务的全局排他证明。

写入限于：

- `crates/keelshell-app/src/tunnels.rs`
- `crates/keelshell-app/src/tunnels/rules.rs`
- `crates/keelshell-app/src/tunnels/tests.rs`
- `crates/keelshell-app/src/tunnels/management_tests.rs`
- `crates/keelshell-app/src/tunnels/test_support.rs`
- `crates/keelshell-app/src/tunnels/workspace_tests.rs`
- `crates/keelshell-app/src/workspace.rs` 的隧道订阅、保存回调和事件准入
- 独立的[规则指南](../../product/NAMED_TUNNEL_RULE_MANAGEMENT.md)与本记录

本轮没有修改 core、AI、HANDOFF、ROADMAP、README、依赖 manifest 或锁文件。

## 行为与审核边界

新增/编辑沿用原有真实 GPUI 输入控件，先生成预览再显式确认。编辑器固定打开时的 state snapshot 与连接绑定；修改保持 rule ID，新建规则在整个草稿期间使用同一个 ID。删除有独立审核，不用“第二次点击同一个删除处理器”隐式确认。

保存、删除、启动与停止审核各自绑定身份；确认检查 state revision、完整状态、规则与连接内容。改变配置、连接绑定或草稿不能借用旧绘制按钮确认新内容。状态刷新保留旧草稿，但不自动重放或 rebase。

Workspace 只接受来自当前 live panel、当前 saved connection 和该 panel 的精确 pending event。复用 core CRUD/validation API 与既有 `persist` 后台 StateStore I/O。删除检查所有 current/archived panel 的活动 owner；保存期间也阻止其他标签启动命名规则，覆盖检查到落盘之间的窗口。

按规则停止使用独立运行时审核身份，绑定原 panel owner、rule ID 与精确 worker ID 集合。预览展示原运行端点；只在确认后向这些 worker 发取消请求。即使权威 reload 删除了规则元数据，原运行实例仍可停止。停止请求不等于完成，仍等待原 `Finished` 回执。编辑、删除元数据不会替换原 worker、SSH session、listener owner 或流量对象。

存储失败/冲突/结果未知保留草稿，阻止继续保存和 named start，直到取得新的权威配置快照。本切片没有隧道专用 reload/merge 按钮；需要重新打开应用或使用已有配置 reload 后重新编辑。停止原 worker 的能力保留。

## 实际执行

环境：macOS arm64，`rustc 1.99.1 (58cf8f9c3 2026-10-13)`，`cargo 1.99.1 (5f94df478 2026-08-27)`。版本是本次实际环境，不是新的 toolchain pin。

检查了主仓库与副本缓存后，按并行作者协调使用本副本已有 `target`，`CARGO_BUILD_JOBS=2`；本任务没有用 root target 构建。

| 检查 | 实际结果 |
| --- | --- |
| `cargo test -p keelshell-app --locked tunnel -- --test-threads=2` | 退出 0；16 passed、0 failed、0 ignored、763 filtered out，执行用例 2.23 秒 |
| `cargo clippy -p keelshell-app --all-targets --all-features --locked -- -D warnings` | 退出 0 |
| `cargo fmt --all -- --check` | 退出 0 |
| `python3 scripts/check.py --policy-only` | 退出 0；x.y 依赖策略通过 |
| `git diff --check` | 退出 0 |
| Hallmark `sloplint.mjs` 对两个 Rust UI 文件运行 `--scope app --genre modern-minimal --render` | 退出 2；不支持 `.rs`，没有可检查的网页文件；静态/渲染均不构成通过 |

本次测试链接器报告 `__eh_frame section too large (max 16MB)`；Cargo 另外提示传递依赖 `block 0.1.6` 的 future incompatibility。保留警告，不把 Clippy 退出 0 解释为所有工具均无警告。

最终日志、退出码及源摘要保存在忽略的 `target/net-01-rule-management-evidence/`。该目录属于本 NET 副本，可供 reviewer 使用。

测试覆盖：

- 规则 local/remote/dynamic 字段校验、默认停用、动态回环限制、隐藏目标字段不持久化。
- 真实指针激活旧绘制的保存、取消、删除、启动和停止控件；替换审核或配置变化时拒绝旧动作。
- 同 revision 下的 state/rule/connection 内容变化、草稿变化、连接切换、挂起面板及迟到保存回执。
- 真实 loopback SSH/SOCKS listener；原 SOCKS 用例验证双向 8 字节 payload 与计数，停止后最终计数和端口释放。
- 同 rule ID 多实例独立停止，其他 rule ID 继续监听；修改规则后保持原 cancel owner、原端点描述和原 SSH session。
- 审核后新增实例时拒绝旧停止动作，重新审核后才停止当前集合。
- Workspace 真实订阅和后台 StateStore 的新增/编辑/删除往返；确认前及取消删除不写入。
- 独立 StateStore 外部写入冲突时保留外部字节和编辑草稿，不能隐式重试。
- 另一个保留面板仍有活动 owner 时，Workspace 删除准入拒绝。
- 900×260 受控 GPUI 工具区，中英文×明暗主题；审核按钮完整位于 viewport 和固定 footer 内，正文滚动不移动 footer。

## 保留的失败与修复

- 初始接续编译发现 `AppState` 不实现 `Eq`、订阅参数是 `&Entity`；修正类型和借用方式。
- 测试编译发现通配导入导致 test 宏递归、遗漏 `AppContext` trait、渲染分支返回类型不同，以及布局断言遗漏引用；均修正后重新编译运行。
- 第一组原 11 项隧道用例已通过；这一中间结果没有替代后来新增测试的最终 16 项结果。
- Workspace 的初始测试夹具试图把已归属于 Workspace 的同一 panel 挂到第二个窗口，在该步骤未收敛。初次组合及定位运行只中断了本任务测试进程；另一次单例运行由 30 秒超时控制器终止并等待回收，退出 124。最终夹具直接操作原 Workspace 窗口。
- 修正挂载后首次 Workspace 集成结果是 2 passed/1 failed：冲突测试的“草稿可见”查找缺少编辑器容器的 GPUI test-support hook。磁盘字节保护断言已越过。补上真实观察 hook 后，最终三项均通过。
- 中断留下的 4 个临时状态目录，按本夹具特有连接/规则字段识别，并核对无打开句柄；原状态先归档到本副本 evidence，再删除临时目录。`fixture-cleanup.json` 保存路径、摘要和删除结果。

## Hallmark 原生适配

已读取 Hallmark 2.0.0 的 App screens、交互状态和相关 gates。现有 `design.rs` 的 `Palette` 与 GPUI Kit 控件是本轮主题依据；不新建 CSS 主题，不改统一设计文档。

界面以规则、端点和活动实例为核心。保留事件/test hooks、中英文与主题；移除隧道文案中的 middle-dot 元信息拼接，采用分行或明确标点。编辑/审核正文独立滚动，审核 footer 固定。空规则提供“新建”，后台保存显示忙碌，存储错误保留草稿并给出恢复动作，full 状态使用真实规则与 worker 回执。

受控布局测试验证的是几何和事件，未完成像素截图、原生键盘导航、焦点对比度、IME 或辅助技术检查。sloplint 无 Rust 解析支持，没有网页 lint 或 render 通过结论。新的独立 reviewer 和组合原生验收仍待主 agent 执行。

## 资源所有权与未验收

本任务未调用 Podman、未创建容器、未操作 CUA/桌面，也未终止其他任务的进程或 VM。测试只使用自有随机 loopback 端口、固定公开测试 host key 与临时状态；成功用例显式停止 worker，夹具关闭 SSH 并终止/等待自己的服务任务。最终命令已结束，本副本 cargo lock 无持有者。已生成缓存保留供新 reviewer/主 agent 复用，避免再次全量编译。

仍未执行：全 workspace `scripts/check.py`、三平台原生 UI、外部 OpenSSH/Podman 的 local/remote/dynamic 矩阵、远端取消未知结果、重连 owner 全矩阵、发布/安装、规则运行状态持久化及重启恢复。测试通过不关闭这些边界。

## 冻结源码 SHA-256

下列摘要属于最初作者冻结点。其后的独立审查修复通过单独精确 patch 导入组合树，不能把这些旧摘要当作最终源码身份。

```text
3319a98c8165d503c1bd940c1e8fec650d808d74caa5c8fa1e5979b82a4dbf0c  crates/keelshell-app/src/tunnels.rs
21cead04469b89e2d165b0cc384ad0d313d337c25a51bfedb4870c17af738188  crates/keelshell-app/src/workspace.rs
52af8a8eae0e72267f9255438379f91d79852d959fe087c8b50f336824218bb4  crates/keelshell-app/src/tunnels/management_tests.rs
372788514701dbf0a8d266a9b45db328504ccd6de574209633e71248c356279e  crates/keelshell-app/src/tunnels/rules.rs
7ff3385cc3707c53f51ded8e0a22dd5e81909d676872bcb271280eb35550ff9e  crates/keelshell-app/src/tunnels/test_support.rs
31187a75f14943c84edba40842bb76de4146d6a0adc365fb8612411894628068  crates/keelshell-app/src/tunnels/tests.rs
8b760735f091385772f1dcbae53b9ff07bd9ec8c5f176ecb8950719f1e3a0a02  crates/keelshell-app/src/tunnels/workspace_tests.rs
```

## 独立审查与组合修复

Curie 首次审查提出 1 P1 / 4 P2：新面板绕过存储不确定性、关闭标签后的 retiring owner 缺失、元数据删除后无法停止、reload 忙碌状态未同步、同快照权威 reload 未解除阻断。Pauli 的独立修复精确导入八个源码路径，新增十项回归；没有修改生产等待期限、放宽失败策略或更换原 SSH owner。

Workspace 持有不确定性并传给新面板；普通 refresh 不能解除阻断。两条知识 reload 入口、成功和错误均传播忙碌状态；权威读回即使内容相同也撤销旧审核。关闭/移除面板后仍保留 worker owner，实际 cleanup 回执后释放；元数据删除检查同时覆盖这些退休中的 owner。

导入组合树后的实际结果：`workspace::tunnel_rule_tests` **11 passed**；`tunnels::` **15 passed**。包含真实 loopback SSH remote cleanup 延迟回执、删除准入与 owner 最终释放。源码前后摘要、精确 patch 和日志位于 `work/themes-20261104/net-repair/`。独立修后复审和完整组合门禁继续记录最终结果。

## 第二轮修复与最终结果

首轮修后 reviewer 仍提出两项 P2：失败的 cleanup 回执不能等同资源释放，配置恢复成功需要清除 Workspace 的不确定性。第二轮六文件 patch SHA-256 为 `241dcf9fa237ee718bf59f9812f53017597b979f8f6ec43233991557790715d5`；导入时全部 preimage/postimage 一致。

未确认清理的 worker 退出后保留规则占用，观察器只持有原 SSH lifecycle watch receiver，不持有强 `SshSession`。只有确认清理或原连接 `Closed` 才解除保护；`Closing`、新连接关闭、元数据 reload 均不能代替原资源终态，原失败结果仍保留。接受的配置恢复通过同一权威 reload 入口解除阻断，不重放 runtime owner 或旧审核。

新增四项回归：真实 SSH 拒绝 cancel 后保留 retiring owner/删除阻断；worker 退出及 `Closing` 不能释放；丢弃最后一个原会话句柄仍会真正关闭；真实备份恢复后新面板可以重新审核启动/停止。修后父任务 **tunnels 17 passed、Workspace 13 passed、fmt exit 0**。Curie 核对六个前后摘要、十个监控文件不变和完整日志，最终 P0/P1/P2 为 0。组合门禁 app 819 passed/2 ignored、完整 Rust 2,007 passed/23 ignored；详见组合记录。

真实超时服务器回归尚未执行：超时分支仅源码复核，`Closing` 控制使用注入 watch channel，不能扩大为原生或外部 OpenSSH 超时验收。完整原始证据保留在 `work/themes-20261104/net-repair-v2/`，两轮外部临时冻结目录在完整保存并核对后清理。
