# 2026-10-14 CI 准入与 Windows 身份等待

## 来源

精确提交 `78c77208e61f347d64c759b15c00daa77ae35a6b` 的 GitHub Actions run
`38500698507`：macOS 26 成功，Ubuntu 24.04 与 Windows 2025 失败。完整失败日志保留在
`work/2026-10-14-ci/failed.log`。前一次 run `38500517205` 因后续推送被取消，不作为独立结果。

## Ubuntu

应用测试 741 通过、2 失败、2 ignored。
`wrapped_file_actions_still_review_the_exact_selected_remote_target` 在 120 秒准入等待后失败。
诊断显示 group 96
`real_file_rows_and_controls_survive_transfer_comparison_editor_and_review_states`
仍为 `owned`，有 4 个队列和 5 个强引用，且只有取得许可和队列注册事件。
随后 `mcp_file_human_approval_writes_exact_bytes_once_and_read_hash_matches`
因同一本地写入域隔离而返回 `MutationQuarantined`。

布局测试的一个 GPUI `TestAppContext` 会保留全部窗口。四个场景原来都等到函数结束才释放，
因此第一张许可被后续场景复用。现改为每个场景完成后关闭窗口、释放 panel 和 group，
并断言该 group 的强引用归零。本机四线程组合中，布局、被阻塞的文件动作和 MCP 写入测试均通过。
这不替代新的 Ubuntu CI。

## Windows

失败发生在 Python 准备控制
`test_real_parent_success_cannot_leave_detached_descendant_running`，早于 Rust 应用测试。
父进程请求退出 0，但身份绑定尚未发生；5.4 秒总期限中的 5 秒清理预留使观察循环提前停止。
这与历史应用异常 `0xc0000409` 不是同一次故障。

控制器现仅在直接退出码为 0 且存在身份观察钩子时，使用剩余的完整夹具时间等待身份发布。
非零退出、未退出、超时和中断仍保留原清理预留。总期限、退出码、回收和管道排空断言不变。
本机新增两个时钟反例，并重新通过 9 项真实 POSIX 进程树控制。该结果不能代替 Windows 原生验收。

## 拖放边界

受控 GPUI 拖放上传此前已验证。原生文件管理器拖放、三平台桌面、签名安装及 Windows 应用异常仍未关闭。
