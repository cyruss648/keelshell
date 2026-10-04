# 失败传输显式恢复验证记录

日期：2026-10-04

范围：远程 SFTP 文件/目录传输失败后的恢复建议。

## 已验证

- `cargo check -p keelshell-app --locked` 通过。
- `cargo test -p keelshell-app --locked -- --test-threads=4` 通过，当前工作区 263 项测试通过；完整 workspace 门禁同时通过核心 66 项、会话库 64 项、批量集成 14 项、SSH loopback 94 项和全部 doctest，OpenSSH 外部互操作 6 项因缺少环境而忽略。
- `recovery_candidate_is_derived_only_from_started_transfer_operations` 通过：实际上传/下载操作可以生成候选，`PlanResume` 和目录浏览等只读操作不会被当作已开始的传输。
- 恢复按钮只在当前文件面板仍绑定同一会话令牌、面板未挂起、传输已失败且没有其他操作运行时显示。
- 恢复按钮只调用新的 `PlanResume` 只读校验；计划完成后仍进入既有确认栏，不会直接写入目标。
- `PlanResume` 的成功、失败或取消不会把旧失败传输卡改成完成或取消；只有真实写入操作才更新传输终态。
- 恢复按钮处理器在执行前重新核对会话、失败阶段、忙碌和待审核条件；状态变化后的旧点击不会替换现有审核，也不会清除待审核操作。
- 面板挂起时会清除恢复候选并更换令牌；晚到的工作线程结果不能跨面板恢复旧操作。

GitHub [Quality 37172838871](https://github.com/cyruss648/keelshell/actions/runs/37172838871) 在 macOS 26、Ubuntu 24.04、Windows 2025 全部成功。该流水线证明构建、测试和打包路径，不等同于 Windows/Linux 原生桌面交互验收。

## 未验证边界

- 本记录没有宣称自动恢复、自动重试或并行传输已完成。
- 未在本轮新增真实 SSH 服务器故障注入；已有 OpenSSH 续传互操作记录继续作为底层内容校验和传输证据。
- Windows/Linux 原生窗口交互仍需目标平台设备验收。
