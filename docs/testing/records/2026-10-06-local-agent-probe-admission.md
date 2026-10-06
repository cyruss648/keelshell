# 2026-10-06 本地 CLI 探针秘密元数据准入 v2

状态：修复作者验证及新的非作者独立复审完成；27 个环境引用/准入/回归路径已在主副本整合，本次源码提交包含该改动。主副本专项及完整组合门禁通过；新桌面验收、后继提交 CI 和发布仍待完成。基线 `6770da0f2d6ca8d42191f33036066a8cf1660da4`。本阶段作者曾担任初版独立评审者，本记录不提供对自身修复的独立结论。

## 原失败与修复范围

初版独立正式 GPUI 反例在自有 arm64 Mach-O 子进程入口记录真实 argv：Codex 地址及 Claude Code 执行路径包含已观察秘密，Apply/Ask 已拒绝，但手动检查仍创建任务并实际运行 3/2 个探针，测试 exit 101。根已完整读回保存原 675 作者 payload 和 1218 BLOCKED payload；此次只读保护，原 stdout/stderr、阶段源码与报告不改。

生产修复在 `start_local_probe` 的 `sync_editor` 后增加全目录秘密元数据校验和通用拒绝反馈，早于任何 probe config/job/cwd/子进程创建。既有完整秘密遍历提取为 `validate_catalog_secrets`，也检查结构无效的草稿；Apply/持久化继续先执行原结构校验，再调用相同秘密遍历。探针沿用原选中 transport 及允许空模型的能力检查规则。导入引用类型、绑定和容量、固定空 cwd/受控环境、API 请求参数、Ask 人工审核及 MCP 对外方向保持。

## 已执行的作者专项验证

- 首批 6 项新增正式 GPUI 通过；提取共享准入后最终 7 项通过 / 16.418 秒，另外增加两 CLI × 3 类结构无效隐藏草稿共 6 案例。其余覆盖为：两适配器的 5 类当前编辑字段共 10 案例、13 类隐藏配置字段共 26 案例、数值存储/线协议采样 32 案例、编辑还原与查找/绑定失败保留值、三类秘密集合超限 6 案例、每配置 16 个值及读取容量拒绝后逐个保护 32 案例。所有拒绝均在创建 job 前，反馈不包含秘密。
- 原封存 argv 反例正文保持，显式提供自有 native fixture 后实际 exit 0 / 0.507 秒；两 CLI 均 `job_scheduled=false`、实际 fixture 调用数 0、`secret_in_argv=false`。
- 合法双 CLI native fixture 专项实际 exit 0 / 1.011 秒，Codex 3/Claude Code 2 个实际能力子过程，固定能力检查成功。argv 不含导入值或引用名；没有模型请求。此测试普通运行标记 ignored，另用明确 fixture 路径与 `--ignored` 执行，不能把普通 ignored 计作执行通过。
- 共享准入修正后，原空模型测试实际 exit 0 / 1.881 秒，13 项既有 Apply/持久化/秘密元数据边界实际 exit 0 / 0.627 秒。原封存 P2 再次以实际 native fixture 执行，exit 0 / 0.779 秒，仍两 CLI 各 0 job/0 invocation。合法两 CLI × 有/无模型共四场景实际 exit 0 / 1.828 秒，各场景 3/3/2/2 次能力子过程；全部保持原能力准入，不请求模型。
- 两次格式化修改源码的命令均直接 exit 0；有界 wrapper 因前后输入发生预期格式化变化返回 125。保留格式化前后正文和空原始 streams，这不是测试或产品失败。

完整门禁首轮实际 exit 1 / 127.014 秒，564 输入前后相等，611 普通通过 / 1 失败 / 3 ignored 后停止，doc 与额外 2 MiB 阶段未到达。直接复用全目录结构校验提前拒绝合法空模型探针，旧 `local_probe_failure_uses_worker_and_cannot_fall_back_to_http` 失败。原断言未修改；提取完整秘密遍历，保留保存/持久化结构校验后重新验证。所有当时输入正文和原始 streams 保留。首次派生摘要遗漏 FAILED 组的通过计数，原派生文件及更正说明也保留，原始 streams 是权威证据。

每次专项使用私有 TMPDIR、当前工作树独立 target、离线依赖和有界等待；实际 direct wait、原进程组不存在、私有目录空且已删除、输入前后相等均有 receipts。真实 native fixture 入口 PID/argv保留；未采集内核出生，完整逃逸后代 census 为 UNKNOWN，不从数字 PID 不存在推导完整普查。

## 完整门禁与产物

第二次完整 `scripts/check.py` 实际 exit 0 / 327.584 秒：1236 普通通过 / 12 ignored、8 doc、6 Python，fmt/x.y 和严格 workspace all-targets Clippy 全部通过。默认和额外 2 MiB controller 各 626 条记录、38 次原 TCP 的 begin/returned，12 connected/26 refused；future 各 5176 字节。普通 ignored 包括另行实际执行的自有 native 合法探针测试，不能把它重复计作普通通过。

原 stdout 122448 字节，SHA-256 `ac9da3c212a6fe16956f6621e01db2d3aad3e8d3b666c5634acd42f09d46545f`；原 stderr 238497 字节，SHA-256 `6244399301c0f4c6b919a9c94a2214275e3b9448a15c0e870252299837dd6115`。两流分别原样保存，不把拼接文本冒充实际交错顺序。

携带未审核环境 canary 的自有 mock-child 默认/2 MiB 两次实际 exit 0 / 11.515 与 13.035 秒；均完成 626/38/future5176，fixture 断言引用项不继承、不进 argv，原固定认证值保持。最终 57 项 packaging 单元回归实际 exit 0 / 1.592 秒。此次没有新 app/companion 构建或 GUI 启动；保留的 Mach-O 仅自有能力探针 fixture，不是应用包或供应商验收。

564 份输入在最终门禁、最终 packaging 与两次 canary 的前后逐字节相等。上述结束后仅本记录和路线图补记完成状态；冻结包另保存两个文档末次差量与实际门禁输入正文，所有 Rust、Cargo.lock、工具链及其余输入保持原字节，不以更新后的文档冒充当时门禁正文。该段记录修复作者结束时点；随后新的非作者结果见下文，原阶段正文与失败保持。

## 新的非作者复审与主副本整合

新评审者未参与产品修复，重新绑定 565 个候选工程输入，并保留原 argv 反例正文。新增正式 GPUI 回归覆盖两 CLI 的非环境凭据、结构无效隐藏草稿及秘密集合超限；有/无模型且含无秘密隐藏草稿的合法自有 native fixture 也显式执行，普通 ignored 未计作通过。原反例实测两 CLI 各 0 job/0 invocation，合法检查保持 Codex 3/Claude Code 2 个子过程，不调用模型。

独立完整 `scripts/check.py` 实际 exit 0 / 477.702 秒：1238 普通通过 / 13 ignored、8 doc、6 Python、fmt/x.y 与严格 workspace all-targets Clippy。默认/额外 2 MiB 控制器各 626 阶段、38 原 TCP、future 5176 字节。额外环境 canary 与 57 项 packaging 回归通过。独立结论为代码、GPUI 与自有 native fixture 限定范围通过，无新 P1/P2；未启动桌面应用或真实供应商新入口。621 份冻结 payload、原 streams、完整源码与 receipts 已在主副本 ignored 目录完整读回保存。

主副本顺序整合作者 26 个完整差量路径和独立新增两路径回归，共 27 个唯一候选路径；所有合入字节与独立冻结源码核对相等，其余并行传输候选保持。另执行 `cargo test -p keelshell-app --locked ai_settings::local_environment -- --test-threads=4`，实际 exit 0 / 93.074 秒，22 通过 / 2 ignored / 475 filtered；格式与实际依赖策略检查通过。580 个输入前后相等。这是专项结果，不是完整组合、供应商或桌面验收。一次根误选不存在的策略 helper 已保留为工具错误，改用仓库 `scripts/check.py --policy-only` 后实际通过。

主副本随后完整组合 `scripts/check.py` 实际 exit 0 / 569.730 秒：1281 普通通过 / 14 ignored、8 doc、6 Python、fmt/x.y 和严格 all-targets Clippy；默认及 2 MiB 控制器各 626 条有序记录、38 次 TCP 返回，实际 future 各 5176 字节。581 个完整工程输入前后逐字节相等。原始合并 stream 367162 字节，SHA-256 `897cf21d19429b633a0ae4d02df56b62d366acd7706c605ef877bbc69eab5443`；这是实际同一输出文件，不将独立两流拼接成时间顺序。owned child 明确 wait/reap、原 numeric process group 不存在，空私有 TMPDIR 删除；未采集内核出生或完整逃逸后代普查。

该组合检查同时包含尚未验收的并行传输工作区候选。此次只提交已经独立复核的本地 CLI 路径及相应文档，传输原生失败状态保持。精确 feature 提交的远程 CI 必须另行核验，不沿用此前 `6770da0` 的成功。

以上公开状态更新发生在各次输入冻结和专项结束之后；当时文档正文与新差量另行保留，不把更新后的文档用于追认旧源码结果。

## 未验收范围

这些证据是代码、正式 GPUI 与自有原生 fixture，不是 macOS 桌面、真实供应商新入口、云模型/账户、Windows/Linux 原生或客户 SSH 验收。语言/主题/最小原生窗口、任意 cwd/环境、订阅登录与 Agent 工作流仍 OPEN；MCP 保持 KeelShell 对外服务，没有第三方客户端。本阶段证据保存于 ignored `work/local-agent-probe-admission-repair-v2-20261006`，完整源码、失败、原始 streams、receipts 和 artifact bytes 将随最终 manifest 冻结。
