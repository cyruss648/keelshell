# 代理就绪诊断分支：Unix 成功、Windows 提供程序初始化失败

诊断分支提交 `3f26ffe78d13b21421bec53eabbca6b6e1104dd0` 的 [Quality37341962489](https://github.com/cyruss648/keelshell/actions/runs/37341962489) 整体 failure。三个 job 的 API head 与实际 checkout 均为该精确提交；原 c7 的三平台失败继续保留，不追溯改写。该分支未整合 main。

## 实际执行范围

| 平台 | 普通 / 文档测试 | 脚本 | 打包 | CLI 控制器 | OpenSSH |
| --- | --- | --- | --- | --- | --- |
| macOS | 1155 通过 / 8 文档；11 ignored | 6 通过 | 57 通过 | 默认与额外 2 MiB 均实际成功 | 9 项通过，原 artifact 已核验 |
| Linux | 1155 通过 / 8 文档；12 ignored | 6 通过 | 57 通过 | 默认与额外 2 MiB 均实际成功 | 9 项通过，原 artifact 已核验 |
| Windows | 部分累计 109 通过、1 失败、2 ignored；文档未到达 | 5 通过、1 跳过 | 53 通过、4 跳过 | 默认实际成功；额外 2 MiB 未到达 | 跳过 |

Unix 的四项新代理夹具及原完整设置/Apply/Ask 回归均实际执行成功；Windows 在前置 AI HTTP harness 失败后，没有执行这些 app 回归。自托管 CLI 控制器不是普通 libtest harness，不重复计入普通数量。

## Windows 新诊断事实

`explicit_proxy_ignores_environment_exclusions_in_isolated_process` 在 `request_options_http.rs:200` 失败。子进程 PID 7044、exit 101，父回执为 `child nonzero exit`，I/O 错误字段为空，reaped 为 true，两路 EOF 为 true；捕获 stdout 225 字节、stderr 126 字节。实际 stderr 为 OS error code **10106**，消息表示服务提供程序无法加载或初始化。

父 HTTP harness 为 17 通过、1 失败；诊断中捕获的子 harness 是 0 通过、1 失败，不能再加入 workspace 的普通计数。五项新的诊断故障回归全部实际成功。现有 stderr 没有标明具体内部 OS 调用，因此不把这个结果推断为旧 c7 被丢弃的错误，也不把回收/EOF 扩大为任意后代出生身份已清理。

微软将 10106 定义为 Winsock 服务提供程序初始化失败，可能发生于加载 DLL 或提供程序启动时；提供程序路径可以包含尚未展开的 `%SystemRoot%`。据此准备一个仅保留该系统变量的测试候选，同时保留环境清空、所有代理隔离设置及原期限。该解释仍需新的 Windows 对照证实，见[最小系统环境候选](2026-10-06-windows-proxy-system-root.md)。参考：[Winsock 错误码](https://learn.microsoft.com/en-us/windows/win32/winsock/windows-sockets-error-codes-2)、[WSCGetProviderPath](https://learn.microsoft.com/en-us/windows/win32/api/ws2spi/nf-ws2spi-wscgetproviderpath)。

## 证据保存状态

独立审查者取得原 Windows job log：63677 字节、SHA-256 `1bacf0a821e4e4264244d92201d975b686a5776da90df5a517cf0e5e372ddd64`，根独立读取的字节和摘要一致。根第一次抓取被 `gh` 的终端转义输出保护拒绝，零字节日志及 99 字节错误保存；第二次只允许将原始字节保存到文件，不执行终端内容。原全 ZIP 为 258766 字节、SHA-256 `992116fde43efcf70d220cfdadf210aa7aec835d3d6e56277a1ac012b3e168f1`，35 个成员的 CRC/字节均读回通过，三个单 job 日志与 aggregate 完全相等。

两个 Unix artifact 的 API digest、原 ZIP、所有成员与 result/九项具名测试读回通过；出生观察回执分别记录 64/67 个内核身份、自有/已观察进程已停止、祖先未确认列表为空及临时目录已删除。范围是回执的累计观察，未证明两次观察之间完全脱离的后代，也不等于审查者在 runner 上直接检查进程。

完整独立包位于 ignored `work/ai-proxy-fixture-readiness-ci-independent-20261006`，144 proof / 145 归档成员；根逐字节和全部归档成员读回通过。manifest SHA-256 为 `4a6049dc74acd0959ca1bcbf6df4ab91f50d5e21ae6d85814468cc3691c25317`；归档 873387 字节，SHA-256 为 `3e01e209881ea9af29a0c2f1a6e7587cb8f0a399b545d60e2b89dba0810402fd`。独立解析的三次失败均保留，修正不改变原 CI failure。

该 CI 证明其实际源码检查范围，不证明新 GUI、供应商、Windows/Linux 桌面、Release 或安装更新。对外 MCP 方向保持为 KeelShell 提供能力。
