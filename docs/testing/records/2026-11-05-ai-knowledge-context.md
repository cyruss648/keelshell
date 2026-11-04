# 2026-11-05 保存知识显式模型上下文

## 身份与协调

- 实现基线：`c10d9f8365c1eecfef9f8042108ec4b3f9f087ee`。
- 独立受管 worktree：`ai-knowledge-context-20261105`；分支：`feature/ai-knowledge-context-20261105`。
- 作者切片没有修改 Files、`workspace/view.rs` 或依赖。父任务已导入 v2，补充全局文档并完成组合门禁及受控 macOS 原生观察；提交和最终非作者复核见文末。
- 新文档：[产品使用](../../product/AI_KNOWLEDGE_CONTEXT.md)、[ADR 0098](../../adr/0098-explicit-saved-knowledge-context.md)。
- 作者证据：忽略目录 `work/ai-knowledge-context-20261105/`。
- 当前为 **v2 组合候选**。v1 独立源码审查为 **HOLD / P1 AKC-SR-001**，v2 源码再审已关闭此项；下方保留 v1 成功与失败证据。作者窄检查、父任务组合门禁及原生观察分别记录。

## v1 已执行（后续审查 HOLD）

父任务释放共享 target 后，以下 Cargo 命令顺序执行，均显式使用协调的 `CARGO_TARGET_DIR` 和 `CARGO_BUILD_JOBS=2`，未运行完整工作区门禁。日志与回执保留在上述作者证据目录。运行时间为 2026-11-05 Asia/Shanghai（回执使用 UTC，日期为 2026-11-04）。所有进程已回收，共享 target 已在本轮窄检查完成后交还父任务。

| 检查 | 命令 | 实际结果 / 证据前缀 |
| --- | --- | --- |
| 核心选择合同 | `cargo test --locked -p keelshell-core --test knowledge_context` | 4 passed；退出 0；`core-context-r1` |
| 保存知识请求链路 | `cargo test --locked -p keelshell-app saved_knowledge_context -- --test-threads=1` | 最终 11 passed；退出 0；`app-context-r3` |
| 知识库与 Workspace 持久化回归 | `cargo test --locked -p keelshell-app knowledge_library -- --test-threads=1` | 14 passed；退出 0；`app-library-r1` |
| 现有推理回归 | `cargo test --locked -p keelshell-app assistant::inference_tests -- --test-threads=1` | 6 passed；退出 0；`app-inference-r1` |
| 现有助手回归 | `cargo test --locked -p keelshell-app assistant::tests -- --test-threads=1` | 43 passed；退出 0；`app-assistant-r1` |
| 配置恢复回归 | `cargo test --locked -p keelshell-app configuration_recovery -- --test-threads=1` | 10 passed；退出 0；`app-recovery-r1` |
| 严格 Clippy | `cargo clippy --locked -p keelshell-app -p keelshell-core --all-targets -- -D warnings` | 退出 0；`clippy-r1` |
| 工作区格式检查 | `cargo fmt --all -- --check` | 退出 0；`fmt-check-r1` |
| 原生 UI / CUA、完整门禁 | 父任务负责 | 本作者未执行；GPUI 测试不代表原生桌面验收 |
| 新非作者源码审查 | Confucius | HOLD / P1 AKC-SR-001；v1 的转义凭据检查不完整 |
| 提交 / 推送 / Release / 安装 | 未执行 | 留下未提交的独立补丁 |

v1 最终成功运行共 **88 个测试**（core 4，app 84），无失败或忽略。此计数只涵盖上述过滤范围，不是工作区全量结果，也未证明后来由审查发现的转义凭据边界。

`run-check.py` 在每次命令前记录 957 个 tracked / 非忽略 untracked 输入文件的 SHA-256，保存 `*.inputs.json`、原始 `*.log` 以及含命令、基线、环境覆盖、UTC 时间、PID、退出码和 `reaped: true` 的 `*.receipt.json`。所有 Cargo 执行期间的 `changed_inputs` 都为空。最终 app、Clippy、格式检查共同输入清单摘要为 `c5eeeb7a8998c58678117eaed4a3910ba4e9c9f651a5d74ab74d6010b7ed6633`；core 单测较早执行，其后只修改 app 测试，core 源码和测试保持不变。本记录在检查完成后更新，交付清单另外记录最终文件摘要，不把文档更新后的全树冒充原运行输入。

本轮不新增或升级依赖。父任务提供了 104 项 freshness 通过信息及其报告路径；本作者未独立运行或重新核验该报告，不能将其当作作者测试结果。

## 保留的失败与警告

- `app-context-r1`：退出 101，编译报 `recursion limit reached while expanding #[test]`。新测试的 `use super::*` 带入 GPUI 的同名测试宏，改为显式导入；没有增加递归上限。失败日志、输入清单、退出回执全部保留。
- `app-context-r2`：退出 101，10 passed / 1 failed。关闭知识库的事件尚未从当前窗口更新派发，测试提前查找 `send-approved-request`。修复测试调度：退出更新、`run_until_parked()`，然后断言完成绘制的助手；没有为通过测试改动生产行为。`app-context-r3` 同一过滤范围 11/11 通过。
- app 测试链接器提示 `__eh_frame section too large (max 16MB)`，可能影响异常处理性能；原始警告保留在各 app 日志中。测试仍退出 0，严格 Clippy 也退出 0；本切片未修改链接配置或掩盖警告。

## AKC-SR-001 修正与 v2 实证

独立审查指出：保存字段先经过 JSON 编码，包含引号或反斜杠的已知凭据会避开字面脱敏。API 发送阶段另有全凭据拒绝，但原默认 API 预览已暴露正文；两种本地适配器最终发送仅检查当前调用凭据，因此非当前配置的值可能经 stdin 送出。原审查由父任务保存于 `work/ai-knowledge-context-20261105/independent-review/source-review.md` 和 `.json`，其结论是源码证明及合成模型，不能冒充 Rust 复现。

作者选择在共享 `ContextDraft::prepare_with_limits` 返回前无条件执行既有嵌套 JSON 凭据检查，移除“仅非默认推理选项执行”的条件；扫描使用调用方及 RequestOptions 中的全部已知凭据。仍可还原的值返回既有 `AiError::CredentialInContext`，不允许形成预览或批准。保存原文及所有历史身份字段保持不变。新增生产写集仅为 `keelshell-ai/src/context.rs`；其测试及现有 `keelshell-ai/tests/local_agent_process.rs` 自托管测试入口也纳入 v2。

| v2 检查 | 实际结果 / 证据前缀 |
| --- | --- |
| 新 GPUI 回归在原生产实现执行 | `akc-sr001-negative-r1`：退出 101；84/84 组合均保留可还原的非当前凭据。`akc-sr001-positive-r1` 用同一测试源码在修正后退出 0 |
| 最终跨平台路径形式的同测试反例 | 测试仅补齐 Windows 所选目录路径处理后，`akc-sr001-negative-r2` 临时使用原 `ContextDraft`，仍退出 101、记录全部 84 个组合；控制器在 `finally` 中逐字恢复修正源码，回执 `akc-sr001-negative-r2-control.json` |
| 最终助手回归 | `cargo test --locked -p keelshell-app assistant:: -- --test-threads=1`：71 passed / 0 ignored，退出 0，`akc-sr001-app-regressions-r1`；包含与 negative-r2 字节相同的最终回归 |
| AI 核心与受控 HTTP 回归 | `cargo test --locked -p keelshell-ai --lib --test known_secrets_http --test provider_http --test request_options_http -- --test-threads=2`：110 单测 passed / 1 existing ignored，HTTP 三组 5 + 17 + 18 passed，合计 150 passed，退出 0，`akc-sr001-ai-regressions-r1` |
| 真实自有子进程 stdin | `cargo test --locked -p keelshell-ai --test local_agent_process -- --saved-knowledge-context-tests`：六次非当前转义凭据准备拒绝且未启动任何子进程；两个本地适配器各一次完整安全结构化记录的 stdin 解码相等，退出 0，`akc-sr001-owned-stdin-r1` |
| 严格 Clippy | `cargo clippy --locked -p keelshell-app -p keelshell-core -p keelshell-ai --all-targets -- -D warnings`：退出 0，`akc-sr001-clippy-r1` |
| 工作区格式检查 | `cargo fmt --all -- --check`：退出 0，`akc-sr001-fmt-check-r1` |

v2 普通测试为 **221 passed / 1 ignored**；不重复累加单独的首次正向运行，也不把自定义进程控制器的场景数混入普通 Rust 测试数。唯一 ignored 为既有 `owned_group_fixture_entrypoint`，由自有清理控制器调用。共享 target 再次按协调独占使用 `CARGO_BUILD_JOBS=2`，全部进程均已回收并交还父任务；未运行完整工作区门禁。

回归组合为：引号、反斜杠、两者混合 × title、summary、source.label、session_id × Chat Completions、Responses、Messages、Codex 隔离/所选目录、Claude Code 隔离/所选目录。每例先证明同一条目在其值尚未成为凭据时可准备，再为非当前配置加入该凭据并保留不同的当前调用值，断言准备拒绝、无预览/确认身份、不能启动发送。正向控制解码整条保存记录，两个本地适配器额外解码实际准备 stdin；条目等值检查确保来源 ID、时间、结果未被改写。失败日志只报告组合坐标，不打印凭据值。

最终 app、owned stdin、Clippy、格式检查输入清单 SHA-256 为 `a37d24d0e55eecdd7f1394d13b2157d3c82fb9c7a595c25b8f9fe6b2999b8364`，957 项；执行期间 `changed_inputs` 为空。AI 回归较早执行，其后只补 app 测试的 Windows 路径条件；AI 源码/测试一致。最终 negative-r2 与 app 成功回归的测试文件摘要均为 `37761165bf48d2284db3f91490e707ffaeda9ea9d168a8b4b19cf04b525d9644`。本记录与产品/ADR 在命令完成后更新；v2 交付清单另记最终文件摘要。

原 `ai-knowledge-context.patch` / `import-manifest.json` 和全部旧日志保持不变，原补丁摘要仍为 `e5d5f2af8549eb2917c8c51401f3183a44a197b86f1d3a47191a8371cbb37485`。v2 使用新的 `ai-knowledge-context-v2.patch` / `import-manifest-v2.json`。父任务需复核 v2，而不能将原 HOLD 自动视为已关闭。

## 实际覆盖范围

- Core：空选择、显式顺序、完整历史身份 JSON 往返、去重/移除/清空、编辑/删除/重复身份拒绝、数量与 UTF-8 序列化字节限制、失败原子性、已有保存入口的敏感内容检查。
- GPUI：真实知识选择按钮、三主题浅深和双语下保留原预览、380×580 确认栏可见、原来源不变且不产生 SSH/捕获事件。
- 请求准备：三种 API 协议与两种本地 CLI 预览、当前已配置的合成凭据脱敏、来源注入文字保持用户数据、未发送时无 HTTP/CLI job。
- 生命周期：发送前同步检查编辑/删除状态，未保存编辑稿拒绝发送；明确 reload 清空，即使条目相同；旧绘制按钮不添加改变后的条目或批准新审核；选择变化、清空、取消及原会话失效拒绝迟到回复。
- 预算与模式：含知识的请求不接受截断；Agent 不接受非空保存知识附件；历史资料不创造命令审核目标。
- 受控网络：真实 loopback HTTP 收到与预览字节相同的正文；选择移除取消已发送请求并拒绝迟到回复。仅使用合成资料与无认证本机监听，无真实供应商或来源凭据。

## 集成边界

唯一新增 Workspace 集成挂钩为配置恢复成功路径调用 `assistant.invalidate_knowledge_review(cx)`，使相同字节的权威恢复也清除选择；已向父任务报告。没有修改 `workspace/view.rs`、Files 或 Cargo 依赖。作者补丁包含未跟踪的新模块和上述产品 / ADR / 测试记录，不能只用普通 `git diff` 导入而遗漏新文件。

父任务集成到更新基线后，仍需完整 `scripts/check.py`、当前依赖策略、原生验证及新的非作者审查，之后才能决定提交。作者窄测试不替代这些门禁，也不将父任务 Files 修复的独立门禁结果算作本功能验收。

## 父任务组合整合

v2 已导入 `146e3c10f4a86d4211f7186cc621c3ebead4a5de`，14 个文件摘要
与作者冻结清单一致；该基线已经包含独立文件滚动轴修复。作者证据完整复制
至本地 `work/ai-knowledge-context-20261105/author/`，原 v1 与 HOLD 不变。
`import-readback-v2.json` 记录导入身份，父任务只继续补齐全局说明与验收记录。

新非作者再审为 **APPROVE_SCOPED_P1_FIX，开放 P0/P1/P2=0**。
reviewer 核对 v2 补丁、14 文件、同测试失败/通过对照；957 个对应输入
仅 `context.rs` 不同，确认 AKC-SR-001 在此源码范围关闭。
报告为 `independent-review/p1-recheck.{md,json}`；reviewer 未重跑 Cargo、
操作 UI 或执行供应商 CLI。这不等于新包、组合门禁或平台验收。

官方依赖 freshness 于 **2026-11-04T21:55:57Z** 开始，104 条声明/使用通过，
`dependency-freshness-r2.json` 保留结果；本功能没有改变依赖。
首次报告写入失败退出 1，回执保留，未算作成功。
Hallmark 实际尝试因 Rust 源码不在其 HTML/CSS 支持范围而退出 2，
记录为不适用；原生设计验收仍以新包真实截图为依据。
组合完整 `scripts/check.py` 退出 0、进程已回收：**2,056 ordinary Rust passed /
23 ignored**，其中 app **863 passed / 2 ignored**；**10 rustdoc**（一项仅编译）、
**Python 运行 128 项 / 1 skipped**，格式、严格 Clippy、依赖策略及额外进程
控制器通过。统计由原日志重新提取，不能将子集重复累加。
`full-gate-r1-summary.json` 保存计数，日志 SHA-256 为
`aa68ef6341d16b6f7bba2382f9128a359df211564952fc096702b9c6da5b5069`。

原生构建退出 0、进程已回收。门禁、构建和捕获后核对的 **494 个输入一致**，
见 `native/package-r1-gate-binding.json` 和 `native/r1-evidence-rehash.json`。
新开发包程序 SHA-256：
`60170e84039875e32e611562c0e14447536f798076d79934911c5c8cd47aa957`；
该包包含 v2 和既有 Files 修复，尚非公开 Release 或本机安装。

## 新包受控 macOS 原生观察

仅使用两条合成历史记录和本机无认证 HTTP 录制端；没有实时 SSH 会话、真实
供应商或供应商 CLI。原始种子、实际请求字节、20 对原始 JPEG/AX、摘要与退出
回执保存于 `native/`。所有原始图像均为 **2880×1866**；按本轮 2× 比例和
33pt 标题栏推算为 1440×900 客户区。第 15、16 次缩放尝试均无尺寸变化，
不能凭文件名称声称窄窗口或最小高度已验证。AX 请求为完整树，但静态文本未完整
暴露，因此不能仅凭 AX 缺项断言视觉不可见。

| 捕获 | 实际观察 |
| --- | --- |
| `01`–`04` | 霜白浅色英文；打开助手与知识库后确认 Alpha、Beta 两条种子，再逐条加入 Ask。助手显示 2/8 条、749 字节，以及各自来源、UTC 时间、完成/失败结果 |
| `05`–`06` | 填写合成问题、生成预览，目标是录制端实际 loopback 地址；确认栏可达，JSON 正文独立滚动。此时录制端连接数、POST 数均为 0 |
| `07`–`08` | 一次明确发送后收到 `Historical evidence only`。实际 POST 1961 字节，解码后两条完整历史记录按选择顺序与原种子逐字段相等；没有当前 SSH 目标，诊断计划入口禁用 |
| `09`–`10` | 再次生成两条记录预览，移除 Beta 后旧预览、旧回复和发送入口消失；只剩 Alpha，问题保留，POST 数仍为 1 |
| `11`–`14` | 切换中文、曜石深色、收起再展开、雾紫浅色，Alpha 和问题保留；录制端没有额外请求 |
| `15`–`16` | 两次缩放未成功，原始尺寸仍为 2880×1866；作为失败尝试保留 |
| `17`–`20` | 重新预览并明确发送 Alpha；录制端收到第二次请求且仍在等待响应时清空选择，忙碌/取消控件消失、选择变为 0。随后放行响应，服务端记录写出，原生界面未恢复迟到回复或发送入口 |

第一份实际请求摘要为
`dbb1e24ab5b75ba80c67fc7de1142f82538a7bf0bf92897e3cd838f584d7cc4d`；
第二份为
`b16c319363312b78b376d18a1f9b037d49b2f93cbec106e994733d41419a4da8`。
第二份解码后仅含原始 Alpha，不含 Beta；普通空终端 selection 不计为知识条数。
两次请求均无认证/Cookie 请求头，没有未预期请求。
这些核对证明实际原生请求的知识来源与内容；**未提取原生预览的完整文本**，
不能将由源码重建的 JSON 冒充原生预览字节相等。预览与实际字节相等由前述
受控 GPUI/HTTP 测试另证，范围不得混同。

应用通过系统关闭按钮结束；随后只观察应用清单，显示未运行。
应用与录制端均退出 0、已回收。历史知识与原种子逐字段保持一致，
外观/语言变化写入隔离配置。核对后清理自有隔离运行目录及放行标志；
原种子、结束状态副本、包和原始证据保留，见 `native/cleanup-readback.json`。

最终首轮组合复核为 **FIX_REQUIRED_DOCUMENTATION_AND_HALLMARK，
P0=0 / P1=0 / P2=2**，Hallmark finish 为 **fix**。reviewer 核对完整日志、
494 输入和 11 个导入 Rust 文件、请求与种子、原始截图和退出证据，
未重跑 Cargo 或操作 UI。原报告 `independent-review/final-review.{md,json}`
与 `hallmark-finish.{md,json}` 保留，不用后续通过覆盖。

- **AKC-FR-001**：ROADMAP 的 AI-04 行仍说组合检查/再审进行中，能力表仍把所有
  知识模型发送列为未实现。已同步源码再审、r1 组合门禁及限定原生结果，真实
  供应商和完整平台矩阵仍开放。
- **AKC-HF-001**：原生预览的路由、代理认证、请求头，以及脱敏状态有多重中点
  串联，违反本项目 Hallmark 原生约定。该文案在基线已存在，并非 v2 功能回归。
  修正仅调整 `assistant.rs` 三处展示字符串：三项元信息分行，中英文状态使用
  普通句子。请求字节、身份和审核/取消逻辑未变，不添加镜像文案的单元测试。

三字符串的旧源码、窄差异和摘要保存于本地
`assistant-before-finish.rs`、`hallmark-finish-fix.patch` 与
`finish-fix-source-binding.json`。`finish-fix-exact-delta.json` 核对两轮全部
494 个路径，仅 `assistant.rs` 有且只有上述三处替换。

## 展示修订后的 r2 门禁与原生

`full-gate-r2` 退出 0、已回收：**2,056 ordinary / 23 ignored**，含
app **863 / 2 ignored**；**10 rustdoc**、**Python 128 / 1 skipped**，
格式、严格 Clippy、依赖策略和额外进程控制器通过。日志 SHA-256 为
`f62405694f53c23ba158763b917d11f2670a8ae9b2046d39859aa671cc53a043`。
新构建退出 0、已回收，全部 **494 个门禁、构建、捕获后输入一致**。
r2 程序 SHA-256：
`c48ae4c76637a488de3e1818ee3c6fb6aa1c0097be0395165654875a783e3208`。

新的隔离原生运行使用霜白浅色、同两条合成知识和新 loopback 端口。
`native-r2/01`、`02` 分别捕获英文、中文预览：路由、代理认证、请求头
分成三行，脱敏状态为普通句子；本次四对原始 JPEG/AX 均为 2880×1866。
确认前连接数和 POST 均为 0，明确发送后仅一份 1961 字节请求，
两条历史记录按顺序与不可变种子逐字段相等，无认证/Cookie 请求头。
实际请求摘要仍为 `dbb1e24ab5b75ba80c67fc7de1142f82538a7bf0bf92897e3cd838f584d7cc4d`。
`03`、`04` 捕获回复及 `Historical evidence only`，无 SSH 审核目标、
诊断计划入口禁用。本轮不重复 r1 的迟到回复和主题矩阵，二者分别属于各自包。

应用以系统关闭按钮结束，之后应用清单显示未运行；应用和录制端均退出 0、
已回收。结束状态中的历史知识未变，自有 `data-r2` 已清理，原种子、
结束状态、包、原始截图/AX、请求和退出回执保留于 `native-r2/`。
`r2-native-verification.json`、`r2-evidence-rehash.json` 与
`cleanup-readback.json` 记录核对和清理。仍未提取原生完整预览字节，
本轮不包含窄窗口、实际供应商/CLI 或实时 SSH。

r2 限定非作者复核为 **APPROVE_SCOPED_CHANGE，开放 P0/P1/P2=0**，
两项 P2 均关闭；Hallmark 对本轮双语霜白预览与显式发送子集为 **ship**。
报告为 `independent-review/final-review-r2.{md,json}` 和
`hallmark-finish-r2.{md,json}`；reviewer 重新核对三处字符串、相关文档、
完整门禁及 494 输入，查看四张原始图像并核对请求/种子/退出证据，
没有重新运行 Cargo 或操作 UI。原首轮 HOLD/fix 文件保留。
上方 r1 包和原生证据只能代表其自己的程序身份。
精确提交绑定、推送及该提交 CI 另记交付回执，不沿用基线 CI。

## 未关闭

三平台桌面原生、完整主题×语言×尺寸矩阵、真实 API 和实际本地 CLI 发送、可访问性/IME、发布安装与完整应用能力均未由本切片关闭。外部磁盘写入在应用接纳新状态前不可见；没有增加隐式读取或跨进程监听。已发往模型服务的资料不能通过取消保证撤回。
