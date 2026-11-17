# 2026-11-17 MCP 客户端模板与未知记录

状态：本批实现、限定非作者源码/功能复核与最终工程门禁通过；
完整产品、外部供应商八工具业务、原生桌面和发行仍开放。
工程基线 `f4562ad17efc918976738b241352e7a4f8af434e`。
设计见 [ADR 0160](../../adr/0160-mcp-client-templates-and-unknown-history.md)，
使用见[对外 MCP](../../product/EXTERNAL_MCP.md)和[提案记录](../../product/MCP_ACTION_HISTORY.md)。

## 实现范围

- 纯客户端配置 renderer 与原生显式复制入口，不含活动能力值，不调用客户端。
- 批量历史清理保留 PendingReview / Running / OutcomeUnknown。
- 未知动作的独立模态核对、精确动作确认与内存核对标记；原状态、内容和远端
  mutation/quarantine 保持，不自动执行或重试。
- 原生三主题 registry、Palette、唯一模态、固定动作和独立正文滚动。

## 运行记录

本批原始命令、退出回执、失败和独立报告持久入口为
[本批原件](2026-11-17-mcp-client-history-evidence/README.md)。
工作目录是父级与 reviewer 的临时副本，Git 绑定后按原件核对回收。

1. `format-r1`：cargo fmt，`0 / reaped / 2.362292s`。
2. `app-mcp-r1`：`101 / reaped / 11.087712s`，测试子模块的 wildcard import
   导入 GPUI 的 `test` 宏，`#[test]` 递归展开。已收窄为两个具体 import，
   没有提高 recursion_limit；原失败保留。
3. `dependencies-r1`：官方直接 registry 新快照检查 `0 / reaped / 10.366745s`，
   37 包/113 声明全部通过；不扩大到间接依赖、Actions 或未来版本。
4. 本轮当前工具链实际 Rust/Cargo 1.100.0；不 pin toolchain、不设 rust-version。
5. `app-mcp-r2`：`101 / reaped / 13.740007s`，新测试缺少 ElementId import、
   ActionState 没有 PartialEq、Kit Selectable 未引入及测试按钮 ID 的非静态引用。
   已使用具体 import、模式匹配和静态测试 ID 修正，没有扩大生产权限。
6. `app-mcp-r3`：`101 / reaped / 45.677801s`，33 通过/4 失败。新增配置区
   挤低原有首条提案审核按钮（三个旧回归），已将提案优先于配置区；
   新 diff 测试把 `+++ replacement` 头计为一行，已改为逐行完整相等统计。
   原失败全部保留，没有放宽可见性或内容断言。
7. `format-r3`：`0 / reaped / 2.205051s`。
8. `app-mcp-r4`：`0 / reaped / 37.221893s`，37 通过/0 失败/0 ignored，
   包含六个新GPUI用例、既有准确SSH/SFTP、八schema、文件审核及首提案可达性。
9. `mcp-client-library-r1`：`0 / reaped / 12.760801s`，13 通过/0 失败，
   包含Windows路径、恶意配置形状、全部C0/DEL、Unicode和非UTF-8拒绝。
   Windows专用未配对surrogate用例不由本机Unix代签。
10. `template-library-build-r1`：精确Cargo JSON artifact指向实际当前生产库。
    首次独立parser工装误猜debug/deps路径，rustc E0463，原件保留；
    r2改从所有实际compiler-artifact位置派生依赖搜索路径，产品代码未变。
    `independent-template-parser-r2`为`0 / reaped / 0.989307s`：
    实际生产renderer输出41例由Python tomllib完整解析，Claude JSON40例语义相等，
    1例含`${`路径静态拒绝；库摘要前后不变。没有客户端或供应商请求。

当前冻结`candidate-inputs-r1`为777非docs输入，aggregate
`b34aedb8d859f1ca0b97934b707c71de8200b3ffd8a53ab602d323809932553e`。
最终完整门禁 `python3 -B scripts/check.py` 实际为
`0 / reaped / 815.372858s`，UTC `05:28:34.684845` 至 `05:42:10.061542`：

- 普通 Rust：3455 passed、0 failed、68 ignored；96 普通套件。
- rustdoc：15 passed、0 failed、0 ignored；与普通用例分别计数。
- Python 主 discovery：526 run、0 failed、3 skipped；包装和内嵌 helper 不叠加计数。
- fmt、严格 Clippy、依赖声明政策及三个 controller gate 均通过。
- 最终完整日志 SHA-256：
  `cb5dc33b8e75e998f43d38cbda1ca80be8527e8b52dfed4097a885e250e931d3`。
- 冻结前后 777 个工程输入相等；唯一 target 连续归属当前 checkout，没有 clean
  或并行 Cargo。96 普通套件与实际二进制 `--list` 逐名相等、结果计数相等，
  二进制摘要前后不变；两个 `harness=false` controller 以实际门禁执行为准。

库存工装 r1 对 session 日志的两条 Rust/Python 交错输出未解析完整，退出 1。
原日志和工装保留；r2 仅移除完整可识别的内嵌 Python unittest 前缀，
保留结果 token 与原字节，逐名、重复名、通过/失败/跳过总数及摘要全部相等。
这是日志解析修订，没有修改或重跑产品测试。

## 新非作者复核

Cicero 对上述冻结输入限定 **APPROVE（SOURCE + FUNCTION）**，
P0/P1/P2/P3 均为 0；要求父级最终完整门禁成功，该条件现已满足。
原报告和执行回执保存在归档 `review/` 下。独立执行父级核验过的预编译副本：

| 选择 | passed | failed | ignored |
| --- | ---: | ---: | ---: |
| MCP 业务 | 37 | 0 | 0 |
| 历史清理 unit | 1 | 0 | 0 |
| 唯一模态 scope | 11 | 0 | 0 |
| 客户端模板 | 13 | 0 | 0 |
| 合计 | 62 | 0 | 0 |

app 实际库存 1407、模板库存 13，新增 20 项全部逐名存在并实际通过；
20 项是上述 62 项的子集，不重复相加。这是独立预编译执行，没有独立重编译。
另独立解析 55 个固定预期夹具，并重新解析父级实际 renderer 的
41 TOML/40 JSON/1 拒绝结果；前者不声称重新执行生产 renderer。
reviewer 工装最初的 raw-string/sentinel 解析失败和路径查找失败均保留。
32 项饱和及确认后 quarantine 保持属于源码复核，没有新增对应动态 stress/
端到端复验。最终门禁由父级执行，不将 reviewer 批准扩为独立完整门禁。

两个 reviewer 二进制副本和一个 standalone probe 核对来源后已回收，
共 3 文件/278,588,320 逻辑字节；正常 target 和旧正常应用包保留。
本批没有创建用户 chat、worktree 或容器，作者与本批源码/功能 reviewer 均已关闭。

## 原生与客户端边界

2026-11-17 本批首轮 CUA inventory 返回 Mac 锁定且自动解锁未成功；
GUI 操作和截图均为 0，没有启动旧包代签当前工程。
纯模板、受控 loopback SSH/SFTP 和 headless 绘制不等于实际 Codex/Claude Code
配置加载、八工具供应商业务或三平台原生验收。

后续实际桌面已可用，使用精确已推送工程
`f910e0013832643f09b112b08443675a956d5551` 新构建正常双程序包：
release build `0/reaped/219.335732s`，打包 `0/reaped/0.147274s`，
arm64/minos15.0 结构检查 `0/reaped/0.136847s`。
app SHA-256 为
`70045d6ff4f278a8a9df8bea075dcaa6bb65b3304895924101af588c90c2f110`；
没有覆盖安装或复用旧包证明新代码。

从新隔离空目录启动的原进程 PID34098，实际
`0/reaped/340.564409s`，无超时；关闭审核显示 0 SSH/0 文件草稿。
11 组原 JPEG/AX 均保留，真实像素 2880×1866，未缩放、转码或修改：
初始霜白 System（实际深色）工作区；三套主题选择页；
Frost Light、Graphite Dark、Lavender Light 各中文/英文的 MCP 空授权面板；
客户端 Codex/Claude 的可见选中样式；Tab 后关闭按钮可见轮廓；关闭审核。
AX 的 focused element 仍报告窗口，不能由轮廓代签完整焦点或辅助技术。
Escape 实际回到工作区。一次截图重编号后使用旧 AX34 被拒，更新为实际 AX7
后主题切换成功，原工装失败保留。

本次没有活动 SSH 授权，复制模板、临时配置、清理和授权按钮保持禁用；
没有点击复制、启动 MCP 或供应商，也没有未知动作模态和远端文件/命令动作。
这些是原生入口、禁用状态和可见布局证据，不是原生复制成功或远端业务闭环。
最小/紧凑尺寸、更多明暗组合、多窗口、IME/VoiceOver、Windows/Linux 和发行继续开放。
新非作者的原图复核与交付回执从
[本批交付原件](2026-11-17-mcp-client-history-evidence/delivery/README.md)接续。
Boole 已实际查看全部 11 图，对可见子集限定 ship/APPROVE，P0–P3 全 0，
独立原件审计347/347、报告读回13/13通过，并已关闭；
没有 GUI 或产品重跑。其首轮辅助清单误计的工装失败保留。

## 精确工程交付

工程 `f910e0013832643f09b112b08443675a956d5551` 已普通提交/推送，
local/tracking/remote 三方相同，读回时工作区干净。
777 个新工程输入、769 个旧 CI 工程输入及两归档 574 成员精确 Git 绑定通过，
`0/reaped/0.197138s`；不由 HEAD 相等或 Cargo 缓存代签。
Git 绑定后按完整文件集及原字节核对回收父级/reviewer 两目录，
260 文件/3,457,053 逻辑字节；唯一 target 目录身份及旧正常包六文件摘要相同。
此后新增的正常 f910 包单独保留，维护入口见当前交接。

本工程 Quality `44026933453` 和两个 Windows 工作流独立跟踪，
不由旧 cdf1e171 的 CI 成功或本机门禁推断新三平台终态。
UTC06:20:45 后次实际 API 快照为 macOS success、Ubuntu/Windows in_progress；
两 Windows 工作流为 success，仍未下载和验收本批新原始测试/artifact。

## 精确 CI 终态补充

后次实际GET读回：Quality`44026933453`、API`44026933510`及真实SSH`44026933475`
均attempt1、completed/success。Windows／macOS／Ubuntu普通Rust3442／3455／3448
通过、0失败，ignored66／68／69，各15rustdoc；Python主526 run、skip53／3／3。
macOS／Ubuntu真实OpenSSH各18项及forwarding各6项通过。
API lab111 run／3 skip、capture replay1通过；SSH lab96 run／1 skip及真实Rust1通过。
平台preflight、packaging与内嵌helper单列，不与主Rust重复相加。

7原始ZIP共172成员、4个artifact服务端digest相等；完整API／命令等131件原件已封存。
新非作者Anscombe独立2301项离线检查通过、限定APPROVE、P0–P3全0，已实际关闭；
65成员复核原件含两轮工装误解原schema的失败，均保留，不改写成产品失败。
原seal HOLD、master cleanup125／非自然终态、非master自然255×5及1×1、
API isolatedSSH skipped与native_gui=UNRUN保持。
精确计数、Git来源与限定边界从
[CI终态封存](2026-11-17-mcp-client-history-evidence/ci-completion/README.md)接续；
此补充不代签后续监控取消工程、供应商客户端、原生矩阵或发行。
