# Windows 监控交付与回收原件

本封存绑定工程 `5d2df2fc5ac8566fe11182b7f4337076296dfa54`；
不改写[根540成员](../README.md)或[精确CI91成员](../ci-completion/README.md)。
`originals.tar.gz` **69成员／189,931字节**，SHA-256
`39ac8275c0fb141052811748296812aef348612580fc3126068e4c027c6ada69`。
manifest逐项保存路径、字节数与摘要，不包含应用包或测试二进制。

## 提交、正常包与回收

工程stage／commit／普通push均exit0、reaped；push **5.253968s**。
原读回同时保存local、tracking、远端分支SHA及干净工作区，三方均为上述工程。
后续文档提交不改工程输入：758路径／字节与最终r3冻结完全相等，aggregate
`6759f672ae1efb83f6a400145109ee9bdc75ed192e2b7647813a252a1eded210`。
本次复算没有运行Cargo或重放产品测试。

正常包当前在 `work/windows-monitor-delivery-20261117/package-r2/KeelShell.app`，
旁侧manifest绑定工程5d2df2f，5个应用文件与提交前包逐字节相同：
app SHA-256 `127526280ffc1409abe9f6519c0f8d5818c4fedf66a22468737bd8e0217e35bf`，
MCP `bb415f3ae3ae7ca62bdb78abe752b50f5a3a796dc65ec99490f54ee28cb3f693`。
arm64／minos15.0结构检查 **0/reaped/0.383260s**；6个文件摘要包含manifest。
未启动、未安装、未签名／公证，不代签原生桌面。

精确Git原件核对后已回收原集成／源码复核／功能复核／oracle／两份stage及旧CI目录：
**1,968文件／106,004,384字节、7个自有目录**，9份辅助原件保存在
`cleanup-retained/`。此前6份专用测试副本另为886,506,992字节，位于根封存的清理回执。
两次合计1,974文件／992,511,376逻辑字节，不宣称实际释放的磁盘空间。
共享target目录身份、当前正常包6个摘要前后一致，没有clean target或清理其他任务资源。
唯一target仍为 `work/recovery-batch-integration-20261115/target`。

此次文档提交后，CI／API复核／交付工作副本只能在与新提交真实Git原件逐字节核对后回收。
工装 `cleanup_committed_originals.py` 原字节亦保存在本archive，运行副本及最终读回／清理
回执在 `work/windows-monitor-finalize-20261117/`；交付目录保留上述当前正常包。
维护入口使用已提交archive；原报告内绝对work路径是历史采集位置，不要求恢复旧目录。

## 新的非作者 API 原件复核

Hypatia限定 **APPROVE，P0=P1=P2=P3=0**。
41输入／357,443字节、34个ZIP唯一成员及11个相关Git源码逐项核对，
第二轮102项离线复算断言通过。实际Windows API执行、测试重跑、Cargo、
target、PowerShell、远端、GUI、Git写均0；只签指定API原件和capture replay，
没有代签Quality、真实Windows SSH或原生桌面。

`windows-api-independent-review/`原样保留8份报告、receipt、脚本、失败和中间原件。
首轮CRLF匹配错误与一次路径误读均保留，不能把复算失败改写为成功。
新增`parent-binding.json`单独绑定原报告／receipt摘要、父级限定批准及关闭事实，
不向原receipt倒填字段。`agent-lifecycle-final.json`引用根封存的既有关闭记录，
新增该复核者已实际close；本批8个自有agent均关闭，没有新用户chat、worktree或分支。

再次`cua.getState`仍返回Mac锁屏、自动解锁失败，GUI操作／截图0；
正常包没有启动。完整产品、真实Windows SSH、原生桌面和六目标发行继续开放。

## 复算与失败保留

首轮根archive候选遗漏`integration/package-r1.json`，实际verifier exit1，
原日志及回执保留；修正目录过滤后的工作区重算exit0，
精确工程Git重算 **0/reaped/2.856026s**。
CI原件封存后的重算 **0/reaped/2.423535s**，其成功回执保存在本archive。

```sh
python3 -B docs/testing/records/2026-11-17-windows-read-only-monitor-evidence/delivery/verify.py
```

本封存实际重算 **exit0/reaped/0.093491s**，69成员、mismatch0。
验证原push／读回、包与清理、原失败、CI完成回执、758冻结、复核报告与关闭记录，
不访问已回收目录、不重新执行测试。此次封存及成功验证回执在上述finalize目录。
