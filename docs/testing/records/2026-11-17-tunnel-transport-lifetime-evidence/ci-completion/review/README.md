# 指定旧工程 CI 的新非作者复核

Heisenberg 严格只读，限定批准工程
`cdf1e171f2f9b179f09db230f4739696d02fe80d` 的
Quality44020265369、API44020265273、OpenSSH44020265248 原件。
独立 13,543 个离线检查通过、0 失败；不重复计数为产品测试。
P0/P1/P2 为 0；历史 checksum 表述 P3 为 1，外层实际说明纠正后已关闭，
未关闭项 P0–P3 全 0。报告不覆盖后续新 MCP、f910 CI 或原生包。
reviewer 已实际关闭。

`originals.tar.gz` 保存 9 文件/10,537,223 逻辑字节，压缩后 1,056,417 字节；
SHA-256 为
`badc7f86abf13975958f2a821e02e048f909c062de03695aae3f0051ae8baf36`。
`manifest.json` 逐件记录路径、长度与摘要。`review/REPORT.md` 是原报告，
`review/review-result.json` 是结构化结论，`audit-r1.json` 保留首轮
argv0 双空格假设造成的工装失败，`audit-r2.json` 保存修正后的实际复算。
原输入字节和 mtime 前后一致；没有网络、Cargo、产品执行、源文档编辑或 Git 写入。

7 ZIP/172 成员和四个 API digest 相等，769 个非 docs 旧工程输入完整覆盖
该精确 Git 树，三平台逐名库存/summary 与外层 314 成员相符。
原件只证明记录的自然终态和清理边界：API isolated-SSH skipped、
native_gui=UNRUN、旧 seal HOLD、中间 22 retained handles、自然 exit255/1、
master cleanup exit125 都没有改写成全 exit0 或 GUI/发行验收。
