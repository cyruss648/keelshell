# Windows 归档根目录发布证据

`originals.tar.gz` 保留本批三文件快照、719 项冻结、实际宿主专项、精确库存、
作者候选、父集成变化、独立评审和上一精确三平台 CI 原日志。
`manifest.json` 记录逐成员大小、SHA-256 和来源；不包含应用、target 或运行态。

使用本目录 `verify.py` 核对当前源码，或以 `--source` 指定完整提交 SHA。
该命令核对原件和源码身份，不重新执行测试，不代表 Windows 条件代码或桌面通过。
范围、失败和待验收项见[本批记录](../2026-11-16-windows-archive-root-publication.md)。
