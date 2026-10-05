# 外部 MCP 受审阅文件替换 — 2026-10-05

状态：A 新非作者独立复核发现 P2，不准整合；B 新非作者限定复核、主树完整门禁和新源码绑定的 macOS 八工具文件审阅流程通过。新提交 CI、供应商第八工具和其他平台原生仍待验收。下文作者历史状态保留，不覆盖本节最新结论。

基线 `23310ee13286adb488a451277addf49b05cd476b`，独立 managed worktree 分支 `feature/mcp-file-proposals`。新增第八项固定 MCP 工具、独立目录/工具授权、内容 SHA、仅后台 baseline/diff、独立固定页脚审阅、捕获 handle 与单次批准、既有 POSIX rename writer 的两次完整 baseline 检查及原有状态增加 action_kind。没有依赖或工具链版本改动。

作者缓存由 root 授权 APFS clone 并核验：545,682 文件、6,558 子目录、109,782,259,995 bytes，0 size/type mismatch、0 shared inode、0 symlink。第一份逐项验证脚本低效 list membership 扫描由作者 SIGINT 中断（exit130/traceback 保留）；第二份使用 stat type 完成 11.42 秒后停止读取 root cache。后续 Cargo 只写自有 target 与绝对私有 TMP。

MCP crate authority/IPC/stdio/EOF/stream/doc 首轮全部通过，新 19 authority 测试含独立权限、组件路径边界、错误 hash/未知字段/UTF-8 byte overflow、唯一 ID/digest 绑定、内容 SHA 防伪。隔离真实 TCP-SSH/SFTP 的 4 个 reviewed-file 专项通过，覆盖 exact bytes/rwx 保留/临时清理、staging 前内容和 mode 冲突 0 atomic write、缺原子扩展 0 write、RAII WRITE 屏障内并发写原文件后最终复核拒绝并保留外部写入。

应用 MCP 首轮编译发现测试辅助 Option `.checked` 的方法名错误；第二轮发现不存在的测试 `try_bounds`，均保留失败日志并修正为既有 `.checked_option` / `window.try_find`。新 17 项应用 MCP 专项最终通过（8 项文件提案和原 9 项回归），经完整门禁再次运行。仅文件提案授权的新测试曾暴露 list_sessions root 元数据还只认可旧 read/list 权限，已限定补入显式 FileChange roots；原失败和实际 grant 工具集诊断保持。重复授权辅助现精确选择工具集，不继承旧 toggle 状态。

所有夹具为进程内隔离 SSH/SFTP/IPC/stdin 协议对象，未连接客户服务器、实际供应商模型或云服务；未启动 GUI、安装或覆盖用户应用。旧七 tool macOS 开发包/Claude/Codex 原生实验不迁移为新增八 tool 验收。SFTP v3 不能原子比较或排除最后检查后的恶意服务端竞争；未知结果不自动重放。原失败、自有缓存和证明保留待 root 冻结核验及可恢复归档。

最终 `scripts/check.py` exit0：x.y 策略、格式、全仓 strict Clippy、1075 普通测试、8 文档测试、6 脚本测试及默认/显式 2 MiB 本地控制器全部通过；默认 11 项忽略仍为 2 项供应商选择和 9 项系统 OpenSSH，未冒充执行。应用共 388 项通过，session 单元 66 项通过，SSH loopback 共 102 项通过（包含 4 项新增 reviewed-file）。57 项打包测试独立通过。完整 compact 文件审阅覆盖 >1 KiB 精确 canonical 路径/AX label、40,000 bytes 旧文与 48,015 bytes 新文共 6,501 正文行、方向控制字符/末行无 newline、中文/英文及 System/Light/Dark 六组合 900×580 固定页脚、关闭重开仍待审和授权背景卸载。

原完整门禁中的 diff 计数错误（标题也被字符串子串匹配）保留，现改逐行精确匹配。共用 atomic writer 的实质 future 帧回归也保留：普通 atomic upload 17,008 bytes 超原 <16 KiB 守卫；没有放宽阈值，新增两次 baseline 子 future 单独 Box::pin 后原守卫通过，queued upload/download 为 3,696/3,688，atomic upload/write 为 8,312/8,120 bytes，最终完整门禁再次通过。严格检查首次要求两个大 enum payload 间接持有，已 box FileChangeProposal；测试 AX 方法/Role 名拼写的失败也保留并修正为既有 aria_label/Label。

作者仅额外进行宿主 GUI/MCP 两程序编译，结果与 binary hash 置于 ignored 证明；编译不是原生窗口验收，未启动 GUI。生产及测试源码在最终完整门禁启动时捕获 hash，结束后保持不变；仅 ADR/本记录追加真实状态。冻结 patch/source/proof manifest 由 root 核验后交 fresh 非作者独立审查。没有 commit、push 或发布标签。

## 独立复核否决 A 与新 B

新非作者实证 P2：有效 FileChange 请求占用 preparing=1 后，生产 grant_mcp 对不存在的授权目录失败；旧 authority 仍有效，但旧完成帧因 revision 不同先 continue，未释放准备计数。32 次同样的失败重新授权产生 preparing=32、0 live preparation，第 33 个有效请求错误 Busy，0 atomic write、原文不变。诊断 exit0 只证明原缺陷可复现，不是产品通过。原 A 26 项源码/补丁/38 份证明均冻结且由 root 核验复制。

B 从同一生产基线创建独立 managed worktree，只增加 generation + preparation ID 所有权集合；计数是集合长度的投影。完成帧先精确释放自己的 ID，再保留原 revision/lease/目标 guards；成功新授权或 stop 清空旧项。迟到旧帧不能释放新 generation 的准备槽，也不能进入审阅。失败重新授权、成功授权、撤权、闭合 IPC 和迟到帧均分别验证，不放宽原 7 秒测试等待或原 future 尺寸守卫。

B 缓存只从已停 A 自有 target 经授权 APFS clone：562,159 files、6,760 子目录、113,254,495,874 bytes，0 mismatch/shared inode/symlink，16.59 秒完成核验。没有访问 review 或 root shared cache，后续仅写 B 自有 target 和绝对私有 TMP。新增两项长期回归使用仅 cfg(test) 的真实 SFTP 完成帧暂存门，验证失败重授权完成后 preparing=0/旧 scope 保留、新 grant 已占槽时旧帧不能释放它或进入审阅、closed reply 自项释放及撤权后 0 write。此前直接调用 private grant_mcp 的编译失败和自动 maintain 抢先消费完成帧的 7 秒观察失败保留；改实际授权按钮与精确测试帧所有权，不改生产分支或原等待。

从根已封存的非作者 proof 逐 bytes/SHA 核验三份原始 RS；只在私有 cfg(test) overlay 追加，执行后按三个备份文件 bytes/SHA 原样恢复。原单次 P2 的 preparing=0 断言、新旧 painted frame 拒绝和 staging 后 mtime 冲突/临时清理均原样通过。原 32 次“观察缺陷”诊断保留原断言，在 B 第一次结束时 left0/right1、exit101，是修复后旧观测不成立，未改写为产品 PASS。另一个独立 fixed32 版本保留全部 32 次、7 秒等待、旧 scope/原文/0write 断言，使用真实完成帧暂存门后每轮 idle count=0、下一请求 pending_file_change，exit0；其首轮自动准入抢先消耗 count1 的观察超时也保留，未扩大期限。

B 最终完整 `scripts/check.py` exit0：1077 workspace 普通、8 rustdoc、6 scripts、11 原显式忽略；strict workspace Clippy、fmt、x.y 和默认/显式 2 MiB 控制器通过，重复控制器另记而不加计。应用 390、MCP 应用专项 19、session 单元 66、loopback 102 全通过；57 打包另通过。原 <16 KiB future 守卫随完整门禁通过，无阈值或断言改动。最终生产/测试 hash 在完整门禁启动前捕获并保持，结束后仅本记录、产品状态和 ADR 追加真实结论，补丁/源码/证明再冻结交 fresh 非作者。

第一轮显式 MACOSX_DEPLOYMENT_TARGET=15.0 双程序 dev build 命令 exit0，追加 Mach-O 复核实证 app minos15.0、MCP minos11.0，作者额外“双 minos15”期望断言失败。命令通过和实际 minos 分别记录；MCP minos11 本身兼容在 macOS15 运行，不据此认定产品缺陷。原 log、load commands、两个 binary hash 和 MCP 原 binary 均保留在 ignored 私有证明；Cargo 重用旧产物是候选解释，未宣称已证明上游原因。只对 B own target 的 MCP package cache 做有记录的失效处理，同一显式 MAC15 命令重新构建 exit0，新的两份 Mach-O minos 均实际15.0。没有改生产/测试源码、工具链、依赖、构建策略或发布配置，没有读取 root/review/A cache；两次编译和 minos 核验均不是 GUI、供应商或 macOS 原生验收。

## Fresh B review and root import

A fresh non-author review passed the limited B scope with no newly proven
P1/P2. Original MCP71 ordinary/1doc, GPUI19, reviewed TCP-SFTP4 and the
unchanged future guard passed; private exact ownership/capacity/digest/path/
approval/file-boundary probes and restored-source strict workspace Clippy,
formatting, x.y policy and6 script tests passed separately. Original failures
remain, including the old leak-observation assertion whose expected leaked
count is correctly no longer true; it is not a healthy32-cycle pass.

The corrected lifecycle fixture measured actual SFTP handler and READ
ownership. Across6 grants with8 blocked reads each, new-grant readiness had
zero old local workers/reservations. Remote work could remain:48 READs and48
handlers after the final grant, peak56 handlers during preparation. Releasing
the owned READ gate eventually brought every count to zero, with zero writes
and unchanged original bytes. The first wrapper-based remote count is not
valid remote teardown evidence because the SDK processing task outlives the
server-run wrapper. A local cancelled future or queued close is not remote
completion; these observations do not establish a cross-generation global
remote limit of32. SFTP v3 final-check/rename is still not atomic CAS.

The reviewer wrote REVIEW and FINAL reports and restored all26 source files;
model-service capacity interrupted only final proof packaging. Root preserved
and verified145 existing files, with a root preservation manifest SHA256
`0c5d5c9554dd4c3fc538e46bc95151035f36abe710f8a344106e39576bd216ba`.
No uncreated reviewer manifest is claimed. Actual recorded strict-check
receipts were inspected independently.

Root imported the reviewed production/test files into main based on773a113;
they retain exact author SHA. EXTERNAL_MCP also keeps later main history,
so its full-file SHA intentionally differs from the older233 candidate.
Main integration checks and a new source-bound eight-tool native desktop slice
passed as recorded below. New exact-head CI remains pending. No API
request-options candidate was imported with this feature.

## 主树整合与新八工具 macOS 原生

主树完整 `scripts/check.py` exit0，382.385 秒：1077 普通、8 rustdoc、6 脚本、fmt、严格 workspace/all-targets Clippy、x.y 策略及默认/显式 2 MiB 控制器通过；43 个 Rust harness，原 11 ignored 不计执行。工程输入启动前后一致，与后续构建输入一致；21 份生产/测试 Rust 文件逐 SHA 与独立复核的 B 冻结相等。没有整合 API C/D、修改依赖、锁或工具链。

显式 MAC15 主树 GUI/MCP 编译、真实 loopback fixture 编译、标准 macOS 双程序打包与原生结构检查分别 exit0。新 app SHA256 为 `4d5bd6f06f4b604f964d326d79b3efd7ee7718b83b8899e333b168a50738eeb0`，MCP 为 `2963d9bb7023aa476ecfab3f88638043afc4c4e02c6824e1baa13606c5f32503`；这些值绑定工作副本工程输入，不冒充尚未产生的新提交。没有安装、签名、公证或覆盖用户现有应用。

自有隔离 HOME/TMP 的新原生应用实际连接 TCP SSH/SFTP 夹具；先核对自有主机指纹，再输入一次性测试凭据。桌面仅授权一个当前会话的枚举、目录/文件读取、文件提案和状态查询，canonical 根为 `/bin`。新外部 stdio 测试客户端协商八项工具，实际列目录、读 UTF-8 原文和完整 SHA；启动配置中的临时能力秘密只在内存使用，没有保存。

首次操作的提案已进入待审，但点击当时屏幕外的 AX 按钮后未打开审阅，桌面授权变更，客户端 EOF/exit1；没有执行批准或写入。原失败及退出身份记录保留，具体原因未确立，不把该次记为产品 PASS。第二次使用新授权配置，先滚动让提案按钮完整可见，再实际打开独立完整审阅：路径、路线身份、前后摘要、中文原文/替换的全部 diff 与固定页脚均可见。

第二次闭环 exit0：等待人工审阅时实际原文件字节不变；点击“确认完整目标并替换文件”后状态 succeeded，真实 SFTP 读回中文替换和精确 SHA；另一提案在完整审阅内点击“拒绝修改”后 rejected，原内容不变；第三条提案准备完成后，测试方独立写入不同的原文件内容，再批准时状态 failed，真实 SFTP 读回仍为外部新内容，未覆盖。界面最终分别显示成功、拒绝和失败。最后明确撤销全部授权，再停止自有 GUI/fixture。

根读回核验两次 companion 均已退出且原 PID/birth 不存在、两次客户端私有目录删除；GUI/SSH fixture 原身份消失，fixture 监听已关闭，两个未使用 HTTP 控制监听/线程退出，原生私有目录删除。HTTP origin0/proxy0，仅作未调用控制，不作为 API 测试。原生后工程输入及 21 份 B Rust SHA 仍相等。原始日志、失败、客户端回执及根核验在 ignored 证明中保存。

本切片只证明 macOS 新标准开发包、自有八工具客户端与自有 SSH/SFTP 的批准/拒绝/已观察并发变化保护。900×580 中英/三主题文件审阅属于独立 GPUI 测试，本次没有重复该原生矩阵；实际供应商第八工具、Windows/Linux 原生、六目标 Release、签名和安装更新仍未验收。已观察并发修改拒绝不提供 SFTP 原子 CAS，也不排除最后核对后的服务端竞争。新提交 CI 单独记录，旧七工具供应商记录保持原范围。
