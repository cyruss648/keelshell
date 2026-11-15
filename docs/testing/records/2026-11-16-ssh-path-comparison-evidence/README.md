# 审核式 SSH 路径比较：持久证据

完整说明与交付状态见[集中记录](../2026-11-16-ssh-path-comparison.md)。
本批仅本地提交，不 push；完整产品、三平台原生桌面与 IP 丢包验收继续开放。
原始 UTC、失败和中间报告保留，不改签为后续成功。

最终 r3 共 669 项工程输入，集合 SHA-256 为
`066d7835e75d09895ae7a97f5b83863904a8dcbb0419bb1d8e865f89b7a047e0`。
父方完整门禁 `full-gate-r1` 实际 `exit 0 / reaped / 698.757s`：
普通 Rust `3103/0/40 ignored`、rustdoc `14/0`、工程 Python `203 run / 1 skipped`、
helper Python `25/25`，格式、严格 Clippy、x.y 策略和三组 controller 全部通过。
全部 26 个新增默认 Rust 名逐项通过，已包含在上述 Rust 总数中。
两名新非作者在 app、session/runner 各自范围均 APPROVE，累计六项 finding
全部关闭、各范围零开放 P0–P3；最终独立预构建执行分别为 11 与 15 项。
两名 reviewer 均已关闭，没有独立重编译。

正常 arm64 GUI/MCP 构建通过；GUI 实际重编译，MCP 是同一父工作树的有效缓存。
最终真实 localhost OpenSSH `openssh-path-lab-r2` 五次 opt-in 测试进程全部通过，
三场景各保留 18 个完整样本。用户态 TCP 流延迟/限速不等于 IP 丢包/netem。
这些工程与协议结果均不能替代真实桌面、IME、辅助技术或发行安装验收。

## 保存与定位

[archive-manifest.json](archive-manifest.json) 逐项保存 **258** 份原件的相对路径、字节数、
SHA-256、导出路径与压缩后的 SHA-256。每份 `raw/` 原件都可 gzip 解压并与
原字节核对；共核对 `7,556,590` 原始字节，压缩后 `1,029,380` 字节。
局部 binary 属性保留原始行尾，不裁剪日志或报告。
[omitted-artifacts.json](omitted-artifacts.json) 只省略可重建的编译二进制副本，
共 13 份、`690,793,648` 字节，保留每份名称、字节数和摘要；来源、库存与实际执行
输出继续保存。
临时私有密钥目录不在导出范围，真实凭据不进入原件。

原件里的旧绝对路径属于采集现场，临时目录回收后可按 manifest 的键查找：

| 原始路径键 | 内容 |
| --- | --- |
| `frozen-inputs-r3.json` | 完整 669 项工程输入与集合摘要 |
| `freeze-delta-r2-r3.json` | 五项修改、一项新增、663 项不变 |
| `final-gate-summary-r1.json` / `full-gate-r1.*` | 最终分类计数、26 个具体新名、真实命令和终态 |
| `binaries-r3.json` / `bins-r3/*-inventory.txt` | 当前父预构建来源、摘要和完整库存 |
| `focused-build-r1`–`r5` / `focused-tests-r1`–`r3` | 编译与测试失败、修复、成功原件 |
| `openssh-path-lab-r2/result.json` | 最终真实 target/jump、三个场景、网络条件、进程身份与清理 |
| `openssh-path-lab-r2/*/report.json` | 54 个最终原始样本及中位数 |
| `openssh-final-readback-r1.json` | 两轮 197 条 kernel birth 身份、16 次端口与私有目录读回 |
| `normal-build-r1.*` / `normal-build-readback-r1.json` | 正常 GUI/MCP、Mach-O 与摘要 |
| `review-app/report-final-r3.md` | app 范围、三个 finding 关闭、11 项独立预构建执行与限制 |
| `review-session/` | 传输/runner 范围、15 项独立预构建专项、Python 与真实实验审计 |
| `resources-before-seal-r1.json` | 两个作者 worktree 与三个 owned 容器 absent 的再次核验 |
| `final-review-resolution-r1.json` | 两范围原签署、相同冻结门禁与 agent 关闭的对应关系 |

审核报告中的执行使用父预构建稳定副本，没有独立重编译。报告签署时的条件
按其原文保留；后续由父记录相同冻结的门禁与最终整合，不改写原报告。
重复运行、主题/语言参数、wrapper 内 Python 和 reviewer 重跑均不重复累计覆盖。

## Git 与清理

[verify_binding.py](verify_binding.py) 同时校验完整工程路径集合、当前文件、
冻结摘要、Git index 或精确 commit 的 blob，以及每份 gzip 原件和解压字节。
`index-binding-r1.json` 为提交前读回；`commit-input-binding.json` 绑定精确本地
源码提交。后续文档回执提交不修改工程输入，不重复运行相同 Rust 门禁。

只有完成上述核验后才删除批次临时目录和二进制副本。最终资源与回收字节从
`cleanup-final.json` 读取；唯一有效 Cargo target、正常程序、其他任务的
worktree/容器/共享镜像保持。没有新建用户 chat。
