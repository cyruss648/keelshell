# 三平台工程门禁修复原件

结论从[批次记录](../2026-11-16-platform-gates.md)读取。本目录保存 221 份无损
原件，源字节 `7,651,429`。`archive-manifest.json` 逐项记录原始和 gzip 摘要；
例如可用 `gzip -dc raw/review-source/REPORT-r4.md.gz` 阅读非作者报告。

- `raw/engineering-inputs-r4.json.gz`：最终 717 项完整工程集合，SHA-256
  `04b232fb2095b490614d509d3545ea72afa65a998646300617df87f6af11180f`。
- `raw/full-gate-r1.*.gz`、`raw/final-gate-summary.json.gz`：
  `exit 0 / reaped / 771.347s`，Rust `3210/0/48 ignored`、rustdoc `14/0`、
  Python `290 run / 1 skipped`、archive helper `30/30`；fmt、严格 Clippy、
  x.y 与三组 AI controller 通过，1,331 项 app 库存及完整工程字节绑定通过。
- `raw/review-source/`：新的非作者源码与功能复核、独立专项和原始 notice。
  r4 有条件源码批准，开放 P0–P3 均为 0，两个历史 P2 在限定源码/回归内关闭。
  `raw/final-review-gate-binding.json.gz` 另行满足原报告待定的完整门禁条件，
  不改写原签署，也不代签真实平台。
- `raw/author-posix/`：六文件独立作者原件，精确 19 项宿主能力 skip，
  其余纯合同继续跨平台；模拟 Windows/FreeBSD 不是原生验收。
- `raw/review-macos-investigation/`：独立调查和原因纠正。旧 CI 第 166 行是
  post-reap OSError，原 errno 未知；不能写成已确认的 0.2 秒超时。
- `raw/archive-*-red-*.gz`、`raw/archive-cleanup-green-r2.*.gz`：
  原始时钟/EPERM 红例与中间真实夹具的 exit -9 失败；后续成功没有覆盖它们。
  最终 `raw/archive-cleanup-green-r3.*.gz` 为 `30/30`，
  `raw/archive-rust-risk-r1.*.gz` 为 `11/11`，内含 Python `30/30`。
- Windows、macOS 和 Linux 基线 CI 的 raw、ANSI 阅读副本、job 快照：
  精确 run `43789081172 / attempt 1 / head 4fd1932`。
  Linux success，Windows/macOS failure；本批新真实 CI 仍待推送复验。
  raw ANSI 不直接渲染终端，阅读副本没有替代原件。
- `raw/normal-binaries-before-cleanup.json.gz`：现有正常 GUI/MCP 摘要保持；
  本批没有重建正常包、进行原生 GUI 验收、安装或发行。
- `raw/agent-lifecycle.json.gz`、`raw/author-cleanup-r1.json.gz`：
  本批作者/调查/复核均关闭，作者受管树已回收。下一批焦点补丁已经复制，
  其作者也已关闭、树已归档，尚未集成，不在本批冻结内。

`verify_binding.py` 检查全部工程路径、字节、原件和归档元数据是否存在于
Git index 或精确提交的 blob；可用 `--source-commit` 额外核对源码与后续文档
树的工程 blob 完全相同。提交、推送、三方 SHA 读回和临时数据清理各自单列，
以实际 JSON 回执为准。原生反向 Tab、真实平台/IME/辅助技术和完整产品仍开放。

源码及无损原件提交
`80647edca4baa1a07532f328849f24d43819e864` 已推送，`source-push-r1.json`
实际三方 SHA 一致。`index-input-binding.json`、`commit-input-binding.json`
和 `post-cleanup-binding.json` 均零 mismatch；后续文档树应通过
`--commit <文档提交> --source-commit <源码提交>` 同时绑定两棵树。
verifier 同时核对本目录所有交付元数据的 Git blob，输出放在新的 ignored 路径
或尚不存在的指定文件，不覆盖旧回执。

`cleanup-final.json` 记录回收 221 个临时文件、逻辑 `7,651,429` 字节，
原批次目录 absent，正常程序、唯一有效 target 及其他任务资源保持。
`ci-initial-raw.json` 与 `ci-initial.json` 是新精确源码 Quality run
`43797131194 / attempt 1` 的初始快照及摘要；不代签终态。
后续 CI 原件仍在独立的活动工作目录，待实际终态后再归档。
