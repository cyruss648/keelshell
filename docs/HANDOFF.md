# KeelShell 开发交接 — 2026-10-06

当前主线检查点为 `2f7ca1e`，公开仓库为 `https://github.com/cyruss648/keelshell`。AI脱敏修复及test-only阶段记录已提交推送，完整本机门禁与限定原生独立复核通过。该源码新CI为Linux/macOS成功、Windows额外小栈整体45秒预算失败；Windows默认完整控制器用了73.5秒，其中26次实际拒绝连接合计52.5秒。本候选仅将Windows小栈整体测试预算更正到90秒；本机完整门禁1179普通+8doc+6脚本及严格检查通过，尚待独立审查及新Windows执行，见[预算记录](testing/records/2026-10-06-windows-small-stack-budget.md)。下面按当前范围交接；以前的时点及失败保留在[历史交接](history/2026-10-06-handoff-before-redaction.md)与对应测试记录中。

## 产品范围与不可省略的要求

- KeelShell 使用Rust与原生GPUI Kit，面向macOS、Windows和Linux。仅管理远程SSH，不开发本地终端、RDP或串口；不能将部分切片视为完整产品。
- 默认简体中文，维护中文/英文及两份README。外观默认跟随系统，可切换浅色/深色；切换须保留会话、草稿和在途任务。应用图标白底并适配各平台，工作区保持连接、监控、终端、文件/命令与AI的清晰分区。
- AI配置与交互研究见[本机与官方参考](research/dbx-ai-reference.md)。API和本地Claude Code/Codex推理入口独立；上下文由用户明确选择，回答与提案可完整审阅，模型不能自行执行动作。
- **MCP仅由KeelShell向外部智能体提供服务**，不开发通用第三方MCP客户端。外部访问默认关闭，限定已授权会话、工具、路径及终端片段；命令/文件修改为人工审阅提案。见[方向与计划](product/DESIGN_AND_AGENT_PLAN.md)、[对外接入](product/EXTERNAL_MCP.md)。
- 工程保持UI、领域/存储、SSH传输与AI分层，阻塞I/O不进入UI线程。直接registry版本使用`x.y`，保留Cargo.lock补丁锁定和精确工具链。公开库API有rustdoc，行为测试有界并使用隔离数据；凭据默认临时，显式保存走OS存储或认证加密vault，主密码与明文凭据不入metadata。
- 已只读评估Reef及模板的工程/生命周期方案，见[研究目录](research)。不引入私有Git依赖、开发者绝对路径、真实凭据或客户日志；仓库通过自身能力介绍产品，保持既定命名约束。
- 用户授权公开GitHub、提交推送和标签触发多平台Release；不授权遥测、自动安装CLI或覆盖现有应用。功能完成后新开非作者子agent审查，合入后保留失败证据并清理不再需要的工作树、分支、缓存与容器。

## 当前实现与本轮证据

现有功能包括连接目录/标签/收藏/回收与快速SSH连接，密码/密钥/Agent/MFA与加密vault，跳板及上游代理，远程终端/搜索/命令建议，SFTP文件管理/编辑/权限/目录比较与审核式合并，暂停与校验续传，监控/端口/TCP诊断/隧道，批量命令与依赖工作流。实际范围及未完成项以[路线图](ROADMAP.md)为准；旧历史结果不验收后来新增功能。

本轮修正普通 `ask-progress` 中内部 `sk-` 被当作token的整行脱敏误报。仅在固定前缀启发式排除前一个ASCII字母/数字；显式已知秘密、PEM、Bearer与其它规则保持。未知token嵌在ASCII标识符中的识别限制明确保留。新的非作者14项正式Rust、2项正式GPUI、6项补充Rust、2项补充GPUI及严格Clippy通过，旧函数仍11通过/3失败；见[设计](adr/0053-ai-token-prefix-boundaries.md)、[修正记录](testing/records/2026-10-06-ai-redaction-boundaries.md)。

新macOS标准开发包的两次隔离运行显示普通命令完整可读。首次无SSH上下文，人工送入未确认；第二次明确捕获测试SSH屏幕后，人工点击使精确建议进入命令栏，随后清空。8条进度事实的最后一行可滚动看到。实际Codex与自有SSE/回显SSH夹具参与；没有执行shell命令，没有云模型或客户主机验收。GUI/SSH明确wait/reap，HTTP活动与线程为0，private删除；根对各三个已记录出生及两个端口另外核验，不声称完整供应商后代普查。新非作者原生证据复核通过限定范围，最小逻辑窗口和其它平台未关闭。

本轮test-only增量增加固定阶段/时序/TCP结果记录，保留原50处Duration、82个断言、11个TCP调用位置及夹具协议字节。非作者默认/2MiB控制器各626条记录、38个原TCP调用与5080字节future通过；三个Rust边界探针与严格检查通过。原Windows45秒总期限超时的pending和根因仍UNKNOWN，本增量不宣称修复；见[阶段记录](testing/records/2026-10-06-ask-controller-stages.md)。

主树完整门禁384.014秒exit0：1179普通/11ignored、8doc、6脚本、格式、x.y和严格整仓Clippy通过；默认与2MiB控制器各626记录/38原TCP/5080字节future。261工程输入前后相等，所有候选Rust与非作者冻结源码相等。原始失败、截图和收据均保存在ignored `work/`，公开记录只包含隔离fixture与事实摘要，不将编译/fixture/原生窗口的证明范围混为一体。

## CI、发布与原生验收边界

已验证的源码CI检查点cfac的attempt1：Linux成功；Windows普通与doc通过，但额外2MiB控制器45秒超时；macOS未取得运行器。attempt2终态cancelled，Mac明确因同并发组更高优先级等待请求取消；Windows/Linux步骤复用旧attempt，不是新执行。后续ad0912c的三个job均取消且runner0/无steps，顶层failure不代表产品断言失败，取消原因UNKNOWN。后续源码head须单独核验；见[原CI及终态补充](testing/records/2026-10-06-local-ask-progress-ci.md)。

GitHub标签发布矩阵、白底图标、内置变更日志生成、项目链接、检查更新、SHA-256下载与审核式安装/回滚helper已经接通。完整六目标Release产物、签名/公证、Windows/Linux桌面与已安装目录更新原生验收仍未完成。没有因为本轮开发包而发布新标签、签名或覆盖安装，见[发布记录](testing/records/2026-10-03-release.md)、[更新记录](testing/records/2026-10-03-update-panel.md)。

对外MCP当前八项工具。此前实际Claude七工具授权流程在macOS标准双程序包中完成限定验证；供应商第八文件工具、Codex完整MCP业务、最小窗口及其它平台原生仍开放。内置本地CLI Ask当前使用独立空目录、受控环境和显式API密钥；不复用订阅登录或用户工具/hooks/MCP配置。详细边界见[CLI指南](product/LOCAL_AGENTS.md)、[对外MCP指南](product/EXTERNAL_MCP.md)。

## 后续开发与并行工作

1. 四个已整合的脱敏/控制器作者及复核工作树已保存ignored材料并归档，原本地分支与checkout/cache已核对删除。继续完成Windows整体预算候选的门禁、独立审查与精确源码CI，保留原failure，不提前计为修复。
2. 非默认API推理/采样参数（Messages思考与努力值可组合）、远程传输并行队列及AI命令审阅目标提示分别在隔离工作树开发；完成后需新非作者复核、主树门禁与各自原生验证。不能据计划或作者结果提前计为完成。
3. 继续补齐本地智能体目录/环境引用、订阅登录与受限Agent工作流；MCP方向保持对外服务。未绑定会话的建议入口提示仍可进一步改善，不放宽审阅目标检查。
4. 完成传输并行/自动恢复、定时工作流/逐目标参数映射/任务级持久审计、镜像删除/冲突合并/差异应用、可编程参数补全、协议级网络诊断与更丰富监控。
5. 继续三平台原生主题/语言/最小窗口/键盘和辅助技术验收、六目标发布/签名/公证及安装更新。每项实际测试应记录自己的精确源码和范围，不能沿用旧包或旧CI的通过结论。

所有需求、设计、验收与剩余范围分别管理于 `docs/product`、`docs/design`、`docs/adr`、`docs/testing` 与[路线图](ROADMAP.md)。项目位于用户指定目录，迁移与公开历史清理的旧证据保存在历史交接及既有记录，后续继续使用本仓库。
