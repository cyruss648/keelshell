# 文件列表键盘与原生证据

本目录保存本批无损原件和 Git 绑定回执。结论从
[批次记录](../2026-11-16-file-browser-keyboard.md)读取；
完整产品与多平台桌面不在本批限定批准范围。

- `raw/engineering-inputs-r2.json.gz`：717 项工程冻结，集合 SHA-256
  `d849a4fc427ef1b2abb114fabb88f5fa6c67a167b1e35d0021e7026e737e7166`。
- `raw/full-gate-r2.*.gz` 与 `raw/final-gate-summary.json.gz`：
  最终完整门禁实际成功、完整工程集合与 42 个新增默认名绑定。
  首轮编译/fixture/几何失败、Tab 红例及被主动停止的 `full-gate-r1` 均保留。
- `raw/review-source/`：新非作者源码复核、唯一 Tab P2 闭环及独立预构建执行；
  没有独立重编译。`raw/final-source-review-gate-binding.json.gz` 另行满足
  原签署时待定的最终工程条件，不改写原报告。
- `raw/review-native/`：另一个新非作者对实际原件的限定复核；
  结论与未核实边界以其报告为准。
- `raw/native-frost-r2/`：真实 macOS Frost 中文默认窗口的 AX/PNG、
  改名完整 28 字节、取消/重复键不写与删除后 absent 读回。
  `raw/native-notes.md.gz` 解释各失败与接续观察；反向原生 Tab 保持 HOLD，
  其他主题/平台、IME/VoiceOver 与完整焦点矩阵继续开放。
- `raw/normal-artifacts.json.gz`、两份 `raw/package-r*/package-manifest.json.gz`：
  正常程序、包内完整摘要与门禁后字节核对；候选包的 commit 为 null，
  精确源码由工程冻结/Git blob 另行绑定，没有安装或发行。
- 各 `raw/native-frost-r*/session-final.json.gz`、
  `raw/native-cleanup-final.json.gz` 与中断读回：两原生回合 147 条 birth 身份、
  六个根进程、四次端口与两个私有目录的清理。外层 PTY 已不存在不等于
  取得其退出码；该未知边界保留。
- `raw/offline-commits-push-readback-r1.json.gz`：此前 159 项离线提交已推到
  当前功能分支，本地/tracking/远端读回 `e709d02`。本批后续提交/推送回执单列。

便捷预览为未修改的实际原生截图：
[末行可见](previews/remote-list-last-row.png)、
[重命名审核](previews/rename-review.png)、
[删除审核](previews/delete-review.png)。
`previews.json` 将它们逐字节绑定到相应 gzip 原件。

`archive-manifest.json` 保存 491 份原件，源字节数 `93,201,142`，
逐项记录源与压缩摘要。`omitted-artifacts.json` 登记 14 份
可重建暂存包、编译 helper/测试副本和缓存，未把二进制副本重复提交到 Git。
`verify_binding.py` 核对所有原件、预览、717 项工程路径与字节，以及 Git index
或精确源码提交的 blob。暂存、源码提交、推送及清理是否完成，以各实际 JSON 回执为准。
例如可用 `gzip -dc raw/native-notes.md.gz` 阅读原件。

源码提交 `6d80c259b9a80facbf0f2fdeea986e18978c714f` 与归档提交
`e67266107e7493dc97235c97a759c1c6904b4c9a` 都已推送，分别读回本地、
tracking 和远端一致。`index-input-binding.json`、`commit-input-binding.json`
和 `post-cleanup-commit-binding.json` 均零 mismatch；后两份以
`--commit <归档提交> --source-commit <源码提交>` 同时绑定两棵 Git 树，
没有把后补原件误称为早先源码提交中的内容。

`archive-push-r1.json` 保存归档推送的实际退出、reap 和三方 SHA。
`cleanup-final.json` 记录归档推送与字节核验后回收 505 个临时文件、逻辑
`1,067,658,510` 字节；正常 GUI/MCP 与唯一有效 target 保持，
原批次工作目录已 absent。持久原件不依赖已删除的暂存包或测试副本。
