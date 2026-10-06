# 定时依赖工作流作者测试记录 — 2026-10-06

## 来源与范围

本候选从公开提交 `24a19b954b2634c3f54d9554e5ce9b04a0062e9b` 独立开发。
这份记录是作者工程证据，不是非作者独立审查或原生验收。源正文、完整 git 可见
输入集合、日志、退出状态、私有临时目录及进程组停止结果在 ignored
`work/scheduled-workflows-20261006` 的逐次 receipt 中保留；最终封存另绑定窄补丁
和该基线的修改前正文，根整合应保留其它候选的参数、传输和磁盘工作。

产品范围是应用存活期间的单次和有界间隔重复，详见
[产品说明](../../product/SCHEDULED_WORKFLOWS.md)和
[ADR 0063](../../adr/0063-scheduled-workflows.md)。没有改变 MCP schema、工具或授权，
没有引入 MCP 客户端、自动重连、凭据保存或任务级持久日志。

## 已执行的行为检查

- `cargo test -p keelshell-core --locked scheduled_workflow -- --test-threads=2`：
  19 个领域单元通过，0 失败。覆盖 Gregorian/固定偏移、Unicode/控制字符拒绝、
  1–32 次、至少 60 秒、宽限 1–60 秒、七天及溢出边界，inclusive grace/+1 ms，
  不重复领取/完成、前次占用、不补跑、取消、跨计划/修订 token 拒绝、失败或未知停止、
  时钟倒退与累计 2000 ms 偏差。执行总耗时 78.461 秒，包括编译及零命中过滤的其它
  集成目标；实际 19 个单元耗时日志为 0.00 秒，不能把过滤目标当作通过的集成测试。
- `cargo test -p keelshell-app --locked scheduled_workflow -- --test-threads=2`
  的 v6：5 个 GPUI/workspace/TCP SSH 用例通过，0 失败，测试段 65.09 秒，
  总耗时 75.073 秒。真实墙钟两次触发间隔 60 秒，没有调快应用时钟；测试使用
  自有 Tokio timer 的完成标志并仅在完成后收取结果，避免向 GPUI 测试调度器注册
  外部线程 waker。SSH peer 记录真实 exec 帧并返回受控回执，不运行任意命令或真实 shell。
- 上述界面范围覆盖：完整逐次审核、确认只启用而不立即执行、隐藏后到时执行与
  同一面板重开、取消不触发、同 endpoint 的新认证连接拒绝、没有 InputEvent 的
  `15 → 015` 等价值修改拒绝、900×580 下中文/英文 × System/Light/Dark 六种组合，
  全部输入、审核、固定页脚和滚动可达；没有写入任何 PTY。
- 新增明确失败与在途取消的
  `cargo test -p keelshell-app --locked scheduled_workflow_c -- --test-threads=2`：
  4 个匹配用例通过（包含两个已覆盖的 controls/cancel 用例），0 失败，测试段
  9.85 秒、总 19.279 秒。退出 7 的回执把当前槽记 Failed、后续槽停止；peer 保持
  已确认 exec 打开时，取消只完成本地等待并撤销未来槽，不声称远端进程停止。

`python3 -B scripts/check.py` 的 full-gate-v2 实际 exit 0，总耗时 412.852 秒：
版本策略、格式、严格 Clippy、6 个 Python、1264 个普通 Rust 和 8 个 rustdoc 测试通过，
13 个 opt-in/平台用例在普通门禁中忽略。默认与 2 MiB 栈各自完整实际 controller
future 都到 sequence 625/end；没有调用供应商 CLI 或模型。572 个 git 可见输入在运行
前后完全一致，原始输入 map SHA256 为
`169d319b6db4e4626c96a7d421cda50489c7d212c444b9049ac2032e9c6f10ea`。
日志 365590 字节，SHA256 为
`2f7227df2d227239fe53e55bf27138cc18acc9af526b21548f162851805d3ca1`。

`python3 -B scripts/openssh_interop.py --output <new-owned-receipt> --timeout 300`
实际通过 9 个 opt-in 互操作测试，harness 17.948 秒、外层 18.052 秒，exit 0。
原始 receipt 记录 50 个累计观察且带内核出生身份的自有进程已停止、无未验证 ancestry、
临时目录已删除；该 census 不能证明观察间隔内完全脱离的未知后代。输入前后同为上述
572 文件且完全一致。真实 loopback OpenSSH 包括既有 SFTP/exec/workflow 适配器，
不能把它单独视作新定时 UI 原生运行。

本记录在上述运行后仅补充 Markdown 的实际结果；最终 packet 单独记录这一文档
差异，并逐正文验证所有其它输入、Rust/Cargo/scripts 与成功门禁完全一致。
没有因文档记录而重复完整测试。作者证据不取代独立复审、根组合门禁和原生
时区/休眠/应用退出交互，后者仍需各自证据。

## 失败保留与修复

所有下列失败保留各自原始源 map、日志和退出状态，后续通过不覆盖旧证据：

| 运行 | 原始结果与原因 |
| --- | --- |
| full-gate-v1 | exit 1；严格 Clippy 报 nested if 可合并，Python/格式/版本策略已过；保留日志后修正 |
| draft-fmt-check | exit 1；尚未格式化的候选 |
| compile-ui-v1 / v2 | exit 101；新 UI 模块的组件/trait 导入遗漏 |
| scheduled-ui-v1 / v2 | exit 101；测试上下文类型和 Selectable 引用路径错误 |
| scheduled-ui-v3 | 0/5；测试把输入 Change 和 Review 放在同一 UI 回合，排队输入事件撤销审核 |
| scheduled-ui-v4 | 3/5；过早检查 Hide 订阅结果，以及将 65 秒 GPUI 测试时间误作真实墙钟时间 |
| scheduled-ui-v5 | 1/5；直接 await Tokio JoinHandle 把 GPUI waker 从工作线程唤醒，被确定性测试调度器拒绝 |

v6 将输入/点击置于不同回合、在 Hide 订阅处理后检查，并用真实时间且无外部 waker
的受控等待；没有降低生产执行边界、延长 grace、改变间隔下限或注入加速时钟。
作者源码自查另修正了启动拒绝的 Running 收尾、claim 最后槽过期的终态 UI 同步，
以及明确失败/未知分类。后一类源码自查不是已执行反例或独立评审结论。

## 尚未验收

- 非作者冻结候选审查、根组合源门禁与发布构建；本轮不提交、推送、打标签或覆盖安装。
- 本切片 macOS/Windows/Linux 原生桌面的定时运行、休眠恢复与系统时区变更。
- 客户 SSH、生产任务、真实云模型及第三方智能体；全部未访问。
- 持久调度、后台 daemon/cron、重启恢复、无限重复和任务级持久审计，仍是开放产品目标。

即使其它功能已有真实原生证据，也不能继承为本切片的定时验收。
