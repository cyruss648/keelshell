# 2026-10-06 本地智能体密钥环境引用

状态：初版 BLOCKED 及原作者结果保留。v2 秘密准入修复经新的非作者复审通过，环境引用和新增回归已在主副本整合，本次源码提交包含该改动；主副本 22 项专项、格式、依赖策略及 1281 普通完整组合门禁通过，新桌面入口、后继 CI 与发布仍待完成。准确范围见[探针准入记录](2026-10-06-local-agent-probe-admission.md)。基线 `6770da0f2d6ca8d42191f33036066a8cf1660da4`。没有发布标签或供应商模型请求。

## 范围

复用 `AiSecretRef::Environment`、原名称校验与持久化 schema；两个官方本地适配器新增来源/引用名称编辑、显式导入与临时接收方绑定。准备请求冻结导入值，发送不自动读取环境，不转交任意环境项，固定空临时 cwd 和 CLI 隔离参数保持。已观察值持续参与全配置及保留草稿秘密保护；环境引用不能授权隐式库保存。

## 已执行验证

- 实际安装 Codex `0.160.0` / Claude Code `2.1.285` 的 version/help：四个 owned 子过程直接 wait exit 0，10 秒等待/5 秒清理上界，私有工作区删除。Codex version 原 stdout 含缺少临时 CODEX_HOME 的 PATH aliases warning，仍以原文保留；没有把 help 或退出码用于隔离/模型/账户验收。
- 首次正式 GPUI 编译因 glob 导入的 test 属性递归拒绝，保留 101；改为显式测试 import。后续缺少组件 trait/import、测试误用主题函数的编译 101 均保留。没有调整全局 recursion limit。
- 首批四项正式 GPUI 行为通过；增加实际 pointer/输入/滚动条场景后，首次测试对未注册观察的根 ElementId 查询失败，保留原 101。改为核对明确的 580 逻辑像素窗口边界，五项正式 GPUI 通过，其中一个测试执行两 CLI × 两语言 × 三主题的 12 个最小窗口场景，实际拖动 scrollbar、点击来源/读取/应用、键入无效引用并确认拒绝保存。
- 这些 GPUI 场景不是操作系统原生窗口，不是像素/辅助技术验收。成功导入测试使用注入值/查找器，不修改测试进程全局环境。

- 第一次完整门禁在新增测试断言的 `expect_used` Clippy 项拒绝，原 560 输入前后相等，已明确限制该断言写法只在测试中。第二次严格 Clippy 通过，但新增合法模型编辑断言证实：来源编辑遗留的待清空标志会擦掉刚导入的值。原 560 输入前后相等、470 通过/1 失败完整保留；修正仅在显式清空完成时结束旧清空义务，防止晚到 Change 擦除新值。修正后五项 4 线程行为测试通过，并新增专门的排队事件/值与容量上界测试。

## 完整门禁

最终 `scripts/check.py` 实际 exit 0，用时 288.617 秒：1220 普通通过 / 11 ignored、8 doc、6 Python、格式、x.y 与严格整仓 Clippy。默认和额外 2 MiB controller 各 626 条记录、38 次原 TCP 调用；实测 future 各 5176 字节。560 个输入在完整门禁、额外 canary、构建、打包与检视 END 之间逐字节相等。原始 gate log 为 358819 字节，SHA-256 `325a08edd602d23e9ede61c04065364ac622fe902be0e292fee5344651789cbb`。

单独携带 `KEELSHELL_IMPORT_ONLY_KEY=unreviewed-env-fixture` 的 owned mock-child controller 实际 exit 0 / 10.960 秒，626 条记录和 38 次 TCP；两适配器的自有 child 断言引用变量未继承/未进入 argv，固定显式凭据仍保持。它证明 fixture 边界，不是供应商模型调用。

57 项 packaging 回归实际 exit 0 / 1.640 秒。macOS 双程序 debug 构建实际 exit 0 / 37.725 秒，使用显式 15.0 deployment target；打包实际 exit 0 / 0.314 秒，原生文件结构、Info.plist、双 Mach-O 架构/动态依赖/最低系统检视实际 exit 0 / 0.210 秒。应用与伴随程序实际字节保留，但未启动、签名、安装或发布；不能将此产物检查称为桌面或功能原生验收。

该记录与交接的作者状态在上述 END 后更新；全部 Rust、工具链、Cargo.lock、配置与其它输入保持。冻结包同时保留 END 时 560 份原输入正文和两个后续文档差量，未把更新后的正文冒充旧门禁字节。原始失败、日志、receipt、阶段记录、源输入与标准开发包保存在 ignored `work/`；等待新的非作者复核。

## 未验收

新本地入口的真实 CLI 模型/云账户、macOS GUI、Windows/Linux 原生、最小原生窗口与完整语言/主题矩阵未执行。任意自定义 cwd、任意其他环境转交、订阅/OAuth、受限 Agent 工作流继续 OPEN。帮助说明未证明所有现有/祖先项目、平台管理配置或供应商发现的排除边界；见[ADR0058](../../adr/0058-local-agent-key-environment-references.md)。对外 MCP 是独立服务入口，未新增第三方 MCP 客户端。
