# 工具版本与流式自测证据

状态：`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_APPROVED / NATIVE_DESKTOP_NOT_RUN`。
实际范围和失败见[集中记录](../2026-11-15-archive-tool-validation.md)，设计见
[ADR 0139](../../../adr/0139-reviewed-archive-tool-validation.md)。

本目录无损保留 **156 份原件**；[完整清单](archive-manifest.json)映射原工作路径、
持久 artifact、字节数、原文和存储摘要。大于 64 KiB 的原件保存为 `.gz`，
其它逐字保存；`raw/` 使用局部 Git 属性保留 CRLF、空白及原始报告。
不修改历史失败、HOLD、notice 或报告签署时仍为 PENDING 的门禁状态。
原始 `work/archive-validation-batch-20261115/` 路径是运行位置；
临时副本回收后按清单读取本目录。

## 接续入口

| 证据 | 位置 |
| --- | --- |
| 最终 607 项冻结 | [frozen-inputs-r2.json.gz](raw/frozen-inputs-r2.json.gz) |
| 门禁后相同路径/内容快照 | [post-gate-inputs-r2.json.gz](raw/post-gate-inputs-r2.json.gz) |
| 最终完整门禁退出与时间 | [full-gate-r2.json](raw/full-gate-r2.json) |
| 最终门禁原日志 | [full-gate-r2.log.gz](raw/full-gate-r2.log.gz) |
| 普通 Rust/rustdoc/Python 分类与 29 个新名称 | [gate-summary-r2.json](raw/gate-summary-r2.json) |
| 原报告的工程条件后续满足 | [engineering-conditions-satisfied-r3.json](raw/engineering-conditions-satisfied-r3.json) |
| 真实 OpenSSH 16/16 与自有进程/目录回收 | [result.json](raw/openssh-r2/result.json) |
| Linux helper 输入、实际结果 | [input-r2](raw/linux-helper-input-r2.json) / [assertions-r2](raw/linux-helper-assertions-r2.json) |
| Linux Python 25/25 原日志 | [linux-python-r2.log](raw/linux-python-r2.log) |
| 精确 image 与当前 VM 平台补充观察 | [image](raw/linux-image-platform-r3.json) / [VM](raw/podman-vm-platform-r3.json) |
| 正常 arm64 GUI/MCP 摘要与边界 | [normal-artifacts-r2.json](raw/normal-artifacts-r2.json) |
| 后端 r2 独立报告 | [report-r2.md](raw/review-session/report-r2.md) |
| 后端风险执行、输入/产物/日志核验 | [review-closure-r2.json](raw/review-session/review-closure-r2.json) |
| app r2 独立报告 | [report-r2.md](raw/review-app/report-r2.md) |
| app 七项独立执行与零漂移 | [execution-summary-r2.json](raw/review-app/execution-summary-r2.json) |
| 二进制来源、完整库存绑定 | [app](raw/app-binary-binding-r2.json) / [session](raw/session-binary-binding-r2.json) |
| 两项原 P2 | [notice-1](raw/review-session/notice-1.md) / [notice-2](raw/review-session/notice-2.md) |
| 原红色探针与修复后未修改探针 | [r1](raw/review-session/python-descendant-eof-r1.log) / [r2](raw/review-session/python-descendant-eof-r2.log) |
| 中止旧门禁及实际停止回执 | [full-gate-r1.json](raw/full-gate-r1.json) / [stop](raw/full-gate-r1-stop.json) |
| 原 reviewer 信号 fixture 失败与更正 | [adjustment](raw/review-session/signal-fixture-adjustment-r2.json) |
| 精确容器 ID 及 absent、作者 worktree 回收 | [resources-before-export-r3.json](raw/resources-before-export-r3.json) |
| agent 释放读回 | [agents-release-readback-r3.json](raw/agents-release-readback-r3.json) |
| 在线依赖新鲜度检查超时 | [dependency-latest-r1-exec.json](raw/dependency-latest-r1-exec.json) |

两个 reviewer 的 Rust 测试使用已核对来源、摘要、helper 字节及完整库存的父预构建
副本，没有独立重编译。后端实际执行 11 unit、11 SSH、25 Python、11 独立风险
和 1 原探针，app 实际执行 7；重复桥接与全仓结果不重复累计。
app 1246、session unit 205 和 SSH 11 是完整描述符库存，不是全部独立执行数量。
门禁最终普通 Rust 2987/0/36 ignored、rustdoc 14/0，工程 Python 187 run/1 skip；
helper Python 25/25 另列。真实 OpenSSH、正常构建、Linux helper 和原生桌面分开验收。

## Git 绑定与清理

可使用 [verify_binding.py](verify_binding.py) 核对 Git index 或一个完整提交 SHA，
输出路径必须尚不存在。校验包括 607 项工程输入的完整路径集合、内容、
156 份实际 Git blob、压缩/解压摘要和当前文件。冻结集合 SHA-256：

```text
523506564206cbeca415ae2bc9a82ed670cdb210705ba49f3173a84250bd4c17
```

三个稳定测试副本共 `321,900,736` 字节，没有作为源码或原件提交；
逐项摘要和排除理由见 [omitted-artifacts.json](omitted-artifacts.json)。
来源、完整库存、执行日志和回执均保留。全部 607 项工程输入及 156 份原件已从
Git index/blob 读回一致，见 [index-input-binding.json](index-input-binding.json)。
随后重新核对临时目录与每份原文和稳定副本，已回收本批 159 个文件、
`325,398,545` 字节，见 [cleanup-final.json](cleanup-final.json)；
唯一有效 Cargo target、缓存 image、Podman VM 和其他任务资源保留。
自有四个容器的完整 ID 从事件原件取回，清理前再次逐一确认 absent。
Podman 的 `--rm` 同时移除了 cidfile，不以缺少 cidfile 推断容器已不存在。

index、精确提交和最终清理回执放在本目录根部；不覆盖 `raw/` 的原件。
本地源码提交 `df363691fa4ee238bdd1db826f83c916c7d18194` 已逐项核对 607 个
工程输入、156 份原件和当前文件，零 mismatch，见
[commit-input-binding.json](commit-input-binding.json)。后续仅补该绑定与交接文档，
没有改变工程输入或复用历史门禁签署新源码。
本批没有新建用户 chat，已释放的四个 agent 在当前 registry 读回为 not_found；
这不声称永久删除了历史会话。只本地提交，不 push。

本批三平台原生桌面执行均为 0；IME/辅助技术、完整活动传输/草稿动态组合、
direct PTY strict OO `HOLD/PARTIAL`、用户中继、性能收益、FILE-05/M4、
完整产品和发行安装仍分别开放。
