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

2026-11-17 本批 CUA inventory 返回 Mac 锁定且自动解锁未成功；
GUI 操作和截图均为 0，没有启动旧包代签当前工程。
纯模板、受控 loopback SSH/SFTP 和 headless 绘制不等于实际 Codex/Claude Code
配置加载、八工具供应商业务或三平台原生验收。
