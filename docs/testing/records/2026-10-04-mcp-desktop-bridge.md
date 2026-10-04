# 对外 MCP 桌面与真实 SSH 桥接验证 — 2026-10-04

范围：桌面默认关闭的会话/固定工具/选区/目录授权、准确活动handle与撤权、本机双向HMAC及方向AEAD、stdio字节bridge、真实SFTP有界读取、固定监控缓存、精确命令提案及原生人工审阅。设计见[ADR0039](../../adr/0039-authenticated-desktop-mcp-ssh-bridge.md)。不开发第三方MCP客户端，不操作真实客户账户或云模型。

## 检查状态

作者基于`3137712`修复后的整仓`python3 scripts/check.py`通过：934项普通测试、7项doctest、6项脚本unittest、格式、严格workspace Clippy和依赖x.y策略。app共301项全部通过。旧host同步cancel移到新policy安装之前，关闭轮换旧capability的短准入窗口。全新只读agent的独立复审已关闭，没有剩余可复现P1/P2；以下新增根工作区整合和原生记录覆盖当时尚待验证的macOS范围，不沿用旧门禁代表新源码。

- 新增7项GPUI/单元专项通过：默认关闭与精确一次SSH exec、常规完整UTF-8与路径/type/长度拒绝和SFTP cleanup、拒绝/到期/关闭实体撤权、中英双语900/1440窗口固定MCP按钮、明确选区快照与替换授权旧ID拒绝、固定monitor cache样本稳定且无新I/O、控制/方向字符可见审阅。
- 额外1项闭环GPUI通过：stdio adapter经双向HMAC/方向AEAD连接原桌面服务、initialize、枚举真实handle、创建提案、native按钮人工消费、真实owned TCP SSH精确多行中文命令单次exec、同scope查询Succeeded、stdio EOF正常释放；不写PTY。
- 新增2项审查回归通过：900×580中英各自四条超过30 KiB提案，实际wheel滚动到最后提案并只执行其精确字节；真实hold SSH执行后旋转scope标记Unknown，queued Success在UI入队后失去lease时也标Unknown并清空输出。合计10项新app专项。
- 最小窗口既有双语topbar回归通过：MCP入口与待审数量移至底部状态栏，中英900×580的既有动作保留完整。未削弱原几何断言。
- MCP完整crate按项目4线程门禁通过58普通+1doc：18 lib（13 IPC新增）、16 authority、10 IPC integration、9独立stdio、5 stream。严格crate Clippy、格式、依赖x.y策略通过。

真实owned SSH/SFTP测试使用新生成/固定fixture主机密钥与受控loopback协议服务器，不运行命令到客户主机；exec peer记录实际收到的字节并返回固定状态。GPUI test-support不代表系统GPU窗口、OS剪贴板、供应商CLI或Windows/Linux原生接受度。实际stdio binary子进程由IPC integration创建，环境临时能力只在内存交给owned child，不写日志/argv/profile。

成功证据保存在ignored `work/mcp-desktop-gpui-3.log`、`work/mcp-desktop-ipc-gpui-2.log`、`work/mcp-desktop-minimum-toolbar-2.log`、`work/ipc-full-four-threads.log`；IPC source/config和完整日志SHA保存在`work/ipc-transport-evidence.json`，全部本机证据不提交正文/凭据。

整仓初次成功证据为`work/mcp-desktop-full-gate-2.log`（932+7），审查修复后最终全量为`work/mcp-desktop-full-gate-review-fix.log`（934+7）。专项成功证据包括`work/mcp-desktop-review-race-first.log`及`work/mcp-desktop-long-review-first.log`。冻结源、所有失败/成功日志与根工作区复制收据在`work/mcp-desktop-freeze.json`、`work/mcp-desktop-freeze-review-fix.json`管理SHA-256；doc更新不得被当作新原生证明。

## 新独立审查与复审

全新agent只读复核冻结manifest `809c107205e76a0773eb2432ee6a1ad3a886df4b6bb358507e780977aabfee22`里的35个code/config SHA，全部匹配。发现P2：Success completion已入队、authority随后替换，再由UI消费时可能发布已撤权的成功。修复在exec await后与UI接收Completion时各自复核lease；无效输出不发布，状态Unknown；保留Running到本机owned completion收尾，以维持单次人工执行准入。独立重跑真实hold撤权与确定性的生产completion队列竞态回归均通过，该P2关闭。

长/多proposal的flex收缩属于审查疑点；四条超过30 KiB提案的900×580中英实际wheel和最后执行回归通过，未复现布局缺陷，没有据疑点预设改UI。SDK旧的“bridge未连接”说明已更正。

独立复核最终10项MCP/app专项、4项themes、strict app+MCP Clippy、MCP默认并发58普通+1doc全部通过。最终MCP binary SHA为`4e9bd026eb87e8f4f1af2859224579acce7cd1481d0e901c3eb01a724c0da1b9`；8并发×4批的真实stdio EOF探针共32个进程，发现/畸形请求路径均code0、stderr空、无尾部，EOF到exit最大1.18ms；初轮32个最大1.65ms也全部通过。readline/child.wait均保持原3秒界限，owned子进程逐个join。

原默认并发两项child.wait超时未在3次独立完整默认并发运行与64个真实进程探针重现，但其调度/资源/产品原因仍未证明。原失败`work/ipc-full-final.log`保留，不因后来成功宣称已归因修复。审查报告、SHA核对与完整成功日志保存在ignored `work/mcp-desktop-independent-review/REVIEW.md`及同目录；随后与所有实现失败/成功证据一起复制根工作区并逐文件SHA核对。该agent没有改产品源码、读取客户机器、云模型、原生剪贴板或CUA。

## 发现、修复与保留失败

- 初次app编译缺Host API接线、SFTP fixture导入祖先类型与scroll元素id，补全后编译通过；日志保留`work/mcp-desktop-check-initial.log`。
- 第一次SFTP测试把seed写入也计为越权写入：修正为记录seed后的baseline，实际后端读取没有新增写入；失败`work/mcp-desktop-gpui-first.log`保留。
- monitor cache测试首次fixture返回unsupported，补上owned固定Linux collector回应；旧临时connection重新授权后采用新connection UUID，正确typed refusal为Forbidden，修正测试预期，未改变产品权限；失败`work/mcp-desktop-gpui-2.log`保留。
- 新入口首次使900px英语顶栏超出视口，原有测试检测到；入口移至底部状态栏，原几何断言通过；失败`work/mcp-desktop-minimum-toolbar.log`保留。
- 初次stdio闭环测试误以为ActionState有PartialEq导致编译失败，改为matches；失败`work/mcp-desktop-ipc-gpui-first.log`保留。
- 首轮整仓Clippy检测重复载入fixture module与test helper expect；改为共享测试模块与typed helper Result，不新增lint豁免。失败`work/mcp-desktop-full-gate-first.log`保留。
- IPC实际stdio EOF发现SDK close只drop writer而不调用AsyncWrite::shutdown，导致末尾缺认证EOF；host在SDK清理后以250ms有界control送认证EOF，仍拒绝未认证截断、tamper和replay。原失败`work/ipc-full-second.log`保留（精确命名以manifest为准）。
- 一次默认并发MCP全量运行出现两个已有stdio child.wait 3秒超时，协议读取已通过。未证明调度/资源原因，未放宽期限；精确单项分别0.40s/0.29s复查通过，项目4线程全量58+1通过。失败`work/ipc-full-final.log`保留，独立审查须关注该边界。

## 根工作区整合门禁与 macOS 原生闭环

已把独立受审patch整合到`3fe0c98`的本地CLI源码上。原作者patch SHA256为`1511fcf01ff9a323f083a610fe7b5e646f95f634f4adfffb28dd2af38fd99cb1`；37项代码/配置/文档与冻结清单逐项相同，Cargo.lock合并CLI依赖而不同。根工作区后来只纠正`DesktopBackend`旧rustdoc及新增当前状态/接入指南。完整门禁`python3 scripts/check.py`成功：963普通+8doc+6脚本，fmt、严格全workspace/all-target Clippy和x.y策略通过。10个普通忽略项为两项显式供应商CLI及八项外部OpenSSH，不混入963。开发构建同时包含GUI与MCP。日志为ignored `work/mcp-desktop-integrated-gate-20261004.log`和`work/mcp-desktop-integrated-build-20261004.log`。

当前原生检验的GUI SHA256为`1b048aa85c1d2d3df92dac08732cbac9296957415f87f77ec046ff8e3872fe84`，companion为`1fe7a4b658efbd0ad5ae9373854237b5c58916a4f43170cad0df70fba00fa9f2`。原打包脚本当时只有GUI，检验包明确额外复制同目录的companion；`bundle-receipt.json`标明这是业务联调包，不是正式双程序发布打包证明。

自有配置目录只有一项127.0.0.1 SSH测试连接，中文/深色起点，未设置AI供应商。SSH/SFTP fixture使用独立临时文件系统；PTY只echo，不执行本地OS shell。固定exec `keelshell-batch-fixture`返回UTF-8 stdout及控制字符stderr，`keelshell-batch-hold`等待channel关闭。它不代表任意生产命令、真实客户主机或独立OpenSSH业务验收。

实际macOS GPUI窗口与OS剪贴板通过CUA操作，自有外部Python stdio客户端通过临时启动配置连接真正的companion：

- 核对测试主机指纹并完成密码认证；鼠标选择并明确捕获37字节终端片段，逐项启用六项工具、授权fixture目录，监控工具不勾选。复制配置进入自有掩码输入GUI，经内存stdin交给协议客户端；能力未写文件、argv或日志。
- stdio initialize协商`2025-11-25`并发现七项工具。只列出一项精确授权活动会话，读取精确片段、目录七项及含中文的70字节welcome文件；未授予的监控调用返回`FORBIDDEN`。
- 第一个命令提案在人工操作前明确为`pending_review`。应用展示原命令、准确目标及摘要，点击确认后真实SSH固定exec返回成功；客户端读到`succeeded`，应用显示中文stdout与可见化stderr。
- 第二个提案人工拒绝，客户端读到`rejected`。第三个hold提案明确执行后客户端读到`running`，点击“关闭并撤销全部授权”，旧stdio连接关闭；应用显示“远端结果未知”且不继续发布输出。adapter因授权连接关闭exit1是此路径预期，客户端全部检查通过。
- 浅色/英文切换后，既有提案状态保留。重新仅授予ListSessions并正常退出，再启动相同隔离配置：语言/外观保留，MCP默认关闭、无活动授权/片段/提案，复制禁用。

Native截图作为CUA会话证据保留，没有保存含启动能力的截图文件。自有辅助GUI最初Swift MainActor编译错误、首次空粘贴断言失败及修复日志全部保留；它们属于检验工具，未据此改产品。系统Terminal界面不可访问后使用了自有标准AppKit输入窗口，没有绕过界面工具限制读取剪贴板。

ignored `work/mcp-desktop-native-20261004/`管理client/bundle/restart/source/cleanup收据及成功/失败日志。最终controller、app、SSH fixture、client、adapter、restart、辅助GUI均不存在，监听已拒绝连接，fixture临时根删除；state不含MCP能力值、选区或提案。所有62份原作者/独立审查证据复制并逐文件SHA核对，未删除原默认并发超时失败。

接入文档的新增独立只读复核发现两处措辞易误解：捕获新片段只是草稿，重新授权才替换已分享片段；能力可供多个客户端连接，持续至授权变更/撤销/退出，并非单连接消费。指南已明确，中文按钮改为“复制临时启动配置”，双语说明显示实际寿命；上面的原生SHA仍对应修改文案之前的构建。修改按钮/说明后完整963普通+8doc+6脚本门禁通过，日志`work/mcp-desktop-copy-scope-final-gate-20261004.log`；最终将“关闭”细化为“撤销”以区别关闭面板，再通过fmt/policy/strict workspace Clippy和8项MCP/app专项，日志`work/mcp-desktop-final-wording-check-20261004.log`。新增只读复核没有修改代码或把日志核对表述为独立原生操作。

## 尚未验收的边界

实际Codex/Claude Code MCP互通、最终双程序发布包与六目标Release、Windows/Linux原生窗口及其文件占用/重启仍未验收。当前macOS自有外部客户端和受控SSH/SFTP闭环不代替这些证据。[接入指南](../../product/EXTERNAL_MCP.md)的官方格式已核对，仅含占位符，不代表已改用户全局客户端配置。

运行能力持有证明不是OS用户/可执行身份认证；方案不提供forward secrecy。SFTP v3检查是观察，没有文件锁/事务TOCTOU承诺。撤权/取消阻止后续授权与本机future，但不能证明远端已发命令停止，超时/撤权/未知退出标记OutcomeUnknown。临时授权与secret不进入持久profile，SSH密码/密钥/主密码没有MCP通道。
