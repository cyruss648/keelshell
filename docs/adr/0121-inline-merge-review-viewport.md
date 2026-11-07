# ADR 0121：内联合并审核视口与固定动作区

- 日期：2026-11-08
- 状态：源码候选已实现；专项回归与完整工程门禁通过；独立复核和原生验收待完成
- 范围：文件工具区内联三方合并审核的卡片高度、嵌套滚动、动作布局和旧帧防护
- 关联：[远程文本合并](../product/TEXT_CONFLICT_MERGE.md)、
  [测试记录](../testing/records/2026-11-08-inline-merge-review-viewport.md)、
  [当前交接](../HANDOFF.md)

## 背景

内联合并审核此前把正文和动作混在外层文件工具区的滚动布局中。窗口较小时，冲突
正文没有稳定的内部视口，完整版本或手工输入容易把采用、关闭等动作推到不可见区域。
当正文事件继续冒泡到外层时，用户滚动冲突详情还会同时移动整个文件工具区。

GPUI 还允许旧渲染帧在模型更新后短暂保留。动作 listener 如果只捕获坐标而不绑定
审核观察，就可能把旧审核的动作误投递到新审核。

## 决策

### 1. 内联审核使用有界卡片

内联卡片固定为 `220 px` 高度，使用列布局分出正文和动作区。正文设置独立
`ScrollHandle`、纵向溢出和真实滚动条；动作区固定在卡片底部，不能被正文滚轮移动。
紧凑动作区分为两行，以保留完整版本与当前冲突的稳定命中区域，并将展开动作放入
固定动作行。

完整审核表面继续使用自己的 `tools_scroll`、完整版本滚动句柄和固定保存动作，
不复用内联正文句柄。

### 2. 正文滚轮由绑定审核的处理器消费

内联正文使用带 `review_id` 的 `on_scroll` listener。listener 读取 GPUI 已测量的
最大偏移，将事件 delta 转成像素后按 `[-maximum, 0]` 截断；正文确实移动时通知
视图，并始终停止事件传播，防止外层 `file-tools-scroll` 同时移动。

这项显式更新是必要的：GPUI 的自定义滚轮 listener 会在默认滚动 listener 之前注册，
单纯调用 `stop_propagation` 会连正文自己的默认滚动一起阻止。旧帧 listener 若发现
当前审核 ID 已替换，直接停止传播并返回，不触碰新审核状态。

### 3. 审核状态切换复位正文

切换上一项/下一项、选择草稿/远端/手工方案和切换完整版本时，正文句柄复位到原点。
这样每个冲突或完整版本都从可读的起始位置展示，不继承前一个内容的滚动位置。

### 4. 以几何交集验证嵌套滚轮

GPUI headless 测试在派发 wheel 前先让外层文件工具区露出目标控件，再根据目标控件
与正文视口的几何关系发送小步滚轮。完整版本测试同时计算完整版本、外层工具区和
内联正文三者的可见交集，避免把事件派发到已经被父滚动裁掉的坐标。

## 后果与限制

内联卡片在极小文件工具区中仍可能需要先滚动外层工具区，才能同时看到正文和动作；
测试覆盖了这一可达路径。正文滚轮不会在边界继续推动外层，用户需要明确移到外层
区域才能继续浏览其它文件工具。

本决策不扩展连续无换行保存策略，也没有补齐
`merge-adopt → merge-save → late newer draft/readback` 组合回归。Windows/Linux
原生窗口、IME、VoiceOver、真实 SSH/Podman 长流程、外部 AI provider、签名、公证和
安装包仍按独立验收边界处理。

## 验证

当前源码候选已通过：

- stale merge frame action：`1 passed / 0 failed / 1114 filtered out`；
- conflict merge：`27 passed / 0 failed / 1088 filtered out`；
- review viewport：`12 passed / 0 failed / 1103 filtered out`；
- editor format：`13 passed / 0 failed / 1102 filtered out`；
- `cargo fmt --all -- --check` 与 `git diff --check`。

共享 target 归属切换后的项目包失效重编和完整 `python3 scripts/check.py` 已完成并
退出 `0`：Python `131 run / 1 skipped`，workspace Rust 测试 `2,435 passed /
23 ignored`，app 为 `1,113 passed / 2 ignored`，14 项 doctest 和三个 AI controller
专项均通过。源码提交 `e5891d1c8101c00944979ec2761718d376ae60d5` 已绑定到
`work/native-cua-20261107/package-r11/KeelShell.app`，GUI/MCP 均为 arm64
Mach-O，实际 `minos=15.0`；主题三套和中英文切换已完成隔离桌面短验。

源码提交后的两次独立只读复核尝试均未返回可审计报告，当前状态为
`NON_AUTHOR_REVIEW_UNAVAILABLE`，不能写成非作者批准。Windows/Linux 原生窗口、
IME、VoiceOver、真实 SSH/Podman 长流程、签名、公证、安装包和组合时序回归仍待完成。
