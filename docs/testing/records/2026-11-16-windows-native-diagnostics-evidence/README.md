# Windows 原生阶段诊断验证原件

完整工程结论见[本批记录](../2026-11-16-windows-native-diagnostics.md)。
`originals.tar.gz` 保存本批 41 份临时原件、前批 11 份交付回执及四份候选源码，
共 56 个文件、原字节 4,169,804。压缩后 637,500 字节，SHA-256 为
`1832515e4ed819e53bc65a8c7f5811a91ecb08e87e657cdc31690ef2cd1f563d`。
逐文件路径、长度和摘要见 `archive-manifest.json`；全部成员已逐字节回读。

包内 `batch/` 包含完整门禁、focused/cross 检查、在线依赖 UNKNOWN、
两个旧目录的清理、作者 r1/父 r2 差异、非作者报告及冻结。
`batch/temporary-*-log-r1.log` 是截止测试源码 `bc9ebd6` 的三平台原始 CI 日志。
`previous-delivery/` 保留前批 `64ce892` 的交付、推送和读回原件。
原报告保持，reviewer 后续门禁读回及在线抓取边界勘误在独立补充件中。

719 项完整非文档工程输入集合为
`774b46416b9c1ef424d89d7d3cb49799563d1c00237d1e52bcd4598677dd8f29`。
可在仓库根目录运行：

```sh
python3 -B docs/testing/records/2026-11-16-windows-native-diagnostics-evidence/verify_evidence.py
python3 -B docs/testing/records/2026-11-16-windows-native-diagnostics-evidence/verify_evidence.py --index
```

精确 Git 树检查用 `--source` 指定包含候选的完整提交 SHA，
`--evidence` 指定保存本目录的完整提交 SHA。验证器不解包到磁盘，
检查所有成员、父门禁原日志和退出/回收、reviewer SHA256SUMS，以及完整工程
输入的路径集合及字节，不只核对四个改动文件或测试数量。
该检查不重跑编译，不签署 Windows 原生执行或新桌面验收。
