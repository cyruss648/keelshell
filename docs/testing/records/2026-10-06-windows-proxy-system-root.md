# Windows 隔离代理夹具：最小系统环境候选

日期：2026-10-06。接续 [3f26 诊断 CI](2026-10-06-ai-proxy-fixture-readiness-ci.md)。新的实际日志已观察 Winsock OS code 10106，但旧 c7 的具体错误仍未知。本候选基于诊断提交，仅修改 `crates/keelshell-ai/tests/request_options_http.rs`，没有修改生产、依赖、锁、工具链或原代理/认证/脱敏断言。

父进程继续 `env_clear`，仅在 Windows 显式传递现有的 `SystemRoot`；不继承 PATH、用户配置、凭据或父进程代理设置。原 NO_PROXY/no_proxy 与三个固定失效环境代理保持，实际请求仍必须通过显式指定的代理完成，origin 零访问断言保持。已有生产本地智能体启动器也对最小 Windows 系统环境作显式处理，本候选不改变该生产实现。

新增 Windows 专属对照以相同自有 child 分别运行清空 SystemRoot 与明确传递的配置。清空路径若失败，必须确实捕获非零退出和 10106；若安装环境可在没有此变量时完成真实 HTTP，也记录该实际结果，不假定全部 Windows 提供程序目录相同。明确传递的路径必须完成全部真实三请求断言与结束回执。两次均要求 reap 和双 EOF，单次原八秒总预算保持；向 CI 写出的固定短回执仅报告两种结果，不包含系统路径或任何环境值。

依据微软的 [Winsock 错误码](https://learn.microsoft.com/en-us/windows/win32/winsock/windows-sockets-error-codes-2) 和 [提供程序路径文档](https://learn.microsoft.com/en-us/windows/win32/api/ws2spi/nf-ws2spi-wscgetproviderpath)，SystemRoot 是需验证的最小修复假设；当前不宣称 Windows 已修复。

本机 18 项 HTTP 测试通过，22.223793 秒、exit 0，自有 leader 已回收且原进程组为空；Windows 专属对照未在 macOS 执行。完整 `scripts/check.py` 303.024822 秒、exit 0：1155 普通测试、8 文档测试、6 脚本测试、严格整仓 all-targets Clippy、格式、x.y 策略、默认及显式 2 MiB 控制器通过；11 个原 ignored 保持，控制器不重复加入普通数量。258 份工程输入在门禁前后相等，仅上述测试文件区别于3f26，生产字节不变。四个自有 wrapper leader 已回收、原进程组为空，私有临时目录实际为空并删除。

候选源码及全部原运行证据已冻结为 17 payload / 18 归档成员。manifest SHA-256 为 `b30b9455d76f4f3dcde47d5ca72c8c508dbcf853e339576cf296c307cbddc0d8`；归档 92617 字节，SHA-256 为 `af1559488a61d480f765c4a3ad34660f0c83311605e0f8c0264cb6f96e5880c2`。打包源码不变，不以此前3f26的打包结果写成本候选新本机打包运行。原诊断分支失败材料及本次所有运行日志保持，不以新通过覆盖。

新的非作者限定复核 PASS，没有新已证 P1/P2：独立本机 HTTP18 实际通过、15.201433 秒，258 工程输入前后相等，未使用临时 overlay；确认环境增量仅 Windows 的已有 SystemRoot，原限时、容量、路由/认证/脱敏/origin 和收尾代码保持。原完整门禁日志及四次 wrapper 回执已独立核验。Windows 专属测试未在本机执行；官方机制仍为待对照证实的解释。下一步提交诊断分支取得新 Windows 实际回执，再决定修复是否成立，尚未整合 main。

非作者复核材料已冻结为30 payload /31归档成员，manifest SHA-256为 `c6b87b15a9d8bd43def2514b120d6c54cfd3dc4f7567ae456495d05dfcff6863`；归档227128字节，SHA-256为 `88d088aa5f99ab44ee31dab5b35438b127a5d59b4d432465f9f37ea17941859c`。根已逐文件和归档成员核对全部原字节，并确认诊断工作区源码仍与冻结候选一致。此封包核验不增加Windows执行或产品验收范围。
