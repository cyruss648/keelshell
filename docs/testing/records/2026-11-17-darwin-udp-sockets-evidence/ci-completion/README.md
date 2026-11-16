# UDP 工程的精确 CI 终态

源码 `16703293012d798f885e1590f02c348c47e81a10`，
Quality `43945988599` 已 `completed/failure`。
watch 为 `1/reaped/1959.911713s`，三平台终态均已收取，没有活动watcher；
没有取消或重跑该run。

| 平台 | 实际结果 |
| --- | --- |
| macOS / 133575666472 | 完整gate与标准OpenSSH成功；普通Rust3325/0/58 ignored、app1375/0/2、14rustdoc；Python293/2 skipped。92个普通suite的声明数、唯一逐名结果和摘要相等。 |
| Ubuntu / 133575666946 | 完整gate在app发生SIGABRT／stack overflow；声明1378，已观察696 ok／2 ignored，680未报告；没有完整app摘要。后续OpenSSH未执行。 |
| Windows / 133575666982 | 后端预检成功；完整gate在app发生0xc00000fd／STATUS_STACK_OVERFLOW；声明1366，已观察688 ok／2 ignored，676未报告；没有完整app摘要。 |

Ubuntu失败名为
`monitor::tests::independent_disk_monitor::independent_disk_samples_survive_production_theme_buttons_and_focused_input`；
Windows失败名为
`monitor::tests::disk_monitor_minimum_workspace_languages_themes_scroll_and_selection_are_stable`。
两者各有8个完整workspace前缀suite；未报告部分不补写为通过或零失败。
CI没有独立app库存，不能借用宿主库存补签。
本终态只签该729输入，不代签后续渲染栈修复、真实桌面或发行。

封存保存三平台完整原job日志、最初两个失败下载及相等核对、状态和watch、
严格完整parser及显式partial派生、原729输入。初次将完整parser用于中止日志
会拒绝不完整输出；partial首轮遗漏Windows反斜杠路径后退出1。
原诊断和脚本保留；修订派生工具后成功重算，没有修改测试断言或重放测试。
封存校验器首轮比较Python tuple与JSON list时退出1，原脚本、回执和完整stderr
保留在`verification-diagnostics/r1/`；只规范化结果表示后重算，原37成员封存不变。

只读核对原件、重新派生完整／中止计数并绑定精确Git源码：

```sh
python3 docs/testing/records/2026-11-17-darwin-udp-sockets-evidence/ci-completion/verify.py
```
