# Windows 归档根目录发布修订

## 范围与状态

基线 `d4714c02c25f59861cfaa82b057161a9f484837c`。本批三个工程文件分别调整
Windows staging 观察生命周期、Windows 回归以及 CI 必需库存名；没有改动平台
ABI、访问权、分享参数、私有 DACL、审核来源、取消预算或未知结果处理。
宿主专项已通过；当前候选的 Windows 类型检查和实际执行尚未完成。
决策和平台边界见 [ADR 0148](../../adr/0148-windows-archive-root-publication.md)。

## 上批真实失败

源码 `8527cf2af581e50ac0e1f795176ab69737730e18`，Quality `43854113236`，
Windows job `133253865501` 的文件系统预检 `70/0/0 / 0.89s`。
私有归档模块重新编译库存 29 名与精确源码声明、实际执行逐项相等，
`25 passed / 3 failed / 1 ignored / 0.62s`；完整 Windows gate 为 NOT_RUN。
该精确 run 已终态 failure，watch 实际 `1/reaped/1285.532s`，没有取消或超时。
macOS job `133253865923`、Ubuntu `133253865850` 均 success，app 分别
`1341/0/2 ignored`、`1342/0/2 ignored`，真实 OpenSSH 分别 25.356s、16.327s 成功。
三平台完整原日志和终态分别保留，不把旧源码的两平台成功移签当前修订。

失败的三项是：

- `windows_archive_manifest_clone_retains_every_original_ancestor_capability`
- `windows_archive_private_nested_tree_publishes_after_child_creation_handles_close`
- `windows_archive_unobserved_successful_publication_remains_quarantined`

嵌套树的固定诊断为 `publish-stage / SessionError::Io / PermissionDenied /
raw_os_error=5`。空树成功；clone 用例在试图移动仍有打开后代的外层目录时失败。
OS 5 不是具体 syscall 或 NTSTATUS 的独立证据。
Microsoft 两页 MS-FSA 官方规则本轮读取均 HTTP 200，指出目录改名要检查
后代 open，并遍历子目录；分享 DELETE 不免除这个检查。只保存来源身份和原创
释义，未复制整页原文。该规则与实际失败共同支持本批生命周期修订，
不代替修订后原生成功的证据。

## 改动与回归

发布前仍持有完整树完成所有原检查。随后将新 staging 的 `Tree` 消费为每项
完整 `FileInfo`、digest 和 children 基线，关闭其观察句柄；根 `PrivateEntry`
和已审核祖先始终保留，已审核来源 `Manifest` 完全不变。转换后再检查取消，
只执行原根目录无覆盖改名，不再对子项写入。发布后重新打开并完整读回整棵树，
逐项比较全部身份、权限、元数据、清单和 digest，以及审核成员的完整字节，
再执行原最终链接、名字、staging 缺席和授权检查。

不再宣称 staging 后代在根改名瞬间持续由原句柄固定；接受仍依赖完整后验，
不能单凭 file ID、路径或 digest 判成功。原 guard 跨转换保留，错误、取消、
丢弃 await 或未观察结果继续隔离，不清空历史 UNKNOWN。

测试作者先修订 clone 用例并提供同内容替换的发布前候选。父集成时将新增例收敛为
更直接的**句柄释放之后**窗口：用 private DACL 同名创建同字节的新 identity，
根改名必须真实成功，完整 capture 必须接受权限、内容和清单，但发布后基线比较
必须精确拒绝；丢弃原 guard 后已发布树和原证据均保留，reservation 必须 quarantine。
该例验证私有转换与读回边界，不能冒充完整 `publish_stage` 端到端执行；
既有完整嵌套树及未观察成功用例继续承担该回归。

clone 用例丢弃原 manifest 后先正向验证 clone；叶文件改名并同名替换后，
逐个原祖先的完整归一观测、旧文件 identity/bytes/digest 均保持，重新审核检查
精确拒绝。原审核 stamp/digest 不改写。其他 28 个原用例保持。
静态源码库存为 30 名，含原 1 项 ignored；CI 强制新增例的完整名字出现后才执行。
静态声明不算 Windows 编译库存。

## 本机验证与输入

719 项工程冻结 SHA-256：
`56ba4c7b46f7fe7b03b789639db84a997c41f7363b9d856b6148af44086ac83a`。
共享 target 归属未切换，没有清除或修改其他 checkout 产物。

`scoped-gate-r1` 实际 `0/reaped/19.678s`：

- fmt、x.y 直接依赖政策、严格 workspace all-target Clippy 成功。
- 宿主 session lib `288 passed / 0 failed / 4 ignored`。
- app 归档模块 `13 passed / 0 failed / 0 ignored`。
- 305 个重新编译库存名与执行结果逐项相等，719 项工程输入零漂移。

首个行起始解析器被并发 Python helper stderr 与 Rust 结果同一行影响，
漏报 1 名；原日志第 423 行确有该项成功。按 Rust 模块全名及完整结果后缀
修正后，305 个唯一名全部匹配，未重跑测试。原解析偏差另存。
app 链接器原 compact-unwind 大小 warning 保留，命令仍退出 0。

宿主不编译 Windows 条件实现，不能代签其类型检查。前批 Windows cross-Clippy
因 C 依赖缺 SDK `windows.h` 退出 101，未以相同环境无目的重试。
本批未重跑整个产品门禁；精确跨平台 CI、Windows 完整 gate 和桌面分别验收。
最近官方依赖查询为 37 包/113 声明，22 包通过、15 包 registry 请求失败 UNKNOWN；
本批没有 manifest、lock 或 Rust 工具链变化，不宣称全部在线最新已核实。

新非作者 `Dalton` 对冻结三文件静态 APPROVE，P0=0/P1=0/P2=0/P3=0，
14 项自动静态核对和 8 项人工语义审查完成，另独立解析父 305 名原日志。
其执行 Cargo、fmt、Clippy、PowerShell、Rust 测试和 native 均为 0，未签 Windows
类型完成或实际通过，已关闭。评审明确接受完整后验契约，并明确没有持续固定
staging 后代身份的保证；原来源与祖先能力仍保持。辅助读取/解析工具错误保留在
评审说明，不记作 Rust 测试通过或失败。作者原候选及父修改分别保留。

## 证据与接续

当前候选的三文件快照、实际命令、库存、原失败和作者/非作者材料由
[本批持久证据](2026-11-16-windows-archive-root-publication-evidence/README.md)维护。
Windows 新精确执行通过前不关闭原四项 app 失败；旧源码的三项 backend 失败
也不移签为修订已过。Mac 仍锁屏，上一正常包的英文工具栏原生 P2 继续开放。
完整产品、FILE-05/M4、三平台桌面和发行安装未完成。
