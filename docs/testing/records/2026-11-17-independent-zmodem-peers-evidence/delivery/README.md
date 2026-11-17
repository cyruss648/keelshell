# 本批提交、推送与回收

工程提交 `de9768c183c22f1051baabd529600667697247a1` 已普通推送至
`origin/feature/editor-unicode-codec-20261114`。
本地、tracking、远端三方SHA一致，读回工作区干净。
765工程输入及主归档623成员均已逐件与精确Git blob绑定，
验证命令 `0/reaped/0.292576s`；推送 `0/reaped/5.363335s`，
三方读回 `0/reaped/0.897517s`。没有强推或覆盖独立的main状态。

本目录保存21成员、47,995字节的 `originals.tar.gz`，
SHA256 `754317954075d12bc0c428e71c598f4dec8acfbe74b0ebbd8a9028b71e7def75`。
逐件长度和摘要见 [manifest.json](manifest.json)。
归档内 `receipts/feature-*` 保留原提交、推送、读回及Git绑定回执；
`cleanup-result.json` 保留回收前的原件匹配与受保护对象前后摘要。

Git绑定后，622份所选原件与主归档逐件相等才进行回收。
已回收六个本批自有目录、3,087个文件，逻辑字节合计232,321,717，
包含两个peer源码／构建／下载、两个review副本、父级测试副本及stage。
该数值为所选文件的长度合计，不声称文件系统物理可用空间的精确增量。
唯一target的device16777231／inode269342313不变，
正常 `package-r2/KeelShell.app` 的完整文件摘要前后相等。
五位本批作者／reviewer已关闭；无新增用户chat、worktree或容器。

## 精确 CI 快照

`source-ci-snapshot-r2.log` 是对工程SHA的GitHub API读回；
本快照不包含下载artifact或独立运行，不能扩展为原件内容、native或产品全验收。

| 工作流 | 精确run | 快照状态 |
| --- | --- | --- |
| Windows OpenSSH monitor integration | `44013808220` | completed/success |
| Windows monitor integration | `44013808184` | completed/success |
| Quality | `44013808155` | in_progress，三平台最终结论待验 |

后续文档提交不改变765工程输入，也不重放宿主完整门禁；
CI继续按上述工程SHA／run核验，不从文档HEAD推定新的CI成绩。
完整依赖最新仍UNKNOWN，独立peer仍PARTIAL，FILE-05、原生矩阵和发行保持OPEN。
