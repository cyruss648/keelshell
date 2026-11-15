# 显式远程文本旧编码 — 2026-11-15

状态：`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_SCOPES_APPROVED`。本地源码提交为
`eb6869d7f78684de5a978e12ba333d48200d31ec`，没有 push。
[提交绑定](2026-11-15-legacy-editor-encodings-evidence/commit-input-binding.json)
从该提交 Git blob 和当前文件双向核验 612 项工程输入及 286 份原件，零 mismatch。
后续仅补文档回执，完整产品目标保持进行中。

基线为 `2cfe0deb732db78f43a9b76d9e2c7e5c214a1f24`。
core 与 session 作者使用两个隔离受管 worktree，不运行 Cargo；
父负责原生 UI、导入、唯一 target 与集中验证。
target 为 `work/recovery-batch-integration-20261115/target`，没有切换源码归属。
设计见 [ADR 0140](../../adr/0140-explicit-legacy-editor-encodings.md)，
用户合同见[文本编码](../../product/REMOTE_TEXT_FORMAT.md)。

## 功能与检查范围

- 显式 GBK、GB18030、Big5、Shift_JIS、Windows-1252；默认 Unicode 自动不扩大。
- 严格无替换解码、原始字节往返、保存完整文本回解和 best-fit 拒绝。
- 保持原始字节备份、授权、完整 snapshot、POSIX 无降级替换、共享隔离和完整读回。
- 下一次读取选项与当前文档格式分离；脏草稿重读审核与迟到结果绑定实际身份。
- 三主题明暗、双语、900×580 与 1440×900 的受控 GPUI 布局。
- 固定向量的真实 disposable localhost OpenSSH。它不替代原生桌面。

## 执行记录

原始命令、退出、超时、日志摘要和工程冻结已无损导出至
[持久证据](2026-11-15-legacy-editor-encodings-evidence/README.md)：
286 份原件共 `8,630,487` 原始字节，压缩与解压 SHA 均匹配。
提交前 index/blob 已逐项核验 612 项工程输入和全部 286 份原件，零 mismatch。
原 r3 冻结 612 项。core 文本 `63/63`（新 40）、session 专项 `36/36`
（新 30、复用 fixture 6）、app 新专项 `14/14`、真实 OpenSSH `17/17` 已通过。
两名新非作者范围均无开放 P0–P3；Rust 独立执行使用父预构建，没有独立重编译。
签署时均保留完整门禁条件。

`full-gate-r1` 已实际退出 `1 / reaped / 513.085s`；Python `187 run / 1 skipped`、
fmt 和严格 Clippy 通过，但 `server_archive` 为 `39/1`，失败用例是
`server_archive_dropped_dispatched_future_retains_quarantine_and_unrelated_ssh`，
错误为 `Invalid("invalid or unverified server archive result")`。
此命令尚未执行到 rustdoc 与后续 controller，不能按部分成功签署整批。
单例 `1/1` 与同组 `40/40` 随后通过，但没有按此跳过原失败。有界诊断重跑六轮，
第六轮取得同错误的 `37/3`，定位到 `anchored_parent` 的 stat/open 比较：
设备、inode、mode、uid、gid、flags 一致，共享临时祖先被兄弟测试创建/删除旁支，
导致链接数 `7↔6`、大小 `224↔192` 及时间改变。诊断仅在独立 Python shim
保存异常函数、行号及数值 stat，不记录正文或改变生产辅助程序。

祖先打开检查现与最终路径检查一致，只比较上述身份及权限字段。所选父目录、
成员完整 stat、摘要、no-follow、私有目录/payload ACL 与隔离规则保持。
加入六项真实文件系统 Python 回归：旁支创建/删除各一项，权限变化、目录替换、
symlink 替换拒绝，以及所选目录继续拒绝完整快照变化。先取得 `31 run / 2 errors /
1 skipped` 红结果，修后 `31 run / 1 skipped` 全通过；core 归档 Rust `14/14`
（包含嵌套 Python，不能相加成独立 Rust 数）与 session 归档 `40/40` 通过。
最终 r4 为 612 项，SHA256
`e8029ecb7eea9a28e76d4c47f8d4d280afa57de4356049f2d83ab39020749ce6`；
相对 r3 只有 helper 和其 Rust/嵌套 Python 测试两文件变化。最终 `full-gate-r2`
已实际 `exit 0 / reaped / 696.329s`。该差分的新非作者 `report-r3.md` 已批准，
开放 P0–P3 均为 0；
此前 app 测试、编译失败、依赖检查超时和原门禁失败原件均保留。

## 最终集中结果

| 范围 | 结果与边界 |
| --- | --- |
| 工程 Python | `187 run / 186 passed / 1 skipped` |
| 普通 Rust | `3077 passed / 0 failed / 37 ignored` |
| rustdoc | `14 passed / 0 failed` |
| 格式、严格 Clippy、依赖策略 | workspace all-targets、`-D warnings`、直接 registry `x.y` 全通过 |
| AI controller | readiness、observer I/O、small-stack 三组全通过；没有真实供应商调用 |
| 新默认 Rust | 84 个具体名逐项在最终门禁通过；包含在 3077 中 |
| 复用 fixture 自测 | 另一个 target 注册的 6 项包含在 3077 中，不算新增产品行为 |
| 归档辅助 Python | `31 run / 30 passed / 1 skipped`，含 6 项新回归；嵌套在一个 Rust 用例中，不能再加进 Rust 总数 |
| 最终测试库存 | core 262、session 241、app 1260、OpenSSH 17，完整名字与最终源码绑定 |
| 最终真实 OpenSSH | r2 `17/17 / exit 0 / 7.901s / reaped`，含一个新增 opt-in 用例；不与 r1 重复累计 |
| 正常程序 | GUI/MCP `exit 0 / 10.694s / reaped`，实际 arm64 Mach-O；没有把构建当原生交互验收 |

`final-gate-summary-r1.json` 对最终门禁日志 SHA、完整工程路径集合及全部 84 个新名
进行核验，漂移为 0；r4 集合 SHA 如上。最终测试 no-run 原件、完整库存、正常程序
摘要与 OpenSSH 的实际运行 binary、完整通过名另有读回。
MCP 正常程序由当前有效 target 复用（`fresh=true`），GUI 为本次编译；
不表述为全部程序都独立重编译。

真实 OpenSSH 是临时 localhost daemon，不是协议仿真。最终回执累计跟踪 91 个
进程身份，owned/observed processes stopped 与 private directory removed 均为 true；
父另查私有目录 absent，原监听端口连接返回 `ECONNREFUSED`。没有使用客户机器。
原 r1 的 17 项与其清理回执保留；最终 helper 已变化，使用 r2 验证当前输入。

## 非作者复核与冻结关系

本批两名新非作者分别复核 codec/session 与 app/UI/OpenSSH 合同，原范围开放
P0/P1/P2/P3 均为 0。codec/session 在新依赖下独立执行 `63+36`；
app 独立执行新专项 `14` 与相关回归 `54`，去重 `68`。它们使用核对来源、
摘要和库存的父预构建副本，**没有独立重新编译**；app reviewer 没有另跑 OpenSSH。

原报告分别为 `review-codec-transport/report-r2.md` 和 `review-app/report-r1.md`，
绑定 r3。r4 与 r3 的完整路径相同，只有归档 helper/测试两个文件变化，
其他 610 项与原范围源码、Cargo 输入完全一致。归档两文件由 codec reviewer
另做 r3 窄差分审查并独立执行 Python 红/绿回归，结论 `APPROVE_SCOPED_DELTA`，
开放 P0/P1/P2/P3 均为 0。独立红对照 `28 passed / 2 errors / 1 skipped`，
修后 `30 passed / 1 skipped`，均为 31 run、有界并已 reap。
两名 reviewer 的报告已收取，随后均已关闭。
不把原报告的 r3 条件改签为已运行 r3 成功门禁，最终整合依据是原范围审查、
不变集合、r4 差分审查及 r4 完整成功门禁的组合证据。

## 依赖和工具链

本机 Rust/Cargo 实测 `1.100.0`，活动工具链为默认
`stable-aarch64-apple-darwin`。祖先目录没有 toolchain 文件，根 workspace
没有新增 `rust-version`；`toolchain-final-r1.json` 保存重新核验结果。

新增 `encoding_rs = "0.8"`；按官方响应更新 zip 要求至 `"9.6"`，
rand 要求保持 `"0.10"`。lock 分别为 encoding_rs `0.8.44`、zip `9.6.0`、
rand `0.10.4`，新增/更新包缓存与官方 checksum 相同。
全量官方 metadata 采集窗口为 `2026-11-15T15:58:55Z`–`15:59:07Z`：
37 包、112 声明，更新后的 r4 报告全部 passed。r4 复用同一保存快照重新计算，
不是第二次联网查询；此前 60 秒、70 秒超时与 r3 发现旧版本的 failed 状态保留。

offline resolver 曾意外改动无关传递包。中间 lock 保留，最终恢复不相关记录：
基线与最终均 984 包，仅 zip/rand 版本集合变化、8 条既有记录的允许依赖边变化；
zip 新 record 内部 nix 从已存在 `0.31.3` 指向已存在 `0.32.0`。
没有其他传递包版本变化，全部边唯一闭合。Cargo `--locked` 构建、全仓门禁和
非作者独立图核验均完成，不把它简化为“只改了版本字符串”。

## 保留的失败与收口

app r1 `6/5` 暴露合并采用仍走 Unicode-only decode 的生产问题，现按准确已知
format 采用远端正文；r2 `10/1`、文件区 r3 `193/1`、r4 `13/1` 和
r5/r6 的 Popover import/滚动容器编译错误均保留。最终选择器改为文件标题旁的
Kit popover，r7 新专项 `14/14`；之后的最终 r4 全仓也逐项通过全部 14 名。
读取审核只在成功建立新 review identity 时捕获上下文，旧确认、迟到和撤权
不能刷新或继承新权限。完整合同与具体新名见最终库存和原始报告。

补充复核、无损封存、Git index/blob 及精确源码提交绑定已完成，随后核对临时目录
全部 292 份文件与保存清单逐项一致，再移除重复目录和六个可重建测试副本，共回收
`420,086,551` 字节。四个实现/复核 agent 已关闭，两个作者 worktree 及匹配本地
分支 absent。本批没有新建用户 chat 或 Podman 容器；真实 SSH fixture 已自行回收。
唯一有效 target、正常程序和其他任务资源保留。详见
[最终清理](2026-11-15-legacy-editor-encodings-evidence/cleanup-final.json)。
源提交后的文档更新没有改变已验证工程输入，不重复运行同一 Rust 全仓门禁。

## 边界

macOS、Windows、Linux 原生桌面、键盘、IME 与辅助技术未在本批运行。
headless GPUI、TCP/SFTP 与 OpenSSH 分别验收，不签原生接受。
旧编码使用 WHATWG 映射，不承诺任意厂商字节别名或标准修订；
无 BOM 的远端真实编码改变不可可靠自动检测。
不新增转码、持久编码配置、备份历史、自动重试或 AI 执行权限。
strict peer `HOLD/PARTIAL`、用户中继、性能收益、FILE-05/M4 与完整产品继续开放。
