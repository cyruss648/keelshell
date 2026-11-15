# P2-001：仅 Unix 调用的 fixture 方法在非 Unix 测试编译中触发 dead_code

记录日期：2026-11-15。状态：**修复候选已在工作树出现，等待 freeze-r5
绑定和 full-gate-r4；本 notice 保留修复前问题及证据，不代表 Windows 实际编译。**

这是本 reviewer 在最终收口追加检查中确认的 P2，与 core reviewer 的同号
Darwin inherited ACL 发现不是同一问题；各自以所属 review 目录区分。
原 `report-r1.md`、`report-r2.md` 不修改，后续以新 addendum 记录关闭。

## 位置

修复前冻结 r4 的文件：

`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive_fixture.rs`

SHA-256：`d3a784673c23245d77b174efba01d282db37f6ae2e2a1bcd17ce6dae41d25e9c`。

- r4 **31** 行：`pub(in crate::files) fn allow(...)`。
- r4 **44** 行：`pub(in crate::files) fn inputs(...)`。
- r4 **48** 行：`pub(in crate::files) fn hold_next(...)`。

当前修复候选三个方法已移动至同一绝对文件 **32、46、51** 行，各自前面
新加 `#[cfg(unix)]`。以下模块及调用合同也已实际读回：

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:3526`
  对 `test_server` 仅设置 `#[cfg(test)]`，不限制 Unix。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/test_server.rs:31`
  无平台条件地包含 `server_archive_fixture`。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive_tests.rs:209`、
  `:287`、`:359`：`allow`、`inputs`、`hold_next` 的实际 caller 分别处于
  `#[cfg(unix)]` 的 helper/测试中。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files.rs:3531`
  的 `#[expect(dead_code)]` 仅附着于后面的 `sftp_test_filesystem`，不覆盖
  `test_server` 或 `server_archive_fixture`。

## 触发和影响

非 Unix（包括 Windows）下，三个方法的 caller 被 cfg 排除，但 r4 的方法
本身仍被编译。`pub(in crate::files)` 是 ancestor 限定可见性，不会将这些
未使用方法变成免于 `dead_code` 的外部公共 API。

因此仓库 `cargo clippy --workspace --all-targets --locked -- -D warnings`
在这种编译条件下会因 `dead_code` 失败，阻止 Windows strict gate。macOS 的
完整 gate 通过不能覆盖该条件。本项是测试编译/门禁阻塞，不是已观测的
Windows 运行时归档错误。

## 证据

父已实际运行并保留以下原件，reviewer 已读取：

`/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/visibility-probe-r1.json`

命令为 `rustc --edition=2024 --crate-name visibility_probe --emit=metadata
-D dead_code .../visibility-probe.rs.txt -o .../visibility-probe.rmeta`，
实际 exit **1**，stderr 为：

```text
error: method `allow` is never used
  = note: requested on the command line with `-D dead-code`
error: aborting due to 1 previous error
```

这是 macOS rustc 对相同模块可见性合同的最小语义 probe；没有调用实际 Windows
target，也不是工程完整编译、Windows Clippy 或原生验收。本 reviewer 没有
执行 Cargo 或改动源码。

## 修复及关闭条件

在三个只有 Unix caller 的方法各自加 `#[cfg(unix)]`，与 caller 的编译条件
一致；保留跨平台模块、`lookup`、`serve` 及它们所用字段，保持现有可见性和
严格 lint。现场候选已经采用这一修复，没有扩大成整模块平台排除。

关闭时须机械核对 r4→r5 仅该测试文件三个 cfg 属性变化，生产输入完全一致，
并取得最终冻结对应 full-gate-r4 的成功回执。父的最小 probe 与本机 gate
仍不代签实际 Windows 编译；该平台未执行边界继续保留。

## freeze-r5 修复差分复核

2026-11-15 06:52:45 UTC，reviewer 对最终 r5 的 **553 输入 / 10 脚本**重新
计算摘要，**零 mismatch**。冻结 SHA-256：

```text
30770f74d710b231936e8537d5ce4082798e68a2894aa1ad7e5b9110afa08f92
```

r4→r5 的完整差分确认为两个测试文件，全部生产/core 输入和脚本不变。修复
不止初次候选的三个方法属性，还覆盖相同非 Unix 编译条件下的关联未使用项：

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/server_archive_fixture.rs:2`
  对 `Checked` trait 导入加 `#[cfg(unix)]`，防止三个方法移除后出现
  `unused_imports`；`filesystem` 保持跨平台导入。
- 同文件 **33、47、52** 行分别对 `allow`、`inputs`、`hold_next` 加
  `#[cfg(unix)]`。当前方法定义为 **34、48、53** 行，Unix caller 不变。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/test_server.rs:175`
  和 **262** 行分别约束 `Server.server_archives` 的测试控制字段及其 struct
  initializer；这个字段的直接读取也仅发生于 Unix helper 测试。
- 同文件 **61、117、122、216、217、242** 行的 Peer 字段、lookup/serve 路径、
  Fixture 创建、clone 和 Peer 初始化保留，未将跨平台 peer 整体 cfg 掉。
- `Recipe`、`Fixture` 的运行字段、`lookup`、`serve`、`run` 保持跨平台可达；
  没有放宽可见性，也没有添加 `allow(dead_code)` 或降低 strict lint。

reviewer 机械撤销上述 cfg 属性及导入拆分，两个文件 SHA 均精确等于 r4。
Unix 有效代码不变；非 Unix 仅移除没有 caller 的测试控制 API/字段。
修复源码复核通过，没有发现新的关联源码问题。

证据为本 review 目录的 `cfg-fix-verification-r5.json`、
`cfg-fix-r4-r5.patch`、`cfg-fix-snapshots-r5/`。原始 P2 及 r4 复现保留。
最终工程关闭状态由新 `addendum-r3.md` 和 full-gate-r4 回执确定；
Mac 最小语义 probe 继续不等同于实际 Windows 编译。
