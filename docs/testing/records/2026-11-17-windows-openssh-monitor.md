# Windows OpenSSH 真实监控验收 — 2026-11-17

状态：本机工程与两位独立源审通过，等待提交推送后的精确Windows执行；
不签本批真实SSH、原生桌面或发行成功。
基线工程 `bfbea7bcc84ff433d28a635e85daed9cca337fb0`。
设计见[ADR0157](../../adr/0157-windows-openssh-monitor-acceptance.md)。
上一批5d2df2f的真实Windows直接API及三平台质量门禁保持原范围，
见[上一批记录](2026-11-17-windows-read-only-monitor.md)。

## 本批工程分工

- 父负责隔离配置、默认shell观察、真实SSH编排、独立Win32对照与CI。
- Aquinas负责调试创建／原对象／Job终态及其Python专项。
- Galileo负责真实公开Rust SSH／monitor路由及结构化回执。
- 所有写集互不重叠，当前checkout共享，作者不运行Cargo或整仓门禁。
  唯一target归属与此前相同，未切换、不clean。

## 已实际观察

| 范围 | 结果与边界 |
| --- | --- |
| 父级配置／接纳初轮专项 | `ssh-parent-unit-r1`，exit0/reaped，26项通过，0失败、0跳过，外层0.289769s。包括一次实际宿主子进程的显式环境控制；不是Windows执行。此后集成变化需另验，不移签原范围。 |
| 父级配置／报告／清理专项 r2 | `ssh-parent-unit-r2`，exit0/reaped，39 run、0失败、0跳过，外层0.303391s。补齐客户端报告、清理后的接纳集合与PARTIAL状态控制；均为本机离线协议测试。 |
| Rust真实用例库存 | `windows-ssh-rust-inventory-r1`，exit0/reaped/2.685961s，实际编译后逐名读回 `actual_windows_openssh_monitor_matches_owned_fixture`，1 test。显式ignored，未在本机冒充Windows运行。 |
| Rust目标严格Clippy | `windows-ssh-rust-clippy-r1`，exit0/reaped/12.889656s，`-p keelshell-session --test windows_openssh_monitor -- -D warnings`；仅编译与静态检查。 |
| Rust非Windows负向控制 | `windows-ssh-rust-nonwindows-r1`，实际调用ignored用例，以`WindowsRequired`退出101/reaped/0.748447s，0通过／1失败。该失败符合平台边界预期，原失败保留，不记作Windows执行或普通门禁失败。 |
| 集成Windows相关Python专项 | `windows-ssh-python-integration-r1`，exit0/reaped/8.922646s；`test_windows_*.py`实际271 run＝268通过＋3跳过，0失败。包含既有Job／lab回归、调试接纳与真实宿主子进程控制，未代签Windows ABI。 |
| 调试owner作者最终专项 | r7实际49 run＝48通过＋1 native跳过，exit0/0.122686s；原r1超时exit124/60.005464s保留。作者Cargo、全仓门禁和Windows实际执行均0，已实际关闭。 |
| 最终完整门禁r1 | exit0/reaped/699.028665s，94普通Rust套件3407通过／0失败／60 ignored，14 rustdoc通过；app1398/0/2、session404/0/4。Python主discovery493 run＝490通过＋3跳过；格式、严格Clippy、依赖x.y策略及三组独立controller均通过。 |
| 独立编译库存 | app1400、session408、Windows routing10、真实Windows SSH用例1；分别逐名核对最终gate结果，完全相等，新真实用例在普通门禁中明确ignored。routing首轮误写不存在的target名退出101、未执行测试，原命令错误保留；改为实际`windows_monitor`后r2退出0。 |
| 官方Windows原文 | 7个learn.microsoft.com直连均URLError；原失败保留。GitHub官方MicrosoftDocs内容GET成功3份，1条错误路径404。原Markdown与文件blob／SHA回执用于设计核实，不是运行验收。 |
| 依赖新鲜度 | `dependency-freshness-r1` exit1/reaped/159.629281s。37个registry包、113声明／使用，19行通过、94行registry_check_failed；完整新鲜度仍UNKNOWN，不合并旧部分响应。未改Cargo依赖或锁文件。 |
| Windows API／SSH／原生桌面 | 本批Windows尚未执行。CUA最新原生通道返回Mac锁屏且自动解锁失败，操作／截图0；旧直接API与replay不迁移为本批真实SSH成功。 |

工装与原始开发回执暂在 `work/windows-ssh-integration-20261117/`；
两位作者目录分别为 `work/windows-ssh-debug-author-20261117/`、
`work/windows-ssh-rust-author-20261117/`。
官方原文暂在 `work/windows-ssh-official-research-20261117/`，待设计与审查完成后仅保留
必要出处／摘要证据、回收全文副本。最终封存前不把这些临时路径当长期维护入口。

新非作者Chandrasekhar审查Rust／SSH编排，Confucius审查调试接纳／清理，
均已限定APPROVE，P0–P3全0并实际关闭；不与作者身份混用，
独立Cargo和Windows执行均0。Chandrasekhar实际两轮各39项Python通过、
2项编排故障控制、14项静态契约和12项原件检查通过，并独立复算765输入；
Confucius实际6次专项合计173 run＝170通过＋3跳过，0失败／错误，
其中仓库169项、独立续接控制4项。不同轮次不相加声称唯一用例总数。
父发现并已修复
debug owner关闭异常可能跳过管道收口与子进程回执保存的失败路径，
新增控制验证失败状态、双管道原件和后续stream关闭均保留；
两位非作者均窄复核关闭该项。

`_refresh_original` 的早退在离线early-signal注入下已独立续接留证；
官方调试终态语义未证明该顺序在正常受管Win32链中可达，故不计确认平台缺陷。
注入后自然seal保持HOLD，退出码不一致也不能成为terminal_proven；
将来若真实Windows出现同序，保留原句柄／事件并继续HOLD后修复，不放宽验收。

最终完整门禁r1在两位源码批准后执行并收取exit0终态。冻结765输入，
aggregate `71872297a9a7b067fd85723de4e5c9c10f908dcc6a7e63ab91eeed6cb902db3a`，
与候选、门禁后和库存后输入逐项相同。唯一target归属未切换、不clean。

本机原件将封存到[本批证据](2026-11-17-windows-openssh-monitor-evidence/README.md)，
按成员摘要、最终测试日志／库存、两位reviewer及完整Git输入独立重算。
该只读重算不重新运行产品测试，也不签Windows原生成功。

后续顺序：整合／专项 → 新非作者代码与功能复核并关闭发现 →
冻结后的最终完整工程门禁 → 普通提交推送 → 精确Windows CI真实执行 →
原件复核、文档收口与自有资源清理。完整产品目标保持进行中。
