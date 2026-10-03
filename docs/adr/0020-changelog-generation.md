# ADR 0020：可审阅的变更日志生成

日期：2026-10-03。状态：接受。

KeelShell 将变更日志保存在仓库的 `CHANGELOG.md`，发布前可运行
`python scripts/changelog.py generate --version x.y.z` 从 Conventional Commits
生成新的版本段。脚本只读取本地 Git 历史，不联网、不修改标签；发布流水线在标签构建时检查对应版本段存在，避免发布产物与说明脱节。

首次公开版本保留手写的能力摘要，后续版本由脚本按 Added、Fixed、Improved、Changed、Documentation、Build 和 Testing 分组。生成结果仍需和代码一起审阅、提交；脚本不会自动推送或发布。
