# MCP 审核滚动交付回执

工程 `39d9041f350e30788b39121959d2ae7f7efea043` 已普通提交并推送至
`feature/editor-unicode-codec-20261114`。UTC09:14:51 读回 local／tracking／remote
三方 SHA 完全相等，工作区干净。没有强推或改写 main。

父级对[主封存](../README.md)全部 555 成员、777 工程 Git blobs 及顶层封存文件
作准确提交绑定，自然 `0/reaped/8.244846s`。
随后再次只读绑定，再按完整来源文件集、目录身份和逐文件长度／SHA 核对，
回收 460 文件、406,831,883 逻辑字节，10 个源目录完全移除。
`work/mcp-scrollbar-native-20261117/` 仅保留维护正常包六件；
两个旧失败包、reviewer 复制测试二进制和一次性运行库已回收。
唯一 target 目录身份及三份受保护正常包文件摘要前后相等。
回收命令自然 `0/reaped/9.890067s`，没有按名称模糊清理其他资源。

`originals.tar.gz` 保存主封存之后新增／变化的 19 件、57,697 逻辑字节，
压缩后 6,777 字节，SHA-256
`5ca91f0c13ae039016921183994d0b99c61725662e63bee33f6ba7b67b3c3f18`。
完整 manifest 列出 source、member、长度／SHA，以及已在主封存的未变原件，
包含准确 commit、push、三方读回、绑定、回收、普通 GET 的 CI 快照与工装。
自己封存时的外层输出／结束回执产生在本快照之后，继续由当前小型 finalize
目录保留；不把零长度快照日志冒充完整命令终态。
父级读取此包的限定离线复核自然 `0/reaped/0.032395s`，无产品重跑。

UTC09:15:29 的精确工程 GET 为：

| 工作流 | Run | 快照终态 |
| --- | --- | --- |
| Quality | 44053769445 | in_progress／conclusion=null |
| Windows API | 44053769378 | in_progress／conclusion=null |
| Windows OpenSSH | 44053769444 | in_progress／conclusion=null |

均 attempt1／push／准确工程39d9041；不是 completed/success。
GitHub连接器只覆盖PR触发，此处使用 `gh api --method GET` 读取push工作流。
后续按准确 source/run 接续，不用旧监控 CI 或 docs-only提交代签此工程。

维护从主封存及本目录接续，不再使用已回收 work 下的原图／review 展开目录。
`work/mcp-review-scroll-integration-20261117/` 仅用于最后文档推送及回收回执，
已持久化副本在准确文档提交绑定后再按原字节回收。
所有本批作者／reviewer 已关闭；没有遗留本批应用、stdio客户端或Podman资源。
原生滚轮P2／ROOT UNKNOWN、紧凑原生、其他平台、完整产品和发行保持开放。

只读复核：

```sh
python3 -B docs/testing/records/2026-11-17-mcp-review-scroll-evidence/delivery/verify.py
python3 -B docs/testing/records/2026-11-17-mcp-review-scroll-evidence/delivery/verify.py --commit FULL_40_HEX_DOCS_COMMIT
```

本目录的提交绑定只签交付文件；工程绑定结果始终签准确 feature commit。
工具不解包执行、不联网、不访问或修改 target，也不重跑产品。
