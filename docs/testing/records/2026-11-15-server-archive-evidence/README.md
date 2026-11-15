# 2026-11-15 服务器归档证据

结论：最终 r5 工程输入对应完整 gate r4 退出 `0`；两个非作者最终 APPROVE，
零开放 P0–P3。继承 ACL 和 Unix-only 测试声明条件两项历史 P2 均已修复、
独立复核关闭。仅代表本批工程与源码复核范围，产品总目标仍进行中。
详见[集中验证记录](../2026-11-15-server-archive-integration.md)。

## 最终入口

- [逐 suite 汇总](validation-summary-r4.json)：普通 Rust `2747/0/27 ignored`，
  rustdoc `14/0`、Python `131 run / 1 skipped`；全部 `72` 个新增默认用例逐名通过。
- [最终冻结 r5](frozen-inputs-r5.json)：`553` 个 Rust/Cargo/Python 工程输入和
  `10` 个门禁脚本，SHA256
  `30770f74d710b231936e8537d5ce4082798e68a2894aa1ad7e5b9110afa08f92`。
- [最终 gate 回执](logs/full-gate-r4.json)及[原始日志](logs/full-gate-r4.log.gz)：
  已实际 wait/reap，exit `0`、`603.804s`、未超时。
- [core 最终报告](review-core/report-r2.md)及
  [session/UI 最终报告](review-session-ui/report-r3.md)：APPROVE，
  独立执行核对摘要和库存的父预构建程序，未独立重编译。
  两个 reviewer 已关闭；[发现收口](findings-closure-r1.json)记录原报告摘要。
- [真实 OpenSSH r2](openssh-r2/result.json)：`15/15`，自有进程已停止，
  临时目录已移除；[正常 GUI/MCP 产物](normal-artifacts-r1.json)为 arm64 Mach-O。
  r4→r5 仅两个 `#[cfg(test)]` 文件变化，生产输入完全相同。
- [最终 app 库存](app-inventory-r5.txt)实际 `1208` 项；
  [输入差分](final-input-delta-r1.json)与
  [独立最终绑定](review-session-ui/final-binding-r3.json)保留逐项核验。
- 源码功能提交后的 `commit-input-binding.json` 将逐项绑定最终冻结、
  Git blob 和当前文件；最终交接记录该本地提交，不执行 push。

## 保留的失败与修复

`logs/` 保留 SHA2 0.11 十六进制编译失败、session 空隔离 API 预期错误、
app fixture 访问域和新按钮定位失败，以及首轮 Clippy 测试断言失败。
没有删除失败、将其改写为通过或放宽生产检查。

[core 原始 HOLD](review-core/report-r1.md)和
[ACL notice](review-core/notice-P2-001.md)绑定修复前源码。真实
[红回归](logs/acl-red-r1.log.gz)在两种格式中观察到源内容写入前的额外 ACL；
修复后 fd 级清除、查询失败拒绝和中途 ACL 变化回归通过，独立真实探针关闭该项。
`full-gate-r2` 收到阻塞 notice 后被实际终止，exit `-15` 原样保留。

`full-gate-r3` 在旧 r4 冻结上真实通过，不能代签后续两个测试 cfg 修正；
最终 r5 使用新的 `full-gate-r4`。
[session/UI notice](review-session-ui/notice-P2-001.md)、
[中间追加 HOLD](review-session-ui/addendum-r3.md)及原报告均未覆盖；
最终报告核对方法、导入和 Server 字段条件及最终完整 gate 后解除 HOLD。
最小 Rust lint probe 是本机语义证据，不是 Windows 实编译。

## 保存与清理

[归档 manifest](archive-manifest.json)保存 `248` 份原文件的原始/归档长度与 SHA256。
日志和补丁是无损 gzip；Rust/Python 源码快照及运行脚本使用不可执行的 `.txt`
文件名。原始报告中的 `work/server-archive-20261115/` 是执行时路径，在本目录
通过 manifest 的相对路径查找，没有为迁移链接重写原始证据。
四份官方参考原件本身带尾空白，因此也无损压缩，没有修剪文字；
[调整回执](reference-archive-adjustment.json)和原 manifest 均保留。

fixture 的符号链接和 socket 等特殊对象只记录元数据，绝不跟随或重新创建；
ACL 事实来自原始命令输出和回执，不将归档目录权限当成历史 ACL 快照。
临时目录及稳定预构建二进制不保存，摘要和实际清单保留。
`cleanup-before-archive.json` 是历史阶段的 pending 观察，交付后的
`cleanup-final.json` 单独记录最终资源收口。`sha256.json` 索引本目录除其自身外
全部持久文件；新增提交绑定和清理回执后重新生成索引。

仅清理本批自有运行目录和重复产物；保留唯一有效 Cargo target、其他任务 WIP
以及受保护 worktree。两个实现 worktree 已受管归档且本地分支不存在，所有实现
与复核 agent 均已关闭，本批没有创建容器。

三平台原生 UI、实际 Windows 编译、Linux 文件系统/default ACL、旧 Python、
IME/完整辅助技术、安装及发行验收仍开放。Windows 安全解包、Zmodem、中继和
可重复性能收益仍待后续功能完成。两轮在线依赖复查超时，无最新版本报告；
不能宣称本轮全部依赖已核验最新。
