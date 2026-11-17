# 独立 Zmodem peer 交叉联调 — 2026-11-17

状态：本批源码及完整工程门禁通过，独立peer仍PARTIAL，原生／发行另验。
基线`ac7ec43e6a42329417a99b5933bc171faa19caf6`；
设计见[ADR0158](../../adr/0158-independent-zmodem-peer-matrix.md)。
上一轮Windows SSH验收已交付；本轮不改变其结果或旧Zmodem失败原件。

## 当前分工与已取得结果

父负责真实Rust SSH夹具、协议调查、集中Cargo槽和最终整合。
Maxwell只在自有work目录准备未修改上游独立peer与来源／构建回执；
Lovelace只修改Python OpenSSH runner及其测试，不运行Cargo。
没有新建用户chat或worktree，已有其他项目容器保留。

本机Rust／Cargo均实际观察为1.100.0；项目不pin工具链、不设置rust-version。
唯一target继续归属本checkout，device16777231／inode269342313，未clean或移交。
基线765工程输入aggregate为
`fb78496b8d0b22bd6fc38da46df994d16c144d1f040f83ff468a98c49ff633af`。

| 检查 | 当前实际结果 |
| --- | --- |
| Rust格式 | `rust-format-r1`，0/reaped/2.238723s。 |
| 新Rust用例库存 | `peer-rust-inventory-r1`，0/reaped/2.411136s；实际编译16项，含6个默认参数控制及10个opt-in真实peer用例。 |
| 参数专项 | `peer-rust-arguments-r1`，0/reaped/0.217606s；6通过、0失败，10过滤。 |
| 目标严格Clippy | `peer-rust-clippy-r1`，0/reaped/0.821013s。只覆盖当前集成测试目标，未代签workspace完整门禁。 |
| 官方依赖复查 | `dependency-freshness-r1`，1/reaped/59.111712s，未超时；37包、113声明／使用，42通过、71失败。存在registry读取失败，全部最新仍UNKNOWN；本轮未新增依赖。 |

原测试夹具导航错误及一次父级依赖摘要脚本的SyntaxError属于工装过程，
没有执行peer或产品动作。后续真实peer、非作者复核、冻结、最终门禁、
提交推送和清理按独立标签补充，不覆盖失败或继承旧输入的通过结论。

## 捕获报文、修复与实际直接 PTY

外部peer为未修改的Synchronet SexyZ3.7／zmodem2.7，
上游`acce09a1778dc2c61a996a3407ff6ddcaf70acb3`；
归档SHA256`b7dffb423eaa7a4ed4e4b28f346fce4a2399bff6029a74aa6883c712037118ea`。
原版standalone构建22个C单元，arm64／minos15.0／libSystem；
二进制253,304字节，SHA256
`4311ba9bbe5cccca936ccb73c74ac581735af309f5c9c379380b4c11126c4994`。
仅用于验收，不捆绑进应用；协议实现未修改，产品不移植其实现源码。

| 标签 | 实际结果与边界 |
| --- | --- |
| mac-sexyz-r1 | 10项：3通过、7失败，外层1/reaped/6.512283s。全部下载在首offer前被旧core拒绝；捕获的ZFILE有七个数值字段。 |
| core-captured-red-r1 | 独立捕获CRC32原始wire的新增回归实际1失败／102过滤，101/reaped/5.349912s，错误`InvalidMetadata`。 |
| core-zmodem-green-r1 | 最多七字段、无符号数字及原mode策略修复后，106通过／0失败，0/reaped/2.826174s。覆盖原报文每个分片位置、非法末字段／第八字段、溢出、路径、非普通mode及长度预算。 |
| mac-sexyz-r2 | 10项：8通过、2失败，外层1/reaped/10.161404s；全部旧metadata失败已越过审核，两例空文件发送仍失败。 |
| mac-sexyz-r3 | 加入原始marker观察和成功回执后，再得8通过／2失败，外层1/reaped/8.729441s。各自严格OO／保存与同shell结论保持；该版marker不能证明CRC帧格式，见下方第三项审查发现。 |
| python-peer-arguments-r3 | 28通过／0失败，0/reaped/0.276261s。成功Zmodem日志使用`--show-output`，原普通SSH模式不改变。 |
| dependency-freshness-r2 | 有界复查超时，controller124／direct_reaped／0 active，95.060399s，未取得完整JSON报告；不替代r1部分结果，全部依赖最新仍UNKNOWN。 |

接收参数为`["-raw","-e","rz","./"]`，发送为`["-raw","sz"]`，
用例的`-o`放在子命令之前。peer fd0／fd1直接连接生产SSH raw PTY；
stderr为自有普通文件，没有pipe、relay、interposer或替代exec传输。
各轮peer及执行输入前后摘要相等，自有SSH进程清理、未知祖先为空和临时密钥目录删除
由各轮result记录；不从一次成功扩大到未执行平台。

两项余下失败分别为单个空文件下载、含空文件的三文件逐项审核。
实际日志均记录peer拒绝`ZRPOS offset: 0`，后者在前两文件完成保存后失败；
应用不补发空文件保存或严格完成。该peer边界保持OPEN，不改变旧lrzsz缺OO的失败。
官方维护者协议文档的七字段说明只用于规范解释，参考URL／整件摘要在
`protocol-reference-r1.json`，不把构建、CLI选项或peer exit0替代真实握手。

## 非作者发现与最终候选

Fermat首版严格只读审查为HOLD：P2两项，P0／P1／P3为0。
发现摘要计算未轮询已注册的中断，以及batch只核对数量和磁盘内容、没有逐项核对完整回执。
父修复后Python30项通过，包括SIGINT／SIGTERM在hash读取内触发后保留
130／interrupted、关闭文件、不启动SSH或临时密钥准备；
batch逐offer／review／FileSaved核对transfer、名称、方向、路径、长度、SHA256、
ID唯一及offer→review→save→下一offer的事件顺序。

第三项P2发现r3全流marker观察可将`*`后的转义载荷误判为格式前缀；
旧r3／r5／r6 marker布尔值只保留为字节出现事实，不签CRC帧格式。
r7改为单文件审核确认后arm、最多96字节的首ZDATA／offset0完整独立CRC向量匹配，
识别后停止，绝不搜索文件载荷；不添加生产API或权限。
新增两项控制覆盖所有分片点、相反格式转义载荷、坏CRC、噪声和预算。
Rust当前18项库存＝8默认／10 opt-in；默认专项8通过／10 ignored，
`0/reaped/2.077479s`。本测试格式观察限定localhost夹具的首ZDATA表示，
不声明通用协议嗅探或所有peer编码均受支持。

首次r4真实runner在编译期因新版SHA数组不实现LowerHex而停止，没有执行peer。
期望摘要改为逐字节十六进制后r5编译库存16通过，`0/reaped/2.955804s`；
保留原编译失败，不将其记成真实传输结果。

## 同源码下第二实现与重复性

官方原版lrzsz0.13.1由自有目录configure／make构建，未安装、未改协议。
归档483,782字节，SHA256
`112f91059fbd118e016d800c1a5c5a1702e965d8b9bfbda6015e44e20bfb93c9`。
lrz／lsz二进制SHA256分别为
`5cfd57bcc24698caaa784e01a643f1437bea759efab606e8d7b4503c2e589369`、
`2802e822a1514a45fb22e869b587789418b8de137ae758f3432000eb0570d5d0`。
22条准备外部命令exit0、直接子进程reaped、自有剩余进程／组0；
两条执行前工装失败保持。准备者Halley已实际关闭。

| 标签 | 实际结果 |
| --- | --- |
| mac-lrzsz-r5 | 8通过／2失败，外层1/reaped/8.458903s。batch保存3及CRC32保存1后均missingOO1、peer exit0，严格完成断言失败。 |
| mac-lrzsz-r6 | 10通过／0失败，外层0/reaped/10.603836s。只签该冻结的内容／审核／保存和严格OO，旧marker不证明格式。 |
| mac-lrzsz-r7 | 9通过／1失败，外层1/reaped/8.560668s。batch三文件完整回执及strictOO1通过；CRC16／CRC32均核对首个完整CRC帧头且strictOO1，同shell也strictOO1。空文件保存1但missingOO1／peer exit0，严格失败继续OPEN。 |
| mac-sexyz-r7 | 8通过／2失败，外层1/reaped/6.600507s。两个空文件边界仍拒ZRPOS0；CRC16／CRC32首帧与strictOO1、同shell新输入有通过。不把旧观察器的marker当成格式依据。 |

r7冻结765工程输入，aggregate
`7ae00377c1eb95929710c7d8170d444ea293f8c27a3c8690cb83aa79bde3ede6`，
算法为canonical JSON rows的SHA256，文件内明确参数。r7 runner将全部765输入和peer
前后摘要与冻结核对，不能把较早只绑定执行子集的回执升级为765输入。
一次10／10不证明稳定兼容，后续9／10与全部历史失败保持；
完整产品、FILE-05、跨平台peer／原生和发行不关闭。

Fermat最终源审r7为APPROVE、P0–P3全0，历史三P2均关闭；
30项当前Python、5项原失败控制及6项源派生观察器模型共41唯一测试通过。
两审查命令exit0、无error／skip；765输入／三文件前后匹配。
原r1的17份原件摘要保持，未运行Cargo、Rust binary或SSH，不覆盖core或真实互操作。
reviewer已实际关闭。Peirce的core限定批准与106项预编译复跑单独保留，
真实peer补充复核和父级最终门禁不由此代签。

Peirce最后补充对r5／r6／r7原件作1,714项离线核验，限定批准；
765输入、两peer和前后绑定相等，两组首ZDATA独立CRC复算通过，
旧marker强表述明确撤回。该补充3条验证命令2成功、1为工装绑定文件选错后停止，
原失败保留，不重算旧4,138项或重跑SSH。旧decision／summary逐字节保全。
reviewer已实际关闭；本批五位作者／reviewer均已关闭，没有新建用户chat、
worktree或容器。CI、三平台原生与发行另验。

## 最终门禁与持久原件

冻结r7的完整门禁`final-gate-r1`为`0/reaped/796.379603s`，
普通Rust3419通过／0失败／62 ignored，14 rustdoc通过；
Python主523 run／0失败／3 skipped，fmt、严格Clippy、声明政策和三controller gate通过。
94普通套件的逐名结果与各精确二进制库存相等，二进制前后摘要不变；
两个harness=false controller由门禁实际运行，不执行可能重跑行为的`--list`。
765工程输入前后逐项相等。库存工装首两次因诊断与Rust状态同一行而停止，
原失败保持；解析修订后`final-inventory-r3`为`0/reaped/2.013397s`，
没有重放完整门禁或产品测试。

持久入口为[623成员原件](2026-11-17-independent-zmodem-peers-evidence/README.md)，
归档2,573,153字节、SHA256
`9eca3013dbd8424c0f2fa2613a0d7aae58df24d6267e40848d8a8bb45154c888`。
原`work/zmodem-independent-peer-20261117/`及上游／review副本只供交付过程；
Git绑定后按归属回收。正常应用包和唯一target保留。
本批没有重新构建／启动原生包，不把旧正常包当作此协议增量的原生验收。

工程`de9768c183c22f1051baabd529600667697247a1`已普通推送，
local／tracking／remote一致且读回干净；765输入和623成员精确Git绑定通过。
六个本批副本已回收，正常包与target身份／摘要不变。
提交、推送、回收及精确CI初态／后次快照见
[21成员交付封存](2026-11-17-independent-zmodem-peers-evidence/delivery/README.md)。
上述交付快照当时只有两个Windows工作流success的API状态，Quality仍进行中；
该历史状态保留。后续相同工程的Quality44013808155已三平台自然success，
Windows／macOS／Ubuntu普通Rust3406／3419／3412通过、0失败，各14rustdoc，
Python523 run、skip50／3／3；macOS／Linux真实OpenSSH各18通过。
7原始ZIP／156成员和4个API digest已取得，新非作者Feynman限定批准、
P0–P3全0、离线5,091＋16检查通过并已实际关闭；207成员终态与765旧Git输入相符；
见[精确CI终态](2026-11-17-independent-zmodem-peers-evidence/ci-completion/README.md)，
不从本机门禁或peer结果代签。
