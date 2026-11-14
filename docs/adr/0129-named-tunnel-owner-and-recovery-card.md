# ADR 0129：命名隧道会话 owner 与恢复候选卡片

日期：2026-11-14。运行状态恢复的前序实现为
`7e6ecb3ea8c347f7f633ba0f205d936cd58d72ec`；本次 owner 边界实现为
`90fbca73fc52e528d82b23cdd3d57d125c72458d`，恢复卡片视觉与可达性实现为
`1c8df52`。独立复核在文档收口后单独进行。

## 背景

命名隧道恢复候选保存的是过去的非敏感观察结果。用户重新打开原连接后，应用
必须允许一次全新的、经过审核的启动，同时要阻止旧 SSH 会话产生的恢复审核或
停止审核跨越重连边界。仅比较保存的 connection ID 不能表达一次活跃 SSH 会话
是否已经被替换；只在 UI 中清空按钮也不能保护后台事件和迟到回调。

恢复候选原先以紧凑的文本行呈现，规则名称、状态、只读详情和操作按钮之间的
层级不够清晰。候选是风险状态提示，不能使用在线指标的视觉语言，也不能把
恢复动作做成普通的启动按钮。

## 决策

### 1. 为每个 SSH 会话建立瞬时 owner

- `ReconnectSessionId` 只存在于内存中；新建会话和替换会话都生成新的身份。
- `TunnelRuntimeOwner` 由 session identity 和随机 nonce 组成，不写入
  `AppState`、运行快照或凭据存储。
- `TunnelRuntimeReviewToken` 绑定完整 owner 和本次审核的随机 nonce。停止审核
  必须匹配保存的 token、原 owner、规则 ID 和精确 worker ID 集合。
- `ValidatedTunnelRecoveryPlan` 记录生成审核时的 owner；应用使用
  `confirm_for` 再次传入当前 owner。owner 变化返回
  `TunnelRecoveryError::RuntimeOwnerChanged`。
- `TunnelsPanel` 在会话被替换或挂起时轮换 owner、清空停止审核并请求原 worker
  清理。轮换不自动停止另一个进程中的 listener，不自动重连，不重放命令；
  原 panel 仍负责其已拥有 worker 的有界收尾。

兼容的无 owner `confirm` 保留给现有无 UI 的领域调用；原生工作台恢复入口必须
使用带当前 owner 的 `confirm_for`。任何新的工作台入口都不得绕过 owner 绑定。

### 2. 用状态卡片表达恢复候选

恢复区域使用现有 registry、Palette 和 component tokens，业务视图不按 Frost、
Graphite 或 Lavender ID 分支。每张卡片固定分为：

1. 规则名称与类型、监听端点、观察时间；
2. 独立的状态 callout；
3. `Inspect`、`Review start` 和 `Discard` 操作区；
4. 用户选择 Inspect 后出现的只读详情。

`PossibleRunning` 使用 warning 语义；连接不匹配和 `CleanupUnconfirmed` 使用
danger 语义。颜色只辅助状态文字，卡片仍显示“可能仍在运行”“需要重新审核”
等明确双语文案。操作区有上边界和独立间距，保证固定 footer/scroll 下不会与
状态正文粘连或被裁掉。

恢复区域、卡片和状态 callout 分别提供稳定 ID、AX role 和中英文 label。状态
callout 使用 `Status`，卡片与恢复区域使用 `Group`。详情保持只读，明确旧
listener、SSH session、worker 和流量不会被复用。

## 后果

保存连接重新认证后，旧会话的恢复或停止审核不能影响当前会话；用户必须在当前
owner 下重新 Inspect、Review 和 Confirm。旧 worker 的生命周期仍由其原 owner
管理，避免为了撤销旧审核而误杀新会话的实例。

恢复候选的视觉层级更容易扫描，状态与操作之间有清晰边界，且辅助技术可以按
区域、卡片、状态和动作理解同一事实。新增主题只扩展 token 与 registry，不需要
修改恢复业务逻辑。

## 验证与边界

作者 focused 验证见[测试记录](../testing/records/2026-11-14-named-tunnel-owner-and-recovery-card.md)。
当前记录只覆盖 core、GPUI headless 行为、布局几何和辅助技术语义。macOS/
Windows/Linux 原生窗口、真实客户 SSH、Podman 长流程、完整 IME/VoiceOver/NVDA、
签名、公证、安装升级和发布验收仍需单独证据。
