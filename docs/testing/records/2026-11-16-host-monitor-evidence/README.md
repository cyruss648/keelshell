# macOS 资源监控与 Linux 能力恢复：持久证据

状态：五份限定非作者报告已批准，开放 P0–P3 为 0；最终门禁 r3 实际
`exit 0 / reaped / 680.172s`。1035 份原件已无损封存，
713 项工程输入及全部 gzip/解压原文已通过 Git index/blob 逐项核对，零 mismatch。
精确本地提交绑定和临时目录清理另留回执。

普通 Rust `3157/0/48 ignored`、rustdoc `14/0`、工程 Python
`284 run / 1 skipped`、helper Python `25/25`，格式、严格 Clippy、x.y 政策与
三组 AI controller 通过；52 个新增默认 Rust 名逐一通过，门禁后完整输入零漂移。

最终 r6 为 713 项工程输入，集合 SHA-256 为
`e0b6ae274c4fad8d1246df5cba5c21b779b8afa6341385818a99b0a2264effd3`，
定义为排序 `path + NUL + sha256 + LF` 的拼接摘要。
它不是 manifest 文件自身的摘要。相对初始 r1 只有四个测试文件变化，
生产、正常 GUI/MCP 和两项真实平台实验的输入保持。

| 原件入口 | 内容与边界 |
| --- | --- |
| `raw/final-inputs-r1.json.gz` 至 `raw/final-inputs-r6.json.gz` | 各轮独立冻结；中间输入不覆盖最终输入 |
| `raw/full-gate-r1.*.gz`、`raw/full-gate-r2.*.gz`、`raw/full-gate-r3.*.gz` | 测试 lint 失败、磁盘 fixture 失败及最终门禁的各自实际终态 |
| `raw/session-regression-r4.*.gz`、`raw/session-regression-r5.*.gz` | 两次包专项的旧完整命令比较/首行 cfg 失败；r5 使用 no-fail-fast 收全 |
| `raw/pre-final-gate-resolution-r3.json.gz` | 五份批准、六项 P2 闭合与最后门禁启动前的输入核对 |
| `raw/review-resolution-r3.json.gz` | 在旧签署条件之外另行绑定最终门禁；不改写原报告的 PENDING |
| `raw/review-app/`、`raw/review-darwin/`、`raw/review-recovery/` | 三个主要源码/运行范围；Rust 使用父预构建，没有独立重编译 |
| `raw/review-lint/`、`raw/review-disk/` | 测试 lint、两磁盘断言与首行 cfg 的增量复核；内存模型不算原生执行 |
| `raw/runtime-summary-r1.json.gz` | 真实 Mac SSH 六项、Linux 恢复两项及原清理边界 |
| `raw/normal-build-r1.*.gz`、`raw/normal-binaries-r1.json.gz` | 正常 arm64 Mach-O 构建与摘要；非 test 产物，不代签新桌面 |
| `raw/ss-tools-r1/`、`raw/ss-tools-recovery-r2/` | 原 FAILED/UNKNOWN/cleanup=false 与依据原 CLI 实际完成的新恢复分别保存 |
| `raw/cleanup-resource-readback-r4.json.gz` | 自有资源完整 ID/name absent、worktree/分支、正常程序与共享 base 的读回 |

`archive-manifest.json` 列出逐份 gzip 的原字节数、原摘要、压缩字节数与摘要：
原文 `13,722,596` 字节、gzip `5,539,192` 字节，解压后逐 byte 核对。
`omitted-artifacts.json` 只记录 18 个可重建程序、测试副本及缓存，
共 `884,900,440` 字节，保留其来源摘要；不把这些省略件写成已归档的原件。

`verify_binding.py --index --output <新回执>` 对完整工程路径集合、冻结输入、
每份压缩原件及其解压字节核对 Git index/blob 与磁盘。
本地源码提交后使用 `--commit <完整 SHA>` 再绑定同一输入；通过前不删除本批目录。
提交身份、清理和清理后绑定另有独立回执，纯文档收尾不重跑输入未变的 Rust 门禁。
[暂存区核验回执](index-input-binding.json)记录实际逐项检查结果。

原失败、中断、UNKNOWN 和 reviewer 检查构造错误全部保持，不由后来 absent 观察
倒推旧创建完成。真实 Mac SSH、Linux 容器、headless GPUI 与原生桌面分别记账；
新卡片的完整桌面/IME/辅助技术、Windows/Linux 桌面和签名发行仍开放。
