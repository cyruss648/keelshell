# 原生流程与真实 IP 实验：持久证据

本批的范围、原始失败、计数与验收边界见
[集中记录](../2026-11-16-native-network.md)。只本地提交，不 push。
最终工程输入 r3 为 699 项，集合 SHA-256：
`f435b9fa78f9a40bc2962fa3043b983cf5818e1ab4e34db5deae5960dfc50e21`。
完整产品、三平台完整桌面、IME/VoiceOver、签名发行及 M4 继续开放。

最终 `full-gate-r3` 实际 `exit 0 / reaped / 701.952s`：Rust
`3105/0/40 ignored`、rustdoc `14/0`、工程 Python `229 run / 1 skipped`、
helper Python `25/25`，格式、严格 Clippy、x.y 和三组 controller 全通过。
两个新默认 Rust 名及完整输入集合已核对。两个新非作者范围均批准、开放 P0–P3
为 0，唯一 P2 及其四种异常分支全部关闭，reviewer 均已关闭。
app 独立执行使用父预构建 19 项，未独立重编译；网络独立执行 26 项 Python，
核对原始 netlink 与只读清理，没有另起真实 lab 或执行 Rust。
原报告签署时的门禁待定条件由父在最终输入上绑定，原件不改写。

## 原件与来源

`archive-manifest.json` 保存每份原件的相对路径、字节数、SHA-256，
以及 gzip 导出路径和压缩摘要。`raw/` 解压后逐字节等于原采集文件；
不裁剪失败日志、CRLF、原始 UTC、旧报告或非零退出。
`omitted-artifacts.json` 仅省略可重建的程序、测试二进制、包成员与缓存副本，
保留名称、字节数和摘要；正常包 manifest、测试库存、来源及执行原件保留。
临时私钥目录不进入原件，不能由历史绝对路径推断文件仍存在。
共封存 1422 份原件，`64,975,404` 原始字节，gzip 后 `46,813,810` 字节，
每份解压后逐字节一致。导出 helper 自身扫描的初次失败也保留，不改签成功。

| 原始路径键 | 内容 |
| --- | --- |
| `final-inputs-r1.json`、`final-inputs-r2.json`、`final-inputs-r3.json` | 699 项完整工程输入及两轮 Python 窄差分 |
| `full-gate-r1.*`、`full-gate-r2.*`、`full-gate-r3.*` | 首轮成功、中止与最终完整门禁的各自事实 |
| `final-gate-summary-r1.json`、`final-gate-summary-r2.json` | 分类计数、两个新 Rust 名及最后冻结绑定 |
| `ax-*red*`、`pending-jump-red-r1.*`、`affected-green-r1.*` | 原生发现的编译失败、行为红证据与 19 项修后专项 |
| `netem-cleanup-*`、`netem-client-exit-*` | 四种异常创建结果的真实子进程红/绿回归；延迟服务端为模拟 |
| `native-summary-r1.json`、`native-r2/` | 正常包原生窗口、独立 pin、审核、18 样本、主题/语言/窄窗口及清理 |
| `native-flow-r2.*` | 初始自动流程 exit 1；成功滚动接续不改写该退出 |
| `native-build-r2.*`、`package-r2/package-manifest.json` | 正常 arm64 GUI/MCP 构建及包内完整摘要 |
| `netem-author/` | 691 份逐字节导入的原件；r4 零 drop 失败和 SIGTERM 中断保持 |
| `real-r6/`、`real-r7/` | 修后真实 Linux netem、三场景各 18 样本、原始 netlink 和清理 |
| `resources-final-r1.json`、`resources-final-r2.json` | 父独立按 ID/名称、birth 身份、端口及私有目录读回 |
| `review-app/`、`review-netem/` | 新非作者各轮精确源码、发现、独立执行、证据和范围限制 |
| `review-resolution-r1.json` | 原签署条件、最终输入与门禁、发现关闭和 agent 释放的对应关系 |

macOS 原生比较使用自有 localhost sshd，实际监控目标是 macOS；
Linux 资源采样不支持是预期状态。AX 树包括屏外滚动内容，不能据其存在签同屏可见。
三主题切换保留完成状态；不是三主题全部结果、18 样本和整段命令同时可见。
正常 GUI/MCP 构建、原生限定操作、协议互通和 Linux IP 损伤分别记录。
qdisc drops 是调用窗口的 skb 计数，不做逐样本归因，也不承诺生产性能。

## Git 与清理

[verify_binding.py](verify_binding.py) 核对完整 Git-visible non-doc 路径集合、
冻结摘要、当前文件、Git index 或精确 commit 的 blob，以及所有 gzip 原件和解压字节。
正式本地功能提交前已生成 [index-binding-r1.json](index-binding-r1.json)，
[commit-input-binding.json](commit-input-binding.json) 和
[post-cleanup-binding.json](post-cleanup-binding.json) 绑定源码提交
`6699148b0fd9b5455c69a2ae6b9187ecb1cc3633`：699 项工程输入、
1422 份原件和当前文件零 mismatch。
[cleanup-final.json](cleanup-final.json) 记录删除本批 1435 个文件、
逻辑 `781,265,332` 字节以及实际资源读回，原批次目录已不存在。
不得通过修改原成功/失败回执来补签最终冻结。

唯一有效 Cargo target 的源码归属保持父 checkout；
有效正常程序、其他任务 worktree、容器及共享 base image 保留。
本批未新建用户 chat，用完关闭自有 agent，作者受管 worktree 已归档。
