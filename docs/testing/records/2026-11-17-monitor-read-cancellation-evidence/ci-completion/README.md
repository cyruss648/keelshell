# 监控读取取消的精确 CI 终态封存

仅签工程 `6ae91d2e90668e7d9c04a2a7eea7c5de1bf6d3d4`，attempt 1。
Quality `44040248864`、Windows API `44040248806`、Windows OpenSSH
`44040248746` 的原 run／job 均已核对为 completed/success；
API 单独的 isolated-SSH job `133892796479` 仍为 skipped。
收集者 UTC07:56:50 的 Quality pending 与独立复核 UTC08:08:28 取得的终态
分别保存在封存内，没有改写历史快照。

| Quality 平台／job | 普通 Rust passed／failed／ignored | rustdoc | Python 主 run／skip |
| --- | --- | --- | --- |
| Windows／133892795649 | 3453／0／66 | 15／0／0 | 526／53 |
| macOS／133892795291 | 3466／0／68 | 15／0／0 | 526／3 |
| Ubuntu／133892795682 | 3459／0／69 | 15／0／0 | 526／3 |

每平台普通 Rust 96 组、rustdoc 6 组，父级直接读取日志 ZIP，重新解析声明、
每个名字／状态／行号和每组结果，与独立库存和报告逐项相等。
Python 主为 quiet discovery，仅核对单独的 run／skip 总数，不声称逐名清单；
preflight、packaging、内嵌 Python 和 controller 不重复加到主计数。
macOS／Linux 各 OpenSSH interop 18、forwarding 6 通过，四份原 result、
tests.log、steps 及 observed/owned cleanup 相符，累计祖先观察边界保持。
Windows API lab 111 run／3 skip、capture replay Rust 1 通过／18 filtered；
SSH lab 96 run／1 skip、真实 SSH Rust 1 通过，详细原件见封存报告。

## 完整成员与散列

两个只读来源全部普通安全成员封存，排除 0、遗漏 0：

| 原来源 | archive 前缀 | 文件 | 逻辑字节 |
| --- | --- | ---: | ---: |
| work/monitor-cancellation-ci-completion-20261117 | collector/ | 195 | 1,041,746 |
| work/monitor-cancellation-ci-review-20261117 | review/ | 254 | 12,010,610 |
| 合计 | | 449 | 13,052,356 |

`originals.tar.gz`：2,900,131 字节，SHA-256
`0a2d2b7908ca447718542503582d35589c9f2aaeb89dd8926f9652a32ce52e2a`。
`manifest.json` SHA-256
`336cef204b0a7b9212d1d01602a87ce8773bcea5f02daa815cc92fddcbfc3b7c`。
manifest 逐项记录 member、原 source、长度、SHA-256；归档固定顺序、mtime、
uid/gid 和 gzip mtime，可确定重建。无符号链接或特殊文件，未解包到工作区。
原两份 SHA256SUMS 本体也在包内，194＋253＝447 条覆盖各自除自身外全部文件；
外层 manifest 覆盖全部 449 件，包括两个 SHA256SUMS。

7 个原始 ZIP 共 172 成员，逐 ZIP CRC、成员长度／摘要和原样解压副本相等。
四个 artifact 的列表、单项 metadata、run/source、长度及官方 digest 均相等：

| Artifact | Run | ZIP 字节 | SHA-256 |
| --- | --- | ---: | --- |
| 14308219775 | 44040248746 | 54235 | `352d64f9b905acd4cd8ad5f963e77985da4c22aec19d450327962476b0fa639a` |
| 14308319951 | 44040248806 | 46863 | `88b9ed0cf60e25963af31155c227b2b39524ca5dc42767ba2cda7f3b5a690009` |
| 14309783967 | 44040248864 | 5512 | `d44532b440cc0ca0f25a56421463c61865247507c7eb0cd05ec081f41ade9df0` |
| 14309968324 | 44040248864 | 5593 | `32554df4c5b873765d210dbde7417b255ad8adcc762a1bee737058312681c0b0` |

日志 ZIP 没有 GitHub 服务端 digest，只记录本地下载字节摘要。
封存报告内的旧绝对路径是原现场定位信息；来源日后回收后，以 manifest 的
source → member 映射查包内字节，原日志行号保持。

## 复核、失败与边界

独立 Parfit decision 为 `APPROVE_CI_EVIDENCE_ONLY`，3318 检查通过／0 失败，
P0／P1／P2／P3 均 0。两作者已由父级关闭；本封存任务未新增生命周期操作。
`review/analysis-history`、`review/receipts` 保留 recompute-r1／r2 的两个 exit1：
首次错误要求 debug owner 外层 exit0，第二次误读 receipt 为 job metadata。
收集者全部历史 helper、命令、输入、SHA 和 pending 也完整保留。
本封存 helper 首轮 checkout 匹配失败回执保存在自有 seal 工作目录；
修正以原日志规范化行识别 Windows git.exe 后，仅重算原件。

父级先对两份来源作独立复算与精确旧 Git 输入绑定，8004 检查通过；
再从压缩包读回复算及绑定，8408 检查通过、0 差异，
自然 exit0／reaped／0.254373s。
对应原输出及自然退出回执为 `parent-verification*`、`archive-verification*`；
这两个输出形成于添加最终交付文件 SHA 清单之前，保留其当时准确检查数。
最终 `SHA256SUMS` 覆盖本目录除自身外所有文件；`verify.py` 会额外复核此清单。

SSH seal **HOLD**、terminal_proven=false 保持；
21 个原身份中，20 非 master 自然终态为 exit0×14、255×5、1×1，
master cleanup exit125 且 natural_terminal_before_cleanup=false；
owned-sshd 外层 observed_exit=null 保持。
cleanup PASS／active0／pending0／handles0 不等于所有进程自然 exit0。
API isolated SSH skipped、direct API ssh=UNRUN、两份 Windows native_gui=UNRUN 保持。
此封存不签当前 MCP 滚动修复、当前原生矩阵、完整产品或发行。

## 离线复核

在仓库根目录运行，所有命令只读，输出 JSON，不执行包内 helper：

```sh
python3 -B docs/testing/records/2026-11-17-monitor-read-cancellation-evidence/ci-completion/verify.py
python3 -B docs/testing/records/2026-11-17-monitor-read-cancellation-evidence/ci-completion/verify.py --bind-source
python3 -B docs/testing/records/2026-11-17-monitor-read-cancellation-evidence/ci-completion/verify.py --commit FULL_40_HEX_LOCAL_COMMIT
```

默认只读当前封存，来源目录可不存在；`--bind-source` 再读取精确旧工程 Git
对象内的原 candidate-inputs-r3，验证 777 输入长度、摘要和 aggregate
`6d495b849c63e00dfc7ba8b867f3fa8b5ba3481de9e4b66669dee77f40fa22d5`。
`--commit` 还要求本目录所有交付文件及追加后的主记录与指定提交 blob
逐字节相等，777 输入始终签旧工程 SOURCE，不拿当前 MCP 源码或文档 HEAD 代签。
本子任务不提交；该提交绑定步骤留给父级准确本地提交之后执行。
没有 Cargo／产品／workflow 重跑、target 产物读取或修改、Git 写入、推送或来源清理。

操作边界补记：最初查找嵌套 AGENTS 时曾递归枚举 work 目录项，涉及 target
目录条目；未读取、执行或修改 target 产物。之后所有查找与验证均限于明确路径。
