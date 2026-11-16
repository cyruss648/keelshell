# Windows Zmodem 测试契约原件

`originals.tar.gz` 保存 93 个成员、6,422,868 字节原文；归档 1,173,225 字节，
SHA-256 `4a69f8a7772d25a617ff0826143746844b64016068e020e43aa2ede46b70e18a`。
719 工程输入聚合 SHA-256：
`96753dee615c9cd7711a48ad769ec5b14f49c45c581b0bb67323bb51ba508766`。

内容包括两个工程文件的快照和 diff、45 名映射及 43 个未变测试块检查、
集中完整门禁与 1343 名 app 库存、原 `01f6f6b` 三平台 CI 原日志、
旧 Windows 两项失败、原错误 r1 摘要与纠正后的 r2、解析修订说明、
原准备候选与新非作者 Turing 报告。前批 ZIP 活动原件一并保存，供核对后回收副本。
旧条件 HOLD 原报告继续保留在上一批
[CI 终态原件](../2026-11-16-windows-archive-root-publication-evidence/ci-completion/README.md)。

宿主完整门禁实际 `0/reaped/723.521s`：普通 Rust `3225/0/48 ignored`、
rustdoc `14/0/0`、app `1341/0/2 ignored`；app 库存与执行逐项相等。
Python 主 discovery 290 run／1 skipped，archive helper 单列 30 tests。
Turing 只签两文件静态代码／功能复核，APPROVE、P0–P3 全 0；
独立 Rust／Windows／GUI 执行 0。当前修订封存时 Windows 执行为 0，
旧源码 Windows 运行和宿主通过不代签新断言。

运行 `python3 docs/testing/records/2026-11-16-windows-zmodem-test-contract-evidence/verify.py`
可验证当前输入；增加 `--source` 与完整 40 位提交 SHA 可核对精确 Git 输入。
实际 current 验证 `0/reaped/0.297s`，93 成员、719 输入零 mismatch，测试重放 0。
逐成员大小、原始路径与摘要见 `manifest.json`。后续提交／CI 回执另存，不改写本次封存。
