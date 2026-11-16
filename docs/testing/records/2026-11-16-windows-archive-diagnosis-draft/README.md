# Windows 归档审核诊断候选：未集成、未验证

这是下一批的可恢复交接材料，**不是故障修复或测试通过记录**。
对象为源码 `80647ed`、Quality `43797131194 / attempt 1` 的八项共同审核失败。
根因仍 UNKNOWN：旧日志无法区分入口未准入、后台准备错误和过期审核。

`raw/report.md.gz` 为作者完整报告。`raw/archive-review-diagnostics.patch.gz`
只修改 app 归档测试文件，保留原审核断言及既有测试正文，增加固定分类、受限
错误数字和 fixture 状态诊断；有两项待执行的隐私/分类回归。
补丁 SHA-256 为
`ea240445e8c93d337d2a6b9e8bda75607433e6b19de861e72176ea97bc54f745`。
增加 233 行、删除 4 行；父下一批先评估诊断范围，再应用、编译、独立复核和
真实 Windows 运行。不能把原始错误文本、路径、审核正文或凭据写入诊断。

作者仅在独立临时副本修改；正式源码未变。Cargo/rustc/rustfmt、Rust 测试、
target、UI 与真实 Windows 执行均为 0。静态文本应用和反向还原检查保留在
`raw/static-checks.json.gz`，不代表编译成功。作者已关闭，无活动后台任务。

`archive-manifest.json` 记录 25 份原件、各自源字节和 gzip 摘要；包括从精确
基线导出的源码、候选、补丁、报告及作者清单。解压后按原相对路径恢复，
不要直接把候选当成已批准生产源。临时副本回收和 Git 绑定见 `cleanup.json`。
