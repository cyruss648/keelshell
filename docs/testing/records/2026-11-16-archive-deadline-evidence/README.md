# 固定截止时间测试证据

结论见[本批记录](../2026-11-16-archive-deadline.md)。
本目录无损保存 63 份本批选定原件，原字节 `1,232,930`，
gzip 字节 `252,632`。`archive-manifest.json` 逐项绑定压缩及解压字节和摘要。
没有新原生截图、正常构建或包；唯一 target 和其他批次原件不复制到此目录。

- `raw/inputs-freeze-r1.json.gz`：718 项工程输入，集合 SHA-256
  `593459030133909451156982dbb949e53d872cccf6cfa30a18593b2411e66e21`。
- `raw/full-gate-r1.*.gz`、`raw/final-gate-summary-r3.json.gz`：
  实际 0/reaped/785.431s，Rust `3220/0/48 ignored`、rustdoc `14/0`、
  Python `290 run / 1 skip`；fmt、严格 Clippy、x.y 与三组 controller 通过。
  app 1,341 名、session 归档 40 名及完整工程输入逐项相等。
- `raw/deadline-candidate-r1.*.gz`、`raw/archive-target-r1.*.gz`：
  首次精确 `1/0/0` 及完整 target `40/0/0`。
- `raw/deadline-reset-*.gz`、`raw/mutation_red.py.gz`、
  `raw/deadline-restored-green-r1.*.gz`：有效变异实际编译后 `0/1/0`，
  目标阶段的两秒墙钟拒绝，生产字节精确恢复及重编译绿例 `1/0/0`。
- `raw/reviewer/`：新的非作者源码与文档 APPROVE，P0–P3 全 0，
  独立测试/UI 重放 0。原报告保留其签署时完整门禁待定条件；
  `raw/review-conditions-met-r1.json.gz` 另外绑定同一输入的最终成功回执。
  reviewer 已关闭；作者原件在 `raw/author/`。
- `raw/summary-derivation-r1/`、`raw/summary-derivation-r2/`：
  首次派生误取 core 同名 target，第二次检出 MCP 不再为旧字节。
  两次失败和脚本保留，未重跑已成功门禁。采用 r3 的精确包绑定和实际产物观察。
  GUI 仍匹配旧摘要，target 中 MCP 已变化，不称新正常包或原生验收。
- `raw/dependencies-live-*`、`raw/toolchain-r1.*.gz`：
  实时官方检查 113 声明/使用、37 包中 23 核实最新，14 元数据错误 UNKNOWN，
  整体退出 1；本机默认 stable Rust/Cargo 为 1.100.0。没有升级版本或 pin。
- `raw/document-checks-final-r2.json.gz`：当前六文档链接可达；
  旧 HANDOFF 的 2,039 行/146,588 字节与固定基线逐字节相等，
  当前入口 135 行。产品说明只同步此前已持久的限定原生焦点结果。

提交前执行 `verify_binding.py --index --check-current`；
提交后分别传入完整 `--source-commit` 和 `--evidence-commit` 验证两棵树，
必要时加 `--check-current`。它同时核对 gzip/原字节、完整工程路径集合和
源码 Git blob；本批没有预览，`previews.json` 明确为空。
以后源码变化时，旧冻结只用于旧精确提交，不放宽或移签。
提交、远端读回和清理回执在观察到结果后另行追加，不自称已包含在较早源码提交中。

源码及以上 63 原件已在 `bc9ebd6cf4f213b9330c04380f866ead9133fb0a` 提交推送到
`feature/archive-deadline-clock-20261116`，本地/tracking/远端相等；
`source-delivery-readback.json` 保留精确源码/证据与现源绑定零 mismatch。
`delivery-command-manifest.json` 和 `delivery-raw/` 另保存 20 份交付及旧 CI
查询原件，包含错误仓库 GET 的失败及正确 remote 的成功读取。
`source-ci-initial.json` 绑定新 Quality `43809777642 / attempt1`，
它不替代原分支旧诊断 run `43806549649`；当前原分支 push 延后以保留 Windows
诊断，macOS/Ubuntu 已 success。
`cleanup.json` 记录 83 文件/1,297,657 字节回收、两个旧目录 absent；
唯一 target、活动 Windows 调查和仍需使用的临时 CI 分支保留。
