# Windows 测试工装发布屏障原件

归档 `originals.tar.gz` 保存 25 成员、702,712 字节原文，压缩 159,544 字节，
SHA-256 `3b37819297c2c50d8ac3273c77bb3041a490edc83592e84981956ea01489a1f9`。
719 工程输入聚合 SHA-256：
`1a516c0280487e62157ad3ee470206ef91c12a08501f12214ef7348c688cfc3a`。

保存唯一测试文件的基线／候选、diff、冻结、718 项不变绑定、作者专项／完整 Python
discovery、fmt／严格 Clippy／版本政策、两项预期失败负例、原 Windows 实际失败与库存、
Boyle 独立原报告及含实际执行细节的 receipt。独立专项按 receipt 记账，
没有另存或声称存在独立原始完整 stdout。没有程序二进制或 Cargo target。

作者限定 gate 实际 `0/reaped/15.369s`，Python 293 run／2 skipped；
专项 72 run／2 skipped，两项负例分别精确退出 1，控制器退出 0。
Boyle 限定 APPROVE、P0–P3 全 0，独立专项 72 run／70 passed／2 skipped／exit 0，
unittest 6.134s、外层 6.203s，实际 waited。候选实际 Windows 执行仍为 0。

前一源码 `95813a7` 的 Windows filesystem 70、core archive 41、session 332 名
预检已实际通过，完整 gate 随后在 Python 身份记录读取失败，原件保持。
macOS／Ubuntu 在本封存时仍运行，最终日志与签署另存，不改写该时点。
原 958 的 Rust／controller 成绩只按原冻结保留，不移签为本片的全量重跑。

执行 `python3 docs/testing/records/2026-11-16-windows-fixture-publication-evidence/verify.py`
验证当前输入；追加 `--source` 和完整 40 位 SHA 验证精确提交。
实际 current 验证 `0/reaped/0.277s`，25 成员／719 输入零 mismatch，测试重放 0。
逐成员原始路径、大小与摘要见 `manifest.json`。
