# MCP 模板与未知记录增量的精确 CI

仅绑定工程 `f910e0013832643f09b112b08443675a956d5551`，不以文档 HEAD
或后续监控取消增量代签。Quality `44026933453`、Windows API `44026933510`、
Windows OpenSSH `44026933475` 均 attempt 1、completed/success。

| Quality 平台 | 普通 Rust passed／failed／ignored | rustdoc | Python 主 run／skip |
| --- | --- | --- | --- |
| Windows | 3442／0／66 | 15／0／0 | 526／53 |
| macOS | 3455／0／68 | 15／0／0 | 526／3 |
| Ubuntu | 3448／0／69 | 15／0／0 | 526／3 |

每平台96普通套件的逐名结果与summary相等；Python主使用quiet discovery，
仅有总数，不声称逐名库存。平台preflight、packaging、内嵌Python及custom controllers
分别记录，不重复相加。macOS／Ubuntu实际OpenSSH各18项、forwarding各6项通过。
Windows API lab111 run／3 skip，capture replay1通过；真实SSH lab96 run／1 skip，
真实SSH Rust1通过。direct API run 的 isolated-SSH job skipped 保持。

封存131成员，`originals.tar.gz` SHA-256：
`cca5346c9633111467d405ef6a6ebabac229e3992958825abc500edde04717da`。
每件长度与摘要见 [manifest.json](manifest.json)。
其中保留7个原始ZIP共172成员、完整API JSON、命令及自然exit／reaped回执，
4个artifact digest均与API相等；run日志ZIP只有本地下载摘要，没有服务端digest。
收集者99项检查通过，最终130条外层checksum相等；独立非作者复核另存。

新非作者Anscombe独立核对原API、ZIP、Git blob与命令，2301检查通过、0差异，
限定APPROVE、P0–P3全0，已实际关闭。复核65成员归档见
[review/manifest.json](review/manifest.json)，压缩摘要
`8c2dc6266654802f2eced3bd73cbb70ba4c00cc1af73300bff44e435089135b6`。
原两轮工装schema误读失败也保留；没有重新运行产品或CI。

原SSH seal为HOLD：20非master原对象自然exit分布为0×14、255×5、1×1；
最终cleanup为PASS且active／pending／owned handles均0，但master由cleanup退出125，
不是自然exit0。API isolated-SSH跳过、`native_gui=UNRUN`及所有原跳过均保留。
原件不签原生桌面、外部供应商客户端、安装、签名／公证或发行。
归档内 `collector/REPORT.md`、`summary.json`、`verification.json` 和
`SHA256SUMS` 说明计数方法、来源与完整边界；不重跑已验证的旧源码产品测试。
