# 资源趋势与知识管理原生联调 — 2026-11-06

本记录只管理新macOS开发包的实际联调。工程来源及完整门禁见
[组合记录](2026-11-06-resource-knowledge-integration.md)：
530项输入、程序
`14cb801e3d5eed51bbf52534c5160ea62d4a044719fd063c4eb8b2859488ccad`。
旧包捕获、基线CI和受控GPUI测试不代签本包原生。

## r1：PTY准入失败，应用未启动

准备冻结`2216f28c69b506787e807a7aa6c24065a247ee9f55e395c6c0b35c2827f90317`
包含26文件，offline-r4有26项测试、13文件编译通过。
非作者`native-prep-r1`限定批准，无开放P0/P1/P2/P3。
父绑定真实批准及已通过的组合门禁后，执行一次原定900秒运行：
owner为`6a4d8e87baa94e589d3f91bbece2a74a`。

父exec84019已实际wait、退出1并回收；总耗时5.317416秒。
容器、服务及私有材料已创建，但PTY准入的OpenSSH CLI退出255，
原生app尚未启动。sshd原始stderr明确为：

```text
chown(/dev/pts/0, 1001, 5) failed: Operation not permitted
```

15项准备CLI及12项清理CLI均有实际回收回执，除PTY准入外均退出0。
清理耗时3.890672秒，未超过原30秒预留或900秒总期限；
container、network、private均已删除，sshd实际wait/reap为-9。
专用tools image保留给明确的后续夹具消费者，已有桌面应用保持原归属。
`source_after_matches=false`表示未进入app结束后的检查，不表示已发现生产源码变化。
独立的530输入及包绑定继续由组合门禁和后续准备读回核验。

终态保留为`HOLD_OR_CLEANUP_FAILED`，其中`cleanup_verified=true`；
清理通过不改变PTY准入失败，更不产生A/B或原生验收结论。
原件位于本工作副本ignored的
`work/resource-knowledge-20261106/native-preparation/runs/6a4d8e87baa94e589d3f91bbece2a74a/`，
不覆盖或重新执行该run，不延长其原时钟。

## r2：仅修订隔离PTY夹具权限

新候选位于独立`native-preparation-r2`目录，owner为
`58b07cc44bec430893cedaf578a9822b`，已完成一次实际运行。
根据r1真实失败，只在原SETUID/SETGID/SYS_CHROOT外增加CAP_CHOWN，
用于设置PTY属主。创建argv及EffectiveCaps/BoundingCaps必须精确四项，
未知、缺失、重复或额外能力仍拒绝。镜像、生产采样命令、loopback发布、
只读文件系统和无主机挂载规则保持；不build/pull，不增加网络管理能力。

这是一份新的单次900秒候选，保留末30秒清理及一次app启动；
不是重启、续期或放宽已失败r1的原合同。
29文件冻结为
`a4f6a824882b72e13da4ef0e7fc853b4392003b3c88629fc6cab315d88ec9213`。
offline-r2有27项测试、13文件编译通过，耗时0.270312秒，
父同步执行退出0并回收。原三能力形状仍保留并作为新准入的负控；
派生四能力JSON明确仅用于单测，不是实际容器回执。
前一轮离线检查及修正前的控制入口文档保留。

非作者`native-prep-r2`批准精确冻结，无P0/P1/P2，唯一P3为派生controller的
来源文案，不阻断运行。父先有一次相对路径参数拒绝，发生在单次marker与run
创建之前；记录为`native/r2-pre-execution-argv-refusal.json`。改用计划中的绝对
路径后，父exec66176实际wait退出1/reaped，总耗时4.927308秒。

实际四能力及PTY准入通过，应用PID19655已启动，但尚未输出READY或进行CUA。
本地`/bin/ps -eo pid=`的身份读取被拒绝，而强引用子进程最终退出0/reaped。
监督器拒绝该身份缺口，随后应用被精确信号终止，-15/reaped。
全部18项工作CLI及12项清理CLI均有退出0/reaped记录；其中两项进程查询
仍有`failure_type=Refused`与`cleanup_unknown=true`，不能算检查通过。
容器和网络已删除，sshd已实际回收，监督器因应用归属清理未知保留private；
原`cleanup_verified=false`和HOLD保持。

父随后独立核实精确应用PID/birth不存在、PID/parent/group无匹配，并核对
private的dev/inode/uid后删除该目录。补充读回在
`native/r2-parent-cleanup-readback.json`；不回写原失败或声称已证明未观察的
detached descendants。`source_after_matches=false`仍只表示未到后置检查。

## r3：限定处理快速退出的进程查询

新候选owner为`0a4d12079b5344439835b73626296be8`，31文件冻结
`5bc0616a64ce89abfb610a536df5d3e383adad42c4a5b2e7b0e84e66827ce77c`。
仅精确PID查询显式启用快速退出分支：缺birth时，强引用Popen必须已实际
poll/reap得到0，随后仍检查有界输出、EOF及原期限；不伪造birth。
其他命令、仍存活、非0退出、已知group不匹配或输出超限均拒绝。
应用和SSH等身份约束、四能力策略、生产代码、530输入及包均保持。

offline-r1有34项测试和13文件编译通过，含七项新增pipe/进程double回归；
父同步exec退出0/reaped，两个直接子进程均0/reaped，耗时0.287464秒。
派生common与controller的上游/本地摘要分开记录，并修正r2来源说明P3。
非作者`native-prep-r3`批准精确准备增量，开放P0/P1/P2/P3为零。
父exec38609随后实际单次运行，wait退出1/reaped，耗时5.326806秒；
同样在READY前HOLD。应用PID34874以-15退出并回收，容器/网络已删、
sshd已实际回收；应用归属清理未知而保留private，原失败不改写。
父后续核实已知PID/parent/group不存在并按dev/inode/uid清理private，
补充记录为`native/r3-parent-cleanup-readback.json`，不证明未观察的detached子孙。

随后一次无服务、无应用的本地诊断证实：`/bin/ps`模式为04755/setuid，
`proc_pidinfo`返回0/EPERM(1)，当时Popen仍存活，最终退出0/reaped并有6774字节
stdout。因此快速退出只是被推翻的初始假设；r3离线通过和准备批准不能代签
实际可用性，其例外分支在该失败中未触发。诊断原件单独保留。

## r4：系统PID枚举

新owner为`abc8f78e408046648f4457631f680584`，34文件冻结
`0671aebe9973667f2795fa30fb359eccf1085ce5c367fa8f1864edcca0937367`。
common恢复r2严格原字节，不再保留r3例外。仅监督器PID列表改用Darwin
`proc_listallpids`和固定64KiB缓冲；错误、非正数量、达到容量、重复及负PID
均拒绝。PID0排除，原有逐PID出生身份、累计归属、信号前核验、强引用等待和
UNKNOWN保留规则保持，沿用原deadline。

单次只读系统诊断返回1114个PID、errno0、包含自身PID，耗时0.0003785秒；
不启动服务或应用，仅证明本机接口可用。新八项回归随offline-r1共35项测试、
13文件编译通过，父同步exec退出0/reaped，两个子进程0/reaped，
耗时0.284242秒。非作者`native-prep-r4`限定批准，无P0/P1/P2；
唯一非阻断P3是kernel枚举失败时缺少专属失败summary，拒绝仍生效，
本次冻结不改写。父绑定报告后执行一次新run，未重新执行或延长任一失败run。

## r4实际原生观察

父exec42584已实际wait，退出0/reaped。总耗时684.164221秒，
应用PID47967通过UI关闭，退出0/reaped，已观察归属进程清理通过；
清理耗时2.743772秒，container、network、private均删除，sshd实际-9/reaped。
保留原67字节清理警告`cleanup_exit: kill(12): Operation not permitted`及
`no more sessions`；资源清理通过不代表sshd自然退出0。
30项工作CLI及12项清理CLI全部退出0/reaped，530工程输入后置核验相同。
原始终态为`STOPPED_CLEAN_PENDING_NATIVE_REVIEW`，只说明生命周期完成。
原件在`native-preparation-r4/runs/abc8f78e408046648f4457631f680584/`；
`native/r4-parent-readback.json`另记父外层实际等待，
`native/r4-evidence-manifest.json`绑定1562份原件。

本次仅一个新macOS开发包窗口，Frost Light/英文，17对实际AX/JPEG。
原已有的其他开发包进程保持原归属。主机指纹从本run新建密钥材料获得，
父逐字核对UI完整指纹后显式信任并连接；没有外部账户、模型调用或实际用户知识。

资源面板完成16轮真实Linux资源加ps采样：首次暂停前9轮，恢复后7轮。
首CPU样本为unknown；恢复后先重新建立CPU基准，AX及图像显示Gaps，
旧段与新段之间没有补点。数据来自Podman Linux VM的真实内核及进程，
不代表macOS宿主指标或容器cgroup限额。
两次暂停分别通过相隔至少5.5秒的稳定审计；第一次暂停后切换连接/主机面板，
所有exec及PTY计数保持不变。该结论只覆盖观察区间。

同窗文件区显示SFTP通道失败；本夹具明确关闭SFTP，未通过文件功能验收。
这不影响已成功的资源/ps样本，但不能据此声称全部SSH通道或文件操作通过。

知识管理先选Alpha/Beta，完整审核两条UUID、内容和来源，取消后重新审核同一批，
再确认一次。17号捕获显示真实保存成功及仅剩Keep Sentinel。
原监督器首次B0导出在tuple/list身份形状校验处拒绝：内存中的app birth为tuple，
校验要求JSON数组。该失败响应和前后审计保留，运行中冻结代码未修改。
父使用同run只读适配`native/parent_snapshot_export.py`，在各自审计bookends间
读取真实`data/state.json`及原文件元数据，重核应用kernel birth，再以JSON wire
形状调用原校验器。三个独立导出均同步退出0/reaped，各两个控制子进程0/reaped；
它们不是原监督器capture成功，也未创建新服务、延长时钟或重放业务操作。

| 独立实际读回 | 磁盘与集合 | 远端执行 |
| --- | --- | --- |
| B0→B1取消 | 原始bytes SHA、dev/inode/size/mtime_ns/ctime_ns均相同，三条完整内容/来源/顺序保持 | exec和PTY计数相同 |
| B1→B2确认 | 精确删除Alpha/Beta，Keep Sentinel完整保持；其他配置语义相同 | exec和PTY计数相同 |

三个读回均为32次exec、13440命令字节、16次snapshot及16次ps成功，
active/refused/PTY输入字节均为0。离线compare分别通过取消与确认合同，
新的非作者已交叉核实实际读取来源、AX、时间和清理，结论为
`APPROVE_SCOPED_NATIVE_R4_A_B_WITH_PARENT_READONLY_EXPORT`，没有新增阻断，
只批准本次限定A/B证据。报告位于`independent-review/native-r4.md/json`，
JSON SHA-256为`dc31323b4f2aa3ac548ce45e78b27abb010d1e3111379ded9aef442412796aff`。
原capture tuple/list工具P2（NR4-P2-01）和失败枚举summary工具P3（NP-R4-P3-01）
均仍开放；它们是后续取证工具修订，不是本次限定证据的阻断。
本次冻结、原拒绝与旧失败均不改写，完整native finish仍OPEN。

## 后续资源收尾

`native/tools-image-cleanup-r1/result.json`记录自有tools image
`sha256:35b28f15858955ec6efadf50864e94b4e51f15738605b20f24e67fadb9f2c147`
经归属核验、确认无消费者后非强制删除；4个子进程实际回收，
末次exists按镜像已不存在返回预期1，耗时0.351646秒。
旧runtime所写“image保留”是当时状态，原件保持；共享base、VM和target未清理。
`source-evidence/author-worktree-cleanup-r1.json`记录A作者树已由管理器归档，
B作者树因`The task's primary checkout cannot be removed.`拒绝归档而保留，
没有用shell绕过。两份作者证据tar摘要已重新核对。

后续A必须使用同一认证SSH的真实资源与ps采样；B必须先暂停并等在途结束、
完成偏好保存，再取取消零写与确认精确删除的磁盘/SSH计数基线。
完整主题、输入/辅助技术及其他平台桌面仍独立验收。
