# API 请求选项 F：首次三平台 CI 失败

后续状态见 [2026-10-06 就绪时序与隔离诊断候选](2026-10-06-ai-proxy-fixture-readiness.md)。该候选的本机通过及独立复核不改写本页原三平台失败，Windows 原 child 原因仍未知。

功能提交 `c7b6b1a76161c757a08cef3de2c21f740986e4fe` 已推送公开仓库，提交后本地/远端 main 相等、ahead/behind 0/0、工作树干净。258份Rust/Cargo/工具链工程字节与根完整门禁相等。[Quality37332920375](https://github.com/cyruss648/keelshell/actions/runs/37332920375) 实际 `completed/failure`；三个job的API head与实际checkout均为该精确提交，各自Rust quality gate失败。不能以此前本机1146普通+8doc通过，或前一df8的三平台CI，关闭本次失败。

## 实际执行范围

| 平台 | 已执行普通测试总结 | 脚本 | 打包 | 默认CLI控制器 | 专门2MiB控制器 |
| --- | --- | --- | --- | --- | --- |
| macOS | 534通过，1失败，2 ignored | 6通过 | 57通过，0跳过 | 实际成功marker，future4624字节 | 未执行 |
| Linux | 534通过，1失败，2 ignored | 6通过 | 57通过，0跳过 | 实际成功marker，future4624字节 | 未执行 |
| Windows | 104通过，1失败，2 ignored | 5通过，1跳过 | 53通过，4跳过 | 实际成功marker，future4968字节 | 未执行 |

这些普通计数是失败前各harness的部分累计。默认CLI使用自定义harness、没有普通测试总结，不加入普通计数；原两个供应商opt-in ignored保持。格式、依赖策略及严格Clippy先执行通过；workspace测试遇失败退出101，`scripts/check.py`退出1，后续文档测试/其它未到达crate、专门小栈和Unix OpenSSH不计通过。OpenSSH产物API数量0，上传步骤无文件；不借旧CI回执补齐。

## 原日志错误与诊断边界

macOS/Linux的app harness均429通过/1失败，同名新增回归 `assistant::tests::request_options_real_settings_apply_uses_same_proxy_for_discovery_test_and_ask` 失败。代理夹具accept在6秒期限之后返回WouldBlock：macOS errno35、Linux errno11；随后macOS断言请求仍未完成失败，Linux发送回复gate时channel已关闭。源码在创建夹具后即启动该接收预算，接着才构建窗口、滚动和逐项输入；这是需单独验证的测试生命周期边界。原日志没有记录step/点击时间，不把源码顺序或后续受控反例写成原CI具体根因，也不认定为生产传输缺陷。

Windows的`request_options_http` harness12通过/1失败，新增 `explicit_proxy_ignores_environment_exclusions_in_isolated_process` 在检查child状态时失败。原测试对child的stdout/stderr均重定向null，原CI只有“isolated proxy fixture failed”，没有child具体原因。直接运行的`explicit_proxy_environment_child`在没有专用环境标记时会早返回；普通列表里的ok不能作为隔离代理行为通过。

新候选只在独立managed工作区准备测试夹具时序和有界child诊断，保留全部网络、头/auth、原站零请求、Apply/Ask/脱敏、人工审阅及原限时断言；生产、依赖、锁和工具链不变。尚未修复验收、独立复审或主树整合，不把更可观测的失败当成功。

根保存了233675字节失败steps原输出，SHA-256 `c6cf42c1022d9ff42342b466f14a1a5e9f026ac55e45b057058d8314b005f555`，仅用于核对该输出中的计数和错误。非作者取得173266字节完整原日志ZIP，SHA-256 `ad071a4aca17c983b85c8a1bab3af745aa2f285068e40f43025afbcaf58867a7`，独立三个job原日志与ZIP字节精确相等；Windows首两次单job BlobNotFound保持，后续实际取得完整日志。根核验ZIP全部32个成员和CRC，不将失败steps输出冒充完整原ZIP。

独立包位于ignored `work/ai-request-options-ci-independent-20261005`，277 proof/278归档成员，结论 `EXACT_C7_THREE_PLATFORM_CI_FAILURE_PRESERVED`。manifest SHA-256 `1d7d4451a409f2f7ce42e22c68b5a4e7fe24607581329767cce1d96f4a6cec21`；归档1286331字节，SHA-256 `be317e9750053dbb130d32ddc5f6af0bad5e3e32c6a6be843ab5fbf1ba8787e9`。根逐bytes/hash与全部tar成员读回通过。复核者先前将默认CLI也归入未到达的范围判断，以及解析脚本首次错误，均保留并明确纠正；最终只根据原日志的实际marker确认默认模式，不扩大至两模式。

这次源码CI失败不改写已冻结的macOS受控原生[API事实](2026-10-05-ai-request-options-native.md)与[文件六组合事实](2026-10-05-mcp-file-review-native-matrix.md)，也不扩大它们为供应商、最小窗口、其他平台原生或Release验收。MCP仍仅向外部智能体提供KeelShell能力。
