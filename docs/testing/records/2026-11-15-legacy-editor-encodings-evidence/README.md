# 显式远程文本旧编码：持久证据

本批从 2026-11-15 开始，2026-11-16 收口；原始 UTC 不改写。
完整说明见[集中记录](../2026-11-15-legacy-editor-encodings.md)。
状态为 `AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_SCOPES_APPROVED`；仅本地提交，
没有 push，本批三平台 native 桌面为 `NOT_RUN`，完整产品目标保持进行中。

最终 r4 为 612 项工程输入，集合 SHA-256：
`e8029ecb7eea9a28e76d4c47f8d4d280afa57de4356049f2d83ab39020749ce6`。
最终 gate 为 `full-gate-r2`，实际 `exit 0 / reaped / 696.329s`：
普通 Rust `3077/0/37 ignored`、rustdoc `14/0`、工程 Python `187 run / 1 skipped`，
fmt、严格 Clippy、x.y 策略及三组 AI controller 通过。84 个新默认 Rust 名逐项
通过；6 个复用 fixture 不算新增产品行为。真实 OpenSSH r2 `17/17`，
正常 arm64 GUI/MCP 构建通过；这些均不代签原生桌面。

## 保存与定位

[archive-manifest.json](archive-manifest.json) 保存全部 **286** 份原件的原路径、
原字节数/SHA-256、导出路径、压缩字节数/SHA-256。共核对 `8,630,487` 原始字节，
所有 gzip 解压逐字节相同。`raw/` 使用局部 binary 属性保留行尾，不裁剪原日志、
响应或报告。原件中的旧绝对链接属于原始审查现场；按 manifest 的原路径查找
对应 `artifact`，必要时解压读取，不依赖已经删除的临时目录。

[omitted-artifacts.json](omitted-artifacts.json) 仅省略六个可重建父预构建测试副本，
共 `411,456,064` 字节，保留每份名称、摘要、来源、完整库存和执行原件。
真实 OpenSSH 的私有目录、凭据与主机密钥不在导出内容中。

| 原始路径键 | 用途 |
| --- | --- |
| `frozen-inputs-r4.json` | 完整工程输入及集合摘要 |
| `final-gate-summary-r1.json` | 最终门禁分类计数、84 个具体新名和来源检查 |
| `full-gate-r2.json` / `.log` | 成功完整门禁的真实退出和完整输出 |
| `final-review-inputs-r2.json` | 最终编译库存、实际 binary 和工程冻结 |
| `openssh-r2/result.json` / `tests.log` | 最终真实 daemon 联调与清理 |
| `openssh-final-readback-r1.json` | 实际运行 binary、17 个完整名称、私有目录与端口读回 |
| `normal-build-r1.json` / `normal-build-readback-r1.json` | 正常程序构建、Mach-O 与 SHA |
| `review-codec-transport/report-r2.md` | 原 codec/session 和依赖复核；父预构建独立执行，无独立重编译 |
| `review-app/report-r1.md` | 原 app/UI/OpenSSH 合同复核；去重 68 项父预构建独立执行 |
| `review-codec-transport/report-r3.md` | r3→r4 两文件归档差分批准、独立 Python 红/绿、610 项不变证据 |
| `final-review-resolution-r1.json` | 原签署、差分与最终整合的关系，reviewer 关闭读回 |
| `dependency-report-r4.json` / `registry-metadata-r3/` | 37 包官方原响应、112 声明重算；仍绑定原采集窗口 |
| `dependency-lock-retention-r1.json` / `resolver-intermediate-lock-r1.toml` | 无关依赖恢复及真实中间 resolver 状态 |
| `full-gate-r1.json` / `.log` | 原 `39/1` 归档失败，保持失败 |
| `archive-diagnostic-r1/` | 有界六轮诊断和纯数值 stat trace |
| `archive-anchor-red-r1.*` / `archive-anchor-green-r1.*` | 父确定性真实文件系统红/绿 |
| `resources-before-seal-r1.json` / `author-cleanup-r1.json` | 自有 worktree、agent、命令回收；没有新建用户 chat 或 Podman 容器 |

六个新增 Python 方法包含在 31 项 suite 内，修后 `30 passed / 1 skipped`。
它们嵌套在一个 Rust wrapper 中，不能相加为额外 Rust 测试；与 reviewer 重跑也不
累计成独立覆盖。原 r3 报告签署条件保持原字节，最终依据是原范围、610 项不变、
r4 窄差分和同一 r4 成功门禁的组合。

## Git 与清理

[verify_binding.py](verify_binding.py) 同时校验完整工程路径集合、当前文件、
冻结摘要、Git index 或精确 commit 的 blob，以及每份导出与解压后的原文字节。
`index-binding-r1.json` 是提交前读回；`commit-input-binding.json` 绑定本地源码提交。
其后文档回执提交只更新状态，不改变已验证工程输入。

删除临时批次目录前，必须先完成上述验证。最终清理从 `cleanup-final.json` 读取：
已移除本批重复目录和六个测试副本，共回收 `420,086,551` 字节；保留唯一有效
Cargo target、正常程序及其他任务资源。全部实现/复核 agent 已关闭，两个作者
worktree 与其本地分支 absent。
