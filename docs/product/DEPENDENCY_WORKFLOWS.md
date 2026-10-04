# 手工依赖工作流 / Manual dependency workflows

在命令区点击“依赖工作流”，可把当前命令作为第一个任务草稿。任务和输出
只保留在本次应用工作区，关闭应用后不恢复，也不写入普通批量审计或命令历史。
最小窗口打开 AI 助手时，命令区的历史策略、新命令、批量任务和依赖工作流按钮
会按可用宽度换行，保留完整文字；正在运行的任务可通过同一入口重新打开。

1. 先连接需要的 SSH 主机。工作流只列出当前已认证且仍在线的会话，不会自行连接。
2. 为每个任务填写命令、可选名称，并显式选择 SSH 目标。使用“添加任务”和
   “编辑任务”切换编辑；删除任务时也会删除引用它的依赖边，执行前要重新核对。
3. 选择前置任务。前置任务必须全部明确成功才放行；失败、未知或未放行不会允许
   下游开始。最多 128 个任务、32 个不同目标、每任务 32 个直接前置。
4. 设置并发 1–8、每个已放行任务的超时 1–300 秒，并选择失败后停止等待项或
   继续独立分支。合计 stdout/stderr 每任务最多捕获 256 KiB。
5. 点击“下一步：完整审核”。滚动核对每个任务的完整命令、目标、路线和前置。
   元数据模板支持 `{{name}}`、`{{host}}`、`{{port}}`、`{{user}}`、`{{endpoint}}`；
   审核同时展示源文本和最终发送的命令，隐形字符以转义显示。
6. 人工点击“确认全部任务并执行”才会产生 SSH 请求。需要修改时点击“返回修改”。
7. 在任务列表选择“任务输出”查看本次捕获的 stdout/stderr 和精确回执。未知结果
   需要人工判断；应用不会自动重试。返回工作区会隐藏面板，运行和回执仍归原面板持有。

“取消工作流”停止后续放行并取消本地等待，不能证明远端进程已经停止，也不能撤销
已完成的任务。在线会话被替换或目标资料变化后旧审核失效；点击“刷新已连接会话”
后需要人工重新选择目标并审核。原会话不会被替换后的相同端点悄悄继承。

Click **Dependency workflow** in the command area to seed the first transient
command draft. Connect required SSH hosts first, then add tasks and explicitly
select each authenticated session and its prerequisite tasks. Set concurrency,
per-task timeout and failure policy. **Next: complete review** shows every task's
full source and rendered command, endpoint, route, session binding and dependencies.
Only **Confirm all tasks and execute** sends SSH requests.
The four command-area actions wrap to fit a compact window with the AI sidebar
open, keeping complete labels visible. The same entries reopen retained running
batch or dependency workflows.

Choose **Task output** to inspect the complete captured bounded stdout/stderr and
transport receipt. Pending dependencies require explicit successful exits. Failed,
unknown and skipped tasks cannot release downstream tasks; the continue policy
permits independent branches. **Back to workspace** hides and retains the owned
run. Cancellation stops local waits and pending admission, without proving remote
termination or rolling back completed work. No automatic reconnect, retry or replay
occurs. A replacement connection requires new explicit target selection and review.
Task drafts, commands and output remain in memory and do not enter stored metadata.

See [ADR 0044](../adr/0044-reviewed-dependency-workflow-ui.md) and the
[verification record](../testing/records/2026-10-05-dependency-workflow-ui.md) for
limits and the distinction between renderer/SSH fixtures and native acceptance.
