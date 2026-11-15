# 服务器归档 session / app 复核追加 r3

结论：**HOLD（仅待最终 freeze-r5 的 full-gate-r4 成功终态；修复源码复核通过）**。

本追加确认一项历史 **P2-001：非 Unix 测试编译的 dead_code 阻塞**，已即时写入
同目录 `notice-P2-001.md`。修复已完成并通过只读差分复核；最终工程关闭验证
尚待父 full-gate-r4。当前没有未修复的源码发现：
**P0 = 0，P1 = 0，P2 = 0，P3 = 0**；累计新发现为 P2 **1**，
不能把修复前的问题从历史中删除。

原 `report-r1.md` 和 `report-r2.md` 均保留；本追加用于更新原 r2 的条件性 HOLD，
不修改其当时的零发现和待证据结论。当前全批合并条件以本追加为准。

## 最终输入和修复范围

Checkout：
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`。

基线 HEAD：`9112e232b6f0e022d299cba9e8498088c77b3d3f`。
最终候选为该基线加未提交冻结：

```text
work/server-archive-20261115/frozen-inputs-r5.json
SHA-256 30770f74d710b231936e8537d5ce4082798e68a2894aa1ad7e5b9110afa08f92
```

2026-11-15 06:52:45 UTC，**553 工程输入（包含 Python）/ 10 脚本，零 mismatch**。
r4→r5 只有两个测试文件，全部生产/core 输入和脚本不变：

| 文件 | r5 SHA-256 | 核对内容 |
| --- | --- | --- |
| `crates/keelshell-app/src/files/server_archive_fixture.rs` | `4df7cfa53766a05799ec0f198eccacccd8f586795ab8355853e400dbc2d35b69` | `Checked` 导入和三个仅有 Unix caller 的测试控制方法使用同一 `#[cfg(unix)]`；`filesystem`、Fixture 运行字段、lookup/serve/run 保持跨平台。 |
| `crates/keelshell-app/src/files/test_server.rs` | `b954f6c7593ed6efa3622a3f10326943888152bc2b5a33c5658ca91ba2412a63` | 仅约束 `Server` 上的测试控制字段和 struct initializer；Peer 字段、创建、clone、lookup/serve 路径仍编译。 |

机械撤销 cfg 属性和导入拆分后，两文件 SHA 精确等于 r4；Unix 有效代码及行为
不变。完整差分、最终/修复前快照和摘要记录为 `cfg-fix-r4-r5.patch`、
`cfg-fix-snapshots-r5/`、`cfg-fix-verification-r5.json`。

## P2 触发、影响及修复

r4 中 `server_archive_fixture` 从 `files.rs` 的 `#[cfg(test)] test_server`
无平台条件地包含；`allow`、`inputs`、`hold_next` 却只有 `#[cfg(unix)]` caller。
`pub(in crate::files)` 不豁免未使用的 ancestor 可见 API。非 Unix 编译会保留
方法而删除 caller，strict Clippy 的 `-D warnings` 因 `dead_code` 失败。
`Server` 的相应测试控制字段也需要与其 Unix 读取条件一致。仅给三个方法加
cfg 还会使 `Checked` trait 导入未使用，最终 r5 已一并处理。

修复前绝对位置：
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive_fixture.rs:31`、
`:44`、`:48`，明确指 r4 字节。

最终修复的当前绝对位置：

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive_fixture.rs:2`、
  `:33`、`:47`、`:52`：trait 导入和三个方法条件一致。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/test_server.rs:175`、
  `:262`：Server 的测试控制字段及 initializer 条件一致。
- 同文件 `:61`、`:117`、`:122`、`:216`、`:217`、`:242`：Peer 运行路径和拥有关系
  保留，跨平台 fixture 未被整体排除。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:3531`
  的 `#[expect(dead_code)]` 只属于另一个 `sftp_test_filesystem` 模块，不能用于
  解释此处方法或字段为什么免于 lint。

父实际最小 probe 为 `visibility-probe-r1.json`：

```text
rustc --edition=2024 --crate-name visibility_probe --emit=metadata -D dead_code \
  work/server-archive-20261115/visibility-probe.rs.txt \
  -o work/server-archive-20261115/tmp/visibility-probe.rmeta
```

实际 exit **1**，明确报告 `method allow is never used`。reviewer 已读原件；
这是 macOS rustc 上的模块可见性语义复现，**不是实际 Windows 编译或 Clippy**。
最终 cfg 修复的源码正确性已核对，最终全批门禁尚未取得终态。本追加不把
Windows 未实编译改写为已通过。

## 已补齐的父证据

以下都是父实际执行，reviewer 只读核验回执和 SHA，未调用 Cargo。

| 证据 | 实际结果 | 绑定与限制 |
| --- | --- | --- |
| `logs/full-gate-r3.json` / `.log` | `python3 -B scripts/check.py`，exit **0**，**629.306s**，已结束并回收；日志 SHA `3e9e0ff9c5a3586abddba3e63dceb47920f42f443d60a250d3b5a46c7db92a0d` 匹配。Python 131 run / 1 skipped，app 1206 passed / 2 ignored，session lib 154 passed / 4 ignored；fmt、严格 Clippy、workspace 及三个 controller 专项完成。 | 该门禁属于 r4；r5 修改了两个测试输入，因此父已重新启动 full-gate-r4，不能用此成功代签最终全批门禁。 |
| `openssh-r2/result.json` / `tests.log` | 真实 localhost OpenSSH **15/15**，含新归档测试；owned processes stopped、temporary directory removed。 | session/helper/core 生产及 session 测试输入从 r4 到 r5 完全相同；可沿用这项实际互操作证据，不重复执行。 |
| `logs/normal-build-r1.json` / `.log` | `cargo build -p keelshell-app -p keelshell-mcp --locked`，exit **0**，**8.215s**；日志 SHA `fbe9a150b75869e26e7cd715888f01f3623345cd3047fda2237172ba55681a8b` 匹配。 | 普通 macOS arm64 GUI/MCP 构建，非桌面交互、打包或安装验收。 |
| `normal-artifacts-r1.json` | reviewer 重新流式计算两产物 SHA，均匹配：GUI `fef16361ec31e0998f0f533f68ccdcbb76858dd38970a0d8cc9abadd641041b9`；MCP `cf89dbc2b7fb77958d16c225d7d94c536970aca4c3f7675e544696a8a5811c88`。 | 父 metadata 明确绑定 r4→r5 的两个 test-only 输入变化；`#[cfg(test)]` fixture 不属于正常构建生产代码，生产字节未改变。 |
| `review-core/report-r2.md` | **APPROVE**，历史 inherited ACL P2 已关闭，当前开放 P0–P3 为零。 | core 的 r4 受审输入与 r5 完全相同；不重复 core helper 内部审查。 |

本目录的 `parent-final-gate-verification-r4.json`、
`parent-openssh-r2-verification.json` 和 `normal-build-verification-r5.json`
保存独立核验摘要。

## reviewer 执行数量、只读边界及合并意见

本追加没有重复执行 67 项，也没有执行新的工程测试或最小 Rust 编译。
此前 reviewer 独立执行的 r4 稳定预构建专项仍严格记为：
session **40**、session lib **11**、app 新专项 **7**、共用归档回归 **9**，
合计 **67 passed / 0 failed / 0 ignored**，不是 r5 的新编译/新执行。
三次库存命令不计测试数。完整实际命令和各份 JSON/log 保留在 report-r2 及
`independent-r4-summary.json`。

差分机械核对脚本有两次本地 exit 1：第一次删除 cfg 行时保留 initializer
缩进，第二次正则误吞 fixture 空行。改用完整行、仅空格/制表符匹配后，反向 SHA
核对成功。两次均为 reviewer 摘要转换脚本的问题，**0 个测试启动，0 个工程输入
mismatch**；已在 `cfg-fix-verification-r5.json` 保留，不计为行为测试失败。

原报告摘要保持不变：

```text
report-r1.md  280aa9f4c43d26109d8440f18c6b414d6bf3d8fbcd9cae8d7553e89a5e7df87e
report-r2.md  3f85a2197d95466f2e680ea3622b50354ec7563979ae344f2adbf449b23ad85d
```

所有 reviewer 写入仅在
`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/review-session-ui/`。
未修改/格式化工程源码、运行 Cargo、改 target、commit、push、cleanup 或安装。
此前七个自有测试/库存进程组均已退出、tmp 为空；45s 只读观察进程也已实际
wait/reap。父进程没有被本 reviewer 终止或回收。

当前剩余合并条件仅为**最终 r5 的 full-gate-r4 成功终态，以及结束时冻结零
差异读回**。取得后可关闭本分工 P2 验证状态并将本追加更新为 APPROVE。
预构建/headless、本机 probe、正常构建和 localhost OpenSSH 始终不代表
Windows 实编译、Linux 文件系统权限、三平台原生 UI、IME/辅助技术、发行或
安装验收。
