# 参数化片段与 SSH 批量命令验收 — 2026-10-03

状态：功能已接入。本轮本地整仓门禁、6 项独立 OpenSSH 互通和打包回归已通过；新增提交的三平台 Quality 仍需等待 GitHub 运行结果。macOS 原生窗口流程尚未以本轮构建完成验收。

基线：`8cd89fa132f7695400ca50b3297f533d5efcb8ee`；本轮变更尚未以最终提交固定。环境为本机 macOS / Apple Silicon，工具链采用仓库锁定配置。设计对应 [ADR 0016](../../adr/0016-parameterized-snippets-and-batch-exec.md)。原始通过和失败日志保存在 ignored `work/`，不会将临时密钥、个人配置或现场输出纳入仓库。

## 验收范围与事实边界

- 模板为显式开关；旧片段及旧文件中的双花括号保持字面原文。纯领域编译不执行 shell，也不持久化填写值。
- 支持完整词、赋值值和长选项值的 POSIX 字面引用；32 个名称、单值 4 KiB、源/结果 64 KiB。空值必须明确选择，预览不可编辑，填写后仍需单独审核执行。
- 参数弹窗最终插入绑定完整来源、目标 EntityId、原命令与 revision；模板命令默认不记会话历史。批量 exec 与交互终端历史独立。
- 批量只使用显式选择的已有认证连接，1–32 个目标、1–8 并发、每目标 1–300 秒；独立 exec、无 PTY、不重试、不重连。界面默认 2 并发、30 秒、失败后停止等待项。
- 逐目标 stdout/stderr 合计最多 1 MiB，界面各预览前 64 KiB。退出码、明确拒绝、未开始和结果未知分别显示；取消不能证明远端进程终止。

## 已完成的自动验证

| 检查 | 已确认结果 | 证据 |
|---|---|---|
| 模板领域及存储专项 | 20 项通过，包含旧 JSON/真实旧文件兼容、缺失/多余键、边界和失败不修改源 | `work/snippet-template-tests-3.log` |
| 独立真实 `/bin/sh` 引用 | 4 项通过，覆盖引号、Unicode/换行、重复值和命令语义边界 | 同上日志中的 `snippet_template_shell` |
| Core 阶段性全量与静态检查 | 专项 20 项、整仓 core 当前 42 项普通测试与 2 项文档测试通过；严格 Clippy 与格式通过 | `work/snippet-template-tests-3.log`、`work/snippet-template-core-clippy-2.log`、`work/snippet-template-core-fmt.log` |
| 批量真实 TCP SSH 协议专项 | 14 项通过，包括部分失败、停止策略、取消、迟到 OPEN、部分输出、输出/并发限额、32 行不消费事件及 2 MiB 小栈 | `work/batch-protocol-2.log` |
| 片段编辑器真实 GPUI | 9 项通过，包含 3 项新增变量模式回归 | `work/snippet-parameter-editor-tests-2.log` |
| 参数填写真实 GPUI | 6 项通过 | `work/snippet-parameters-ui-tests-5.log` |
| 工作区参数/批量真实 GPUI + SSH 专项 | 7 项通过，覆盖审核失效、关闭会话取消、停止/继续策略、输出隔离、历史保护和双语窗口几何 | `work/command-workflows-gate-4.log` |
| 组件格式与 whitespace 检查 | 整仓 rustfmt、严格 Clippy、依赖策略和 `git diff --check` 通过 | `work/command-workflows-gate-4.log` |

核心和组件测试重叠于后续全仓测试，不应把这些分组计数相加为最终测试总数。普通测试中默认 ignored 的 OpenSSH 用例，也不能计作已执行通过。

参数组件通过真实点击和输入验证：中文、单引号、命令替换外观、多行值、重复变量、精确只读预览、初始空缺与显式空值、确认前重读当前字段、保存中冻结、目标审核拒绝后保留值、Enter 只编辑、取消不发送命令、超限诊断不回显值，以及无变量的显式模板。小窗口测试调用 `simulate_window_resize` 并断言实际 viewport，覆盖 820×640、480×420 中英文；32 个参数可实际滚动到末项并点击编辑，固定按钮保持在窗口内。编辑器另外覆盖 900×560、480×420 实际视口和最多 32 项反馈。

工作区专项通过两个独立认证的回环 SSH 测试服务观察 exec 和终端字节。模板插入只改变命令草稿；单独点击执行后才写原目标，展开值不进入默认会话历史。来源、命令或目标变化拒绝旧审核。批量审核后才发精确命令，输出归属原目标，终端 PTY 和历史不接收批量正文。该受控数据路径与真实 OpenSSH、原生窗口操作是不同证据层。

## 开发中失败与修复

1. 模板编译首轮在变量前后反斜杠换行的词边界上误判。现按 POSIX continuation 判断边界而保留原文字节，补齐赋值/长选项跨行回归。原失败：`work/snippet-template-domain-tests-1.log`。
2. 参数预览最初只对 state 设置只读，实际 Textarea 渲染默认值覆盖该状态；真实点击、键入及 Backspace 测试发现预览被改写。现视图元素明确设置 `.readonly(true)`，最终精确字节不变断言通过。原失败：`work/snippet-parameters-ui-tests-4.log`；修复通过：`work/snippet-parameters-ui-tests-5.log`。
3. 初始组件测试的宏导入和不存在的只读查询接口导致编译失败，已改为明确导入和真实输入行为断言；共享集成文件未落盘期间的失败也保留在 `work/snippet-parameters-ui-tests-1.log` 至 `-3.log`。
4. 批量后端首轮严格 Clippy 发现测试索引循环风格问题，已修复；`work/batch-clippy-1.log` 保留。app 初轮 Clippy 的赋值空格、未使用导入和测试 `expect` 问题记录在 `work/snippet-parameter-app-clippy-1.log`；最终修复验证待下表补充。

模板、批量后端和 app 接线已由不同实现者交叉只读复审；当前没有报告剩余阻断。该结论不替代最终门禁和目标平台验证。

## 最终集成结果

| 项目 | 已确认结果 | 证据 |
|---|---|---|
| `python3 scripts/check.py` | 依赖策略、格式、严格 Clippy、702 项普通测试和文档测试通过；OpenSSH 用例在普通门禁中保持 6 项 ignored | `work/command-workflows-gate-4.log` |
| 打包回归 | 47 项通过 | `work/command-workflows-packaging-2.log` |
| 独立 OpenSSH | 6 项显式执行通过；OpenSSH 配置、临时密钥、服务器和测试进程均已清理，临时目录已删除 | `work/command-workflows-openssh-final/result.json` |
| 公开内容检查 | 306 个公开文件、75 个 Markdown、203 个本地引用；敏感名称命中 0，链接错误 0 | `work/command-workflows-public-audit.json` |
| macOS 最终构建原生流程 | 本轮功能构建尚未完成完整原生窗口验收 | 不将 GPUI 测试或旧包哈希当作本轮原生证明 |
| 新增提交三平台 Quality | `399699e` 对应 Quality `37110717468` 成功：macOS/Ubuntu 各 702 项普通测试 + 2 项文档测试，独立 6 项 OpenSSH；Windows 687 项普通测试 + 2 项文档测试，打包 47 项（1 项 Unix 权限检查按平台跳过） | [GitHub Actions 37110717468](https://github.com/cyruss648/keelshell/actions/runs/37110717468) |

## 证明边界与剩余能力

真实 `/bin/sh` 测试证明所覆盖参数的 POSIX 引用，不证明非 POSIX shell、所有应用参数语义或任意脚本安全。`eval`、`sh -c` 和选项处理仍由所执行命令决定。参数预览是明文审核，不是凭据库。

超时从每行接纳后计算，取消可能发生在 exec 已送达之后。尚未确认 OPEN 可等待本行剩余预算，最长 300 秒；异常关闭可能按既有通道契约断开共享 SSH。不论 UI 显示“停止”还是收到本地清理回执，都不能宣称远端进程已被杀死或操作已回滚。

本轮没有持久化任务审计、自动恢复、定时运行、跨主机事务、逐目标参数映射或完整运行手册编排。批量结果仅在当前工作区保留。Windows/Linux 桌面与原生 IME 候选窗尚未据这些 GPUI 测试验收；生产主机、商业 AI 服务不属于本轮夹具范围。
