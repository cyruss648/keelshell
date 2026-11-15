# 2026-11-15 归档工具观察与 PAX 兼容集中验证

状态：`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`，限本批工程和源码复核范围。
完整门禁、独立复核和原生验收分别记录。唯一 Cargo owner 为父线程，
主工作树和 target 的实际路径记录在命令回执；
本地基线 `e97705c3d773ea6919d76441685c5064e641ca86`。仅本地提交，不推送。
源码本地提交为 `0f4719d72b83238ecaa7bb244c5ba913a7448224`，
[提交绑定](2026-11-15-archive-tools-pax-evidence/commit-input-binding.json)
从该提交 Git blob 和当前工作文件两侧核对 538 个输入及 10 个 gate 脚本，
逐字匹配最终冻结，零 mismatch、零缺失或额外工程输入。后续仅更新文档与回执。

## 集成与输入

两个实现 agent 分别在隔离 worktree 完成 session 工具检测和 core PAX 解码，
没有执行 Cargo。父线程开发 app 审核流程、受控 SSH UI fixture 与 OpenSSH
增量，按新 mtime 写入合入文件，独占原主树的有效 target。未发生 target 源码
归属切换，因此不清除有效产物。

最终冻结为[持久 r4 输入](2026-11-15-archive-tools-pax-evidence/frozen-inputs-r4.json)：
538 个 Rust/Cargo 输入与 10 个 gate 脚本。r2 相比 r1 只有 PAX 测试文件中
8 处等价字节字面量的 Clippy 修复，生产源码相同。r3 再修改两个 core 文件，
修复下述独立 P2，并新增 6 项 raw-size 回归。r4 只将新测试中的两处 NUL
转义改成等价的显式 `\x00`，生产源码与 r3 相同。app 实际清单为 1201 项，
包含 6 个 `files::transfer_tests::archive_inspection_tests::` 新用例。

冻结文件 SHA256：
`96bb5b3bded14516e3167d8281adee35901711cbca6ba9f41f91215afa179aef`。
最终 gate 后，538 个输入和 10 个脚本均零 mismatch。r2→r4 只改变
`crates/keelshell-core/src/archive.rs` 和 `crates/keelshell-core/tests/archive_pax.rs`，
工具/UI reviewer 已核验的写集保持相同；PAX 差分 reviewer 绑定最终 r4。
原始回执内的 `work/archive-tools-pax-20261115/` 路径是执行时位置；
同相对路径的持久原件位于[证据目录](2026-11-15-archive-tools-pax-evidence/README.md)。
日志和原始补丁以无损 gzip 归档，原文 SHA 和归档 SHA 分别保存。

## 最终完整门禁

实际命令：

```sh
CARGO_TARGET_DIR=work/recovery-batch-integration-20261115/target \
python3 -B scripts/check.py
```

外层 runner 的 `full-gate-r4` 已实际 wait/reap，退出 `0`、未超时，
UTC `2026-11-15T05:18:56.376056+00:00` 至
`2026-11-15T05:29:44.281979+00:00`，耗时 `647.902s`。
命令[回执](2026-11-15-archive-tools-pax-evidence/logs/full-gate-r4.json)与
[原始日志](2026-11-15-archive-tools-pax-evidence/logs/full-gate-r4.log.gz)保留。
日志原文 SHA256：
`b7c80776e0df4f62cf074225a4238445a95f2015a5f6151df9e020eddead330e`。

| 普通 Rust 分组 | passed | failed | ignored |
| --- | ---: | ---: | ---: |
| keelshell-app | 1199 | 0 | 2 |
| keelshell-core | 724 | 0 | 0 |
| keelshell-session | 481 | 0 | 19 |
| keelshell-ai | 194 | 0 | 5 |
| keelshell-mcp | 77 | 0 | 0 |
| 总计 | 2675 | 0 | 26 |

另有 workspace rustdoc `14 passed / 0 failed`；Python `131 run / 1 skipped`，
即 `130 passed / 0 failed`。fmt、workspace all-targets strict Clippy、
`x.y` 直接依赖策略以及 readiness、observer I/O、small-stack 三组 AI controller
全部通过。已知 `__eh_frame section too large` linker warning 没有改变退出码。
普通总数排除 rustdoc、controller 重复运行、reviewer 专项及单独执行的 ignored
OpenSSH 用例。

[逐 suite 汇总](2026-11-15-archive-tools-pax-evidence/validation-summary-r4.json)
保留实际通过的具体名称，核对本批新增默认用例：GPUI 6、session parser 12、
SSH fixture 16、PAX 40，合计 74，均已包含在普通总数中。app 清单 1201 项含
2 ignored；清单命令不计入通过次数。汇总解析保留 ZipWriter 预期 stderr 与
用例终态同行、rustdoc 额外 compile-fail 分组的处理，不重复分类。

## 已执行专项

| 检查 | 实际结果 | 范围 |
| --- | --- | --- |
| core archive + archive_pax，r3 | 40 + 40 passed | 既有行为、PAX 子集及完整原始 size |
| session 工具解析单测 | 12 passed | 完整协议、顺序、长度、UTF-8 与危险展示字符 |
| session 工具 SSH fixture | 16 passed | 加密 SSH 通道、取消、late-open、超时与输出上限 |
| app 工具 GPUI 控制器 | 6 passed | 实际审核控件、当前连接、取消、旧回复、失败刷新与三主题双语 |
| app 原归档回归 | 9 passed | 传输审核、源变化、发布、隔离与固定审核动作 |
| 真实 localhost OpenSSH | 14 passed | 当前生产固定命令互通、连接存活及既有传输行为 |
| 正常 GUI/MCP debug build | exit 0 | 当前源码构建；不等于原生操作验收 |

专项命令、持续时间、退出码及日志摘要保存在
[持久命令目录](2026-11-15-archive-tools-pax-evidence/logs)。
OpenSSH 的[结果回执](2026-11-15-archive-tools-pax-evidence/openssh-r3/result.json)
核实所有追踪自有
进程已停止、临时目录已删除；没有创建容器。

## 首轮失败与修复

app 首轮为 `4 passed / 2 failed`：受控 peer 在延迟回复时把普通 exec 消息
误判为关闭，另有结果行缺少测试观察注册。修复后实际 `6/6`。
PAX 首轮为 `33 passed / 1 failed`：稀疏头在拒绝前被底层 size 解析为另一
错误。提前白名单拒绝不支持的有效成员后为 `34/34`，没有放宽断言。
完整 gate r1 在严格 Clippy 的 8 个 `byte_char_slices` 停止，原日志保留；
r2 只修正等价测试字面量，完整 gate exit 0，634.918 秒，普通 Rust
2669/0/26 ignored、rustdoc 14/0、Python 131 run / 1 skipped；fmt、严格
Clippy、直接依赖策略及三组 AI controller 通过。该结果绑定 r2。

PAX 非作者复核 r1 为 `REQUEST_CHANGES / P2 = 1`：锁定 tar 依赖将 12 字节
base-256 size 截成低 64 位，原始声明 `2^64 + 13` 被当成 13。父补充两项
真实 Rust 回归，修复前均失败且明确返回错误接纳的 `Ok`，实际确认该推导。
修复以自有完整数值解析替代有损入口，所有允许头在覆盖和预算前先拒绝负值、
溢出高位、空数字和终止符后隐藏内容。新增 6 项覆盖 x/g、普通文件、目录、
GNU 长名、PAX size 覆盖、合法 base-256、u64 边界和八进制填充。修复后
core `40 + 40` 全过；r3 真实 OpenSSH `14/14` 与正常构建 exit `0`。
r3 完整 gate 在两处 `octal_escapes` lint 停止，10.386 秒，原件保留；
r4 修正等价转义，最终完整 gate 退出 `0`。r3→r4 没有生产输入变化，
因此 r3 OpenSSH 与正常程序构建仍绑定相同生产字节；最终测试门禁使用 r4。

## 独立复核与限制

两个新的非作者 reviewer 分别审查工具 transport/UI 和 PAX 安全；禁止修改
源码或共享 target，使用已核对摘要与清单的父预构建 binary 作独立专项。
其执行不属于独立重编译。工具/UI 为 APPROVE、零 P0–P3，实际独立
`6 + 16` 通过；PAX 首轮 `34 + 40` 通过但有上述 P2。最终 PAX 差分复核为
APPROVE、零 P0–P3，原 P2-01 CLOSED，独立执行 `40 + 40`，没有重复加上
首轮 74 次。两个最终报告分别为
[工具/UI r1](2026-11-15-archive-tools-pax-evidence/review-tools/report-r1.md)和
[PAX 修复 r3/r4](2026-11-15-archive-tools-pax-evidence/review-pax/report-r3.md)；
[PAX 首轮阻塞报告](2026-11-15-archive-tools-pax-evidence/review-pax/report-r1.md)继续保留。
报告签署时父 full gate 尚在运行，其当时条件原样保留；父随后核对最终 r4
成功回执及输入一致性，满足工程条件，没有让 reviewer 代签未来结果。
原始失败、首轮成功和独立阻塞报告均保持原件，不改写为最终通过。

本机工具链实际为 rustc/cargo `1.100.0`，仓库未 pin toolchain 或声明 MSRV。
本次官方依赖在线检查 60 秒超时，没有报告，不能宣称所有依赖版本最新。
`x.y` 直接依赖策略由本次 gate 独立检查。

## 交付与资源

本批实现和两个非作者 reviewer 均已关闭，两个实现 worktree 已通过受管工具
归档，现场 checkout 不存在。两次 localhost OpenSSH 的自有进程均已停止、
临时目录已删除；本批创建容器为 0。保留唯一有效
`work/recovery-batch-integration-20261115/target`。受保护的旧 review worktree
及其他任务 WIP 未清理。阶段性清理回执与最终清理状态分别保存，
不覆盖原先 pending。完成 127 份持久原件校验和提交绑定、确认目录没有打开文件
后，已回收本批重复工作日志与 tmp；两个实现 checkout 和本地分支均不存在，
四个已关闭 agent 的再次查询均为 not_found。详见
[最终清理回执](2026-11-15-archive-tools-pax-evidence/cleanup-final.json)。
只本地提交，不 push。

## 未验边界

PAX 是保守元数据子集，Python Unicode `?` 备用路径、vendor/encoding/sparse/
link 扩展仍拒绝。发现工具路径不表示版本、实际格式或 Zmodem 互通已验证。
服务端实际打包、Zmodem、中继、Windows 安全解包、三平台原生操作、IME/
完整辅助技术、签名、安装升级及发行验收仍开放。headless GPUI 和本机
协议互通不能代签上述边界。产品总目标、FILE-05 与 M4 仍在进行中。
