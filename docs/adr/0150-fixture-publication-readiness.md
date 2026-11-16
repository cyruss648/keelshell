# ADR 0150: 测试身份记录的发布完成屏障

- 状态：Accepted
- 日期：2026-11-16
- 范围：Python 进程清理测试工装；应用和进程所有权实现没有变化

## 背景

源码 `95813a76f0ec9b59fd043cd982e33cdc01dd69f8` 的 Windows CI 先通过
filesystem、archive codec 和完整 session library，随后 Python
`test_real_parent_failure_keeps_exit7_even_with_descendant_pipe_timeout`
在 `reading_publication` 得到 `PermissionError / errno 13 / winerror None`。
控制器保留 `observed_exit=None`、`controller_exit=125`、
`owned_preparation_failed`，没有把它当作原父进程退出 7。

原生产者先关闭 pending 文件，再 `os.replace` 到最终名，然后创建 `published`
标记并等待 ACK。普通观察者却仅凭最终名尝试读取，可能与发布操作重叠；
该时序是源码分析，不是原日志提供的独立系统调用定位。
已有 staged 反例故意暴露空／部分内容，必须继续保留。

## 决定

`ready_pids` 增加必需的发布标记参数。标记缺席时不打开身份记录；
只有生产者完成关闭和 rename 后创建的标记可开启读取，之后仍须通过完整 JSON、
精确字段、PID 类型／范围和原父 PID 校验。原始进程句柄与所属 Job／group 的
绑定继续在 ACK 之前完成，原短执行预算仍只在身份绑定后收紧且不能延长外层期限。

普通 tree 使用已有标记；原生 Job barrier helper 补相同标记与调用参数。
staged 反例读取空／部分内容只用于向正在等待的生产者确认观测，
不能授权身份绑定或 ACK；其部分阶段确认后继续等完成标记。

不捕获或重试发布后的 PermissionError，不增加超时，不把任意失败当作清理成功，
不修改 `windows_native_crash.py`、应用、Job 所有权、UNKNOWN 或清理断言。

## 验证与边界

新增确定性反例分别保证“完成文件存在但标记缺席仍不得读取”和
“标记出现后读取错误仍失败关闭”。Windows 专用反例持有实际 share=0 文件句柄，
先证明普通读被拒绝、标记缺席时不读，释放句柄并发布后才读取完整身份。
宿主对该 Windows 反例保持 skipped；不以模拟签署 Windows 文件／进程行为。

受控旧读取函数必须命中 PermissionError，受控吞错函数必须命中
`PermissionError not raised`。原七类 tree 场景、生成 helper 编译和完整 Python
discovery 均继续执行。精确结果、非作者意见与新 Windows CI 见
[测试记录](../testing/records/2026-11-16-windows-fixture-publication.md)。
