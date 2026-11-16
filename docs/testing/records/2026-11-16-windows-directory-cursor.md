# Windows 独立目录游标与文件回归同步

## 问题与范围

基线为 `80397acc98d5881cbd177d7259a54fd7f1556c4f`，工程字节与诊断源码
`878f13b4cb58a45dfe96f32fc1c389ac7799366d` 相同。
后者 Quality `43815115588 / attempt 1` 的 Windows job `133125296042`
实际 platform `48 passed / 16 failed / 0 ignored / 0.50s`；
16 个失败区块均含同一错误的两层诊断：
`nt-create-file / InvalidFilename / raw_os_error=123 / NTSTATUS=0xc0000033`
及 `open-directory-cursor`。不能仅由该结果代签八项 app 归档恢复。
完整失败原日志、精确源码和 `full gate=NOT_RUN` 见
[诊断记录](2026-11-16-windows-native-diagnostics.md)。

最终批次修改四个工程文件：`windows/native.rs`、`windows/tests.rs`，
以及 app 的 `transfer_tests.rs`、`conflict_merge_tests.rs`。
生产小补丁增加 16 行、删除 3 行；其余为三项 Windows 回归和两处测试同步。
没有改变上层审核、未知隔离、无覆盖发布或凭据规则。

## 机制与回归

原目录枚举用 `open_child(parent, ".", true)`，现在通过私有
`directory_cursor` 用保留句柄和空 NT 相对名重新打开目录对象。
重开前后比较目录 identity、attributes 和 security；沿用同步选项、
禁止 reparse、实际本地卷检查、读访问和分享模式。
没有按路径重建来源、复制句柄作为游标、失败回退或扩权。

UTF-16 存储增加一个不计入 `UNICODE_STRING.Length/MaximumLength` 的终止单元，
保证空名字的 backing storage 存活；非空名称的已检查计数不变。
公共 child 分量校验继续拒绝空名称。目录审核仍忽略兄弟写入引起的时间变化。
机制说明补入 [ADR 0138](../../adr/0138-windows-handle-local-files.md)。

| 新用例 | 行为 |
| --- | --- |
| `reopened_directory_cursors_do_not_advance_or_restart_other_live_scans` | 确定性交错推进 parent 和两个游标，再依次读至 EOF，三个完整集合均精确相等，覆盖中文与非 BMP |
| `directory_cursor_keeps_the_held_identity_when_its_original_path_is_rebound` | 保留目录改名、原路径被另一目录替换后，原 capability 和新游标仍只枚举原对象 |
| `directory_cursor_preserves_private_dacl_and_rejects_regular_files` | 重开后私有 DACL 与目录 stamp 保持，并拒绝普通文件充当目录 |

全部原测试正文与断言保留，包括 16 项旧失败、并发扫描及超过 64 KiB
多批枚举。新 helper 的单次查询和 EOF 循环有固定上限，不依赖调度概率。
候选复制句柄变异、并发 reparse 注入、真实 Windows 执行尚未进行，保持开放。

## 冻结与验证

第一片 Windows 两文件候选的 719 项输入集合为
`67337a389845e537823adf5d10671d52b9fd3ec95df43ac1634161c1b5022749`，
绑定首轮失败门禁及 Windows 静态复核。修订两个 app 测试后的最终 719 项
输入集合 SHA-256：

```text
f5ab8e7180b7e8fcd90610100f496e869468bdf0605f7b073c215ec30d4ebd83
```

唯一共享 target 没有切换源码归属；没有反复 clean，也没有并行 Cargo。
`scoped-gate-r1` 实际 `exit 0 / reaped / 3.213s`，依次完成：

- x.y 政策、全 workspace 格式检查。
- 本机 platform lib `18 passed / 0 failed / 0 ignored`。
- Windows `x86_64-pc-windows-msvc` 全 target 严格 Clippy，exit 0。

完整 `full-gate-r1` 实际 `exit 1 / reaped / 485.787s`，app 为
`1338 passed / 1 failed / 2 ignored / 439.80s`。唯一失败是
`ordinary_folder_transfer_pause_and_cancel_preserve_partial_tree_status`，
GPUI 等待条件 12 秒超时；日志没有标明具体 await，原失败阶段不猜测。
本轮完整 workspace 后续 target/rustdoc/controller 尚未执行，不能签完整通过。
另从真实 macOS job `133125295686` 取得不同的失败：
`stale_merge_frame_actions_cannot_mutate_a_replacement_review` 缺少 replacement
merge review，app `1338/1/2 ignored / 739.41s`；与本机失败分别保留。

新非作者 Windows 两文件静态复核 `APPROVE_STATIC`，P0–P3 均 0，
独立 Cargo、测试、Windows、变异及 UI 执行均 0，reviewer 已关闭。
35 项静态检查有 34 项通过；唯一失败是检查器误把父文档变更计入工程范围，
原失败回执保留，未重跑。起止 719 项冻结相等，19 个原测试、49 处原断言、
并发和跨缓冲正文逐字节保留。此报告仅签两文件候选，不移签新 app 测试修订。

目录暂停测试修订改用原有真实 SFTP WRITE hold，等待实际 session pause
handoff 后才释放写入，保留原暂停、取消、完整前缀和 SSH 存活断言；
替代依赖每次 55ms 延迟的时序条件，不扩时、不重试或 skip。
专项 `folder-pause-focused-r1` 实际编译后 `1/0/0 / 0.41s`，
command 为 `0/reaped/16.627s`。新非作者 APPROVE、P0–P3 全 0，
独立执行 0，已关闭；不把专项或静态批准当作完整门禁。

合并审核测试将 32 次只排空 ready tasks 的循环，换为等待真实 worker 完成，
给线程 1ms 让步并推进 GPUI 测试时钟 1ms，仍保留等待后的全部原代码和断言。
生产接收器以 16ms 测试定时器轮询，ready-only 排空不保证结果送达。
新的 12 秒是墙钟轮询检查点预算，沿用 Harness 数值，不等同于原 GPUI 时钟
语义，也不是可抢占的整例时长上限。没有重发请求、重试、skip 或生产补丁。
`merge-focused-r1` 实际 `1/0/0 / 0.27s`、command `0/reaped/11.634s`；
完整 `conflict_merge_tests` 模块实际 `35/0/0 / 15.33s`，
command `0/reaped/15.734s`。

新非作者批准该同步 hunk，P0–P3 全 0、11/11 静态检查通过、独立执行 0，
已关闭。推进测试时钟可能自动重绘，不能声称最初四个 listener 始终保留；
后续无 notify 的 UUID 再次变化及立即指针派发仍验证可触发的 stale callback
拒绝。Previous/Adopt 可能禁用，四回调独立覆盖与编译变异拒绝未由此证明。
旧 CI 失败具体状态仍 UNKNOWN，不称该竞态是其唯一根因。

最终 `full-gate-r2` 实际 `exit 0 / reaped / 727.352s`。普通 Rust
`3222 passed / 0 failed / 48 ignored`，rustdoc `14/0/0`，
Python `290 run / 289 passed / 1 skipped`；格式、严格 workspace Clippy、
x.y 政策及三组 AI controller 均通过。app `1339/0/2`，
本机 platform `18/0/0`，session `server_archive` 为 `40/0/0`。
app 1,341 名与 session 归档 40 名逐项匹配，完整 719 项工程输入的路径及
字节集合零漂移。首轮失败原件保持；没有以重复运行替代修订。
最终原日志为 645,414 字节，SHA-256：
`8af4c44c790c22c4e6a67f6a1219dc0ec28a39b38b11cc8dc1f8cd2d242564b8`。

旧 Windows 编译库存 64 项已从原日志逐名提取，加三新例形成预期 67 名；
这是验收清单，不是当前 Windows 已执行结果。

本轮实时官方依赖查询已 `exit 0 / reaped / 17.485s`，覆盖 37 包、
113 项声明/使用，37 包全部通过，UNKNOWN 为 0。
`gpui-kit 0.9.1`、`tokio 1.54.1` 已核实；manifest 和 Cargo.lock 均无需更改。
旧批 19/18 的 UNKNOWN 事实保留，本轮成功另记，不改写旧记录。
项目使用环境默认 Rust/Cargo `1.100.0`，不增加工具链 pin。

## 作者、证据和清理

条件候选作者已关闭，父核对固定基线、补丁重放、原测试保留及格式后集成。
保留 15 份必要作者原件和官方来源获取回执；九份第三方参考全文不重新分发，
其来源 URL、字节数和 SHA-256 在原回执中。父重放核验的回执单列，
原作者 validation 已精确恢复并核对其原 manifest。

前批 64 个临时文件、4,244,910 字节已与两份精确提交归档及最后读回副本
逐字节核对后回收；旧目录 absent。当前活动目录为
`work/windows-directory-cursor-integration-20261116/`。
作者原目录 24 文件、457,582 字节也已逐项核对后回收。
合并作者 9 份必要原件、86,385 字节已完整复制、核对并回收原目录，作者已关闭。
全部本批作者与三名非作者 reviewer 都已关闭。
本批 [74 份持久原件](2026-11-16-windows-directory-cursor-evidence/README.md)
共 2,223,793 原字节，保存两轮完整门禁、专项、依赖、作者、评审、失败 CI、
清理及四份最终源码。归档逐文件与最终 719 项源码绑定零 mismatch，
首轮失败单独绑定旧输入，不改写原作者和 reviewer 报告。
本片无新 chat、worktree、容器、正常包、安装或原生桌面操作。
真实 Windows 67 项预检、原完整 gate、八项 app 归档和三平台桌面分别验收。
