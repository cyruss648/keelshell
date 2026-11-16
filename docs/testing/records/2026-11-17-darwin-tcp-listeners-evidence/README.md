# macOS TCP 监听端口原件

本封存绑定基线 `8d95d71cfa08181d5d44b689c108edcad7d995c4` 上的最终 r4
working diff，含 724 个工程输入，aggregate
`baae8aaab5903ad60509c23341b91a23553fac47ea7d1b0ff9ac993031c2e6ef`。
它不是把基线 HEAD 当作新功能提交；提交后使用 `verify.py --source <完整 SHA>`
从真实 Git blob 核对相同输入与封存。

`originals.tar.gz` 为 255 成员、3,791,780 字节原文，压缩 817,934 字节，
SHA-256 `eeea5237cf74e05c65ff4f7b286ecf84e2378355d77fe1b5fb920cb56a2c7330`。
逐项名称、字节与摘要见 `manifest.json`。没有测试二进制、正常程序二进制、
共享 target、临时密钥或 token。报告中的绝对活动路径保留其原始观察时点，
副本回收后从本归档读取原件。

关键成员：

- `inputs-freeze-r1` 至 `r4`、`test-binaries-r4`、`gate-binaries-r1`：
  保留来源与实际二进制身份，单包预编译和 workspace feature 产物分别记录。
- `full-gate-r1.*`、`gate-app-inventory-r1.*`、`gate-summary-r1.json`：
  92 个普通套件、14 rustdoc、1369 app 库存和新增 26 名的实际结果。
- `source-review-r1.md` 保留原 P2／REQUEST_CHANGES，`source-review-r2.md`
  为修后 APPROVE。`function-review/codex-function-20261117/` 是父预编译产物上的
  独立 177 项执行，不代表独立编译或 native。
- `openssh-darwin-r1/`、`r2/`、`r4/` 保留真实 Mac 失败和通过；
  mapped 诊断仅保存自有 listener 的观察形式，没有整机端口原文。
- `linux-recovery-r4/` 与 `linux-recovery-binding-r4.json` 区分基线 SHA 参数和
  实际 WIP 冻结／binary SHA；容器、network、派生及临时基础镜像清理原件同时保留。
- `app-darwin-r1.*` 的零测试、`missing-frame-negative-r1.*` 的修前失败、
  `session-monitor-r3.*` 的旧静态断言失败和 `gate-parse-diagnostic-r1.json`
  的派生诊断均未改写。
- `release-build-r1.*`、打包／结构检查与提交前 manifest 保存正常程序身份；
  `build.commit=null` 如实保留，不能追溯改成后续提交。
- `native-availability-r1.json` 为最新可用性观察：锁屏、GUI 操作／截图 0。
  `reviewer-closure-r1.json` 保存自有 agent 回收情况。

`python3 -B .../verify.py` 重算成员摘要、完整工程输入、声明／唯一逐名结果／汇总、
真实 SSH、Linux 恢复、原失败与正常包检查。初次封存验证 `0/reaped/0.165s`，
后续验证器补入正常包断言，实际结果按新回执记录，不重复任何 Cargo 或测试。
完整事实与未验边界见[测试记录](../2026-11-17-darwin-tcp-listeners.md)。
