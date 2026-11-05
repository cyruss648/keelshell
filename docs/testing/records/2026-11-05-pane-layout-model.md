# 分屏布局领域模型与持久化 — 2026-11-05

## 范围与身份

分支 `feature/pane-layout-model-20261105`，基线
`967c588803df493d06d42cf96221bd691aba46b9`。
独立 worktree 的改动仅面向 `keelshell-core` 的布局模型和 StateStore 接入。
连接树候选位于另一工作副本，其修订原生包不包含本切片；
本核心切片没有启动应用或容器。导航候选另有一次 r2a 受监督启动，
但窗口访问返回 Mac locked，没有交互证据且应用/夹具已清理；该运行既不包含
本核心提交，也不代表布局原生验收。导航记录留在其原工作副本。

新增模型与持久化不是原生分屏验收。设计见
[ADR 0100](../../adr/0100-bounded-inert-workspace-layout.md)，
产品范围见[工作区布局](../../product/WORKSPACE_LAYOUT.md)。
本地原始材料位于 `work/pane-layout-20261105/`。

## 工程状态

- 官方依赖 freshness 检查于 2026-11-05T03:07:00Z 开始，退出 0并回收；
  本轮没有依赖或 Cargo.lock 变化。
- 作者领域专项最终为 14 unit、20 integration 和 1 rustdoc 通过，进程退出并回收。
  首轮 integration 为 19 passed / 1 failed：派生枚举解码接受对象形式，
  不符合轴仅允许字符串的约定。实现改为严格 string visitor 后，20 项复跑通过；
  首轮未保存输入摘要，不主张测试文件字节相同。原失败与修后日志保存在
  `author-model/`。这属于候选开发缺陷，不是旧 main 的行为修复。
- 父任务真实隔离文件专项 `storage-scoped-1` 为 12 passed，退出 0并回收。
- `full-gate-r1` 在 Clippy 阶段退出 1并回收：父新增测试存在 9 处禁止的 `unwrap`。
  该轮此前 Python 128 run / 1 skipped 和格式检查已通过，尚未执行完整 Rust 行为测试。
  已将测试取值改为 `ok_or` 传播、单叶夹具使用完整字面构造；未改生产源码。
  因测试字节变化，不将该项描述为同测试修前/修后行为对照。
- `full-gate-r2` 已退出 0并实际回收：**2,107 ordinary Rust passed / 23 ignored**，
  其中 app **868 passed / 2 ignored**（子集，不能重复累加）；
  **11 rustdoc**（含一项仅编译），Python **128 run / 1 skipped**。
  格式、workspace/all-targets 严格 Clippy、依赖策略和附加进程控制器均通过。
  新增 46 项领域/存储测试逐名通过；499 个门禁与检查后输入相同，
  见 `full-gate-r2-summary.json`、`full-gate-r2-parsed.json` 和
  `post-gate-r2-inputs.json`。
- 九份候选 Markdown 的 478 个本地相对目标检查无断链；该检查不验证网络链接。
- 新非作者源码与功能复核为 **APPROVE_SCOPED_SOURCE，P0/P1/P2/P3=0**，
  已读取全部 17 个候选文件、原门禁日志/回执，并独立核对 499 个当前输入和
  46 项新增测试名称。reviewer 未重跑 Cargo 或独立 probe，不能写成额外执行通过。
  报告为 `independent-review/source-r1.md/json`。
- 九份文档与工程证据的限定终态复核为 **APPROVE_SCOPED_COMBINED，
  P0/P1/P2/P3=0**，报告 `independent-review/final-r1.md/json`。
  该结论覆盖核心源码、测试证据和文档表述，不涵盖原生布局恢复。
- 上述审查成稿时尚未提交、推送或取得该候选的远端 CI；后续精确提交绑定、
  推送、CI 与清理状态由 `work/pane-layout-20261105/delivery-readback.json` 维护。

## 提交与外部结果

功能提交 **`3e2f41f9d44e2ac48973a13ebcc97290b9d7ac91`** 已快进合入并推送 main，
远端分支已读回相同 SHA。后续独立复核 **APPROVE_SCOPED_COMMIT_BINDING，
P0/P1/P2/P3=0**：499 个门禁输入及 17 个候选文件全部与该不可变提交 blob 相同，
8 份源码/测试与之前批准快照一致，4 份文档仅回填终态，另外 5 份文档未变。
报告为 `independent-review/committed-r1.md/json`。该复核未重跑 Cargo，
不把源码身份核对描述为额外测试执行。

精确 push Quality 为 **41963286647 / attempt 1**，绑定上述功能提交。
整体及三个 job 均为 **success**，原始结果如下，app 是 ordinary 的子集：

| 平台 / job | ordinary passed / ignored | app passed / ignored | rustdoc | 新增布局测试 |
| --- | --- | --- | --- | --- |
| macOS / `127034402619` | 2107 / 23 | 868 / 2 | 11 | 46 / 46 |
| Linux / `127034403001` | 2103 / 24 | 868 / 2 | 11 | 46 / 46 |
| Windows / `127034402898` | 2047 / 18 | 859 / 2 | 11 | 46 / 46 |

三平台 scripts Python 各运行 128 项、跳过 1 项；packaging Python 各运行 57 项，
macOS/Linux 无跳过，Windows 跳过 4 项。格式、严格 Clippy、依赖策略及
附加进程控制器通过。rustdoc 均包含一项仅编译，不将其写成额外行为执行。
独立 OpenSSH 互操作在 macOS/Linux 各 **11 passed**，Windows 按工作流条件跳过，
不计入 ordinary 总数。两份 artifact 的 archive digest 与 API 相同，
原始 tests.log、result.json 和清理回执已提取保留；这不代表生产服务器兼容性。

原始 job 日志、run archive、逐名测试及完整 OpenSSH artifact 保存于 `current-ci/`。
四次 GET 因 EOF 失败，后续成功读到同一 run/attempt 终态；此前一次父状态读取
命中了未写完的 JSON，保留在 `snapshot-read-race.json`。这些是获取失败，
不能记作 CI 执行失败或无执行。观察器会话 **72061** 实际退出 0并回收，
60 个已启动子进程均记录回收；`parent-reap.json` 单独保存父方回收结论。
退出 0本身不代表 CI 成功，success 来自精确 run/job 及日志读回。

资源收尾由交付回执维护。没有沿用基线结果，也不将 CI 工作项名称中的 native
解释为桌面交互验收。后续状态文档提交不改变上述功能提交的工程输入。

## 验证内容

布局图的分割、关闭、活动叶和比例调整；深度/数量边界、坏引用、环、多父及
失败不改变原对象。元数据验证完整叶覆盖、空面板选择、跨窗唯一标签和总量上限。
真实文件验证旧配置读取不改写 `state.json`、嵌套布局往返、缺失/回收站连接、旧 revision
拒绝覆盖、显式备份恢复、连接导出不携带布局、导入保留本机布局。
未知动作/秘密字段在每个布局层拒绝，坏文件不被默认配置覆盖。
读取仍沿用 StateStore 的既有锁文件机制，不声称完全没有文件系统写入。

审查者曾误启动嵌套 reviewer，该进程在 app-server 初始化时权限失败，
退出 1并回收，没有产出审查；其临时数据库、sandbox 与 runner 已清理，
失败证据保留。本轮有效审查由新派出的非作者 agent 本人完成，不使用该失败子进程结论。

## 未完成边界

尚未接入原生渲染、分隔条拖动、多窗口、重启恢复入口或完整工作区作用域。
没有三平台桌面、发布或安装结论；核心数据结构通过不意味着这些交互已完成。
此轮不修改 Hallmark 原生结论，也不把锁定工具当作候选包失败。
