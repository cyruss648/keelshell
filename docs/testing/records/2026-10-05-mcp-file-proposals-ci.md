# MCP 文件提案首次整合 CI

验证源码为 `a909609c2bf64455ad48ec6c1704bdd68ec3df9c`，对应 [Quality 37306263578](https://github.com/cyruss648/keelshell/actions/runs/37306263578)。流水线已结束，整体 **failure**：Linux、Windows 成功，macOS 失败。三个 job 的实际 checkout 和 API head 均为该精确提交。

| 平台 | 实际 Rust 结果 | 其它门禁 | 边界 |
| --- | --- | --- | --- |
| macOS 26 | 469 普通通过、1 失败、2 ignored；app 389/1；0 doc | 格式、严格 Clippy、x.y、6 脚本、57 打包通过 | workspace 测试遇失败后停止，后续 MCP/session/doc/OpenSSH 未执行 |
| Ubuntu 24.04 | 1077 普通、8 doc 通过；12 ignored | 格式、严格 Clippy、x.y、6 脚本、57 打包及两个控制器通过 | 9 项 OpenSSH 实际通过；清理只覆盖被观察的自有身份，不证明观察间没有分离进程 |
| Windows 2025 | 1057 普通、8 doc 通过；11 ignored | 格式、严格 Clippy、x.y、5 脚本通过/1 跳过、53 打包通过/4 跳过及两个控制器通过 | OpenSSH 步骤跳过；`Ran` 总数不作为执行通过数 |

新增 17 项具名行为在 Linux/Windows 全部实际通过。macOS 已执行的新增项为 9 通过、1 失败，其后 7 项未执行。失败是 `mcp_file_failed_regrant_releases_finished_preparation_and_keeps_old_scope`，在点击授权后立即断言 `mcp.busy` 为真时失败；原 CI 没有记录足以区分授权未开始、已完成或事件处理顺序的观测，不认定具体调度原因。

原 run ZIP 为 269304 字节，SHA256 `484e87145ccef84a1fcabc6040f24d8bc12e2f5aa8acd01d0ffcaa912b96be11`；34 成员 CRC 和回读一致，三个单独原 job 日志与 ZIP 原字节一致。Linux OpenSSH 原 artifact 与 API digest 一致。非作者材料封装为 133 proof 文件、134 成员归档，根逐字节和摘要复核通过；manifest SHA256 `5cc22aef490887521effa853fd48547d1f1e17796fb1825bef050b91968828e9`，归档 SHA256 `7ba03ce6fec9e9fb684152d51538b3dc3d872ea8a5baabcbaca7b9e17c4bff0c`。

新的[测试同步修正](2026-10-05-mcp-grant-readiness.md)已通过作者完整门禁、全新非作者限定复核及主树1083普通+8doc+6脚本完整门禁，原7秒界面截止和产品授权校验限时保持。修正提交后的三平台CI另核验；这次macOS失败保持，具体时序原因仍未证实。

原主树八工具开发包的批准、拒绝和已观察外部内容变化保护仍属于独立的 [macOS 原生切片](2026-10-05-mcp-file-proposals.md)。本 CI 不证明三平台原生窗口、实际供应商第八工具、正式发布或安装更新。
