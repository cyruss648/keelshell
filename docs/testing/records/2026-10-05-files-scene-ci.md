# 文件场景同步与 MCP EOF 的三平台 CI — 2026-10-05

状态：精确提交 `23310ee13286adb488a451277addf49b05cd476b` 的 [Quality 37257829818](https://github.com/cyruss648/keelshell/actions/runs/37257829818)，首次运行已完成 **success**。新的非作者只读审查结论为 `EXACT_CI_SUCCESS_VERIFIED`，根任务另外核对 GitHub API、三个原始 job 日志及六份精确源码，并逐项验证和复制 160 份冻结证明。

本记录关闭该提交的源码 CI 待确认项。此前 `a19d0c1` 的文件布局失败及 `40d092c` 的 EOF 失败保留原状态和证据，不将新通过结果追溯为旧运行通过，也不补写旧日志未观察到的精确原因。

## 实际执行结果

| 平台与 job | 普通测试 | 文档测试 | 脚本测试 | 打包测试 | 系统 OpenSSH |
| --- | --- | --- | --- | --- | --- |
| macOS 26 / `111598585159` | 1060 通过、11 ignored；应用 380 项包含其中 | 8 通过 | 6 通过 | 57 通过 | 9 项实际通过，清理回执通过 |
| Ubuntu 24.04 / `111598585202` | 1060 通过、12 ignored；应用 380 项包含其中 | 8 通过 | 6 通过 | 57 通过 | 9 项实际通过，清理回执通过 |
| Windows 2025 / `111598585023` | 1040 通过、11 ignored；应用 375 项包含其中 | 8 通过 | 5 通过、1 跳过 | 53 通过、4 跳过 | 准备、执行和回执上传三个步骤均 skipped |

所有已执行目标为零失败。普通、文档、脚本和打包分别统计，不重复相加应用子集，不把 `Ran` 总数或 ignored/skipped 计为通过。Linux 额外 ignored 是既有手工真实 `/proc` 采集测试；其余 11 项为两项供应商 opt-in 和九项默认不执行的系统 OpenSSH。Unix 的九项互操作在后续专门步骤实际执行，不由普通测试中的 ignored 推导通过。

三平台格式、workspace/all-targets 严格 Clippy 与直接 registry 依赖 `x.y` 策略均通过。默认及显式小栈的完整本地 Ask 控制器都实际报告全部场景断言通过，future 大小为 macOS/Linux 4624 字节、Windows 4968 字节；控制器未调用供应商 CLI 或模型，不给这些自托管场景编造普通测试数量。

## 文件与 EOF 回归

三个实际 checkout SHA 与 API head 均为上述提交。此前失败的具名文件布局目标、新增的提前释放和取消清理目标，以及三个 startup EOF、具名 stdio/IPC 未读输出、EOF 和输出背压清理回归，均在三个 job 的原日志中实际为 `ok`。

CI 对成功测试的 stdout 进行了捕获，本次没有输出成功的逐场景 `FILES_LAYOUT_JSON` 或传输诊断行。176 个组合覆盖由该提交中测试源码的精确 SHA、循环组合和实际具名目标通过共同绑定；不声称 CI 独立观察了每个场景的像素或暂停字节数。详细 176 行及真实 Paused/Continue 字节检查属于[本机作者与独立复核的同步记录](2026-10-05-files-scene-readiness.md)。生产实现、原 5 秒传输和 12 秒界面等待未改变。

## 原始证据与复核

| 原始 job 日志 | 字节数 | SHA-256 |
| --- | --- | --- |
| macOS | 193913 | `4b2408617a205afc3e3a63c4e82061717acdab30a1587103a5633e58f1b8afd6` |
| Linux | 225159 | `f59d2969d154df84bc701da3ba95c50a6e6d541da81aff93b720b7a6921347ca` |
| Windows | 191557 | `01ed9f2c6ef2ecbb1262f148b5d0d64e29e809407b56d6c585cc1791813a6af6` |

独立证据保存在 ignored `work/ci-23310ee-independent-20261005/`，根逐 bytes/SHA/mode 核验后复制到 `work/ci-23310ee-root-verification-20261005/`。160 项清单 SHA 为 `862bec0ed3fb4225f624d42db009f60586847644c3e020fee18b6cb2f518d40a`，结构化报告 SHA 为 `16d4e28fd82390061b2e79633d487913ffba64118b6211ff52712e7d5e99cd38`。清单包括 Windows 独立冻结的 49 份材料、六份精确源码和 Unix 原 ZIP/清理回执，旧文件专项的另一套 160 项材料保持独立。

Unix 两个本次 OpenSSH artifact 分别绑定 run、head 与 GitHub digest，内含九项具名通过和清理收据。macOS ZIP 为 2761 字节，SHA `b3544d3245b584c0e84db7042d4770f4f78f8f3a2a9db33b007e5801f2724581`；Linux ZIP 为 2691 字节，SHA `538c132e80376a6335f0dde51a264bdce50ab040d4669637c1f2fde0b201368f`。回执记录 macOS 62、Linux 69 个累计观察身份，observed/owned stopped 为 true、ancestry_unverified 为空且临时目录已删除；这是累计 ancestry 与内核 birth 身份的观测清理，不保证从未观察到的脱离后代，也不是全 OS 资源普查。

独立读取均受 60 秒期限控制，最后远端读取在 03:17:46 UTC 结束；03:23:24 UTC 冻结前仅离线整理。错误位置的 CLI flag、过早离线解析等读取尝试保留，未把审查工具错误记成 CI 失败。根额外在 03:19:51 UTC 实际读取 GitHub run API，确认 completed/success 与精确 head；原 JSON 12196 字节，SHA `154e611401640e36d0c374c30fcd840d8f7e7db40f9d9581b15f02cf7309a14f`。

## 清理与验收边界

文件同步工作树在 79 份作者与 160 份独立证明重新核验复制后已可恢复归档；真实目录、Git worktree 注册及已合并的功能分支均已移除。独立审查的隔离 target 已删除，主目录共享 target、原始失败、源码补充探针和证明保留。该清理没有删除 Codex 客户端失败或原生开发包。

本次证明源码 CI、受控 GPUI/loopback SSH/SFTP、打包回归和 Unix 系统 OpenSSH 互操作。它没有新增 Windows/Linux 原生窗口、IME/读屏、供应商客户端、云模型、正式六目标 Release、签名/公证或安装更新验收。MCP 方向仍为 KeelShell 向外部智能体提供服务；[Codex 的业务调用失败](2026-10-05-codex-authorized-mcp-failure.md)仍开放。
