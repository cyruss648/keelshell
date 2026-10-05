# 2026-10-06 AI 命令审核目标候选

基线为 `2f7ca1ee4a3abc248bc76e30eac1911110b5c76f`，隔离分支 `feature/ai-command-review-target`。本切片仅修改助手的捕获目标可用性、目标与原因呈现、普通/诊断人工提案入口和专用回归；不修改 ProviderConfig 装配、核心/provider 推理参数、Workspace 生产订阅者、HANDOFF 或 ROADMAP。

作者验证已完成，以下均为此隔离工作树的实际执行结果。原日志/退出码/输入前后绑定、失败源码快照和缓存复制收据保留于 ignored `work/ai-command-review-target-author-20261006`；最终候选及全部原证据封包为 `work/ai-command-review-target-frozen-20261006`，旁有同名 `.tar.gz`。不沿用旧源码 CI 或原生运行的通过结论。

| 执行 | 实际结果 | 耗时 |
| --- | --- | --- |
| 首次新增专项 `command_review_target` | exit 101；新测试缺少 `InteractiveElement` 导入，未运行测试 | 133.770s |
| 补充导入后的同专项 | exit 101；1 passed / 4 failed，Kit Button 的 `snapshot.disabled()` 为 `None` | 21.094s |
| 改用实际点击/焦点/事件观测后的同专项 | exit 101；3 passed / 2 failed，测试在 GPUI 窗口更新闭包内过早读取延后交付的 Entity event | 21.030s |
| 在事件循环交付后读取的最终同专项 | exit 0；5 passed / 0 failed | 18.993s |
| 既有 `local_progress` 专项 | exit 0；5 passed / 0 failed，含 8 个中英/明暗/展开组合 | 0.706s |
| 原 `python3 scripts/check.py` | exit 0；策略、6 个脚本测试、fmt、严格 Clippy、1184 个普通测试与 8 个文档测试通过，11 项 ignored；另 2 MiB 自有进程控制器返回成功 | 407.837s |

前三次失败保留原日志、原退出码及各次全部 264 个工程输入的源码快照；没有覆盖收据或把失败改记通过。Kit 0.7 的 Button 实现省略禁用入口的焦点/点击处理，但未提供 `aria-disabled` 元数据，所以 `None` 不能证明按钮启用或禁用。最终新增测试采用真实 GPUI pointer 派发、点击后无焦点、Enter/Space 后无回调副作用/无提案，再单独直调防守守卫；有效目标逐次人工点击后核对精确命令、原会话和事件数量。未补造元数据或修改依赖库。

专项还覆盖空白/部分目标的中英文邻近可见/ARIA 原因、陈旧回复及诊断计划不能改绑、重新捕获和迟到结果丢弃。Workspace 用两个确定性终端实体实际点击捕获/切换/审核；另一活动会话拒绝保留原草稿，回到原会话后手动填入精确命令，两条传输均无自动 `Write`。长回复测试验证 900×580 / 320px 助手列下的 16 个中英/明暗/目标可用性/进度展开组合，邻近目标及审核按钮完整位于滚动区内，固定进度栏可见。

完整门禁日志为 355505 字节，SHA-256 `01e64c1c4bad274865cff88fd5dbcac86b350d31896d2a7a0c2ce948742858fe`。默认及 2 MiB 控制器各记录 626 个阶段，`controller/end` 分别为 10.657534s / 10.674783s；它们使用自有夹具子进程，未调用供应商 CLI 或模型。完整门禁的 264 个工程输入前后逐 bytes/hash 相等。所有作者包装命令的原 leader 已回收、原进程组已消失、私有 TMP 已删除；该基本包装器证明范围是原进程组，并非完整内核 birth 后代 census。格式化产生的输入变化是显式编辑步骤，不算门禁输入相等。

本轮不启动原生 GUI、供应商 CLI、真实模型或客户主机；GPUI 测试平台及确定性传输实体仅用于生产控件/回调/目标路由/无写入回归。独立非作者复审与新的原生验收仍待完成。旧 Windows 控制器总截止超时以及更早的 MCP 2 秒失败原因保持 UNKNOWN，本修复不改变期限或宣称解决它们。


## 新非作者复核与主线整合准备

新 reviewer 结论为 `PASS_SCOPE`，无 P1/P2。独立格式、依赖策略和严格整仓 all-targets Clippy 实际通过；5 项候选 GPUI、5 项原 local_progress 及 5 项新增反例加 1 项既有匹配测试均实际通过。新增反例覆盖 NUL/Unicode 空白/无回复ownership、busy旧焦点、host-only陈旧、诊断错目标及生产suspend后的新terminal identity；有效 Enter/Space 各只发出一笔原会话事件，busy的旧焦点没有事件且status不变。五轮审查者探针失败完整保留，不记为候选缺陷或通过。

候选八文件及264工程输入始终与作者冻结一致。独立复核通过生产控件派发；最终键盘正例以有界 `Window::focus_next` 取得焦点后派发真实按键，不声称多行问题框的 Tab 导航或原生 AX 已通过。插入时检查 active terminal EntityId，Run 检查目标/传输可用性，重连检查 route/trust；三个边界各自保持，不能称每次插入或 Run 都重新审计路线。

作者861payload/862tar与reviewer630payload/631tar的全部字节已经根完整读回并保存到 ignored `work/ai-command-target-main-integration-20261006/`。独立报告 MANIFEST SHA-256 为 `586b4dc2a690bc04b7b0005a2593e9ca4f86ff4af5dc57ee607bfeb15341b3b3`。主线以Windows预算修正5b6b7b4为基线精确整合8文件；唯一根差量是按reviewer建议将测试注释 `real native` 改为 `real GPUI pointer/key`，没有可执行行为改变。主线完整门禁、新标准macOS包及原生控件结果随后按实际追加；旧源码通过不验收此整合。

## 主线组合门禁与新 macOS 原生窗口

主线在5b6b7b4预算修正之上整合本切片八文件，唯一源代码差量为测试注释精度更正。根完整 `scripts/check.py` 实际 exit0，372.033s；1184普通、11ignored、8doc、6Python，格式、x.y及严格整仓all-targets Clippy通过。默认/2MiB控制器各626记录，结束10.710639/10.999742s。542个可见仓库文件在门禁前后完全相等，并保持至新构建和原生窗口结束。原始日志354775bytes，SHA-256 `7848dac09a450b450577f9fc6203ee150b557c79b3606312e1faae6ddad36c1b`。根统计器首次漏算带compile后缀的rustdoc，原解析失败单独保留；修正统计器得到8项，不改变原通过门禁日志或测试。

新标准app/MCP开发构建11.502s exit0，夹具构建0.540s exit0；打包、macOS实际结构检查与57项包装回归通过，均未安装覆盖已有应用。两项程序为arm64 Mach-O；实物app最低15.0、MCP最低11.0，标准bundle最低15.0；打包前后app SHA `ef7b5c379a26a7cb9cb38a1c79ab7d06740f445acc8ab0991dfdae0bc9162cf8`、MCP SHA `2963d9bb7023aa476ecfab3f88638043afc4c4e02c6824e1baa13606c5f32503`一致。shell第一次未引用unittest glob在wrapper启动前退出1，原失败保持；引用后57项实际通过。

新原生窗口使用独立状态目录、自有回环SSH/SFTP及无认证的自有回环HTTP响应夹具。中文默认System在该系统呈深色：未附上下文的回复显示明确原因，禁用入口点击后没有命令值；后来连接SSH仍未把旧回复改绑。主动捕获124字节终端屏幕后，第二次明确发送得到绑定主机与原会话的新回复。中文浅色及随后独立切换的英文浅色显示完整目标；人工点击英文审阅入口，根当时的实际工具AX输出与保存的像素显示精确 `printf keelshell-target-check`，随后New command清空；保存的插入AX文件仅是无变化diff，独立复核不能从该文件回读插入Value，完整AX只保留于清空之后，未点击Run且终端没有新增回显。HTTP记录仅两次Ask POST。没有供应商CLI、云模型、客户服务器或任意OS shell执行验收。

原生证据24payload保留ignored `work/ai-command-target-native-20261006`，含四张实际截图、控件树、源码/二进制哈希与清理回执。初次Chinese typeText未观测到输入，paste才建立实际值；首次批量主题/语言点击仅确认Light，随后单独语言操作才确认EN。离屏AX确认及数次wheel尝试没有取得滚动结果，最终通过按钮可见但裁切部分的坐标点击发送；不称请求预览滚动已通过。目标说明在屏幕可见，但实际完整AX树没有这些静态文字，Kit禁用元数据问题与屏幕阅读器保持开放。当前会话标签包含内部EntityId，可读会话标签仍待优化。最小逻辑窗口、原生Tab/键盘、完整主题/语言矩阵及其它平台未关闭。

controller终态0；app -15、SSH夹具0明确wait/reap。根另核对三个已观察PID/birth身份不再存在及三个自有端口关闭，HTTP线程结束、私有目录删除，六个构建/门禁wrapper的空TMP在检查为空后删除。不是完整内核后代普查。新非作者原生证据复核结论随后追加，不以这些限定事实关闭完整产品验收。


## 新非作者原生证据终态复核

新复核结论为 `PASS_LIMITED_NATIVE_SCOPE`，无新P1/P2。审查者完整读回24份原生payload、门禁/构建/包装原始收据与日志，查看全部四张实际截图，并独立核对源文件、五项二进制绑定、Mach-O最低版本及三个已记录出生身份已停止。复核没有重启GUI、供应商、模型或夹具，也未修改候选源码；不是第二次原生运行。

独立复核确认上述未绑定中文提示、捕获后的中文/英文浅色目标保留、人工插入的像素及清空后完整AX状态。保存的插入AX仅152字节无变化diff，不能独立回读精确Value；静态目标文字的AX、英文未绑定原生、Tab/键盘、最小窗口、预览滚动和全主题/语言矩阵仍开放。三个已记录birth和端口关闭的证明不能扩展为完整内核后代普查。

23份新复核payload及24个tar常规文件已由根完整读回保存至ignored `work/ai-command-target-main-integration-20261006/native-review-proof`。新MANIFEST SHA-256为 `c0a46e1a440bb819e151f17d3cf4e36ae428a214d023402d7d953915abad7f0b`，tar SHA-256为 `b82afa6b18a6f36e545183369842cff2b992212bbdc8c36404a31e279f41a973`。根封包复制器首次误把列表MANIFEST按字典读取，在复制payload前退出1；失败单独保留，按实际schema读取后全部通过，不改原证据。

542输入相等绑定的是记录的门禁及原生结束检查点。随后的交接、路线图、ADR及测试状态追加属于明确的文档更新；当前整个文件映射不再声称等于旧检查点，Rust、Cargo、脚本与包装输入保持原已验证字节。新提交的精确源码CI须另外核验；旧预算提交的三平台CI成功不验收此新增界面。


作者与复核两个受管理工作树的恢复快照已归档；根已另外核对实际checkout及Git注册均不存在，并在确认各分支HEAD为主线祖先且未被工作树占用后删除两个本地分支。原作者861份、旧复核630份、新原生复核23份payload及tar在主树ignored证据目录继续保留；此次清理未删除失败证据。隔离原生应用和夹具早已关闭，没有安装覆盖现有应用。
