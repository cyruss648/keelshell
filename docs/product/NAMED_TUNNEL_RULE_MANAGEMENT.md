# 命名隧道规则管理

本指南描述 NET-01 的原生 GPUI 管理入口。它只管理已保存 SSH 连接的规则元数据；活动监听器属于创建它们的原 SSH 会话。

## 使用流程

1. 从已保存连接打开 SSH 标签，选择“隧道”。临时连接仍可使用临时端点，但不能拥有命名规则。
2. 选择“新建”，填写名称、类型、监听地址和端口。监听端口 `0` 表示由转发实现分配端口。本地和远程转发必须有目标主机及非零目标端口；动态 SOCKS5 仅允许回环 IP，不保存目标端点。
3. 新规则默认停用。启用只表示允许后续显式启动，不会自动创建监听器。
4. 选择“审核保存”，检查端点、规则 ID 和启用状态，再选择“确认”。取消审核返回草稿，取消编辑丢弃该草稿；确认前不写入配置。
5. 编辑已有规则保留 rule ID。修改只影响下次显式启动；活动实例继续使用原端点、原会话和原流量计数。
6. 删除需要独立确认。当前标签、其他标签或保留会话面板仍有该 rule ID 的活动实例时，Workspace 拒绝删除。停止请求尚未确认清理完成也算活动实例；远端拒绝或清理超时不会解除保护，只有确认清理成功或原 SSH 连接已关闭后才解除。
7. 选择规则并“审核并启动”。规则必须已启用，确认只消费这一次审核。
8. 活动命名行的“审核停止”列出本面板中该 rule ID 的活动实例及其原始端点。“确认停止”只取消审核中的这些实例，不影响其他规则或临时隧道。清理回执到达后才显示停止终态。

重复启动同一规则会产生多个独立实例。同一规则在另一个 SSH 标签中的实例属于另一个面板，需在该标签单独停止。

## 审核与状态变化

编辑器固定保存打开时的完整配置快照、连接绑定和稳定 rule ID。确认前再次校验 state revision、完整状态、规则内容及连接内容；不把旧草稿自动重放到新状态。

保存、删除、启动按钮绑定各自审核身份及配置快照。停止审核独立绑定原面板运行时 owner、rule ID 和精确 worker ID 集合，不依赖仍存在的规则元数据。旧绘制的按钮不能批准或取消替换后的审核；新增实例或原实例结束会使旧停止审核失效。权威配置重新加载即使删除了规则元数据，也保留停止原实例的能力。

配置刷新保留草稿文字，但旧审核失效。需要取消旧草稿，基于最新规则重新编辑。会话挂起或切换绑定后，旧操作不能重定向到新会话。

StateStore 写入在 Workspace 后台 executor 执行，保存期间阻止其他面板启动命名规则或修改规则。外部写入冲突以及保存失败/结果未知会保留草稿并阻止再次保存或启动命名规则；Workspace 持有这项阻断，关闭面板后新开的面板也不能绕过。当前切片没有专用的隧道 reload/merge 按钮：重新打开应用，或使用已有配置重新加载/恢复入口，取得权威快照后重新编辑；普通界面刷新不会解除阻断，也不会自动重试。权威读回即使内容相同也会撤销旧审核，恢复保存和命名启动的准入。现有 worker 的停止和会话清理能力保留。

## 界面与可验证边界

本增量按 Hallmark App screens 原则使用现有语义颜色、字体和 GPUI Kit 控件，保留中英文。规则与端点是正文，元信息分行展示；编辑和审核正文可滚动，操作 footer 固定。空规则引导“新建”，后台保存显示忙碌状态，错误说明恢复动作，有数据时显示真实规则和运行结果。没有营销区、模拟指标或装饰动画。

受控 GPUI 事件、临时 StateStore 和 loopback SSH/SOCKS 测试与原生桌面验收分别记录。此切片不声明 macOS/Windows/Linux 原生布局、键盘与辅助技术、外部 OpenSSH/Podman、远程转发未知结果、运行状态持久化或重启恢复已经验收。

## English

Open **Tunnels** from a saved SSH connection. **New** creates a disabled draft. Local and remote rules require a destination host and nonzero destination port; dynamic SOCKS5 accepts only a loopback IP and stores no destination. Port `0` requests an allocated listener port.

**Review save** presents the exact candidate. **Confirm** persists metadata through the workspace background executor. Cancelling the review retains the draft; cancelling the editor discards it. Editing retains the rule ID and affects the next explicit start. Active instances keep their original endpoints, session and traffic ownership.

Deletion requires its own confirmation and is rejected while any current or retained panel still owns active instances for that rule. Failed or timed-out remote cleanup retains this protection until cleanup is confirmed or the original SSH transport closes. **Review stop** binds the runtime owner, rule ID and exact current instance IDs in this panel, independently of rule metadata. Confirmation requests cancellation of those original workers only. Removing metadata through an authoritative reload does not remove the ability to stop the original instances. Instances in another SSH tab require separate review in that tab.

Save, delete and start reviews bind state, rule and connection snapshots; stop review binds runtime ownership and workers. Painted controls cannot approve a replacement review. Changed configuration invalidates the draft's review without silently rebasing it. Failed or uncertain storage blocks further rule saves and named starts across existing and newly opened panels until an authoritative configuration reload or recovery succeeds. Even an identical authoritative snapshot invalidates old reviews and clears this block; an ordinary refresh cannot. This slice has no dedicated tunnel reload/merge control; reopen the app or use an existing configuration reload or recovery entry, then review again.

The interface keeps the native theme, bilingual controls, scrollable content and fixed review actions. Automated GPUI, storage and loopback forwarding evidence does not constitute native desktop or external-server acceptance.
