# Windows OpenSSH 有界计数复核修复

本封存只覆盖首轮工程 `41671be1a7fdfd96cec3a57f876519aeab2fa0d5`
之后的四个 Python 文件增量、修复后的本机门禁及首轮远端 CI 原件。
修复后的真实 Windows OpenSSH、原生桌面与发行仍待后续精确回执。
首轮真实 SSH 的 FAIL 保持，不用 Rust 采集成功或最终空 Job 补签。

`originals.tar.gz` 共 **50 成员／1,534,547 字节**，SHA-256
`80267e52512c12b94422468f4be62a7ede55ab6dd7efbf1d42dd8e6e8ebedb60`。
manifest 逐项绑定原件，selection 保存来源路径；
CI 原始 ZIP 及其全部成员摘要都保留，artifact 同时核对 GitHub digest。
上一层的首轮 168 成员 archive 和原摘要未改写。

修复只在原有截止时间内重泵调试事件、重取非原子的 Job 计数；
最终仍须完整累计计数、已接纳原对象集合、自然终态及封接纳前后复核全部成立。
持续不完整时仍失败，封闭后累计计数改变立即失败，异步终止不能提前返回零 active。
诊断保留前 16 次不完整读数和最后一次读数，公开快照不能修改内部历史。
实际 cmd.exe 在 argv0 后加入的 ASCII 空格可以归一化；生产选项与 Base64 保持精确。

实际本机结果：

- SSH 专项 **96 run＝95 passed＋1 native skipped**，0 失败，
  `0/reaped/0.364127s`。
- Windows 相关整合 **279 run＝276 passed＋3 skipped**，0 失败，
  `0/reaped/8.971765s`。
- 新非作者 Copernicus 对四文件增量限定批准，P0–P3 全 0；
  独立 36 个风险用例和 13 个自备控制共 **49/49** 通过、0 跳过，已实际关闭。
  原两位非作者只签首轮基线，不冒充审过新增量。
- 修复后最终完整门禁 r2 为 **0/reaped/731.529731s**：
  94 普通 Rust 套件 **3407 passed／0 failed／60 ignored**，14 rustdoc 通过；
  app1398/0/2、session404/0/4，Python **501 run＝498 passed＋3 skipped**。
  格式、严格 Clippy、x.y 声明策略与三个 controller 通过。
- 765 工程输入在门禁前后逐项相等，aggregate
  `fb78496b8d0b22bd6fc38da46df994d16c144d1f040f83ff468a98c49ff633af`。
  相较首轮只有四个 Python 文件改变，Rust、Cargo 和锁文件未变；
  原独立编译库存 app1400／session408／routing10／live1 与新 gate 逐名相等。
  复核库存未重跑测试或 Cargo，唯一 target 归属未切换。

首轮远端原件全部绑定 `41671be`：

| 工作流 | 精确结果 |
| --- | --- |
| Quality `43990824137` | 三平台 completed/success。 |
| 直接 API `43990824533` | completed/success，artifact14287416838、34 成员、46,698 字节 ZIP 与 GitHub digest 相等；手动 SSH 前置 job skipped。 |
| 真实 OpenSSH `43990824640` | completed/failure，artifact14287312342、44 成员、29,884 字节 ZIP 与 GitHub digest 相等。Rust 认证与三类采集 PASS；21 原对象中 20 非 master 自然终态、4 个 PowerShell 自然 exit0。完整 census 缺失和清理失败仍为 FAIL，首次坏读数的精确子条件 UNKNOWN。 |

只读重算 archive、原始 ZIP、修复完整门禁、继承库存、增量审查和全部源码输入：

```sh
python3 -B docs/testing/records/2026-11-17-windows-openssh-monitor-evidence/repair/verify.py
python3 -B docs/testing/records/2026-11-17-windows-openssh-monitor-evidence/repair/verify.py --source EXACT_REPAIR_COMMIT_SHA
```

工作区模式首次已 `0/reaped/0.275840s`，重放测试 0。
archive 内的生命周期是封存时快照：源审均已关闭，另有首轮 CI 原件 reviewer 在复核。
后续真实 CI 及其非作者报告另行封存，不改写本 archive。
依赖完整新鲜度继续 UNKNOWN；原生 GUI 操作与截图仍为 0。
