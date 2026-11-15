# 编辑器键盘与原生文本证据

本目录保存本地批次的无损原件与 Git 绑定回执。产品与测试结论从
[批次记录](../2026-11-16-editor-keyboard-native.md)读取；这里不扩大其原生范围。

- `raw/final-inputs-r2.json.gz`：715 项工程输入，集合 SHA-256
  `a17d4a12b9187755a59af96f99afa5f0998498b0de1a980de0bb9c13f2916c3c`。
- `raw/final-gate-summary.json.gz`、`raw/full-gate-r2.*.gz`：最终完整门禁原件。
  原失败 `full-gate-r1`、两轮有效/无效红例及键盘编译与夹具失败分别保留。
- `raw/review-source/`：新非作者源码复核、11 项独立预构建执行与 P2 闭环；
  `raw/review-runner/`：Python 九行窄审及 23 项独立运行；
  `raw/review-native/`：限定原生原件复核。Rust 复核没有独立重新编译。
- `raw/candidate-frost-r2/`：两条明确接续的真实原生流程与完整字节/原始备份读回。
  `raw/native-notes.md.gz` 解释各失败和接续，不能把失败脚本改签为成功。
  `raw/candidate-graphite-r1/native-hold.json.gz` 保持 HOLD。
- `raw/native-cleanup.json.gz` 与各 `session-final.json.gz`：四会话、181 条
  birth 身份、12 个根进程、八次端口关闭和四个私有目录的清理。
- `raw/normal-artifacts.json.gz`：正常程序与暂存包完整文件摘要。
  派生包、编译后的 Swift helper、缓存与临时 fixture 未重复保存在 Git；
  `omitted-artifacts.json` 明确列出它们。

审核截图为真实原生窗口，未裁剪或修改。旧程序的
[UTF-16 误报](previews/baseline-utf16-review.png)、新程序的
[UTF-16 审核](previews/utf16-review-frost.png)及
[GBK 英文审核](previews/gbk-review-english.png)各有原始 AX 和 PNG 原件。
`previews.json` 将这些便捷副本逐字节绑定到相应 gzip 原件。

`archive-manifest.json` 记录每份原件的完整路径、源字节数/摘要及压缩摘要。
本批共 278 份原件、源字节数 `42,326,696`；15 份可重建派生文件以完整摘要
登记在 `omitted-artifacts.json`。
`verify_binding.py` 核对原件、预览、完整工程集合与 Git index 或精确提交的 blob；
提交/清理后回执单列，不能用后来成功覆盖旧失败。

例如可使用 `gzip -dc raw/native-notes.md.gz` 阅读原件；暂存、绑定与清理回执
是否完成，应以实际存在的 JSON 和批次记录为准。
