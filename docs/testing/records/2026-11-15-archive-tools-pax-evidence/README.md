# 2026-11-15 归档工具观察与 PAX 兼容证据

结论：最终 r4 作者完整工程门禁退出 `0`，两个非作者源码复核均 APPROVE、
零开放 P0–P3；首轮 PAX P2-01 已通过实际红回归、完整原始 size 修复和独立
差分执行关闭。仅本批工程与源码复核范围，不代替原生或发行验收。
完整说明见[集中验证记录](../2026-11-15-archive-tools-pax-integration.md)。

## 最终入口

- [逐 suite 汇总](validation-summary-r4.json)：普通 Rust `2675/0/26 ignored`、
  rustdoc `14/0`、Python `131 run / 1 skipped`，含全部 74 个本批新增默认用例名。
- [最终冻结](frozen-inputs-r4.json)：538 个 Rust/Cargo 输入与 10 个门禁脚本；
  SHA256 `96bb5b3bded14516e3167d8281adee35901711cbca6ba9f41f91215afa179aef`。
- [full-gate-r4 回执](logs/full-gate-r4.json)与[日志](logs/full-gate-r4.log.gz)：
  实际 wait/reap、exit `0`、`647.902s`、未超时。
- [工具/UI 独立报告](review-tools/report-r1.md)及
  [PAX 最终差分报告](review-pax/report-r3.md)：独立执行已核验的父预构建二进制，
  没有独立重编译。原报告签署时的 gate 条件保留，由父最终成功回执满足。
- [真实 OpenSSH r3](openssh-r3/result.json)与
  [正常程序输入绑定](normal-artifacts-r3.json)：OpenSSH `14/14`、GUI/MCP 构建成功；
  r3/r4 的生产输入相同，最终测试冻结为 r4。

## 失败与修复

首轮 fixture/观察注册错误、PAX 拒绝顺序及三轮 gate 的不同终态均保留。
[原 PAX 阻塞报告](review-pax/report-r1.md)和
[立即通知](review-pax/notice-p2-01-r1.md)没有改写为通过。
[真实红回归](logs/pax-size-reproduction-r1.log.gz)为 `0 passed / 2 failed`，
显示旧实现错误返回 `Ok`；[原恶意夹具](review-pax/fixtures/pax-metadata-base256-overflow-r1.tar.gz)
与修复前源码、测试快照保留。快照以 `.rs.txt` 存放，不属于工程 Rust 输入。

`logs/full-gate-r1.*`、`logs/full-gate-r3.*` 的 Clippy 失败，及
`logs/full-gate-r2.*` 的旧冻结成功分别保留，不能代签最终 r4。PAX 最终差分报告
明确核对 r2→r3 两个 core 文件及 r3→r4 仅两处等价 NUL 转义。

## 保存与清理

[归档 manifest](archive-manifest.json)记录每份原文件及保存后文件的长度和 SHA256。
`.log.gz` 和 `.diff.gz` 是原日志与补丁的无损 gzip，原文与压缩字节的摘要分别
记录；其他原件逐字复制。补丁上下文的空行自带单个空格，初次 staged whitespace
检查将其报告为 7 处尾空格，因此改为压缩保存；没有修剪取证原文或修改工程源码。
该保存调整另见[回执](diff-archive-adjustment.json)。
原回执和报告中的 `work/archive-tools-pax-20261115/` 是执行时位置，在本目录按
相同相对路径保存；不为链接迁移改写原始取证内容。

命令包装器、汇总解析器及 reviewer 运行器以 `.py.txt` 归档，便于后续审计，
不会加入 Python 测试发现。`cleanup-before-archive.json` 保留阶段性 pending，
最终回收和提交绑定另存新回执。`sha256.json` 覆盖本目录除摘要文件自身之外的
完整持久文件清单；新增交付回执后重新生成该索引，不替换原始运行证据。

不保存工作 target 或测试临时目录。唯一有效 Cargo target 保留，两个实现
worktree 已受管归档，agent 已关闭；自有 OpenSSH 进程及目录已回收，本批未创建
容器。其他任务及受保护 worktree 保留。

服务端实际打包、版本/功能验证、Zmodem、中继、Windows 安全解包、三平台原生
交互、IME/完整辅助技术、签名安装与发行验收仍开放。在线依赖检查超时，没有
最新版本报告；不得据此声称所有依赖最新。
