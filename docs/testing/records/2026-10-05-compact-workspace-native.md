# 紧凑工作区与受控 SSH 原生验收 — 2026-10-05

状态：旧标准macOS开发包完成有限业务原生验证并在900×580英文/Light/AI/真实Files组合复现约37px终端P2。新紧凑布局、32完整Files/16完整候选GPUI场景与根1031普通+8文档+6脚本/57打包门禁通过；fresh独立完整工作区复审与新macOS包八组合/显式补全原生闭环PASS。本记录将修复前后的源码与程序分开归属，旧二进制/截图不能作为新修复原生通过。

## 源码、程序与运行范围

构建来自HEAD `d9eeaef28ca54c611086f4355273967d24a11924`及根四个Rust修正，生产patch SHA-256为 `b39a0a97d873fec2ed849d377dd7f27126cf74484c16b7de863dc379ad3799d6`。`source-build.json`保存256个生产/测试/工程文件的精确hash，逐项等于独立整合审查的根before/after冻结快照。聚合文档在同步，不能声称整棵工作区clean或全部文件冻结。

`cargo build -p keelshell-app -p keelshell-mcp --locked`与标准`packaging/package.py`生成`aarch64-apple-darwin`开发包。`package-manifest.json`的`build.commit=null`，明确表示未提交工作树构建；不是以HEAD冒充clean提交的Release。后续紧凑预算修复会改变生产源码，必须另建源码/程序绑定和原生记录，不能声称新提交等于这里的旧构建。原生结构检查确认GUI与MCP均在包中、清单5项文件hash匹配、两个Mach-O的`minos=15.0`；结构检查本身没有运行程序或完成业务验收。

| 旧包证据 | SHA-256 |
| --- | --- |
| GUI `keelshell-app` | `19f81c1c1bccd1ff9f7d19436f690991e588d603c04958fedb13593e1dbfb62e` |
| MCP `keelshell-mcp` | `069bbb239b5382f5c5203172ffd9fe080f7dc124c1fe1ab2de2e7d2a3477f53d` |
| `source-build.json` | `15d113a13ab625693ac1cbb029ec609618807d51f5bd1e0af29fdcdf67ee7c3b` |
| `stage/package-manifest.json` | `7df23b21c1d6cd9aae2ae109893c89e5295059af3faba4b46df9eb393c11866e` |
| `package.log` | `db2add7f32e1df35d3f04965bba20c1a04235c37976a368798efa1d0241cd30b` |
| `inspect.log` | `9f86d161a0d5aaf24edf3a67d389308951a57ac7d8a48438fd96b19a58f0afdd` |

根任务使用原生UI操作这份包，配置与证据位于ignored `work/workspace-integration-native-20261005/`。隔离配置含5个自有测试连接：两个业务验收目标和3个不选中的元数据样本，没有AI供应商、外部MCP授权或已保存凭据。两个SSH/SFTP服务只监听回环，分别对固定命令返回退出0和退出7；终端为受控echo，exec只接受明确的fixture命令，不是通用shell解释器。连接前分别核对夹具回执中的精确主机指纹，再在原生界面批准信任与瞬时测试密码。未连接客户主机、调用供应商服务或覆盖已有安装。

## 实际交互结果

| 场景 | 观察与证据 | 证明边界 |
| --- | --- | --- |
| 窗口、语言、主题与AI | 01为中文/System宽窗口；06成功缩为900×580，07为英文/Light/AI开启及真实SFTP文件区 | 没有完成完整原生主题/语言/AI矩阵或真实OS样式变化 |
| 文件区 | 活动SSH通过SFTP加载7个真实条目，900×580仍可见目录及真实文件行、换行工具操作 | 本轮没有原生编辑/传输/权限/比较的写入与内容读回闭环，先前GPUI场景不能代替 |
| 连接库标签审核 | 原生选择恰好两个目标，09完整显示对象、旧→新标签、凭据引用缺失和相关会话计数，确认/取消固定可见 | 不是全部批量移动/收藏/回收/恢复/永久清理路径验收 |
| 标签写盘时机与选区 | 审核前`library-before-confirm.json`的5个对象均只含`controlled-fixture`；确认后readback只有两个已审核目标新增`native-review`/`原生验收`，其余3个保持原标签 | 是本次受控标签操作的磁盘读回；没有把所有元数据事务或并发冲突路径扩张为原生通过 |
| 两目标依赖审核 | 12完整显示两个任务的源/精确发送命令、目标与会话绑定；任务2依赖任务1。13显示并发2、单任务30秒、失败后停止待执行项、256KiB每任务输出上限；固定人工确认按钮可见 | 按钮与当前审核可达；不证明128任务/32目标最大规模、原生键盘/IME或供应商生成流程 |
| 人工确认与两任务结果 | 点击确认后14实际收集2/2终态：成功前置退出0，退出七后续失败7，精确命令均为`keelshell-batch-fixture` | 本图的前置成功才产生后续结果；本轮没有添加失败节点之后的第三任务，不声称原生证实失败前置阻止下游 |
| 输出检查 | 15实际展示中文stdout和转义后的ESC控制内容，以及固定fixture stderr；输出正文可滚动，返回/复制按钮固定可见 | ESC转义位于stdout，不将其误记为stderr；没有证明复制到OS剪贴板或全部输出上限 |
| 返回工作区 | 16返回原工作区，根实际AX观察记录两条SSH保持，当前endpoint为第二目标且SFTP条目仍在 | 窄截图标签栏只显露一个名称，16单图不独立显示两个会话身份；AX sidecar可能是diff/无变化响应，不是完整AX树 |

审核前磁盘读回SHA-256为 `30fce4a133eff8f29c403e3f997cfe03fa936db0af7149e2d8492269ff21b56e`，确认后读回为 `485aa79c323032211fcbc0d34b3f2787fa8eac535082720f9afdfa7be2cc18db`。文档整理任务逐项核对5个对象与两目标变化，并重开07、09、12、13、14、15、16原图检查；该只读核对没有再次执行UI操作或业务测试。

## 新的终端垂直预算 P2

07的实际JPEG为1800×1226，Retina 2×对应900×580客户区加标题栏。顶部工作区工具条、64px命令正文、两行远端补全、换行后的四个命令动作、底部Files与AI同时占用高度，终端只剩约37px，达不到现有80px可用终端目标。文件首行和工具的先前修复仍然成立，但独立FilesPanel布局场景和默认Commands面板的入口回归没有覆盖这个完整Workspace组合。

因此原先独立整合PASS不关闭这项随后由实际屏幕发现的P2。新完整Workspace回归已通过真实非空SSH/SFTP文件区、AI/监控与命令输入复现72.5px，随后以短窗口单行补全修复预算；保留终端至少80px、文件真实首行与有界工具、64px命令输入、远端补全及四个动作，没有提高窗口下限或删除功能。输入/目标/实体/审核保持的作者专项及根整仓门禁已通过；fresh独立复审和新包实际八组合/补全矩阵已通过，详见末节，设计在[ADR 0046](../../adr/0046-compact-workspace-terminal-budget.md)追踪。

## 截图尺寸与标签校正

尺寸由系统`file`读取JPEG头确认，原始像素未改动。原始截图和AX响应保留；AX响应有增量或“无变化”，需结合操作顺序使用，不能当独立完整可访问树。完整16张截图均在证据清单中逐SHA/bytes登记。

| 截图编号 | 实际JPEG像素 | 归属 |
| --- | --- | --- |
| 01 | 2880×1866 | 宽窗口中文/System连接库 |
| 02 | 2880×1866 | 原文件名误含`minimum`；第一次缩放未生效，只是失败resize attempt，不能作为最小窗口证据 |
| 03–04 | 2880×1866 | 后续两次缩放未生效的尝试 |
| 05 | 2880×1866 | 第一个自有目标主机指纹批准 |
| 06 | 1800×1226 | Raise窗口后实际缩放成功，900×580客户区 |
| 07 | 1800×1226 | 英文/Light/AI/真实Files，约37px终端P2 |
| 08–10 | 1800×1226 | 连接库最小窗口、两目标标签完整审核、第二目标指纹批准 |
| 11–13 | 1800×1226 | 任务目标滚动、精确双任务命令/绑定/依赖及执行选项审核 |
| 14–15 | 1800×1226 | 两任务真实退出回执、中文/控制转义输出 |
| 16 | 1800×1226 | 工作流隐藏后返回活动SSH/SFTP工作区 |

关键截图07 SHA-256为 `4ee895f7a1cbbad0f36f3bc9675acbbd60945d4bf7b12dcd383dd57f1eb5759f`，09为 `a25d1c929de5f54ec9fb11902657ba4ebf9492730519cc52ec177c4b02c2cd38`，14为 `b9fb5ecd7a4a2799cfe6b9451bf6de84f13a21110646e9e64e18013f7b58ee68`，15为 `5657e59ce8ba4ab621cf7fdf6ff3cb9bd14cd7ebfe502f36b188361dcc961493`。02误标文件保留原名和原hash，不重命名覆盖原失败证据。

## 证据核验与清理

根在原生Quit后没有继续AX读取，防止自动重新启动同一路径程序；controller、GUI和两个自有fixture均退出。`receipt.json`记录应用退出0、两个fixture退出0及各自临时根已移除；根随后独立`ps`核验仅剩表头，没有owned controller/app/fixture PID。该清理结论来自根操作与`cleanup-verification.json`，不是文档任务再次执行的进程检查。

`receipt.json` SHA-256为 `f123751a4d079261387e6982b54fc28a95d8ae9ad7ca276bfa2f4a486461bdd1`；根清理回执为 `ee9da07155dc585b6e36ae90782aa9fa02eed0545a380f12f36daafab9d5a625`。原目录52个既有文件，加本任务只读核验脚本和核验回执，共54个文件已逐SHA/bytes登记；自指清单本身单独记录，不放入自己的文件集合。它覆盖程序/清单、源码绑定、配置快照、截图/AX、操作读回、日志与清理回执，仅保存哈希元数据，未公开临时能力或凭据值。

`native-record-evidence-manifest.json` SHA-256为 `61513bec5eca24b52c2bcda2220288290dae9dae02d2c830ebbe47b14a15bf32`（8805字节）；`native-record-verification.json`为 `89b059e05558c3cf6353da85edc2f411df3a66680f0b3ff396c5815405c73160`（9401字节）。同一只读脚本还逐SHA/bytes复核独立整合审查的36份证据，并比对256个冻结生产文件；没有编译、启动应用、提交或清理review target/失败记录。

原生Windows/Linux、背景AX模态隔离、完整屏幕阅读器/键盘/IME路径、真实系统主题变化、供应商MCP互通、失败后下游阻止的原生闭环、最大规模原生流程、正式六目标Release及已安装应用自动更新仍开放。本轮固定fixture上的macOS业务证明不扩张为客户主机、真实shell或其他平台桌面验收。

## 紧凑修复的新源码与通过的原生范围

新范围仍以HEAD `d9eeaef`为基础，生产patch SHA-256为 `d0cf9487277ba8c830783957839fd4f711ad3db7b570847ee8466a7a83635b09`，256文件freeze保存在新ignored目录 `work/compact-workspace-native-20261005/`。相对前述旧开发包仅工作区视图、补全视图和依赖测试三文件变化；新标准包与实际截图已在这个独立目录建立绑定，不改写旧包清单和16张原始图。

作者完整Workspace的32个真实Files/监控组件场景和16个满144px候选列表场景通过，分别测得89.5–284.5px和100.5–403.5px终端；真实最后候选可经平台滚轮到达，关闭恢复同一Files实体和草稿。它们是GPUI/TCP测量，不是48次原生UI验收；32场景的监控服务器拒绝固定探针，也不证明真实主机监控采样。原72.5px失败、编译/duplicate module与caret诊断保留，43项作者证据经根逐SHA/bytes核验。

修复后根整仓1031普通（含362应用）+8文档+6脚本、两种完整CLI控制器和57打包通过，新GUI/MCP build亦通过；精确日志hash与11项忽略归属见[整合记录](2026-10-05-workspace-workflows-integration.md)。前一范围9项OpenSSH因传输/打包源码未改而不复跑，仍归属旧执行，后续新源码CI单独核验。fresh独立复审、新包结构/二进制hash、真实最小窗口中英/明暗/AI/Files矩阵与owned清理PASS；原37px终端P2在本次受控macOS范围关闭，证据见下节。

独立复审的127工作区/31工作流/12补全集合存在重叠，另1项私有probe覆盖80个正常态与40个实际候选态，验证700px阈值、长目标、同一SSH/Input/Files及审核绑定保持、静止指针双语tooltip和最后候选滚轮可见。39份独立证据已只读核对SHA/bytes，报告hash `41131c7815f7be992510c66a9b878765afb9edc41c5408c54073956377c5fcf1`，清单hash `299d22d47aa37dc9982657f66383b312000f92ccd7545a20195e4eb173c2b65a`。该PASS关闭代码/GPUI/受控TCP范围，原生像素、OS可访问性及本记录其余平台/供应商/发布边界保持开放。

## 新包实际900×580八组合与补全闭环（PASS）

新包GUI SHA-256为 `1bfd5e6a0b448abff6545130e3e4832efd47af7a9c77b94d125562b1252ecd84`；MCP仍为 `069bbb239b5382f5c5203172ffd9fe080f7dc124c1fe1ab2de2e7d2a3477f53d`。`source-build.json` SHA-256为 `838f3ea591ae62201ee8d560c81c308653fe7acd12e8d732828e76f0619c0c09`，新package manifest为 `fafc3bdf07978986816024ba50d1fa8bea9d7ade0e4f84b4701365cdf34eafc9`；`build.commit=null`明确未提交工作树快照，256生产hash等于新freeze。清单5个包文件hash已核对，结构检查成功，不声称安装、签名或Release。

| 原生证据 | 实际结果 |
| --- | --- |
| 07–14 | 900×580客户区，中文/英文×Light/Dark×AI开关八组合完整通过；原始JPEG均1800×1226 |
| 08/09 | 英文AI开启两主题终端人工估读约89px；其他六组合约121px，真实SFTP已加载7条目，文件首行与命令/补全操作可见 |
| 15 | 显式SFTP base查询得到/；没有布局变化自动查询的主张 |
| 16/17 | 显式Complete remotely得到7个真实SFTP候选，实际原生滚轮使末项`报告 draft.txt`可见；终端约100px，Files暂时折起 |
| 18/19 | Dismiss恢复Files，保留同一endpoint、目录/和`cat `尾随空格草稿；Files directory按钮回填/。19完整AX原文确认尾随空格与两个目录动作 |
| 04/05/06 | 04误名minimum但角拖拽失败仍2880×1866；05只改高度为2880×1226；06起才真正900×580。失败原图不改名或删去 |
| owned清理 | controller/app/两fixture均退出0；根独立核验四PID、两个监听消失和两个临时root移除，未安装应用 |

原生高度来自对原始Retina像素的人工近似换算，不能当作GPUI精确bounds或为其添加小数精度。文档任务重新打开08、09、17、18原JPEG，读19完整AX并用`file`确认全部19张尺寸；这是只读复核，实际UI动作由根完成。新round只连接fixture0，第二fixture7仅启动后退出；fixture终端是UTF-8 echo、监控实际报服务器拒绝exec，没有成功主机监控采样。没有在这份新包重复文件写入/传输/工作流执行、云模型/外部CLI/MCP，旧0/7工作流只能归属前述旧包范围。

`native-result.json` SHA-256为 `96954fc8a20cd564949e2fc3f624a8d32420166eb7bcd0709c818b3710422235`；根cleanup回执为 `5abd4261d56ce39a2c47a7c9fa5ee6cad9a84cbaa7018f8e4ae9fabd9a76ba45`。新 `native-record-evidence-manifest.json`登记52项metadata/source清单、日志、19 JPEG/AX、controller、结果及清理，排除data凭据、stage二进制/资源和tmp，其SHA-256为 `802827059486ab2aee71e1f7125d7a9e693651ba3fdb6065d6267521c775e43d`（8759字节）；文档只读verification为 `4523775fa3fc0fd23ca2122a47a0b594cd053b89a159802ae2d99fdaed9545e3`（8691字节）。根另逐SHA/bytes核验全部52项通过，回执在 `work/workspace-integration-evidence-20261005/compact-native-verification.json`。

约37px原生P2在本次受控macOS紧凑布局范围关闭。生产提交40824f0已推送main，新Quality37232614315已结束：Linux/Windows成功，macOS的SSH测试在认证阶段超时，整次失败；Windows/Linux桌面、OS主题真实变化/背景AX、完整键盘/IME与视觉矩阵、其他文件/工作流业务原生路径、供应商MCP、正式Release/已安装更新继续开放。根实际删除两个独立review的idle target/private-tmp，追加清理回执，原36/39冻结清单和source/probe/失败证据保持。文档与ADR可单独更新，不声称它们与各次审查snapshot逐字节相同。

后续仅测试夹具的修复、原CI失败及新源码验证见[SSH夹具记录](2026-10-05-ssh-timeout-fixture-stability.md)。这两个测试文件与本记录开发包的256文件快照不同；新本机构建和测试不重新归属上述19张截图或新建桌面验收。
