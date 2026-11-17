# Windows OpenSSH 工装本机工程证据

本批基线为 `bfbea7bcc84ff433d28a635e85daed9cca337fb0`。
最终工程冻结 **765** 个输入，aggregate
`71872297a9a7b067fd85723de4e5c9c10f908dcc6a7e63ab91eeed6cb902db3a`；
候选、完整门禁后及独立库存后逐项相同，唯一 Cargo target 归属未切换。

`originals.tar.gz` 保存 **168 成员／505,490 字节**，SHA-256
`a1b85007ce551cf517534cf7691914c34eb9dde63841ce94567488783ad3fe73`。
`manifest.json` 绑定每个原件的路径、长度与摘要；
archive 内 `selection.json` 保存原路径和 20 份上游原文／响应体的引用摘要，
官方固定 Git blob 与 GET 回执保留，重复手册正文不再复制。

本机实际结果：

- 最终完整门禁 r1：`0/reaped/699.028665s`。94 普通 Rust 套件
  **3407 passed／0 failed／60 ignored**，14 rustdoc 通过；
  app **1398/0/2**、session **404/0/4**。Python 主 discovery
  **493 run＝490 passed＋3 skipped**；格式、严格 Clippy、x.y 策略和三组 controller 通过。
- 独立库存 app1400／session408／Windows routing10／真实 SSH 用例1，
  与最终门禁逐名相等。新真实用例在普通门禁明确 ignored。
- Windows 相关 Python 整合专项 **271 run＝268 passed＋3 skipped**，
  0 failed；真实宿主子进程与离线故障注入不代签 Windows。
- 两位新非作者分别限定批准，P0–P3 全0。source reviewer 两轮各39项，
  2项编排故障控制及静态检查通过；lifecycle reviewer 六次共173 run，
  170 passed／0 failed／3 skipped，其中4项是其独立续接控制。
  作者和 reviewer 均已实际关闭，没有新建用户 chat、worktree 或容器。
- 保留作者首轮60秒超时、依赖请求失败、非Windows opt-in 用例实际退出101，
  以及 routing 库存命令误用不存在 target 的原退出101。
  后者修正命令后 r2 成功，没有修改产品代码或重放完整门禁。
- 依赖完整新鲜度仍 **UNKNOWN**；未修改 Cargo.toml 或 Cargo.lock。
  本轮最新 CUA 原生通道为 Mac 锁屏且自动解锁失败，GUI 操作／截图0。

只读重算首轮封存、最终 gate、库存、两位评审和完整源码绑定。
当前工作区已有四文件修复，使用首轮精确提交模式；无参数模式仅适用于首轮工程输入：

```sh
python3 -B docs/testing/records/2026-11-17-windows-openssh-monitor-evidence/verify.py --source 41671be1a7fdfd96cec3a57f876519aeab2fa0d5
```

首次工作区重算已 exit0；工程`41671be1a7fdfd96cec3a57f876519aeab2fa0d5`
已普通推送，完整765输入及168成员原件的精确Git绑定重算
`0/reaped/2.410740s`，三方SHA一致且读回干净。
推送后Windows实际运行另行记录。
本封存的 Windows SSH 实际执行为0，不签 Win32 ABI、OpenSSH token/broker 链、
原生 GPUI Kit 桌面、安装或发行成功。维护入口为[本批记录](../2026-11-17-windows-openssh-monitor.md)；
archive 内原工作路径是历史出处，回收后不作为当前运行入口。
首轮真实CI与后续四文件修复的门禁／审查另见[修复证据](repair/README.md)，
本页168成员与原摘要保持不变。
