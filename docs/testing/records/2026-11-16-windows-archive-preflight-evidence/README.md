# Windows 归档预检持久证据

`originals.tar.gz` 保存本批三文件快照、719 项工程冻结、命令与完整回执、
非作者材料，以及源码 `8c07d61009dba14f989dd0c00a2a370dc7a2d37b` 的三平台
完整 CI 日志和终态。`manifest.json` 记录每个成员的原始大小及 SHA-256。
归档不包含应用、target、运行态、密钥或无关浏览器 inventory。

执行 `python3 docs/testing/records/2026-11-16-windows-archive-preflight-evidence/verify.py`
核对当前工程输入，或添加 `--source` 和完整提交 SHA 核对已提交版本。
验证只核对证据和源码身份，不重放测试、不代表 Windows 原生通过。
测试事实与未验证边界见[本批记录](../2026-11-16-windows-archive-preflight.md)。
