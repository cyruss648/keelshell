# 有界 Zmodem 收尾：持久证据

本目录保存本批原始命令、成功和失败、冻结、逐名测试库存、源码连续性证明、
三份非作者复核及清理。最终状态为
`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_APPROVED / DIRECT_PTY_ACCEPTANCE_OPEN`。
完整叙述见[测试记录](../2026-11-15-bounded-zmodem-farewell.md)，设计见
[ADR 0137](../../../adr/0137-bounded-zmodem-farewell.md)。

## 最终输入与结果

- [最终 r5 冻结](frozen-inputs-r5.json.gz)：571 个工程输入及 12 个脚本，
  原文 SHA-256
  `d8d27989768ae1827697458ee52d366d407ca8543a9eebd41baf6d8f283a2b2e`。
- [最终门禁回执](logs/full-gate-r5.json)及[完整原始日志](logs/full-gate-r5.log.gz)：
  实际 wait/reap，exit 0，646.751 秒，无外层超时。
- [逐 suite 与新增用例绑定](final-validation-r5.json)：普通 Rust 2935 passed /
  35 ignored，rustdoc 14 passed，Python 148 run / 1 skipped；59 个新增默认
  Rust 名字逐项通过，七个新增 Python 方法纳入完整 discovery。
- [完整 app 库存](app-r6-inventory.txt.gz)：1238 项，实际 1236 passed / 2 ignored。
  [门禁后产物绑定](post-gate-artifact-binding-r5.json)核对五份测试产物与两份
  正常 GUI/MCP 产物的摘要未变；没有重跑库存或给旧二进制改签构建来源。
- [正常 arm64 构建绑定](native-build-artifacts-r4-r5-binding.json)区分构建与桌面
  交互。[父确认工程条件](review-conditions-satisfied-r5.json)将真实最终门禁与
  相同冻结绑定，没有改写报告签署时尚待门禁的原始条件。

## 非作者复核原件

- [core r5 连续性批准](review-core-r5/review-core-r5.md)及
  [最终短报告](review-core-r5/final-closeout.md)。
- [session、transport、bridge 与 UI 最终报告](review-session-ui-r5/review-session-ui-r5.md)
  及 [P2-003 / P2-004 关闭 notice](review-session-ui-r5/notice-P2-003-P2-004-closed-r5.md)。
- [Python 最终限定批准](review-python-r4/final-r5.md)。

三份范围内均为 APPROVE，开放 P0/P1/P2/P3 为零。Rust 独立执行使用核对完整
库存及 SHA 的父预构建副本，没有独立重编译。core 102、session 25、barrier 4、
app Zmodem 30 和 final bridge 15 的复用范围、交集与原始冻结在各报告中明确；
不能重复累加为新的唯一测试数。Python 最终独立 17 项与基线预期红测也分别保留。
所有旧 HOLD、notice、drift、失败和被中止门禁原件均在对应目录和 `logs/` 下。

## 实际 peer 与开放边界

`native-current-r3/` 保存 macOS 原版 peer 8/8，与 r5 的 core/session/root
输入相同。`linux-peer-r3/` 保存严格 6/8 和缺 OO 场景的同一 shell 继续输入：
`strict_OO=0 / missing_optional_OO=1 / same_shell_continued=true`。
`linux-peer-r4/` 在公开包安装连接失败时停止，没有进入协议。
所有旧严格断言保留，严格互操作仍为 `HOLD/PARTIAL`。管道对照、构建和
headless GPUI 均不代签三平台桌面、Linux 本地 Rust 文件后端或 Windows 安全发布。

`tail-probe-r2/`、观察器源码及日志保留受控调度下的最终 PTY flush 诊断。
它不证明所有历史缺 OO 的唯一原因。第三方 archive、编译目录和临时 peer
未作为应用依赖捆绑；下载身份、命令、原始摘要和重建脚本保留。

## 保存与清理

[无损清单](archive-manifest.json)覆盖 708 份原件；大于 64 KiB 的文件采用
确定性 gzip，解压后的字节数和 SHA 与原件逐项相同。小文件原样保留；
局部 `.gitattributes` 保留原件的换行和空白，`.gitignore` 显式纳入原始日志。
清单中的 `artifact` 是实际路径，
`source_sha256` 是原文摘要，`artifact_sha256` 是持久文件摘要。

[临时产物清单](omitted-artifacts.json)只列不持久保存的测试二进制、上游展开和
编译目录、动态库及 scratch 数据，保留其摘要与排除原因。全部原件与临时产物
在删除前再次核对，随后[最终清理](cleanup-final.json)删除 869 个本批工作文件，
回收 626,097,502 字节。此前[过期二进制清理](cleanup-superseded-binaries.json)
另回收 921,528,032 字节，合计 1,547,625,534 字节。

[agent 收口](agent-closeout-r5.json)记录三个 reviewer 已关闭；
[容器缺席核验](container-absence-r5.json)确认本批四轮容器全部不存在。
自有 SSH/peer 进程和临时密钥的原始清理回执继续保留。未创建新的用户 chat 或
实现 worktree；保留唯一有效 Cargo target、当前工作树和其他任务资源。
只本地提交，不 push。后续提交绑定属于文档回执，不改变已验证工程输入。

[提交绑定](commit-input-binding.json)将本地源码
`590cdfb654c41d615f01bc135375ffc02d525461` 与 `583` 个工程输入及 `708` 份原件
从 Git blob 和当前文件两侧逐项匹配，零 mismatch。本地提交命令及其原始输出
分别保存在 `local-feature-commit.json` 和 `local-feature-commit.stdout.log.gz`。
`post-export-index-check-r1/r2.json` 保留 EOF 空行属性与全局日志忽略规则的
原始检查失败；最终 `precommit-input-binding-r5.json` 核实全部原件已经纳入 Git。
