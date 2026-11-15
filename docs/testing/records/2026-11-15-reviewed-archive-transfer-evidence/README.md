# 客户端归档传输批次证据

入口为[测试记录](../2026-11-15-reviewed-archive-transfer.md)。
最终源码绑定 `frozen-inputs-r3.json`，532 个 Rust/Cargo 输入和 10 个 gate 脚本。
作者最终门禁为 `logs/full-gate-r3.json`，逐 suite 计数及实际名称为
`validation-summary-r3.json`。源码提交后另记录 `commit-input-binding.json`；
它不是在源码提交正文中自引用。

`archive-manifest.json` 记录从本批工作目录复制的 51 份原件及其原始摘要。
`.log.gz` 解压后与原始日志字节相同；其他文件按原字节复制。
`sha256.json` 校验本目录当前所有文件（自身除外）。
`command-runner.py.txt` 仅保留执行器原文，不作为可直接运行的项目工具。

`independent-review-r1.md` 保留两项 P2 和 REQUEST_CHANGES。
`independent-review-r3.md` 保留修复闭环、预构建执行限制和报告时点的 gate HOLD。
父线程在随后完成的相同冻结 gate 回执上核验剩余工程条件，不改写原报告。
原报告中的绝对路径和工作目录链接属于当时执行环境；对应原件按同名路径
保存在本目录，运行目录清理后通过本目录查阅。

OpenSSH 回执只证明一次性本机协议互通。正常 Mach-O 构建不证明原生归档操作，
Windows 的静态 lint 修复也不证明 Windows 编译或运行。在线依赖检查超时、
Windows 解包缺口、原生矩阵及其余高级传输范围继续保留。
