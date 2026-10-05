# 对外 MCP 长文件审阅：macOS 六组合

MCP 方向是 **KeelShell 向外部智能体提供已授权的 SSH 能力**。本轮不引入在应用内访问任意第三方 MCP 服务的通用客户端。

精确主树 API 请求选项 F 的新标准 macOS 双程序包已完成受控原生验证。258 份 Rust/Cargo/工具链输入与完整门禁相等，新 app/MCP/fixture 摘要绑定相等。只连接隔离回环 SSH/SFTP fixture，临时授权枚举会话、目录列表、UTF-8 文件读取、文件修改提案与状态查询；没有授权命令、终端片段或监控。自有 stdio 客户端实际协商八项工具，并调用五种工具；不能把目录协商写成八项工具全部调用。

## 六项实际审阅

每项先通过原生工具栏选择语言和主题，再创建独立新提案。根打开完整审阅，查看目标与原/新 SHA-256，滚动到替换正文 BEGIN 及中文 END（无末尾换行），核对固定操作栏，点击人工拒绝。客户端再经实际 SFTP 读回完整字节和摘要。

| 语言 | 主题 | 新提案标签 | 人工终态 | 实际文件读回 |
| --- | --- | --- | --- | --- |
| 中文 | 浅色 | `cn-light-2` | rejected | 40499字节，原摘要不变 |
| 中文 | 跟随系统 | `cn-system` | rejected | 40499字节，原摘要不变 |
| 中文 | 深色 | `cn-dark` | rejected | 40499字节，原摘要不变 |
| 英文 | 跟随系统 | `en-system` | rejected | 40499字节，原摘要不变 |
| 英文 | 浅色 | `en-light` | rejected | 40499字节，原摘要不变 |
| 英文 | 深色 | `en-dark` | rejected | 40499字节，原摘要不变 |

原 SHA-256 为 `a72c3584b347805e86bf5d27b4b8b511fa06b75edcc33c582d92b5ce5a6f9499`。待审状态均保持零写入；每次拒绝后精确原字节保持。三种模式的选中状态、语言和正文首尾分别留原像素截图；本机跟随系统当时呈深色，没有切换 macOS 系统偏好。

原 `cn-light` 轮在人工操作前审阅超时，客户端 exit1、UI终态已到期；原失败及截图保留，之后使用新标签和新 action 验证。首次分界定位截图不含 BEGIN 的尝试也保留，不按文件名推断内容；英文两轮经进一步滚动才观察到 BEGIN。一次旧 AX 序号未打开完整审阅，随后重新获取完整树并点击正确可见入口；该尝试不计审阅通过。

## 尺寸与清理

原生截图实际2880×1866像素，逻辑窗口 frame 没有测量。两次通过 CUA 拖动窗口边缘未观察到尺寸变化，失败尝试保存；本轮 **不关闭900×580最小窗口矩阵**，也不沿用旧包最小窗口证据。已观察的六组合证明该实际窗口中的正文可滚动、首尾可见及操作栏可用。

六个成功 companion 均实际 exit0、自然 EOF退出，未发TERM/KILL，已观察出生身份及PGID成员消失、私有目录移除。最后原生点击撤销全部，截图显示“MCP is off; all temporary grants are revoked”，列表无授权。控制器在明确 stop 后实际 exit0，外层新记录器工具亦实际0、无超时、1023.116秒，508份全部输入前后不变。GUI收到自有TERM后回收、fixture收到自有INT后实际0；两个HTTP和SSH监听关闭、handler及启动读取线程退出、私有目录清理。只证明已观察的进程，完全脱离且未观察的后代不在清理结论中。HTTP计数0，不将此轮作为API验证。

冻结材料位于 ignored `work/native-ai-mcp-preflight-v2-20261005/runs/native-F-matrix-1`，128 proof/129归档成员，含原日志/receipt、截图/AX、root-gates和根限定读回。manifest SHA-256 `1789bb622aeaa2091eb5ccc08d6c65f021ccfa57ba2d5b1fb42113e3e6027dd0`；归档6883890字节，SHA-256 `d214533c7b28c3fe67f6275760bca4b83472a58870e22be027bf9fbe1bd04d38`。归档已逐bytes/hash核验。

新非作者限定复核已完成，结论 `LIMITED_NATIVE_RECEIPT_AND_PIXEL_REVIEW`，该范围无新已证P1/P2。复核实际查看33张原JPEG、七客户端与控制器原退出回执、完整门禁及57打包原日志，并再次验证258份工程输入绑定。它没有另跑GUI、Rust、RPC或供应商；外层工具exit0依据根明确报告，工具会话transcript未包含在包内，不能从控制器exit0推断。第一次解析工具名失败及原脚本保留，修正解析在同一冻结输入上实际exit0。

独立材料位于 ignored `work/native-F-matrix-independent-review-20261005-1dbb6138`，143 proof/144归档成员。manifest SHA-256 `ffb6454a88bac54118be63cbeb43f5dc3a088c2cd12e716af14b827c6565a512`；归档13779963字节，SHA-256 `0532c9ae22ad5f2bde217ee4d267001dcc505b94d2c6f156f14cfd2e3238a87a`。根再次逐bytes/hash与tar成员读回通过，原128文件也再次保持相等；API与记录器旧独立包未修改。

本轮不关闭最小窗口、真实系统变化、Windows/Linux原生、供应商第八工具、当前F的原生批准回归、实际远端服务器或Release/安装更新。旧包批准/拒绝/并发变化事实保持原范围，见[文件提案记录](2026-10-05-mcp-file-proposals.md)；API新原生证据见[API记录](2026-10-05-ai-request-options-native.md)。
