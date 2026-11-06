# 开发路线 — 2026-11-06

完整产品目标保持进行中。精确SHA、执行会话和Cargo归属从[当前交接](HANDOFF.md)接续；
[历史路线](history/2026-11-06-roadmap-before-editor-traceroute.md)保留上轮冻结状态。

## 当前集成

`feature/main-visual-integration-20261106` 正在把 `main` 的最新功能基线与工作区
外壳视觉增量合并。集成后的 UI 使用 Frost、Graphite、Lavender 三套注册表主题和
语义 surface/chrome 层级，活动会话、连接导航、工具栏和状态栏共享同一套可扩展
组件 token。视觉来源的定向门禁与独立复核已经通过；集成分支仍需重新执行完整
工程门禁、非作者代码复核和开发包检查后，才能形成新的可交付结论。新主题继续只扩展
注册表、Palette 和组件形状，业务视图不增加主题 ID 分支。

## 最新交付

main已普通推送`fa7c05914e0eefc0f24ab32f8af690bdc50a63ed`，交付读回一致。
本批按项监控能力和审核式知识分类的536输入fresh完整门禁、开发包、
限定macOS A/B原件及正式文档均获各自非作者批准。
2313 ordinary/23 ignored、14 rustdoc、Python131/1 skip；28对原生AX/JPEG，
首轮HOLD/UNKNOWN保持，完整native finish开放。精确三平台CI均已成功；
唯一观察器exit2已实际wait/reaped；原Linux附件GET失败保留，新单次补充读回与
两平台各11项OpenSSH获独立`current-ci-r2`限定收口批准。三平台各204候选实际通过。
CI不代签桌面验收。见[本批记录](testing/records/2026-11-06-monitor-knowledge-categories.md)。

## 当前并行开发

| 优先级 | 用户功能 | 真实状态与下一步 |
| --- | --- | --- |
| P1 | [远程文本格式](product/REMOTE_TEXT_FORMAT.md) | r3三项连续编辑P2关闭；父543输入门禁/包通过；native-r1限定批准macOS原生BOM/CRLF取消与确认，完整编辑器验收HOLD |
| P2 | [审核式有界traceroute](product/TRACEROUTE_DIAGNOSTICS.md) | stderr精确版本合同与最终门禁通过；同包真实SSH UI各审核确认UDPv4/v6一次，均一跳回复；原生ICMP/取消和完整平台另验 |
| P1 | 独立工作窗口 | 下一片已委派隔离开发：真实空白新窗、各窗独立会话/审核、安全关闭及共享配置协调；尚无实现/验收结论，Cargo归属见当前交接 |
| P1 | 编辑器动作可达性 | 原生观察发现inline编辑器底部动作难以触达；Carver在隔离源修复，合并采用/保存原生保持NOT_RUN，不混入本批543输入 |

父已装配543输入、21源码路径，保留两片唯一公共文件的双方导出；
五个项目crate已失效重编，42新增具体名均在新库存中。r2工程门禁2355/23 ignored通过，
但版本流缺陷需r3另验；r3最终完整门禁已实际0/reaped，575.972秒，
2355 ordinary/23 ignored、14 rustdoc、Python131/1 skip；42新增及基线204名实际pass。
新包构建与macOS包检查完成，全部成员和41项资产已读回；
Gauss的combined-r3/r4已批准限定源码及门禁/新包；
新runtime-r3原生797.330164/900秒结束、外层28510实际wait0/reaped，
38组AX/JPEG及完整字节原件获native-r1限定批准；
新增P2 NR1-P2-01与完整native HOLD保持，下一片修复。
源代码通过、实际工具、原生、CI和发布安装分别验收，见[组合记录](testing/records/2026-11-06-editor-traceroute-integration.md)。

## 后续产品缺口

| 优先级 | 范围 | 当前边界与下一步 |
| --- | --- | --- |
| P1 | 工作区 | 已有分屏、比例、逐叶标签、显式保存和单窗恢复；多OS窗口、外框及完整关闭/重连审核矩阵仍开放 |
| P1 | 平台与发行 | 继续Windows/Linux真实桌面、完整几何/IME/辅助技术、六目标签名/公证和隔离安装回退；三平台CI不等于桌面验收 |
| P1 | 外部AI集成 | API/受控CLI Ask、逐步审核Agent及对外MCP已有实现；供应商账户/完整外部客户端、CLI登录身份复用的请求前隔离证明仍待验 |
| P2 | 日常文件操作 | 继续原生文件管理器拖放、更多文本编码、完整键盘/辅助技术及其他平台；现有字节与备份合同不扩大 |
| P2 | 监控与网络 | 按项能力与缓存已交付；继续df/ps缺失后恢复、ss可用、真实诊断错误/取消，以及远端macOS/Windows适配 |
| P2 | 持久化隧道 | 命名规则、三方向转发、流量与显式审核/停止已有实现；重启后的运行态恢复与重连owner完整矩阵开放 |
| P2 | 配置恢复/同步 | 已有审核恢复、回退与认证加密目录同步；跨机器挂载、断电耐久性及最终一致同步器待验 |
| P3 | 高级传输 | 审核式归档/路径穿越防护、rz/sz实机、用户中继与可重复收益验收另行实施 |
| P3 | AI知识复用 | 本地分类、显式Ask和审核式批量管理已交付；跨设备同步及更多复用待设计，不恢复旧SSH权限或自动执行runbook |

清理遵循资源所有权。已合并分支、无消费者自有容器/镜像及时清理；
受其他任务或primary保护的工作树保留，失败/UNKNOWN与不可变原件不删改。
产品能力从[能力清单](product/CAPABILITIES.md)及对应指南查询。
