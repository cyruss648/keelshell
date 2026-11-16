# Windows 远端只读监控原件

本批源码基线为 `4209e272260aff6374e6216423ce8e0157bea2f4`。
最终 r3 冻结 **758** 个工程输入，aggregate
`6759f672ae1efb83f6a400145109ee9bdc75ed192e2b7647813a252a1eded210`。
根目录最终 archive 为 **540 成员／2,162,791 字节**，SHA-256
`12efad3087af26c0603352c950be2ca34c4204d29c231ec2a14fdd5ca01a0bc2`。
`manifest.json` 逐项绑定路径、大小与 SHA；`selection.json` 保存来源及快照去重映射。

保留的主要原件：

- 三位作者、两位非作者及 oracle 修正报告，原缺口、失败及最终限定批准。
- r1/r2 完整 gate 在严格 Clippy 的原停止，修正后三处 helper 的最终 r3 完整 gate。
- 93 普通 Rust 套件 **3407/0/59 ignored**、app **1398/0/2**、
  session **404/0/4**、Windows routing **9/0/1**、14 rustdoc。
- Python 主 discovery **405 run／2 skipped**，内嵌 helper 与 controller 独立记录。
- 精确 gate 二进制的独立库存 app1400／session408／routing10 及逐名匹配；
  23 个 Windows app、44 个 Windows parser 和两个延迟 admission 回归实际 ok。
- 非作者 prebuilt app101／routing9/0/1、独立 Python111/0/0、
  11 个自备 oracle 对照；其三处 helper 最终 delta 只静态复核，独立编译 0。
- 真实 Darwin SSH15、Linux 恢复2及原 Linux identity allowlist 失败、清理回执。
- 三轮依赖请求原件与部分官方 metadata，完整新鲜度仍 UNKNOWN。
- 正常 release／提交前 arm64 包结构／minos15.0、包摘要、最新锁屏观察；
  六份专用二进制按原 SHA 回收 886,506,992 字节及全部 agent 关闭。

正常程序及测试二进制不入 archive。可从最终 Git 精确读回的快照只保存引用；
历史不同内容按 SHA 去重保留 19 个版本，而不是重复封入所有完整源码副本。
`candidate-r1/` 保留首轮目录过滤错误的原封存和 verifier；
它漏了打包／清理回执，后续校验实际失败，不能作为最终验收。
最终修正仅影响封存工装，没有重放或改写产品测试。

只读重算 archive、完整 gate、独立库存及源码绑定，不运行产品测试或已回收二进制：

```sh
python3 -B docs/testing/records/2026-11-17-windows-read-only-monitor-evidence/verify.py
python3 -B docs/testing/records/2026-11-17-windows-read-only-monitor-evidence/verify.py --source COMMIT_SHA
```

工作区重算 r2 已实际 exit0。首轮缺失回执的失败及后续精确 Git 绑定见交付补记。
实际 Windows API／ABI、cmd／PowerShell 原进程终态、capture replay、真实 Windows SSH、
原生桌面、签名安装和发行不由这份本机记录签署。
同一工程5d2df2f的两个hosted workflow已成功，原job／API原件及逐名结果保存在
[91成员精确CI终态](ci-completion/README.md)，不迁移上一批CI成绩。
推送读回、精确来源正常包、后续副本清理、额外非作者API回执复核及原失败
见[交付封存](delivery/README.md)。上述根540成员封存保持原字节，不添加或改写旧原件。
