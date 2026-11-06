# ADR 0114：独立工作窗口、共享配置闸门与窗口级清理

- 日期：2026-11-07
- 状态：源码候选已实现；定向 GPUI 回归与完整工程门禁通过；独立复核与真实桌面验收待完成
- 关联：[能力清单](../product/CAPABILITIES.md)、[测试记录](../testing/records/2026-11-07-multiwindow-editor-layout.md)

## 背景

KeelShell 原先只有一个原生窗口。`Workspace` 同时持有 SSH 会话、终端输入、
文件草稿、AI 审核、网络诊断、隧道和本地配置状态。直接重复创建一个根实体会
把会话、审核或保存中的配置状态误认为全局状态，也会让关闭一个窗口时难以判断
哪些后台 worker 和 SSH 连接仍属于另一个窗口。

`StateStore` 已经提供基于快照 revision 的丢失更新检测，但两个窗口若各自持有
旧的 `AppState`，仍需要一个进程内协调层来串行化保存、显式传播变化并保留
Conflict、Durability 和未知结果语义。

## 决定

### 窗口所有权

新增 `AppWindows` 作为按 `StateStore` 路径注册的进程内共享实体。它只共享：

- `StateStore` 的引用；
- 一个 `ConfigurationGate`；
- 更新服务及更新操作 owner；
- 窗口注册表和退出状态。

每个 `Workspace` 都拥有自己的 `window_id`、原生窗口句柄、布局窗口 ID、终端
实体、SSH 会话、远程面板、输入草稿、AI 审核、补全和关闭审核。新窗口从空的
工作区开始，不自动恢复已保存布局，也不复用另一个窗口的会话或 modal。
窗口最多八个，沿用 core 中的 `MAX_LAYOUT_WINDOWS` 上限。

### 配置写入

所有配置写入先取得当前窗口的 `WriteLease`。闸门绑定：

- 当前 `AppState` 的 snapshot revision；
- 当前配置 epoch；
- 当前窗口 owner；
- 写入、重新加载和维护操作的互斥状态。

写入成功后，闸门采用新的状态和 epoch，并通知其他窗口。其他窗口只同步主题、
语言等外观偏好；连接目录、知识审核、命令审核和其他本地目录保持原状，同时
标记为 stale，要求用户显式重新加载后再审核。Conflict、Durability 或未完成
lease 会进入不确定状态，禁止自动重试或重新开放旧授权。

配置维护要求只有一个空闲窗口，并阻止新窗口和其他窗口的旧面板事件进入维护
过程。共享更新服务通过 owner 约束，避免多个窗口同时控制检查、安装或重启。

### 关闭与清理

关闭窗口先生成绑定窗口、SSH 连接、终端输入、文件格式草稿和审核状态的
`CloseReview`。确认前不撤销权限；窗口、会话、草稿或格式发生变化后，旧票据
失效，必须重新审核。

确认后只撤销当前目标窗口的输入和审核，暂停其远程面板，关闭其 SSH 会话，
并按 owner 停止和 join 其 transport worker。清理使用有界 deadline；未确认的
worker 或 SSH 结果保留 owner 和原窗口，不自动重试，也不影响其他窗口继续输入
或使用 SSH。应用级 Quit 只有在所有参与窗口完成清理后才会退出。

## 后果与验证边界

窗口间的会话、草稿、AI 审核、文件操作和 worker 生命周期得到隔离；共享配置
变更不会静默覆盖另一个窗口的草稿。代价是用户需要在 sibling 配置变化后显式
重新加载，关闭和安装也会经过完整审核。

当前定向测试覆盖空白新窗口、八窗口上限、共享配置冲突与 reload、主题和语言
同步、关闭票据失效、窗口级 worker join、真实 loopback SSH 双窗口存活以及
Quit/Install 前的聚合审核。app 当前为 1,075 项测试、2 项 ignored；完整
`python3 scripts/check.py` 实际退出 0，workspace 为 2,397 ordinary / 23
ignored、11 rustdoc，Python 为 131 run / 1 skipped，三组 AI controller 通过。
独立只读复核、真实 macOS 多窗口/编辑器动作验收、Windows/Linux 原生桌面、多
显示器几何、辅助技术和安装验收仍未完成。
