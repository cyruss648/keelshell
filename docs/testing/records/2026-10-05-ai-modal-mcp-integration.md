# Ask 预算、弹窗隔离与 MCP EOF 整合 — 2026-10-05

状态：整合工程门禁、三个增量的非作者独立复核以及限定 macOS 原生界面检查通过。精确 `a19d0c1` 的新 CI 已结束 failure：macOS/Windows 成功，Linux 文件布局 GPUI 条件等待超时，未执行到 MCP/OpenSSH，见[三平台记录](2026-10-05-integration-ci.md)。其他平台原生窗口与完整发布仍须分别确认。MCP 方向仅为 KeelShell 向外部智能体提供服务端；应用内 API / CLI Ask 是独立入口。

## 冻结源码与工程门禁

整合基于 `40d092ca61c4c229acb4a004d2852dd858e86aed`。最终候选 C 的 469 个源码、工程和文档输入在完整工程检查、显式 macOS 构建及 stage/inspect 前后保持逐字节 SHA 不变。审查清单 SHA 为 `8f59fb6398ef80b11a4a30e0be6bf63123f6714b2618dbd437d545671f81d813`；根门禁清单用另一 JSON 编排记录同一输入，SHA 为 `a8264a6652ca77a1d6fd06e9cc6d109ee0b9375c75a5a0f921cf9bee994bcfdb`。本记录等后续状态文档不属于这次原始清单，生产源码保持冻结。

`python3 scripts/check.py` 退出 0，255.015 秒：**1053 普通、8 文档、6 脚本测试**，格式、严格 workspace/all-targets Clippy、直接 registry 依赖 x.y 策略全部通过。376 项应用测试已包含在普通总数内。11 项沿原政策忽略（2 项供应商 CLI、9 项系统 OpenSSH），不能计为执行。默认与显式 2 MiB 完整 CLI 控制器均通过，future 为 4624 字节，未调用供应商或模型。完整日志 SHA `ea9f65f449fac7cfae99235d67acef35d8d23a52998a88476d16e59854fa8904`，TMP 空。57 项打包回归在本轮早期范围另行通过；打包源码此后未改，不重复相加或冒充原生安装。

独立复核分别绑定各自输入，不能互相替代：

| 范围 | 结论与证据 |
| --- | --- |
| [本地 Ask 预算](2026-10-05-local-agent-limits.md) | 原专项 197、3 私有 GPUI/vault、两个适配器的 8 组实际自有进程场景通过；作者 34 / 复核 50 份证明由根复制核验 |
| [MCP 初始化 EOF](2026-10-05-mcp-startup-eof.md) | 独立 MCP 68 普通 / 1 文档、5 私有场景通过；作者 10 / 复核 23 份证明复制核验 |
| [弹窗输入生命周期](2026-10-05-modal-isolation.md) | 最终 C 的 11 原模态 / 8 私有场景、严格 app/all-targets Clippy 通过，76 个中英/明暗完成帧场景；28 份证明复制核验，无剩余限定范围 P1/P2 |

原弹窗 A 和 B 范围仍为 FAIL，41 / 33 份完整证明保持原字节。原 Enter、Space 与重复 mouse-up 的断言和事件顺序原样重放后只在 C 通过。早期完整套件成功不能代替这次修复验收。额外同类 HostApproval/Login 任务身份诊断未作为新行为结论或优先级问题；更广泛认证和原生辅助技术保持单独验收。

## 新 macOS 开发包

显式 `MACOSX_DEPLOYMENT_TARGET=15.0` 的 GUI / MCP `aarch64-apple-darwin --locked` 构建退出 0，353.569 秒；两项 Mach-O 均为 minos 15.0，Info.plist、链接和标准包清单检查通过。开发包 `build.commit = null`，以源码摘要绑定；不冒充正式签名发布。构建日志 SHA `16469521c2b2d25aadda828a52e5db785d7b1f0d6c23da476381e50aa8700e29`。GUI 二进制 SHA `15bfd3c568fbc0d035eb137ff3fa5626422eef415a703338fcd2e9a0e81da5b6`。

根使用 CUA 操作新开发包，数据、HOME 和 TMP 均隔离；只连接一个自有 SSH/SFTP 夹具，没有覆盖已安装应用、读取客户配置、启动 CLI 或发送模型请求。初始设置为中文 / System，显示深色；没有测试实际系统主题变更通知。

- 原生登录、AI 设置、管理器、连接草稿和 MCP 面板打开时，当前辅助树只保留活动弹窗；背景终端/文件动作、父层管理器节点移除。该证明是当前 AX 树观察，不是激活已经移除的旧 AX 对象。
- 两种 CLI 后端显示默认 120 秒 / 1024 KiB 回答 / 2048 KiB 累计输出。无效 0 秒阻止 Apply 并显示原因；切换配置、中英/明暗与 Cmd-T/Cmd-J 均保持同一草稿和活动层。
- Codex metadata 配置保存为 9 秒 / 1 KiB / 2 KiB，Claude Code metadata 配置为 12 秒 / 2 KiB / 4 KiB；实际状态文件投影和重开原生表单均精确读回。这仅验证设置，不是供应商问答。
- 拖动窗口后得到 1800×1224 Retina 位图，扣除约 32pt 标题栏的内容区约为 900×580；这是读图判断，不是仪器测得 viewport。紧凑英文浅色 AI 表单能滚动至所有预算及操作，Apply/Cancel 固定；中文深色连接草稿在偏好切换后保留。
- 键盘从管理器搜索框 Shift-Tab 四次后 Enter 打开“新建连接”，Escape 返回后 Space 再次打开同一入口，验证了真实键盘触发的焦点返回。鼠标触发的另一场景返回原搜索焦点，两者分别记录。逐层 Escape 回到原 SSH。
- 同一 SSH 终端显示 `native-modal-before` 和 `native-modal-after` 两条回显，SFTP 目录仍保留。夹具明确只回显，不执行 shell 命令；监控仅显示 server rejected exec，不能声称新监控/传输/真实命令验收。
- MCP 面板显示“对外 MCP 授权与审阅”、七项固定能力、默认关闭、0 提案；没有授权、复制启动配置或调用工具，不产生新的供应商 MCP 通过证据。

原生诊断也保留：最初直接点击被裁剪输入未改变回答/输出；Tab 输入和滚动后可见鼠标输入均读回。CUA `down` 在顶部不移动，相同坐标 `up` 移至底部，反向对照解除该疑问；没有据此修改产品或将未证实原因分类为缺陷。原生 AX 文本、截图由 CUA 在会话中返回，ignored journal 是观察投影，不声称保存了原始截图文件。

## 清理与尚未证明

自有 GUI、SSH 和控制器均已结束，按进程出生身份核验不存在，唯一测试监听端口关闭，私有数据和夹具根删除；控制器无失败。成功/失败日志、输入清单、原生观察及 readback 投影保存在 ignored `work/ai-modal-mcp-integration-20261005/`，不作为用户数据发布。

Windows/Linux 原生 GUI、VoiceOver/Narrator/AT-SPI、滞留旧原生 AX 对象激活、IME、硬件持续按键、真实 OS 主题通知、新预算下供应商问答、其他授权组合、最终六目标发布签名与实际安装更新尚未证明。Codex 授权 MCP 的既有两轮失败仍见[独立记录](2026-10-05-codex-authorized-mcp-failure.md)，不会因本次工程检查而变为成功。本次新 CI 的 Linux 文件场景失败保留，后续候选及新流水线必须单独绑定，不能将旧成功或单次重跑作为修复验收。
