# 本地智能体进程 Future 的小栈回归

日期：2026-10-04。范围：`keelshell-ai` 本地 CLI Ask/probe 的读流 Future 内存布局和自托管进程控制器。起点提交为 `3fe0c98e6916e4981f22e52ca38128af3c948179`。

## 原始 Windows 失败

起点的 [Quality 37208098775](https://github.com/cyruss648/keelshell/actions/runs/37208098775) 已结束：macOS、Ubuntu 成功；[Windows job 111453446256](https://github.com/cyruss648/keelshell/actions/runs/37208098775/job/111453446256) 在运行自托管 `local_agent_process` 测试时，主线程栈溢出并以 `0xc00000fd / STATUS_STACK_OVERFLOW` 退出。打包步骤通过；这不是 Windows 打包失败或编译失败。

原始完整 job 日志保存在 ignored `work/local-agent-windows-ci/run-37208098775-job-111453446256.log`，94,065 字节，SHA-256 为 `01f27ebf9dfe95f3a16a4cc6c262200d8b2b2133eeca9868392770390834c43b`。复制时逐字节核对，原根仓库的日志继续保留。

自托管测试设置 `harness = false`，因此 `--test-threads=4` 不会将其控制器放到 libtest 工作线程。原控制器直接在原生进程主线程构造并 `block_on(integration_cases())`。stdout 与 stderr 的两个 8 KiB 数组跨越 `await`，被包含在读流以及外层 join/select、probe、Ask 状态机中。

## 修复与尺寸回归

两处数组改为直接 `vec![0_u8; 8192]` 堆分配，读取仍使用同样大小的可变切片。产品输出预算、协议解析、凭据隔离、审批、取消、清理与所有超时保持原来的行为。

新增尺寸测试通过类型推断测量未轮询的 Future，不启动 CLI、不使用凭据：叶子读流限制为 4 KiB，公开 probe/Ask 限制为 16 KiB。完整控制器也检查 16 KiB 上限。这些界限为平台布局差异保留余量，并阻止重新内联完整读流缓冲。

本机 macOS 调试构建实测如下；数值不是跨平台固定 ABI：

| Future | 修复前 | 修复后 |
| --- | ---: | ---: |
| stdout reader | 8,496 B | 328 B |
| stderr reader | 8,264 B | 96 B |
| probe | 19,496 B | 3,160 B |
| Ask | 20,360 B | 4,024 B |
| 完整集成控制器 | 20,960 B | 4,624 B |

`future-before.log` 中新增的 stdout 尺寸断言按预期失败；修复后的同一测试通过。没有通过提高原生进程主线程栈、设置 `RUST_MIN_STACK`、关闭 Windows 测试或放宽产品界限来绕过失败。

## 完整控制器与夹具竞态

自托管测试新增显式 `--controller-small-stack` 模式：在 2 MiB 线程中构造并运行同一个 `integration_cases()`，使用原有两个 Tokio worker，并增加 45 秒的完整控制器 deadline。常规测试入口继续在原生主线程运行，原有全部场景保留。`scripts/check.py` 在普通 workspace 测试后额外执行该模式，使后续三平台 CI 持续覆盖显式小栈路径。

第一次修复前的 2 MiB 运行发现夹具端口文件读取竞态：子进程刚创建或截断 `descendant-port`、尚未写入内容时，控制器读到空字符串并在 `parse().unwrap()` 中失败。原失败日志 `controller-before-2mib.log` 保留。读取改为对尚未完成的可解析端口继续轮询，仍使用原来三秒的就绪截止；没有放宽等待期限。

仅修复夹具读取之后，原缓冲代码在本机 2 MiB 栈上已经通过完整控制器，见 `controller-before-2mib-port-read-fix.log`。因此本机没有重现 Windows 的栈溢出；修复前的本机通过不能替代 Windows 原生失败证据。

缓冲修复后的常规主线程与显式 2 MiB 控制器均通过：两个 CLI 类型、完整中文回答、异常/截断/迟到/工具事件拒绝、stderr/行预算、凭据回显脱敏、审批后取消、请求超时、成功 leader 退出后的子进程收尾、继承 pipe 的子进程、Future abort、秘密上下文在 spawn 前拒绝、未来版本和不安全 feature 拒绝。测试只调用自身复制的原生进程夹具；不调用供应商 CLI、云端账户或客户 SSH。

## 本地证据与当前门禁

以下 ignored 日志保留在 `work/local-agent-windows-ci/`：

- `windows-ci-failure.sha256` 与原始 Windows job 日志。
- `future-before.log`、`future-after.log`：尺寸测量及预期失败/成功回归。
- `controller-before-2mib.log`：夹具端口空读失败。
- `controller-before-2mib-port-read-fix.log`：缓冲修复前本机 2 MiB 完整场景通过。
- `controller-after-default.log`、`controller-after-2mib.log`：修复后两种控制器路径通过。
- `packaging.log`：47 项既有打包回归通过。
- `whole-gate.log`：依赖策略、六项脚本回归、格式、全 workspace/all-targets 严格 Clippy、930 项普通测试、8 项文档测试，以及追加的完整 2 MiB 控制器回归全部通过。
- `gate-summary.json`、`fixture-cleanup.json`、`evidence-sha256.json`：门禁计数、零存活所属夹具进程/空私有临时目录及逐文件摘要。
- `independent-review.json`：新的独立代理只读复审结论与冻结源码摘要。

新的独立代理完成冻结源码静态复审并核对作者日志，未发现可复现 P1/P2；该代理没有修改源码或启动 Cargo，也没有执行 Windows 原生进程。复审确认产品分块与预算保持不变、默认 main/worker 栈未扩大，显式小栈门禁追加在普通 workspace 执行之后。完整本机门禁由作者运行；独立源码结论不被表述为独立 Windows 功能验收。

## 尚未验证的边界

修复提交`be9590f2ee124599a2aa13f6a654c5b9b5adf84b`已推送并逐项核对远端ref。[Quality37210745161](https://github.com/cyruss648/keelshell/actions/runs/37210745161)的[Windows job111461250990](https://github.com/cyruss648/keelshell/actions/runs/37210745161/job/111461250990)成功：914普通+8doc、6脚本、47打包（其中一项Unix权限跳过），默认主线程和追加2 MiB完整控制器均通过，没有再次栈溢出。日志保存在根仓库ignored `work/local-agent-windows-ci-review/run-37210745161-job-111461250990.log`，SHA256 `755b486b71e90655a95b7c6a58d6b8580eaf2ebb1b996c1cbf191916389a2487`。这关闭了该冻结提交的Windows自托管栈回归，不代表Windows供应商CLI或原生桌面。

同次macOS job成功；Linux job在既有MCP `malformed_shapes_and_unsupported_actions_have_bounded_protocol_errors`读取响应3秒超时，因此整个Quality结论仍为失败。原Linux日志保留在根仓库ignored `work/local-agent-windows-ci-review/run-37210745161-job-111461250972.log`，SHA256 `a563e0dc16cfb0018e5031ea77efcff11dd776ff97e780eeb8f52600eff1602d`。当前根MCP源码的4并发40次同一测试复查也有一次3.012秒超时，不能将它归为已证明的CI资源偶发；机制继续诊断，不放宽原期限。

根工作区与MCP桥接合并后门禁通过964普通+8doc+6脚本及追加2 MiB控制器，日志`work/mcp-windows-merged-gate-20261004.log`。仍未验证Windows/Linux供应商CLI、原生桌面、账户或模型服务。
