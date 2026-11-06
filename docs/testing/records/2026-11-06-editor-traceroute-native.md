# 文本格式与路径探测限定原生记录 — 2026-11-06

状态：非作者 `native-r1` 批准已观察子集，完整原生验收保持 HOLD。
新增发现 P0=0 / P1=0 / P2=1 / P3=0。该记录只覆盖单个 macOS 原生窗口、
霜白浅色、英文和自有 Linux OpenSSH / 外置 SFTP 合成服务。
工程门禁与来源见[组合记录](2026-11-06-editor-traceroute-integration.md)；
完整桌面、跨平台和发布安装验收保持开放。

## 程序与运行身份

| 项目 | 实际身份 |
| --- | --- |
| 组合输入 | 543 项工程输入、21 个源码路径 |
| 最终冻结文件 SHA256 | `1b5c7cf42d8a4d9ede5de6fcca29caf11da5daeaef59fc8e580ea4a1172590bd` |
| 正常应用 SHA256 | `9e83d83af253ffe3c3aba29997450ea75f4c70a6dbe9311e61469228bd0158b9` |
| 包 | `work/next-batch-20261106/package-macos-r1/KeelShell.app`，未覆盖安装 |
| 独立运行 owner | `01e8d214a619471b8ad93786b6083afe` |
| 原 900 秒起点 | 2026-11-06 04:43:21.351513 UTC |
| 实际应用 / 监督器 PID | 34654 / 34602 |
| 外层执行 | 28510，父实际 wait，exit 0 / reaped |
| 总耗时 | 797.330164 秒，未重启或延长期限 |

runtime-r3 的必要权限修订、完整冻结与包绑定由 Curie 的 `report-r3.json`
批准准备；该批准本身没有授予原生验收。应用由夹具启动，全部 GUI 操作通过
CUA 完成。首次信任前在真实审核中核对夹具公钥指纹，再建立 SSH 与 SFTP；
CLI 只准入服务、观察完整字节/stat、执行一次固定竞争写和清理。

## 实际操作与结果

| 场景 | 原生操作与独立观察 |
| --- | --- |
| BOM / CRLF 取消 | 打开 22 字节合成文件，编辑 Unicode 草稿，展开完整 29 字节候选审核后取消。`checkpoint-cancel` 与完整 snapshot 证明目录、原文件字节和记录的 stat 全部不变，没有新增备份。 |
| BOM / CRLF 确认 | 再次展开完整审核并确认一次。应用明确显示原子保存和全部目标读回成功；独立 `checkpoint-confirm` 核对完整 29 字节目标、唯一 22 字节原始备份，以及其他文件字节/stat 不变。 |
| 合并准备 | GUI 草稿粘贴 `U+FEFF + A + LF + b + LF`；夹具仅一次将远端固定改为 `a + LF + B + LF`，随后完整读取 `merge-before` snapshot。没有完成合并采用或保存。 |
| UDP / IPv4 | 真实审核执行主机、`127.0.0.1`、3 跳、每跳 1 次、100 ms 和完整命令后确认一次。界面返回 Destination address responded，完整跳为 TTL 1、`127.0.0.1`、44 μs。 |
| UDP / IPv6 | 重新编辑目标 `::1` 并选择 IPv6，按同样参数重新审核、确认一次。界面返回 Destination address responded，完整跳为 TTL 1、`::1`、59 μs。 |
| 退出 | 最后捕获后通过 Cmd+Q 退出；CUA 返回 App quit。应用 exit 0 / reaped，父随后实际回收唯一外层执行。 |

合成原始文件为一个 UTF-8 BOM 加 `Alpha\r\n中文🙂\r\n`，保存后为一个 BOM
加 `Alpha edited\r\n中文🙂\r\n`。这里的备份完整字节来自独立夹具观察；
生产保存仍只要求备份写入 ACK，不能据此声称生产会完整读回备份或保证崩溃耐久性。
目标的完整读回由应用自己的保存流程完成。

## 未完成与界面问题

inline 编辑器底部的 Read remote and merge 在 AX 中存在，但屏外点击没有触发操作。
调整滚动方向后，底部动作栏仍只露出部分区域，未取得可确认的合并操作。
该问题已转入独立布局修复；本次保持 `merge-adopt`、`merge-save` 为 **NOT_RUN**，
不能以源码测试、竞争写或已粘贴草稿替代原生合并结果。
捕获 19–22、26–27 保留原始失败尝试。原生滚动方向与初次预期相反，
不把所有未变化的滚动尝试都归因于产品缺陷。

late-save 时序、连续保存为空或无换行、原生 ICMP、完整取消/远端回收矩阵、
外部路由、其他主题/明暗/语言/尺寸、IME/辅助技术、Windows/Linux 桌面、
多窗口及签名安装均未在本次验收。

## 回收与原件

24 条工作期 host 命令与 1 条 guest 版本命令均实际退出 0 并回收。
应用退出后按精确 owner/ID 回收容器、网络和 sshd；sshd 为 -9 / reaped，
容器 exit 0。新的自有工具 image 在零消费者检查后移除，本轮 private 目录移除，
公共 base、Podman VM 和共享第三方 target 保留。

继承的 `cleanup.json` 保留 `verified=false` 与原错误
`app ownership cleanup unknown; private retained`。后续 `editor-cleanup.json`
记录应用 wait、专属 image/private 清理及 `verified=true/errors=[]`；
两份原件都保留，不互相覆盖，不把最终收口解释为旧阶段从未 UNKNOWN。
应用回执对未观察到的 detached children 仍为 `not certified`。
更早 runtime-r2 的失败、
原 HOLD、旧 image/private 是另一归属，不能借本轮成功自动关闭。

Curie 随后按新编号独立处理旧 b3 残留：2026-11-06 05:13:12 UTC 完成精确
image/private 删除，0.298188/30 秒，外层 exit 0/reaped。
删前 owner/digest/purpose、零消费者和 private 的 dev/inode/uid/0700 身份匹配；
7 条命令均实际 wait/reaped，6 条 exit 0，image absence 检查预期 exit 1，
private 最终不存在。12 项旧证据摘要复核不变，旧 HOLD/false 仍保留。
原件位于 `work/editor-format-20261106/native-runtime-review/cleanup-b3-execution-r1/`，
`receipt.json` SHA256 为
`fc8239e707c1da9d22e28e8b09e4f9632816ed3921f3fb3b999f80830274f26f`；
`outer-exec-readback-r1.json` 记录实际数值回收。该补充清理不改变原生验收范围。

本地 ignored 原件根为
`work/editor-format-20261106/native-runtime-r3/runs/01e8d214a619471b8ad93786b6083afe/`：

- `cua/` 含 38 组原始 AX、JPEG、时间与 owner/PID/包摘要元数据。
- `audit-*.json`、两个 `checkpoint-*.json` 与 `merge-remote.json` 保留完整合成字节、
  stat、目录以及唯一竞争写。
- `app-wait.json`、`server-terminal.json`、工作命令和清理子回执保留实际退出身份。
- `parent-exec-readback-r1.json` SHA256
  `400c24dae371ba2c2159528b5db574d1a30252490d6ba340411021d713320f0a`
  记录父实际 wait。
- `parent-native-observations-r1.json` SHA256
  `9d0d698bb4d4ca35b52740e59626f4df0092e5a8baf34d984ef934fc57d16136`
  绑定 38 组捕获及原件摘要，只是父观察汇总，非独立批准。

## 非作者结论

Gauss 在 2026-11-06 05:18:22.939217 UTC 完成原件限定复核：
`PARTIAL_NATIVE_ACCEPTANCE_FULL_NATIVE_HOLD`，
允许当前 543 输入批次在显式保留开放项的前提下限定交付。
新增 P2 `NR1-P2-01` 为 inline 底部动作行裁切，阻止完整编辑器原生验收，
不阻断本批已观察场景的限定交付。修复放在下一片；没有关闭合并 NOT_RUN。

报告复核 38 组 / 114 份捕获的摘要和绑定、完整合成 snapshot、
工作/清理子回执、父实际 wait 及预算。审核/result 场景限定通过；
精确点击次数来自父操作记录，截图 metadata 本身没有逐点击轨迹。
不可见 U+FEFF 的粘贴是父操作记录，不从 AX 可见 A/b 推定合并语义已经通过。
基础设施收口仅在实际观察范围内批准，未签完整原生、发布或安装。

报告位于 `work/next-batch-20261106/independent-review/native-r1.json`，
SHA256 为 `9ce082e4be171262753f750ae4f931f37ca8be61cdeb71b985bc3960c3e07f70`。
该报告继承精确 combined-r3/r4 的源码/工程/包批准，不重复或扩大其范围。
ignored 原件路径不表示公开发布。
