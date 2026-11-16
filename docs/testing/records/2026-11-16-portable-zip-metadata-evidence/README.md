# ZIP 私有权限元数据修订原件

三个工程文件的 719 项冻结为
`bf46afc32603f312f3ff9e7edb5e48ea9bb4c3073b109c072e2f6ded44a8f4fd`。
`originals.tar.gz` 保存 32 成员、485,703 字节原文；归档 121,622 字节，
SHA-256 `09d557db11c14309f8ff06f9e9c167e08d805564abe776c6b3e2816722984fa3`。

原件包括三个文件的初稿／最终快照、diff、输入冻结、fmt、首轮 41 项绿例、
两项受控 DOS 预期红例及精确恢复、最终专项门禁、346 名库存／结果、
上游锁定包摘要、旧 CI 原件验证、新非作者原静态报告与独立预构建 41 项结果。
稳定测试程序仅保存摘要和来源绑定，不将二进制或 Cargo target 加入归档。

最终专项 `0/reaped/51.622s`：342 passed、4 ignored，全部 346 名逐项相等。
非作者限定 APPROVE、P0–P3 全 0，独立预构建专项 `41/0/0`；
独立编译／Windows native／桌面 GUI 均 0。旧 f094 CI failure 和本批
封存时 Windows NOT_RUN 保持，不由上述宿主通过覆盖。

验证当前输入：
`python3 docs/testing/records/2026-11-16-portable-zip-metadata-evidence/verify.py`。
验证精确提交时增加 `--source` 和完整 40 位提交 SHA。
实际 current 验证 `0/reaped/0.164s`，32 成员、719 输入零 mismatch，测试重放 0。
逐成员原始路径、大小和 SHA-256 见 `manifest.json`；失败／诊断原文不删改。

源码交付为 `01f6f6bd8272303fe049bc7b52976afefdb44c37`，三方读回一致、干净。
精确 Git 绑定 `0/reaped/6.950s`，32 成员／719 输入零 mismatch。
[delivery](delivery/manifest.json)保存源码 push、SHA 读回、提交绑定、清理与新 CI
首轮快照；它独立于上述 32 成员封存，不改写提交前 Windows NOT_RUN。
