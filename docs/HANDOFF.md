# 开发交接 — 2026-10-06

最新源码CI：[Quality37346734818](https://github.com/cyruss648/keelshell/actions/runs/37346734818) 对精确 `e821d5ed350379fbd08426d99b2658611a8c0d6f` 三平台全部success。macOS/Linux各1155普通+8doc；Windows1136普通+8doc，原ignored分别11/12/11保持。Windows实际短回执明确清空SystemRoot得到10106、显式传递后完成真实三HTTP请求，两child回收及双EOF；根已独立读回run/3jobs API、实际checkout与Windows原日志。c7原被抑制错误的原因仍未知，不追溯改写；旧c7和3f26失败材料保持，见[首次失败](testing/records/2026-10-05-ai-request-options-ci.md)与[诊断失败](testing/records/2026-10-06-ai-proxy-fixture-readiness-ci.md)。

主工作树已从c7快进到e821，严格保存并恢复根最新8份文档，258工程输入与候选完整门禁相等。根整合完整门禁370.567841秒通过1155普通+8doc+6脚本/严格检查/两控制器，258输入前后相等；173/174完整独立CI封包已根全量读回。尚未推送main，生产字节不变，本增量仅两份测试。已有作者303秒完整门禁及新的限定非作者30/31复核材料均经根读回，不冒充新原生验收。见[主树整合](testing/records/2026-10-06-ai-proxy-fixture-main-integration.md)、[新CI记录](testing/records/2026-10-06-windows-proxy-system-root-ci.md)和[SystemRoot候选](testing/records/2026-10-06-windows-proxy-system-root.md)。

本地Ask真实过程显示候选在独立工作区冻结：16份候选（10代码/6文档）、260工程输入前后相等，1162普通+8doc+6脚本、严格检查/默认及2MiB控制器、57打包通过；77/78作者材料经根完整读回。独立非作者已在另一工作区开始正式复审，尚未整合生产或验收新版原生/供应商。根首次封包读回误比较含额外metadata的整份before/after JSON而失败，记录已保留；重新核对实际head/files260完全相等，未改源或证明。

MCP方向始终为KeelShell向外部智能体提供服务，当前八项固定工具；内置API/本地CLI Ask独立。供应商第八工具、Codex完整授权业务、最小原生窗口、其他平台原生、最终Release与安装更新仍开放，见[接入指南](product/EXTERNAL_MCP.md)。下方既有macOS原生与历史源码记录各保持原范围。

本仓库已整体迁移到用户指定的项目目录。迁移保留 `.git`、所有已跟踪/未跟踪文件、ignored 构建目录和未提交改动。用户已于 2026-10-03 授权公开 GitHub 仓库、推送和标签发布；当前 remote 为 `https://github.com/cyruss648/keelshell.git`。发布与验证状态见 [发布记录](testing/records/2026-10-03-release.md)。

## 2026-10-05 API 请求选项 F 精确整合与最新源码 CI

API 请求选项 F 已在全新非作者差量复核通过后导入主工作树，48 份候选文件逐字节与冻结源码相等。E 原有完整门禁通过仍保留，但独立真实 GPUI 测试发现固定认证头大小写导致合法配置不能保存的 P2；F 仅修正该协议名比较并增加两条正式回归。原始 2957 字节反例原样通过，真实配置写入/回读、秘密拒绝及 HTTP 用途交付回归通过；656 proof/657 归档成员经根逐字节读回核验。C/D/E 原失败不改写。主树 F 完整门禁422.111秒通过1146普通+8doc+6脚本、严格检查与默认/2MiB控制器，57打包通过；显式MAC15新app/MCP/fixture构建与arm64/最低版本检查通过，258份Rust/Cargo输入与门禁相等。新标准包原生 API 临时/环境引用、重启及手动 Ask 已验证限定事实；原 Discover 零请求预期未满足、origin内容未知及外层PTY退出120保持，新记录器真实PTY独立回归通过。中英文三种主题均以独立新提案完成长文件首尾/固定操作栏与人工拒绝，SFTP40499字节及摘要不变；首cn-light到期失败保持，最小900×580未验证。原生API独立59/60证据包根核验，文件矩阵128/129已冻结，新独立限定复核143/144及根完整读回通过，未见新已证P1/P2。见[整合记录](testing/records/2026-10-05-ai-request-options-main-integration.md)、[API原生记录](testing/records/2026-10-05-ai-request-options-native.md)与[文件六组合](testing/records/2026-10-05-mcp-file-review-native-matrix.md)。MCP仅向外部智能体提供KeelShell能力；不开发通用第三方MCP客户端。

前一测试同步提交 `df8cf8be4ea66f1186d511ef84c202709bd4fd4e` 的 [Quality37314626129](https://github.com/cyruss648/keelshell/actions/runs/37314626129) 三平台全部 success，API head、实际 checkout、原日志及 Unix 产物经非作者和根分别核验。macOS/Linux 各1083普通+8文档，Windows1063普通+8文档；原失败重授权及新屏障具名回归实际通过。168 proof/169 归档成员根保存核验；这是该提交源码 CI，不验收 F 或新原生应用，见[CI记录](testing/records/2026-10-05-mcp-grant-readiness-ci.md)。原生辅助脚本v2经26离线测试与独立追加生命周期复核通过，85proof/86归档经根核验，随后才启动本轮新GUI；原NOT_READY及首次Python版本失败保留，不改变产品源码。F新提交CI、供应商第八工具、最小窗口和Windows/Linux原生仍待完成。

## 2026-10-05 文件提案 CI 失败与 API D 新隐私缺口

重授权测试同步修正已通过作者1083普通+8doc+6脚本完整门禁和全新非作者限定复核（3app/3session门控、19MCP、105真实TCP、6脚本及严格检查）；disabled点击补充反例证明无效入口不会伪造操作开始。根逐bytes/hash保存28/29作者与99/100复核材料，精确两份测试Rust已导入，生产/依赖/锁/工具链不变，主树完整门禁439.533秒通过1083普通+8doc+6脚本、严格检查和两控制器，工程输入前后相等。作者/复核工作区已可恢复归档，实际目录/cache消失，证明保留。原7s界面/5s产品限时保持，不认定原CI具体原因，见[同步记录](testing/records/2026-10-05-mcp-grant-readiness.md)。API E同时补齐网络与Apply/持久化前的秘密metadata拒绝，作者最终门禁1121普通+8doc+6脚本通过；正在完成构建和冻结，新独立复审尚未执行，D不可整合。

精确 `a909609c2bf64455ad48ec6c1704bdd68ec3df9c` 已推送且远端核对一致；[Quality37306263578](https://github.com/cyruss648/keelshell/actions/runs/37306263578) 整体 failure，Linux/Windows success、macOS app389通过/1失败。失败发生于失败重授权测试点击后立即检查 busy；具体 CI 时序原因未确立。完整原日志与133proof/134归档经非作者和根核验保留，新的测试同步候选在独立工作区诊断。见[CI记录](testing/records/2026-10-05-mcp-file-proposals-ci.md)。

API D 新独立完整门禁1109普通+8doc+6脚本通过，但另一个正常自有HTTP反例实证P1：已知秘密作为另一配置合法自定义请求头名称时，3协议×发现/测试/Ask的9请求均实际发出该名称，审核摘要却隐藏它。真实GPUI草稿来源确认另记，0网络。D禁止整合；E仅修metadata名称的已知秘密边界并重新审查，原D和此前C失败不覆盖。MCP仍仅向外部智能体提供能力。

## 2026-10-05 文件提案主树与新 macOS 八工具闭环

MCP文件提案B的新非作者限定复核通过后，21份Rust源码与冻结逐SHA相等导入主树；根完整门禁382.385秒通过1077普通+8doc+6脚本、fmt/严格workspace all-targets Clippy/x.y与默认/2MiB控制器，原11ignored保持。显式MAC15新GUI/MCP+fixture构建、标准双程序打包及原生检查通过，输入前后/与门禁相等。新原生应用连接自有TCP SSH/SFTP；自有外部客户端协商八工具并实际读取，人工完整审阅批准后精确字节/摘要读回，拒绝后不变，提案后独立修改再批准被拒并保留外部内容。首轮屏幕外AX操作未进入审阅、授权变更及EOF失败保留，具体原因未确立；第二轮先滚动到可见按钮的闭环exit0，不改产品源码。最终撤权；两次companion、GUI、fixture出生身份消失，SSH与控制监听/线程关闭、私有目录清理，HTTP0/0不计API验收。见[文件提案记录](testing/records/2026-10-05-mcp-file-proposals.md)。新提交CI另核验；第八工具供应商、最小原生六组合、Windows/Linux原生、Release/安装更新仍开放，不转移旧七工具证据。

API C重复请求头草稿秘密P1仍阻止整合，133proof与134成员归档完整保存；新的D正常隐私修复只在独立树保留全配置无效草稿原值用于脱敏和请求拒绝，不赋予交付权限。D作者完整门禁338.138秒通过1109普通+8doc+6脚本和严格工程检查，11ignored保持；独立复审、主树整合和原生尚未完成。D不属于本MCP增量；下方旧“D仅基础准备”“B根门禁正在执行”保留为历史，由本节覆盖。MCP方向始终为KeelShell向外部智能体提供服务。

## 2026-10-05 SSH 三平台 CI 与候选复核更新

主线 `773a11388bd4cbf2aa7e8cb3eb41e607f7db9428` 已推送，远端相等、0/0及干净状态核验；[Quality37274762365](https://github.com/cyruss648/keelshell/actions/runs/37274762365) completed/success，三个实际checkout一致，SSH输出队列具名回归三平台实际ok。macOS/Linux各1060普通+8doc、Windows1040普通+8doc；11/12/11 ignored明确保留。完整原ZIP与日志经根保存核验；旧78 Windows失败不追溯改写，也不作为三平台GUI验收。

API C作者38文件/49proof与原两反例通过仅属作者范围；新非作者实际证实P1：非活动配置的重复请求头草稿仍保留遮罩值，用途重建使该值遗漏于已知秘密集合，另一有效配置经Models→InputState→Test两次实际GET1/POST1。C禁止整合。133proof/134成员归档根逐bytes/hash保存；新审查被系统风险检测中断，最终fmt/diff/x.y/严格Clippy未执行，不计通过，38候选/258工程输入已恢复、10个原进程退出，0timeout。D独立树只完成精确C基础与原探针复制，尚未修复/测试；作者恢复遇模型容量错误，根接手继续推进。

MCP文件提案B新非作者完成限定PASS：原MCP71普通+1doc、GPUI19、TCP-SFTP4、补充生命周期/身份/容量/路径/审批检查、恢复后fmt/严格workspace all-target Clippy/x.y/6脚本通过，26源恢复，无新已证P1/P2。最终报告已写，但proof封装遇模型容量错误中断；根保存核验已有145文件并生成根保存manifest，不称reviewer已生成未存在的manifest。实际6代×8阻塞读取在新授权后本地worker为0，远端48 READ/48 handler、峰值56 handler仍可等待READ返回；释放自有gate后全部0/零write。不把本地取消或close入队当远端停止，也不称远端跨代全局32上限。精确生产/测试代码已导入主树工作副本（继承较新文档历史），根整合门禁正在执行；新八工具原生未完成，MCP仅向外部智能体提供能力。

## 2026-10-05 新 CI 失败与修复复审状态

文档提交 `78eae7cddf3d8540d5d060bc620233af966fd1b5` 已推送并核验远端、0/0与干净主树。它只改八份文档，不含新产品代码；[Quality37266941663](https://github.com/cyruss648/keelshell/actions/runs/37266941663) 已 completed/failure：macOS/Linux成功，Windows app374通过/1失败，SSH输出队列取消测试在未记录输出到达时间时断言队列非空。新的测试候选先观察真实SSH数据并明确证明队列Full，再取消，原6秒截止与最终断言保持；根专项3项和完整门禁1060普通/8doc/6脚本、格式/严格Clippy/x.y/默认及2MiB控制器通过，20份作者证明经根核验保存，全新非作者原3/私有5 TCP-SSH及严格整仓Clippy复审PASS，无限定P1/P2，64份材料经根核验保存，精确测试代码已导入主树，主树完整门禁316.964秒通过1060普通/8doc/6脚本与严格检查，两控制器/工程输入前后及与作者相等；提交后新精确CI待核验。首次外层540秒编译加测试wrapper超时保持，不计完整通过。原Windows失败保持，具体CI调度原因未证实。

API A的P1保持；新B仅修非活动配置代理裸Basic/前缀派生秘密的全配置脱敏、槽生命周期和迟到回复，原2998字节五路径反例不改断言，修复前五true/后五false、0send/CLI/job。B34文件、35proof及36成员归档经根逐bytes/hash保存；作者1098 Rust通过（含8doc）、6脚本/严格工程门禁与MAC15双程序开发编译通过，新非作者已实际证实另一个P1：Models接受非活动代理裸Basic，经过生产InputState/sync和Test路径实际GET1/POST1，将秘密作为model发送；裸Basic/Chat/authNone/临时inactive代理已测，其它组合及分页目前仅源码关注，B禁止整合，C完整发现/测试入网修复已冻结：38文件/49proof根逐bytes核验保存，作者完整门禁1098普通/8doc/6脚本及严格检查通过，原两反例精确通过（五false/0sendCLI/job，GET1/POST0），MAC15开发编译仅构建；全新非作者已开始独立复核。尚未主树整合/原生。文件提案A新非作者实际32次失败重授权证实准备名额泄漏、第33有效提案Busy，P2阻止整合；120proof经根核验保存。B仅修请求所有权集合，原单次P2不改断言通过、原32缺陷诊断不改断言预期失败、独立fixed32与mtime补充通过，B完整门禁1077普通/8doc/6脚本、严格检查/两控制器/57打包通过，26源码及53proof经根核验保存，双程序MAC15开发编译与实际minos分别记录；全新非作者已开始独立复审，主树整合/新八工具原生仍未完成。两项均未产品提交，不用旧七工具验收第八工具。

F目录诊断的新非作者112离线/1自有Python进程通过，限定生命周期预审PASS；根一次实际Codex0.160.0探针1.913秒、1POST/0SSE/0业务，观察到七名称与源码证实的限定schema转换，但记录器缺owned_exit，整体仍PROBE_ABORTED。根与helper均未TERM/KILL任何该次客户端，三项观察出生身份已消失、端口/TMP/线程清理通过；缺退出记录的具体原因未证实。旧A至E失败不改写，F不可重跑，目录事实不能关闭授权SSH业务。见[方向记录](testing/records/2026-10-05-codex-catalog-direction.md)。

## 2026-10-05 MCP 方向与新目录观察：授权业务仍开放

最新作者候选状态：API请求头/代理A的31文件冻结，完整作者门禁通过；新非作者以五条实际GPUI prepare路径证实非活动配置的代理Basic派生值进入其它配置已审核payload，0send/0CLI/job，P1阻止整合。新B仅修全配置派生秘密边界，A源码/78证明已由根核验保存。文件修改提案26文件已冻结，作者1075普通/8doc/6脚本、严格门禁、57打包和host arm64双程序开发编译通过；38证明/26源码根核验保存，新非作者与根整合/原生未完成。两项均未提交或推送，旧七工具材料不迁移成第八工具验收。新的原生夹具仅完成源准备，尚未启动GUI、监听或供应商。

MCP 仅供外部智能体调用 KeelShell，内置 API/CLI Ask 独立，不做通用第三方 MCP 客户端。实际 Codex 0.160.0 的新默认拒绝入口完成 initialize/tools-list，首 POST 已出现七项 KeelShell 名称，1POST/0SSE/0业务；但六项 schema 约束被转换且缺记录器 owned_exit，完整探针仍 PROBE_ABORTED，不关闭授权桌面/SSH读取。A schema P2、B features兼容错误、C假IPC认证deadline以及原授权失败均保持。独立A/B/C与根非作者D离线复核分别记录，不把根D复核叫fresh子agent。E限定schema映射经新非作者核验，但调用方进程扫描被两项mock反例证实复用PID误认领P1，预审FAIL/E不执行；71份proof根核验复制，新F修复进行中，旧实际证据不改写。见[目录诊断](testing/records/2026-10-05-codex-catalog-direction.md)。API请求头/代理31文件作者候选已冻结并进入新独立复核；第8文件修改提案仍在独立树完成门禁，未整合，不以旧七项实验验收新功能。

## 2026-10-05 文件同步新提交：三平台源码 CI 已核验通过

精确 `23310ee13286adb488a451277addf49b05cd476b` 的 [Quality37257829818](https://github.com/cyruss648/keelshell/actions/runs/37257829818) 已 completed/success；API head、三个实际 checkout、原日志和 Unix artifact 均独立核验。macOS/Linux各1060普通+8文档+6脚本+57打包，Windows1040普通+8文档、脚本5通过/1跳过、打包53通过/4跳过；Linux12ignored含既有手工/proc，其余平台11，均不计通过。原文件目标和EOF具名回归三平台实际ok，Unix各9项OpenSSH与观测身份清理通过，Windows相关步骤跳过。CI没有逐场景成功JSON，不冒充原生像素或新客户端通过。160份独立材料经根核验复制，79作者/160文件证明保留；文件工作树可恢复归档、分支与自有cache已实际清理，原失败保持。见[三平台验证](testing/records/2026-10-05-files-scene-ci.md)。

## 2026-10-05 文件场景同步修复：本机与新独立复审通过

只有两个测试文件改动，生产/依赖/锁/工具链及原5秒传输/12秒界面等待不变。精确基线探针观察远端完整数据与旧Running快照，点击后Completed再等Paused失败；旧Linux CI具体时序仍只作候选归因。目标绑定的真实WRITE屏障保持全176场景，并要求Running/Paused/Completed阶段及控件、部分内容稳定和继续后完整字节。作者/根门禁均1060普通+8文档+6脚本、严格工程检查及默认/2MiB控制器通过，作者57打包另记；新非作者44文件/208session+1doc/4私有Handler及严格检查复审PASS，无剩余限定P1/P2。79作者/160复核证明根逐项核验复制，原失败保持；目录句柄计数和subsystem仅观测清理，新233提交CI已按上方范围通过，旧原生包不迁移。见[同步记录](testing/records/2026-10-05-files-scene-readiness.md)。

## 2026-10-05 前次整合 CI：Linux 文件场景失败

精确 `a19d0c1` 的 [Quality37253864420](https://github.com/cyruss648/keelshell/actions/runs/37253864420) 已结束 failure：macOS/Windows 成功，Linux app 为375通过/1失败，文件布局GPUI等待12秒超时，未执行到MCP/OpenSSH。新macOS/Windows的EOF回归通过，macOS另有9项OpenSSH及观察身份清理通过；Windows脚本5通过/1跳过、打包53通过/4跳过，不能把Ran计为执行通过。原始失败与独立读取证据保留，新文件布局候选在独立工作区诊断，尚不认定原CI根因；见[三平台记录](testing/records/2026-10-05-integration-ci.md)。此前三个已合增量worktree已可恢复归档，旧分支和两个审查cache实际清理，241份证明保持。

## 2026-10-05 Ask 预算、弹窗与 MCP EOF 本机整合通过

三个增量均已整合并通过非作者独立复核。根最终门禁通过 1053 普通、8 文档、6 脚本测试、格式/严格 Clippy/x.y 与默认/2 MiB 控制器；原弹窗 A/B 失败保持，最终 C 的 11 原/8 私有模态回归通过。显式 macOS 15.0 新双程序开发包完成隔离原生预算保存/无效草稿/中英明暗、当前 AX 背景节点移除、键盘触发焦点返回及同一 SSH 前后回显；没有新供应商或云调用，不关闭旧 AX 对象、屏幕阅读器或其它平台原生。自有进程、端口和私有数据已清理。原 Linux CI 失败保留，新提交 CI 待单独确认，见[整合记录](testing/records/2026-10-05-ai-modal-mcp-integration.md)、[预算记录](testing/records/2026-10-05-local-agent-limits.md)、[EOF记录](testing/records/2026-10-05-mcp-startup-eof.md)。

## 2026-10-05 Codex 尝试与最新 CI 更正

MCP 仍仅由 KeelShell 向外部智能体提供服务。新的 Codex 0.160.0 授权只读尝试没有通过：首轮执行前脚本误解析功能表，0 POST/未 MCP exec；新范围只协商七项目录，首笔模型请求缺少 KeelShell 工具且含额外工具，安全门停止，实际 1 POST/0 业务 RPC。额外工具来源未知，原字节及两次失败保留；自有客户端与原生 GUI/SSH 已清理，根已明确撤销全部授权。新的非作者只读复核确认失败证据一致、无剩余限定范围P1/P2，15份证明由根核验复制；失败仍未关闭，详见[Codex 尝试记录](testing/records/2026-10-05-codex-authorized-mcp-failure.md)。该限定失败覆盖下方历史“Codex MCP 未执行”，不替代原 Claude 授权通过。

此前精确 `40d092c` 的 [Quality37244888512](https://github.com/cyruss648/keelshell/actions/runs/37244888512) 已结束 failure（macOS/Windows success、Linux MCP EOF 退出断言失败），Linux OpenSSH 未执行；旧 e166 三平台通过保持为旧范围。完整失败日志保留；新的 EOF 分类修复已通过作者、新非作者独立复审及根整合门禁，最新新提交CI失败范围见上方，不能将本机通过追溯为原Linux通过。

## 2026-10-05 外部 MCP 授权客户端更新

以下状态优先于下方历史“供应商授权 MCP 未验收 / Codex 文本失败”描述。MCP 方向仍是 KeelShell 服务端向外部智能体提供能力，内置 API / CLI Ask 独立，不做通用第三方 MCP 客户端。

- 新 clean `e16689b` 标准 macOS arm64 GUI/MCP 包与自有 SSH/SFTP 完成实际 Claude Code 2.1.285 授权场景：七 schema、15 次模型 POST、13 条业务 RPC、42 字节明确选区与 UTF-8 文件、授权外路径/未授权 monitor 的 `FORBIDDEN`、错误 route 的 `STALE_SESSION`、桌面批准后的 `succeeded` 与拒绝后的 `rejected` 均回到同一 CLI。桌面明确撤权后，旧 CLI 收到 not-connected 工具错误，未产生第 14 条业务 RPC；不能称新服务端权限拒绝。Claude 自动再启动第二代 companion 尝试重连，两代退出 1、第二代 BrokenPipe/无回复完整保留，CLI 最终精确答复/exit0。模型是自有 SSE、PTY 仅回显、exec 为固定夹具；根 CUA 点击不是外部 AI 自行批准。原两次 helper/迟到 marker 失败保留，新生命周期没有超时，owned GUI/SSH/CLI/两代 companion/端口/线程/private 清理。独立复核已通过该限定范围，无剩余 P1/P2，见[授权原生记录](testing/records/2026-10-05-claude-authorized-mcp.md)。
- Codex 0.160.0 新 strict 文本复现直接观察到额外 loopback 代理路由被 EPERM 拒绝；只给子环境增加 `NO_PROXY/no_proxy=127.0.0.1` 的新对照实际 1 次 Responses POST、精确 canary、turn.completed/exit0，网络限制不放宽、不改系统代理。旧冻结失败没有 errno，不补写其原因。79 份新诊断与 14 个已观察身份清理已核验；23 份 MCP 适配准备/13 项 synthetic 检查不等于真实 Codex MCP，后者仍待执行。
- 同一精确 `e16689bcd027a28d8035d271787ad53ab165d0ae` 的 [Quality37240943183](https://github.com/cyruss648/keelshell/actions/runs/37240943183) 三平台全部 success；macOS/Linux 各1031普通、Windows1011普通，各8doc、严格工程门禁/默认及2 MiB控制器/打包通过，macOS/Linux 各9项 OpenSSH 及owned/TMP清理通过，189份新CI材料逐bytes/hash核验。源码/工程256hash与包内5文件及3binary保持；不是 Windows/Linux GUI、正式六目标 Release、签名/公证或安装自动更新验收。

下一步保留 Codex MCP、完整 Agent 工作流、外部客户端 Running 撤权/重启、其余授权组合和各目标原生/发布验收。不能将这一有限 Claude 切片标为全产品完成。

## 2026-10-05 工作区整合与紧凑布局（观察修正已推送，三平台CI通过）

连接库、依赖工作流 UI 与文件响应布局均已合入 `main`（生产提交 `40824f0`），三个增量及其冻结整合范围的独立复审通过。标准macOS开发包完成受控标签审核/保存、两条SSH/SFTP及依赖任务退出0/7，但实际900×580英文/Light/AI/Files组合暴露终端约37px的新P2；紧凑补全布局与完整工作区回归已实现，新根整仓门禁通过，fresh独立复审与新macOS包八组合/补全原生闭环PASS，新源码已推送并核对远端精确SHA，Quality37232614315已结束：Linux/Windows成功，macOS的SSH测试在认证阶段超时，整次失败。本节覆盖下方历史“编辑器待接入”和“页脚待修”等状态；旧包不能关闭后续修复。见[整合记录](testing/records/2026-10-05-workspace-workflows-integration.md)与[紧凑原生记录](testing/records/2026-10-05-compact-workspace-native.md)。

- 连接库：明确选区、完整对象审核、完整候选一次保存、跳板依赖检查、主题/语言保留与元数据冲突拒绝，支持批量移动/标签/收藏/回收/恢复/永久清理及上次批量回收撤销。独立 core321 普通+4文档、24项连接库 GPUI 及5项补充探针通过；默认线程栈与显式2 MiB的两个布局场景分别记录，不扩张为整套小栈或原生验收。见[连接库记录](testing/records/2026-10-05-connection-library-bulk.md)。
- 依赖工作流：手工编辑1–128个临时任务、最多32个现有已认证 SSH 目标、每任务最多32个直接前置，完整审核源/展开命令、目标/路线、依赖和执行选项，确认后在捕获的连接上调度并收集逐任务有界输出/回执。失败/未知/跳过阻止下游；可隐藏/重开与取消，不自动重试、重连或重放，不保存任务级命令/输出。最小窗口四个命令动作已换行；新独立29项工作流 GPUI/TCP、fmt、全工作区 strict Clippy 与x.y策略通过。原冻结128任务/32目标补充探针及9项 OpenSSH 另作原源码证据，未冒充修复后复审。见[指南](product/DEPENDENCY_WORKFLOWS.md)与[工作流记录](testing/records/2026-10-05-dependency-workflow-ui.md)。
- 文件布局：至少64px浏览区包含表头和真实首行，工具/编辑/比较/传输卡片在有界垂直区域滚动，完整审核文字另行滚动且确认/取消保持固定。独立21项文件专项及176个生产主题 GPUI 场景通过，补充探针验证固定按钮位置与长草稿保持；176场景覆盖11个真实状态，不是原生截图或完整工作区验收。已转换 tooltip 在保持鼠标位置时随当前语言重绘，余下5个静态调用点已由根整合补齐，动态完整目标/目录文字保持原样。见[文件记录](testing/records/2026-10-05-files-responsive-tooltips.md)。
- 紧凑工作区：高度小于700px时补全目录和操作收为一行，保留64px命令输入、文件预算与全部命令动作；候选展示暂时折起文件区，关闭后恢复原实体。32个完整Workspace真实Files/监控组件场景测得终端89.5–284.5px，16个13项真实候选/144px列表场景测得100.5–403.5px，并以滚轮到达最后候选；这些是GPUI/受控TCP精确测量；新原生900×580八个中英/明暗/AI组合通过，终端人工估读约89/121px，实际SFTP补全/滚轮/关闭恢复通过，监控仅显示服务器拒绝exec。作者7项工作流专项、fmt/x.y/app strict Clippy通过，43份证据经根逐SHA核对；原72.5px失败、编译/重复模块与caret诊断保留。见[ADR0046](adr/0046-compact-workspace-terminal-budget.md)。

根混合真实TCP回归的审核互斥已修复，剩余静态tooltip已补齐，冻结整合审查PASS。紧凑修复后的新根门禁通过1031普通（含362项应用）+8文档+6脚本、格式/workspace严格Clippy/x.y、默认与显式2 MiB完整CLI控制器（future4624字节）及57打包；11项默认忽略为2项供应商选择测试和9项OpenSSH。前一冻结源码9项系统OpenSSH已通过，紧凑修复未改transport/打包源码，本轮不复跑，后续新源码CI单独核验。GUI/MCP新native build通过；fresh紧凑独立复审127工作区/31工作流/12补全集合及80正常/40候选场景通过，集合重叠不相加；新标准双程序包以commit=null和256源码hash绑定，实际900×580八组合与7候选/末行滚轮/Files恢复通过；仅连接一个自有echo夹具，没有新文件写入/工作流执行/云AI/MCP验证。四个owned进程、两个监听/临时根清理，两个审查cache也实际删除，源码/失败留存；根接续是验证新源码CI，不将本切片扩为全产品/其他平台原生验收。MCP仍向外部智能体提供服务端，内置CLI Ask单独负责问答；内部桌面IPC客户端不是第三方MCP入口，29文件定向审查PASS但不证明供应商互通。

夹具修复更新：只修改两个session测试文件，先以150ms延迟完成认证，再观察真实READDIR/句柄或监听lease/AddrInUse后验证精确操作超时与清理。最终作者95项及两目标各6次、根1031普通+8文档+6脚本、严格工程门禁、57打包、9项本机系统OpenSSH与GUI/MCP build通过；旧TCP探针副作用候选及共享源中断的gate均保留且不作验收。新独立复审无剩余P1/P2，95项及6项私有补充探针通过且62证据经根核验；测试修复f09651b已推送main并核对256源码/工程hash及远端SHA，新Quality37235821726结束failure：macOS/Linux成功，Windows在远端转发已分配/清理后的1秒TCP观察超时，下一观察修正另建范围；两套审查owned cache均清理、原95/62证据保留，最终三平台CI结果继续追加，见[夹具验证](testing/records/2026-10-05-ssh-timeout-fixture-stability.md)。旧原生UI包仍属于408快照。

Windows观察更新：唯一后置TCP探针已改为相同1秒期限内单次复绑原完整端点，保留before AddrInUse、精确转发超时、SSHclosed与lease0。最终作者95项/12专项/工程门禁、根1031普通+8文档+6脚本及fresh独立95/7探针复审通过，无剩余P1/P2；修正896073a已推送main并核对256源码/工程hash与远端SHA，Quality37238796819三平台实际success：macOS/Linux各1031普通+8文档、Windows1011普通+8文档，两个目标case均ok，默认/显式2 MiB控制器均通过。macOS/Linux各9项实际OpenSSH与owned/TMP清理回执通过，159份新CI证据经根核验，见[观察记录](testing/records/2026-10-05-windows-forward-observer.md)。作者与新独立审查的五条owned cache/TMP路径已实际删除，原55/49及两次失败CI证据保持。只改一个测试文件，不迁移原生包或扩张产品限时；桌面/供应商MCP/Release仍另行验收。

## 2026-10-04 新增用户要求

2026-10-05外部客户端更新：MCP方向只向外部智能体提供服务端，内置CLI Ask是另一入口。新精确端口/空配置前置中Claude2.1.285实际文本成功，Codex0.160.0保留strict的exec连接失败、0 POST，不能计为通过。另一新scope的真实Claude→生产companion完成七schema协商、唯一list_sessions的DISABLED及同客户端下一Messages结果回传；没有GUI/SSH/授权操作，不关闭完整供应商互通。fresh独立复审通过，无剩余P1/P2；原报告插件范围措辞由独立addendum关闭P2，builtin存在、50ms census仅覆盖已观察身份、companion退出码未采集。原108/80及新13复审材料经根逐bytes/SHA核验，已观察PID/birth、线程/监听与scratch清理，见[客户端记录](testing/records/2026-10-05-external-client-mcp-preflight.md)。

- 明暗两种主题，默认跟随系统，显式切换与配置持久化；采用更丰富但克制的元素/语义色和专业一致的视觉。主题基础已完成System默认、明暗切换/保存与语义palette；866普通+6文档、47打包、新独立审查及macOS部分原生通过，边界见[主题记录](testing/records/2026-10-04-system-themes.md)。
- AI配置新增本地Claude Code/Codex CLI，不限模型API；提供MCP服务端给外部智能体调用。用户明确排除KeelShell接入其他第三方MCP服务的通用客户端，后续不得扩大该范围。
- 独立`keelshell-mcp` stdio服务端已接通受认证桌面IPC、真实SSH/SFTP句柄与人工批准，官方`rmcp = "3.5"`、默认关闭，七项固定工具只提供授权读取/待审提案。与CLI合并的本机门禁963普通+8文档、6脚本通过，新独立复审关闭；macOS受控原生证实OS复制启动配置、片段/目录/文件读取、越权拒绝、精确提案执行/拒绝、运行中撤权结果未知和重启默认关闭。真实供应商客户端MCP、最终发布包和Windows/Linux原生仍须验收；该历史原生包显式复制companion不作为发布打包证明；2026-10-05标准双程序开发包的完整原生闭环见下方整合更新，不代表正式Release或安装更新。见[ADR0039](adr/0039-authenticated-desktop-mcp-ssh-bridge.md)、[记录](testing/records/2026-10-04-mcp-desktop-bridge.md)与[指南](product/EXTERNAL_MCP.md)。
- 正式任务ID、顺序、架构边界与验收见[设计与智能体计划](product/DESIGN_AND_AGENT_PLAN.md)，参考选型和现有skills见[设计资料库](design/README.md)，来源/版本核对见[计划记录](testing/records/2026-10-04-design-agent-planning.md)。先收敛已开始后端，再建立主题基础；新增界面复用同一体系。
- 主题实现已复用System/Dark/Light字段，不升schema，默认/缺字段为System，保留旧明示Light；Kit模式与palette同步，保存成功才应用，系统回调不写配置，主题保存不推进命令来源revision。真实OS变化/最小原生窗口/WindowsLinux原生仍未验收，见[ADR0036](adr/0036-system-appearance-and-semantic-palette.md)。
- 用户追加并行工程要求：允许多个子代理/worktree并行不相关功能，完成后新开独立代理评审与功能复核，合并后及时清理分支/worktree/临时进程或容器；可使用本机Podman建立隔离服务真实联调，不能将容器或模拟视为目标桌面验收。
- MCP伴随程序的六目标发布/更新清单已接通；每个平台包必须包含GUI与MCP、分别绑定SHA/架构/Unix权限，旧的不完整包在写入前拒绝。两轮独立审查发现并关闭回滚备份清理与重复进入缺陷，29项updater、57项打包独立通过；根整合981普通+8文档+6脚本及2 MiB控制器通过。实际macOS开发双程序包已通过标准打包、原生结构/依赖/最低版本检查和归档校验，并在隔离零连接配置中观察到MCP默认关闭、无会话禁用与临时配置/授权生命周期文案；owned monitor终止并确认进程及临时目录清理。原有业务原生联调使用的手工复制包另行记录。最终六目标Release、已安装目录更新、Windows文件占用及新包SSH业务原生仍未验收，见[ADR0041](adr/0041-mcp-companion-packaging-and-update-recovery.md)与[记录](testing/records/2026-10-04-mcp-companion-packaging.md)。

## 2026-10-04 本地智能体接入

2026-10-05整合更新：Linux MCP错误响应超时已由真实锁定SDK的确定性取消探针证明，修复`93f35ca`使用官方codec与两个连接拥有的I/O任务，SDK接收取消不再丢失排队/部分写入的错误响应。新独立审查无P1/P2，原3秒压力与EOF/背压/撤权保持；根整合988普通+8doc+6脚本、额外2 MiB完整CLI控制器、格式/严格Clippy/x.y及57打包通过。失败证据与独立审查已逐份SHA复制到主工作区；标准macOS双程序包第二轮完成实际选区/SFTP读取、越权拒绝、批准/拒绝、Running撤权结果未知与重启默认关闭；首次人工操作超时保留，后续观察到CUA滚轮方向差异，独立32原始事件场景通过，未改生产布局。根新增首条短提案32原始滚轮场景的最终门禁989普通+8doc+6脚本、默认/2 MiB完整控制器通过；fresh独立审查无P1/P2，独立9项MCP UI、格式/app strictClippy/x.y通过。生产提交34cff1b的Quality37217653868三平台及macOS/Linux OpenSSH全部成功；新增回归提交958903c已推送main，Quality37220102376三平台成功，macOS/Linux OpenSSH也通过；已合本地/远端功能分支与worktree、probe/reviewer隔离target均清理，失败证据和源码副本保留。供应商MCP与Windows/Linux原生仍需验证，见[取消修复记录](testing/records/2026-10-04-mcp-response-cancellation.md)。

- 命名AI配置已显式区分模型API、Codex CLI与Claude Code，旧metadata默认API；路径/base URL校验、probe、临时密钥/vault v2精确绑定、完整stdin审核与后台Ask/取消已接通。最终本机门禁929普通+8文档、6脚本、自托管进程harness、strictClippy/fmt/x.y与47打包通过；新独立core/app复审无剩余可复现P1/P2。
- 最终macOS包SHA与原生证据见[记录](testing/records/2026-10-04-local-agent-ui.md)：实际安装版Codex0.160.0/Claude2.1.285→自有SSE问答，选择125字节SSH上下文、长JSON滚动/显式发送、建议入审核区、语言/主题保留、慢请求取消、重启密钥缺失均证实；退出后owned PIDs/listeners/scratch均清理。没有云端账户或客户SSH验收。
- 使用见[指南](product/LOCAL_AGENTS.md)；固定单次Ask不继承现有项目/hooks/MCP/订阅登录。Agent、目录/预算/环境编辑与Windows/Linux native继续保留。新功能提交的远端CI另行追加。对外MCP方向不变，独立于内置Ask。
- 提交`3fe0c98`的[Quality37208098775](https://github.com/cyruss648/keelshell/actions/runs/37208098775)中macOS/Linux成功，Windows自托管process harness主线程stack overflow，整次CI失败。堆缓冲与2 MiB控制器修复`be9590f2`的新[Quality37210745161](https://github.com/cyruss648/keelshell/actions/runs/37210745161)中Windows与macOS成功，Windows914普通+8doc及两种控制器通过；同次Linux发生既有MCP错误响应3秒超时，整次CI仍失败。根40次4并发复查也复现一次，当时原因未证明，后续取消探针及修复见本节更新；不放宽期限。根合并门禁964普通+8doc+6脚本通过，见[小栈记录](testing/records/2026-10-04-local-agent-windows-stack.md)。

## 2026-10-04 继续交接（覆盖下方历史状态）

本节记录当前工作区相对于下方历史交接内容的最新状态。后续实现和验证应以本节、`docs/ROADMAP.md`、能力清单和对应测试记录为准；历史章节保留用于追溯，不代表当前未完成项已经关闭。

- 主题`8ee085d`与MCP基础整合`6644245`的本机整仓门禁通过900普通+7文档、格式/严格Clippy/依赖策略，47打包通过。MCP原worktree的22份日志已复制并逐份SHA-256核对，已合分支删除、worktree可恢复归档。文档提交`c66b7e2`的CI三平台Rust/打包和Linux OpenSSH成功，但macOS测试脚本`ps`单次0.5秒超时；失败回执仍保留。伴随修复保持overall截止与身份清理，将单次枚举上限改为3秒；六项脚本回归与本机独立八项OpenSSH通过，修复提交3137712的Quality37201272070三平台成功，macOS/Linux各独立8项OpenSSH且owned清理全部通过；该CI不代表本地CLI新源码，见[修复记录](testing/records/2026-10-04-openssh-process-inventory.md)。
- 最新冻结源码（含SSH依赖调度适配器）的整仓门禁通过：859项普通测试、6项文档测试、严格全工作区Clippy、格式和依赖策略；此前47项打包回归通过，新一轮独立8项OpenSSH互通通过。目录合并已完成macOS受控原生双向内容哈希验证。初始AI布局/断言失败和并行编辑时格式失败日志保留；本次提交的三平台CI另行追加，不沿用旧提交结论。
- 代码提交`9802ce9`已推送，GitHub[Quality 37197083353](https://github.com/cyruss648/keelshell/actions/runs/37197083353)三平台成功：macOS/Ubuntu各859普通+6文档，Windows843普通+6文档；打包47项（Windows一项权限检查跳过），macOS/Linux独立OpenSSH步骤成功。完整源码CI与主题后续计划分别记录，不视为新增主题/MCP验收。
- 批量依赖计划核心新增1–128个任务、32个目标的确定性拓扑审核和纯内存放行账本，指纹绑定精确命令/目标/依赖；只有前置明确成功才能放行下游，失败/未知/跳过阻止下游，取消不把运行任务标为远端已停止。10项单测、3项公开API集成、1项文档测试通过；纯核心层仍无网络/执行/定时器；后续session适配器已接通捕获会话上的真实SSH依赖调度，11项TCP协议专项及独立8项OpenSSH互通通过。图形工作流编辑器、完整目标/选项审核和任务级持久化仍待接通，见[ADR0035](adr/0035-reviewed-workflow-ssh-adapter.md)及[适配器记录](testing/records/2026-10-04-workflow-ssh-adapter.md)。见[ADR0034](adr/0034-reviewed-batch-dependency-plan.md)和[依赖计划记录](testing/records/2026-10-04-batch-dependency-plan.md)。
- 独立UI审查后，连接行与操作的可访问名称现含完整连接名称及user@host:port；窄连接区域采用可换行操作卡片和顶部目录树，AI侧栏受窗口42%限制；更新关闭标签双语可辨认。110项工作区回归通过，当前构建原生复审见[独立审查记录](testing/records/2026-10-04-independent-workspace-audit.md)。背景AX模态隔离、最小原生窗口与其余审查缺口继续跟踪。

- 文件面板现接通有界内容校验与显式确认后的双向目录合并。源/目标完整快照在执行前重新构建，每项再次核对类型/大小/SHA-256；文件以同目录临时文件原子替换并读回，保留目标独有项。当前限制为64 MiB/文件、256 MiB/两侧、10000项/侧、32层、128 KiB审核路径；静态链接、特殊对象、类型冲突和跨平台危险/大小写冲突名称拒绝。取消/失败可能保留已完成项，SFTP v3/本地检查不是排除外部并发替换的事务。五项真实 GPUI+TCP SSH/SFTP专项通过，最终门禁/原生证据见[ADR0032](adr/0032-reviewed-directory-merge-execution.md)和[合并记录](testing/records/2026-10-04-directory-sync-execution.md)。
- AI 设置现支持可选输出Token上限与声明上下文窗口，精确JSON审核使用协议专属字段；输入预算保守按UTF-8和JSON转义估算，无法容纳完整问题则拒绝，未配置保持既有请求形状。无效数字草稿跨配置/语言保留并阻止请求/保存，修改撤销旧审核与在途请求。高级请求头、代理、非默认推理与Agent继续拒绝。验证见[ADR0033](adr/0033-ai-token-limits-and-local-context-admission.md)和[Token记录](testing/records/2026-10-04-ai-token-limits.md)。
- 批量任务现在支持受限的逐目标元数据模板：`{{name}}`、`{{host}}`、`{{port}}`、`{{user}}` 和 `{{endpoint}}`。模板只读取已保存路由或一次性会话的非敏感元数据，在本地展开；未知变量和不支持的上下文会在审核前拒绝。审核面板逐目标展示最终命令，确认前不会发起 SSH 请求，确认后按目标绑定执行。审计摘要的命令摘要同时覆盖源文本与逐目标绑定，但仍不保存命令正文、输出、地址或凭据。实现见 `crates/keelshell-core/src/batch_template.rs`、`crates/keelshell-app/src/batch_commands.rs`，设计与证据见 [ADR 0027](adr/0027-reviewed-per-target-batch-templates.md) 和 [测试记录](testing/records/2026-10-04-reviewed-batch-templates.md)。
- 失败的文件或目录传输现在会在同一活动 SSH 会话中保留一个显式恢复提议。点击“检查并续传”只会创建新的只读校验计划，之后仍需用户审阅并确认；它不会自动重放、自动重连、跨会话复用或绕过现有内容校验。只有实际开始过的失败传输可产生提议；列表/规划操作本身不会产生新的提议，且只有失败卡片在非忙碌、无待审核时才能使用既有提议。只读计划不会改写旧失败卡的终态，状态变化后的旧点击会被再次拒绝。实现见 `crates/keelshell-app/src/files.rs`，设计与证据见 [ADR 0026](adr/0026-explicit-transfer-recovery-proposal.md) 和 [测试记录](testing/records/2026-10-04-transfer-recovery.md)。
- AI transport 现在按配置快照显式区分 Chat Completions、Responses 与 Anthropic Messages。设置页可切换协议，已知 `/chat/completions`、`/responses` 和 `/messages` 后缀会同步替换并清除旧临时密钥；Responses 预览使用 `instructions`/`input`，Anthropic 预览使用 `system`/`messages`/`max_tokens`，回复分别只接受 `output_text` 或 assistant `text` blocks，不会启用 tools 或自动操作。Anthropic 使用 `x-api-key` 和固定版本头，模型发现按有界游标分页。协议契约、回环 HTTP 和 GPUI 回归见 [ADR 0029](adr/0029-ai-responses-transport.md)、[ADR 0030](adr/0030-anthropic-messages-transport.md)、[测试记录](testing/records/2026-10-04-ai-responses.md) 与 [Anthropic 记录](testing/records/2026-10-04-anthropic-messages.md)。
- Anthropic Messages 增量已完成本地整仓验证：`cargo test -p keelshell-ai --locked` 通过 30 个单测、19 个发现回环测试、11 个请求回环测试和 1 个 doctest；`keelshell-core` 70 个单测及集成/文档测试、`keelshell-app` 269 个 GPUI/业务测试通过；工作区严格 Clippy 和 `scripts/check.py` 通过。回环 fixture 不证明供应商账户、计费、真实模型质量或 Windows/Linux 原生桌面交互，边界见 [Anthropic 记录](testing/records/2026-10-04-anthropic-messages.md)。
- 目录比较的核心增量现提供有界 SHA-256 内容摘要和审核式同步计划：`hash_directory_content` 单文件最多 64 MiB；两侧摘要相同可跨越 mtime 差异证明内容一致，单侧摘要或缺失元数据保持 `Uncertain`；`plan_directory_sync` 只生成复制/显式删除操作和审阅指纹，并把规划时源/目标的类型、大小、摘要带入未来执行器的复核字段，正确确认只产生无写入能力的回执。纯核心计划本身仍无写入能力；应用层已通过ADR0032接通有界内容收集和保留目标项的双向合并，差异应用/镜像删除仍待补。设计与证据见 [ADR 0031](adr/0031-directory-content-hash-sync-plan.md) 和 [测试记录](testing/records/2026-10-04-directory-sync-plan.md)。
- 文件工作层新增有界本地目录快照 worker、SFTP 远程元数据快照和纯核心比较引擎。文件面板现在可以显式发起只读“比较目录”，目标绑定最近一次成功加载的 canonical 远程目录，显示一致、变化、仅本地、仅远端和待确认统计，并列出前 100 条相对路径结果；缺失字段标为 `Uncertain`，超过深度/条目/路径边界直接失败。核心层现提供 64 MiB 有界 SHA-256 内容摘要和审核式同步计划；应用已接通保留目标项的双向合并；差异应用/镜像删除仍未接入。设计与证据见 [ADR 0028](adr/0028-bounded-directory-comparison.md) 和 [测试记录](testing/records/2026-10-04-directory-compare.md)。
- 本轮新增切片在本机完成 `cargo fmt --all -- --check`、严格 Clippy 和完整 workspace 门禁：应用 267 项、核心 69 项、会话库 64 项、批量集成 14 项、SSH loopback 95 项，OpenSSH 外部互操作 6 项因未提供 `KEELSHELL_OPENSSH_*` 环境而忽略，doctest 全部通过。GPUI 还新增中英文 accessibility label 回归，记录见 [可访问名称记录](testing/records/2026-10-04-accessibility-labels.md)。后续修复提交 `cf7c4a6` 的 GitHub [Quality 37181528476](https://github.com/cyruss648/keelshell/actions/runs/37181528476) 已在 macOS 26、Ubuntu 24.04、Windows 2025 成功，覆盖目录比较 UI、canonical 目标绑定、worker 取消边界和可访问性改动；流水线证明构建、测试与打包路径，不等同于 Windows/Linux 原生桌面交互验收。
- 仍未关闭的产品差距包括自动传输恢复与并行调度、任务依赖/编排/定时、交互 shell 可编程补全、目录镜像删除/冲突合并/差异应用、更丰富的网络协议诊断、Anthropic 高级参数/Agent 等更多 AI 工作流，以及 Windows/Linux 原生窗口验收、签名/公证和已安装目录更新验收。

## 当前产品要求（优先于旧文档）

1. 名称 KeelShell，Rust + GPUI Kit，跨平台。
2. 仅远程 SSH 管理，不做本地终端、RDP 或串口。目标覆盖连接组织、认证、会话、文件、监控、隧道及进阶远程运维；不能将部分实现视作完成。
3. UI 采用紧凑的远程运维工作区，要求现代、美观、交互清晰：跟随系统的明暗工具栏（主题基础已实现）、左主机监控、中央深色终端、下方命令/文件工具区、独立连接管理器。应用图标必须白底，外部透明，适配各平台。
4. 多语言，默认简体中文，当前支持中文/英文。切换不能丢草稿、会话或在途任务。
5. AI 配置与功能交互参考 DBX。官网与本机设置已查看，详见 `research/dbx-ai-reference.md`。
6. 依赖尽量最新实用版本，所有直接 registry 版本必须 `x.y`；维护锁文件、设计文档、单元/集成测试与原生验收记录。
7. 已只读评估 Reef、reef-template，见 research；参考工程与生命周期模式，不引入私有后端框架。

## 已有本地提交

- `b5ed0ce`：工作区与设计研究基线。
- `4bc6084`：SSH、SFTP、监控、隧道、AI 审阅与第一版原生界面；这是用户纠正范围之前的检查点。
- `6e79fe4`：远程专用/双语/现代工作区、DBX式命名AI配置与取消、白底三平台图标及本地打包；181项测试和Mac受控流程通过。
- `3cd322f`：从剪贴板导入受限 OpenSSH 配置，并加入有界、零持久化的 keyboard-interactive 传输响应；本地整仓门禁通过。
- `3bb67c7`：同步中英文 README 的 OpenSSH 导入能力说明和审阅边界。
- `51c0eac`：OpenSSH 剪贴板导入候选审阅、确认后保存、来源定位、`key=value` 解析及重复/未知指令警告。

截至本交接版本，提交 `51c0eac` 的 GitHub Quality `37155464824` 已在 macOS、Ubuntu、Windows 全部通过；此前 `3cd322f` 的 Quality `37133899213` 与 `3bb67c7` 的 Quality `37134477105` 也均已通过。流水线验证的是代码、测试和打包路径；Windows/Linux 桌面交互仍不等同于本机原生窗口验收。

本阶段完成用户纠正范围后的远程专用工作区、命名 AI 配置与白底图标实现，按本地 Git 保存检查点。不能用早期提交的测试记录代表当前代码，最近验收如下。

## 当前实现与证据

2026-10-04 的 OpenSSH 导入增量已把剪贴板解析改为候选审阅流程：解析支持常见空格和 `key=value` 指令写法，候选连接、跳板、认证类型、来源行与 Include 来源会在确认弹窗中展示；未知/语义敏感/重复指令及 Host 块内 Include 会保留为可定位警告。确认后才写入连接库，取消保持原状态。核心 52 项单元测试、7 项 OpenSSH 集成测试，以及应用侧确认/取消 GPUI 回归已通过；完整整仓门禁与本次提交的多平台 Quality 需以本轮提交后的记录为准。

最新主线继续增加了六个可验证的远程工作流切片：连接库支持 JSON 剪贴板导入/导出、收藏和删除；每个 SSH 标签拥有仅驻留内存的有界命令历史；SFTP 提供单 worker FIFO 队列、分块进度与边界取消并已接入文件面板；Linux 监控面板可按需读取监听 TCP/UDP 端口，并能从 TCP 行显式发起由远程主机执行的固定 `nc -z` 连接探测；SSH 初次连接对瞬态传输失败使用可取消的有界退避重试；终端提供限定在活动 SSH 标签滚动区内的搜索覆盖层、上一项/下一项定位和匹配高亮。对应实现与记录分别见 `testing/records/2026-10-03-connection-library.md`、`testing/records/2026-10-03-command-history.md`、`testing/records/2026-10-03-files-transfer-queue.md`、`testing/records/2026-10-03-transfer-queue.md`、`testing/records/2026-10-03-socket-diagnostics.md`、`testing/records/2026-10-03-tcp-service-probe.md`、`testing/records/2026-10-03-connect-retry.md` 和 `testing/records/2026-10-03-terminal-search.md`。

- 删除本地 PTY 产品后端和依赖。`events.rs` 承载 SSH/界面共享事件；`remote_only.rs` 扫描产品边界，使用 Cargo 运行时目录以支持移动后的构建缓存。
- `workspace.rs` 管生命周期与保存，`workspace/view.rs` 管布局，`workspace/modals.rs` 管连接/认证弹窗。`design.rs` 统一白色/浅灰/蓝色控件主题。
- 默认空会话、中文；新标签打开 SSH 管理器。分屏显式记录两个 EntityId，焦点切换不交换左右位置。命令从首次输入起绑定会话，切换标签不能发往另一目标，清空后才重新绑定。
- 窄窗口打开 AI 时隐藏左监控栏；空状态不显示空监控列；文件区高度随窗口调整；连接表支持横向滚动。
- 早期门禁 `work/integration-gate-12.log` 的 181 项测试仅证明当时版本。后续功能的测试和失败修复分别记录于 `testing/records/`；最新代码需以对应提交的本地门禁与 GitHub Quality 结果为准。
- 原生 macOS 已走通：创建/保存中文 SSH 配置、指纹核对/信任、密码登录、SFTP 列表/读取/审核保存、SSH 分屏、AI 配置发现/测试/保存、精确请求预览/手动发送、建议入命令栏/手动发送、中文英文切换保留状态。
- 夹具只绑定回环地址，SSH terminal 只回显而不执行系统命令；SFTP 仅临时目录；AI 仅本机 HTTP 模拟服务。不能据此宣称真实 Linux 指标、生产 SSH 互通或商业模型服务已验收。

最新连接组织与动态代理增量见 [集成记录](testing/records/2026-10-03-library-socks-integration.md)：持久化嵌套目录树、空目录导入、标签编辑、连接移动、可恢复回收站及最近 50 次成功连接；旧分组保留名称并自动迁移。目录同时改名/移动使用 `update_folder` 一次校验最终树。最近记录排队等待当前保存完成，并在入队及保存时检查目的地快照；编辑目的地清除旧成功记录。异步连接通过 `runtime_bridge` 执行，完成后按当前可见弹窗恢复焦点。

连接管理器同时提供 **导入 SSH 配置**：从剪贴板读取受限 OpenSSH 子集，只接受精确 Host、HostName、Port、User、IdentityFile、ProxyJump 及调用方显式提供的 Include 内容；通配/条件/ProxyCommand 等语义会进入警告报告，解析和路由错误保持连接库不变。实现、设计和测试边界见 [OpenSSH 导入记录](testing/records/2026-10-03-openssh-import.md) 与 [ADR 0023](adr/0023-openssh-config-import.md)。

动态 SOCKS5 仅绑定回环，IPv4/IPv6/域名经 SSH CONNECT；正常停止保留 SSH，异常通道清理到期可断开共享 SSH，并以同 socket shutdown 兜底。界面提供实际代理 URI、复制、逐行停止与汇总状态；不包含 UDP/BIND、代理认证或保存规则。详见 [专项记录](testing/records/2026-10-03-dynamic-socks.md)。

## AI 模块

`keelshell-core/src/ai_profiles.rs` 与 Settings 实现命名配置目录、默认项、供应商/协议/认证引用、高级参数元数据及旧配置迁移。配置 JSON 严格校验，API Key 不序列化。

`keelshell-ai/src/discovery.rs` 实现可取消的异步模型发现、固定无终端上下文的连接测试、已审核 payload 发送。模型地址按完整 Chat Completions、Responses 或 Anthropic Messages endpoint 同源推导；没有自动跨地址尝试、重定向或重试。响应大小、模型/地址长度、超时和错误分类均受限。

`ai_settings.rs` / `ai_settings/view.rs` 独立配置页支持多配置 CRUD、默认项、预设、端点、模型、临时遮罩密钥、发现、测试和取消。保存采用 revision 快照，不覆盖保存期间的新编辑。助手选择临时配置不改变已保存默认项。

当前请求后端支持 Chat Completions、Responses 与 Anthropic Messages 三种显式协议；Anthropic 使用 `system`、`messages`、有界 `max_tokens`、`x-api-key` 和固定版本头，回复只读取 assistant `text` blocks。切换协议会安全替换已知 URL 后缀并清除旧密钥。自定义请求头与显式 HTTP(S)/SOCKS5 代理已接通发现、测试和审核式 Ask，具备临时/环境/加密凭据引用与秘密元数据拒绝；当前 F 整合门禁和原生验证见顶部及[请求选项指南](product/AI_REQUEST_OPTIONS.md)。非默认推理参数和 Agent 工作流尚未接通，不支持的设置明确拒绝。AI 密钥默认驻留内存，现支持显式加密保存、每次启动后主密码解锁、清除临时密钥及解除关联。

SSH 密码与私钥口令已接入显式保存、每次主密码解锁和解除关联流程。凭据库 schema 2 认证完整 manifest，保存时检查经过认证的文件快照；加密 payload 绑定连接目的地，配置中只保存不透明引用。解除关联不会删除加密条目；vault 与 state 两次写入不是一个事务，失败可能留下孤立密文。后台已开始的保存可在关闭弹窗后完成，但不会自动连接。设计及边界见 `adr/0006-authenticated-credential-vault.md`，验收见 `testing/records/2026-10-03-vault-search-integration.md`。

## 凭据维护与目录传输增量

主工具栏新增凭据库维护。打开前等待保存/连接完成并排除其他草稿；模态存在时冻结本进程配置写入。条目显示关联配置名称，回收站和所有 AI 存储引用均受保护；磁盘引用每次重新加载。`load_existing` 防止文件删除竞态误创建空库。主密码轮换分阶段重加密并保留旧快照的并发检查；忙碌关闭等待后台结果并把最终消息传回工作区。参见 `adr/0008-credential-vault-maintenance.md` 与对应专项记录。

文件面板新增“上传目录”，选择远程目录后“下载选中项”。后台只读扫描后审核源、完整目标、数量与字节，确认后重新扫描并在同一 SSH 连接执行。空目录保留，目标必须是新目录；32 层、10,000 项、16 GiB、30 秒扫描/15 分钟执行限制，拒绝静态符号链接及跨平台危险名称。失败/取消可能保留部分目录，不自动递归删除。长确认条采用受限高度滚动正文与固定按钮，防止完整路径挤出确认/取消。参见 `testing/records/2026-10-03-recursive-transfer.md`。

最终本机整合通过 344 项单元/集成测试、2 项文档测试和 47 项打包回归。原生 macOS 已核验 AI 重启锁定/错误与正确解锁、手动测试、凭据主密码轮换与未关联条目清理，以及递归目录上传/下载哈希一致、中英文审核及取消不创建目标。程序、两项回环服务已退出，SSH 临时文件树已清理。详细构建哈希和证据见 [集成验收](testing/records/2026-10-03-credentials-tree-integration.md)。

shell/exec/SFTP 通道在打开前即由独立任务持有，覆盖迟到确认和结果交接取消；SFTP 的高层关闭拥有独立取消控制，避免写入背压阻塞关闭。正常清理保留共享 SSH，异常超时允许有界断开同一 TCP；CLOSE 入队不代表远端确认。独立复审及 80 项 session 测试见 [专项记录](testing/records/2026-10-03-session-channel-ownership.md)。

随后 Quality 在 Windows 目录取消/FIFO 测试发现栈溢出。四处传输缓冲改为直接堆分配，保持原有分块大小与取消语义；新增两项 Future 尺寸回归，取消/FIFO 在显式 2 MiB 线程运行。修复后的本机整仓门禁通过 346 项单元/集成测试与 2 项文档测试，session 为 82 项。代码提交 `9c88c58` 的 Quality 运行 `37092321464` 在 macOS、Ubuntu、Windows 全部通过，Windows 取消/FIFO 及尺寸回归成功。原始失败、本机未复现边界及重跑证据见 [小栈回归](testing/records/2026-10-03-transfer-stack.md)。上述 GUI 哈希属于内存布局修复之前的构建。

## 命令片段与本地建议增量

命令工具区现在无需 SSH 会话即可管理显式保存的片段，支持新建、编辑、搜索与二次确认删除。领域 CRUD 保留 UUID，以候选快照验证，存储继续使用现有 revision/锁/原子写入。编辑器保留名称、说明、CSV 标签及多行命令原文，保存中立即冻结输入；失败保留草稿。历史与未执行命令不进入配置文件。

命令栏已改为多行 Textarea，解决单行控件删除换行的问题。最多 8 项本地建议来自当前 SSH 历史和片段；后台单 worker 合并输入，迟到结果验证目标、输入 revision、原文和来源 generation。点击候选再次检查来源与目标，只填入不执行。程序填入显式推进 revision，因为 set_value 不产生 Change。建议的键盘交互捕获 Input action 而不是原始按键；IME 组合态交还输入组件，上下键将所选行滚动至可见范围。详见 [设计](adr/0010-command-snippets-and-local-suggestions.md) 与 [集成验收](testing/records/2026-10-03-command-snippets.md)。

该增量本机整仓门禁通过 385 项单元/集成测试、2 项文档测试、格式、严格 Clippy 和依赖策略。最终 macOS 构建验证了 Enter 仅填入多行片段、明确执行后回显和会话历史显示；程序、夹具正常退出，临时根目录与监听已清理。二进制哈希和失败回归记录保存在上述验收记录；Windows/Linux GUI 未据此验收。

领域提交 `2ea9b1c` 与整合提交 `02fc8fc` 已推送。整合提交的 Quality 运行 `37094089051` 在 macOS、Ubuntu、Windows 全部通过；Rust 测试总数分别为 387、387、382（含文档测试），各有 47 项打包回归，新增 10 项工作区 GPUI 回归均通过。按平台条件编译的数量差异已在集成记录列明，尚未创建发布标签。

## SSH 跳板路线与首页增量

已保存 SSH 配置可关联最多四个跳板，逐跳独立指纹、密码/密钥/agent认证，最终目标独占终端和最近记录。每次尝试持有专属父链，取消或关闭目标释放链，迟到网络/凭据/信任回调按请求token隔离。主机信任与版本2加密凭据绑定规范化路线；首跳兼容旧直接信任，版本1凭据仅可直连。修改上游目的地或认证会使下游凭据引用和最近记录失效；删除、恢复、导入遵守完整依赖图。设计见 [ADR 0011](adr/0011-owned-ssh-jump-routes.md)。

跳板选择器支持搜索、分页与拒选原因。连接编辑、认证、指纹弹窗采用固定按钮和滚动正文，普通认证为紧凑布局；进度卡不抢占其他编辑器焦点。最终整仓门禁通过439项单元/集成与2项文档测试，47项打包回归通过，新增8项真实SSH工作区测试涵盖取消A后启动B及目标Save→Unlock交接。macOS原生完成逐跳认证/指纹、SFTP与终端、英文路线、取消/重连及链清理；最终构建哈希和证明边界见 [集成验收](testing/records/2026-10-03-jump-integration.md)。

中英文README按成熟开源项目首页结构重新整理，展示真实截图、能力、首次连接、AI审阅、源码运行与贡献入口。仓库公开内容和提交说明持续进行名称扫描，研究来源与链接检查见 [首页记录](testing/records/2026-10-03-readme.md)。

## 传输暂停、续传与 OpenSSH 互通增量

普通文件和目录传输支持后台 ACK 确认的暂停、继续、取消。显式续传先只读扫描、审核，再在同一 SSH 连接复核完整源 SHA-256 和目标全部前缀；目录先检查整棵树，通过后补齐缺失项和空目录，拒绝额外目标、链接和冲突。暂停期间保留 FIFO 位置且不消耗活动预算；重连或重启后重新建立计划。完整内容保证只适用于显式续传，取消仍可能留下部分输出，SFTP v3 不提供文件系统事务。

文件面板按 worker、传输状态、视图和协议夹具拆分。新增 8 项真实 GPUI+SSH/SFTP 回归覆盖审核、取消、迟到消息、语言、目标和窄面板；本机整仓门禁 469 项单元/集成与 2 项文档测试通过，47 项打包回归通过。真实 OpenSSH 4 项测试另行执行，发现并修复 subsystem 确认前合法 WindowAdjusted 被误判为关闭的问题。新增 macOS/Linux CI 互通步骤与独立进程/临时密钥清理回执；最终 macOS 构建完成双向文件与目录续传、只读审核、暂停/语言切换/继续及内容哈希比对；应用、测试服务、临时密钥和远端树已清理。首次 Quality 的 macOS/Ubuntu 普通测试和 OpenSSH 互通通过；Windows 揭示目录下载续传查询不完整 verbatim 盘符路径的错误，已改为检查完整绝对祖先并补真实文件系统回归；原失败流水线保留，修复提交 `7f95c46` 的 Quality `37099262632` 三平台全部通过：macOS/Ubuntu 各 472 项普通测试、2 项文档测试和独立 4 项 OpenSSH；Windows 465 项普通与 2 项文档测试，47 项打包检查按平台记录跳过。设计见 [ADR 0012](adr/0012-resumable-sftp-transfers.md)，当前构建、原生验收与失败证据见 [集成记录](testing/records/2026-10-03-sftp-resume.md)。

## 每跳上游代理增量

已保存配置支持 SOCKS5 或 HTTP CONNECT，代理可以分别置于本机到首跳、前一跳 SSH 到后一跳之间；匿名与用户名/密码认证、远端域名解析、严格协议边界和无直连回退已接入。配置只保存端点及用户名，代理密码仅本次使用，并与 SSH 保存/解锁流程分开。代理配置参与规范化路线身份；无代理保持版本 1 的旧键，含代理使用版本 2，单跳代理也不继承旧直连指纹或旧凭据。

连接编辑器增加可折叠代理配置；认证、指纹和路线预览显示每跳协议及端点。取消、失败、保存后路线变更及迟到网络/凭据回调继续受请求身份和资源所有权检查。固定操作按钮、可滚动正文与内容最小高度回归覆盖中英文和 480/760/1280 宽度。

本机最终整仓门禁通过 523 项普通测试与 2 项文档测试，独立 OpenSSH 4 项及打包回归 47 项全部通过。独立复审未发现剩余阻断；macOS 最终构建验证 SOCKS5 → SSH 跳板 → HTTP CONNECT → SSH 目标、逐跳认证/指纹、错误密码、取消后重新连接、英文认证、目标终端回显和 SFTP 中文读取。退出后两类代理连接、专属父链、夹具进程、监听与临时根全部清理。最终二进制哈希、初始失败与证明边界见[验收记录](testing/records/2026-10-03-upstream-proxies.md)，设计见 [ADR 0013](adr/0013-upstream-ssh-proxies.md)。此项不覆盖 HTTPS/PAC/企业代理认证或代理密码保存，也不代替 Windows/Linux 桌面验收。

## 已建立会话重连增量

默认手动、可选有界自动重连已接入保存配置。typed 连接/shell 终态区分正常退出、显式关闭、传输丢失和保活超时；Ready 以 PTY/shell 确认为准。原标签位置替换为新 EntityId，按帧排空旧输出后移交有界历史，旧输入/解析器模式不继承。完整 SSH/代理路线重新建立，认证和信任提示需显式继续，预算按整条路线计数并在稳定 30 秒后重置。

旧命令原文保留并要求重新审核；旧 AI 请求/上下文撤销。文件、监控、隧道面板保留最多一份上一会话快照，远端能力已停用；真实取消/写入回执保留，不自动恢复任务。旧快照有未保存草稿时，同意绑定面板及文本，完成前再次复核，迟到结果不能丢弃新编辑。设计和最终验证进度见 [ADR 0014](adr/0014-remote-session-reconnection.md) 与[验收记录](testing/records/2026-10-03-reconnection.md)。

上一增量提交 `6ac8f90` 的 Quality `37100961437` 三平台全部通过：macOS/Ubuntu 各 523 项普通与 2 项文档测试、47 项打包检查和独立 4 项 OpenSSH；Windows 516 项普通与 2 项文档测试、46 项打包通过及 1 项 Unix 权限跳过。本次重连增量不能引用该提交结果代替自己的门禁。

## 远端命令与路径补全增量

命令栏显式远端补全已接入。Core 分析光标词并安全引用；Session 固定 PATH 探针与 SFTP 只读扫描；App 按会话、原文、光标、目录和请求身份复核候选，只有显式接受才局部替换，Enter 不执行。每会话补全目录与交互终端 cwd 独立，可编辑或显式取 Files/SFTP 起点；普通输入不发查询。取消、IME、Undo、新实体隔离及中英文小窗口布局均有真实 GPUI 回归。

本机整仓 642 项普通与 2 项文档测试、严格 Clippy、格式和依赖策略通过；47 项打包、独立 5 项 OpenSSH 通过。macOS 原生完成多行中文词替换、引号/空格名称、目录继续输入、单步撤销、仅光标失效与语言切换，最终包另做冒烟；所有应用/夹具、监听和临时树已清理。设计见 [ADR 0015](adr/0015-remote-command-completion.md)，精确构建哈希、原失败和平台边界见[验收记录](testing/records/2026-10-03-remote-completion.md)。交互 alias/function 与可编程参数补全仍未实现。补全提交 `8cd89fa132f7695400ca50b3297f533d5efcb8ee` 的 Quality `37105632462` 已在 macOS 26、Ubuntu 24.04、Windows 2025 全部通过；macOS/Linux 独立 OpenSSH 成功，Windows 按配置跳过。后续变量片段和批量 exec 增量如下，不能沿用该提交的通过状态。

## 参数化片段与批量 exec 增量（已通过本地与远端 Quality）

片段增加显式 `parameterized` 开关，旧配置缺字段默认关闭，旧双花括号正文不解析。Core 纯编译推导最多 32 个参数，完整词/赋值值/长选项值按 POSIX 字面引用；值不会保存到片段。编辑器显示语法和变量列表，新参数弹窗提供未填写/显式空值、只读完整预览和固定按钮。确认前重读输入，工作区最终复核原文/revision/目标 EntityId/完整片段快照；拒绝后保留值。展开命令默认不记会话历史，仍需单独点击执行。

批量任务对当前已认证会话显式选择 1–32 个目标，审核共同命令、并发、超时和失败策略后用独立 SSH exec 执行。命令不注入 PTY、不继承终端 cwd、不自动重试或重连；后端持有旧连接，实体变更不会把任务导向新会话。每行显示有界 stdout/stderr 和明确退出/拒绝/未开始/未知结果；隐藏面板保留当前批次，取消不证明远端进程终止。

已确认模板 20 项领域/存储与 4 项真实 `/bin/sh`、14 项真实 TCP 批量协议、编辑器 9 项和参数组件 6 项 GPUI，以及当次 6 项工作区专项通过。实际小视口回归包含 32 参数滚动和中英文固定按钮。曾发现预览 state 的只读状态被控件渲染默认值覆盖，现视图显式只读并以真实输入不变验证；原失败保留。最终整仓门禁、最终包哈希与 macOS 原生、六项 OpenSSH 和新提交 CI 结果须由集成负责人追加到[验收记录](testing/records/2026-10-03-parameterized-snippets-batch-exec.md)。本节尚不代表完整集成通过。设计见 [ADR 0016](adr/0016-parameterized-snippets-and-batch-exec.md)。

## 图标与打包

`assets/icons/source.png` 是用户选定白底版本；`SOURCE.md` 记录来源。macOS ICNS、Windows 九尺寸 ICO、Linux hicolor PNG 共35个产物经过尺寸、alpha、容器目录、像素、哈希与重复生成检查。macOS 原生 iconutil 解码通过。

`packaging/package.py` 已用真实本机二进制生成 .app 并启动；主程序接入 app identity、Linux desktop/app_id 和 X11 内嵌图标，Windows build.rs 接入资源。六平台原生构建与发布流水线已配置，实际运行状态见发布记录；Windows/Linux 桌面显示仍未验收，未签名、公证或安装。

## 接续顺序

### 2026-10-04 一次性 SSH 快速连接

无活动会话时已加入可操作的快速连接表单：主机、端口、用户名、Agent/私钥或密码认证均在表单中完成，连接库保留在表单下方。点击“连接”构造不写入 `AppState` 的直连路线，复用现有认证和指纹核验流程；取消或成功连接都不会创建连接库条目或最近记录，成功的临时会话也不创建重连绑定。点击“保存为连接”只打开标准连接编辑器，仍需再次点击保存才写入配置。Core 路线、GPUI 交互和无持久化回归见 [ADR 0024](adr/0024-one-time-quick-connect.md) 与 [验收记录](testing/records/2026-10-04-quick-connect.md)。

该切片的本地测试和 GitHub Quality `37157850288` 证明一次性路线和存储边界，不代表真实生产主机或 Windows/Linux 原生窗口已验收。高级跳板、代理、重连设置继续使用持久化编辑器。

1. 最新 .app 已验证文件编辑器高度修复和访达白底图标显示，详见 testing/records/2026-10-03-remote-ai-icons.md。所有测试夹具和配置位于 ignored work，仅用于受控验收。
2. 本阶段测试记录和实现已本地提交，测试App与两个夹具均已退出，SSH临时目录已清理。最新用户授权已覆盖原“仅本地提交”限制。
3. SSH 凭据库 UI 与终端搜索已接入并补齐焦点隔离测试。连接目录/回收/最近与动态 SOCKS5 已补齐本轮切片；凭据维护、AI 加密密钥和有界递归传输已接入；命令片段与本地建议已补齐；已保存跳板路线已补齐；传输暂停/继续及显式内容校验续传已补齐；每跳上游代理已补齐；已建立会话重连和显式远端 PATH/字面路径补全已接入；变量化片段和基础批量操作审核已接入；远程文件面板新增审核式 POSIX 权限修改，使用独立 SFTP `SETSTAT`，执行前复核目标 mode/类型及父目录、执行后读回确认，仅允许 0000–7777 八进制 mode 并拒绝符号链接；公开起点提交 `790bf1c` 的本地专项已通过，Quality 运行 `37119306924` 在 macOS 26、Ubuntu 24.04、Windows 2025 全部通过，其中 macOS/Linux 的独立 OpenSSH 互通也通过。当前仍未完成全部远程 SSH 能力目标。

## 待补的完整性

凭据备份/恢复与跨文件事务、外部 ProxyCommand、远端进程的断线恢复、交互 shell 可编程补全、逐目标自定义参数映射/定时工作流/任务级持久化审计、传输自动恢复/并行、ACL/所有权/差异应用、TCP 协议级服务健康、更多监控、同步、打包和 Windows/Linux 原生验收均需继续追踪。手工依赖工作流与基础批量摘要审计已实现，当前整合验收边界见顶部最新状态。键盘交互认证的 UI 提示收集已接入：密码或私钥认证可显式切换为 server-driven MFA，挑战逐批显示在双语模态中，答案只经一次性有界通道传给当前路由，不写入凭据库；外部 MFA 服务与跨平台原生窗口仍未验收。当前 TCP 探测只证明远程主机完成握手，不代表协议或应用已就绪。终端搜索目前只搜索活动标签的本地滚动区，不读取远端文件或重新执行命令。

工具栏已接入“关于/更新”面板：它显示内置变更日志和项目主页，用户点击后在后台检查固定 GitHub Release，下载当前平台资产并验证同一发布的 SHA-256。已校验包保存在唯一临时目录，可从面板查看；显式点击“自动安装并重启”后，由同二进制的隐藏 helper 再次校验 `package-manifest.json`，仅替换清单文件，文件占用有界重试并在失败时回滚。开发构建或非标准安装会退回手动安装；签名、公证、安装权限和三平台原生验收仍未完成。设计与验证见 `adr/0021-about-and-update-panel.md` 和 `testing/records/2026-10-03-update-panel.md`。

## 迁移核验

本阶段功能提交 `2ee0693c02f1ceca4796d5ccc500ff48a487bd6a` 与验收记录提交 `184d214d38480d21e05ab5638d3cc3e741e7a1e1` 已推送到公开仓库；功能提交的三平台验证见 [Quality 37125780395](https://github.com/cyruss648/keelshell/actions/runs/37125780395)，验收记录提交的三平台验证见 [Quality 37127192429](https://github.com/cyruss648/keelshell/actions/runs/37127192429)。旧产品别名已从可达 Git 历史清理，详细证据见 `testing/records/2026-10-03-final-feature-slice.md`。

目录通过同一文件系统 rename 移动，82 个源码/配置文件 SHA256 前后一致；迁移时 Git HEAD 与 porcelain status 前后一致，旧目录不存在。`git fsck --full` 初次发现 Finder 的 `.git/refs/.DS_Store`，已可恢复地移至 ignored `work/relocation-quarantine/refs/.DS_Store`，复检通过。迁移后的历史清理和远端更新见上方最终验收记录。

测试日志与本应用的安全空状态截图复制到 ignored `work/relocation-evidence/`。第三方应用中含用户连接树的截图留在原任务私有工作区，不进入仓库。原始失败记录保留，不能删除以掩盖失败。
