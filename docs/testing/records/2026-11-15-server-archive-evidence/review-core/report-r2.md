# KeelShell server archive core — ACL 修复差分独立复核 r2

结论：**APPROVE（仅本次 core 修复及下列证据范围）**。

当前开放发现计数：**P0 = 0，P1 = 0，P2 = 0，P3 = 0**。
历史 **P2-001 已关闭**：真实继承 ACL 复现，在修复版中不再产生带额外 ACE 的
stage、payload 或发布结果；实际 ACL API 错误和打包中途新增 ACL 均拒绝成功回执。
未发现新的源码阻塞项，因此没有新增阻塞 notice。

完整工程 gate 是父线程另行完成的提交条件。本报告没有核验其最终成功回执，
不代签 session/UI、完整 gate、Linux 原生文件系统或三平台桌面验收。
core 代码可在父线程确认**相同最终输入**的必要工程 gate 成功后纳入本地功能提交。

`report-r1.md`、`notice-P2-001.md`、r1 源码快照和真实复现全部保留，未覆盖。
r1 的 HOLD 结论仍准确描述修复前输入；本报告另行签署冻结 r4。

## 输入、摘要与只读边界

HEAD 前后均为 `9112e232b6f0e022d299cba9e8498088c77b3d3f`。
最终绑定：

```text
work/server-archive-20261115/frozen-inputs-r4.json
SHA256 c01ae7a78099b4805bff413a795176e0aa7732871bf6a909cd83fff6277c5eda
553 inputs + 10 scripts，执行前后均零差异
```

稳定父预构建快照：

```text
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/prebuilt-r4/core
SHA256 2db85e430ddfa8cef5b1d915b413579e4b7ebb49e0aa431458aafd0eb013a88c
```

二进制摘要匹配 `core-binary-r4.json`；独立 `--list` 的字节匹配
`core-inventory-r4.txt`，**14 tests / 0 benchmarks**，清单 SHA256：
`776cf65a661dd9da33508358dcbd1e0f3cdf99628c91c9e1cdc7e29c453fc2b9`。
实际执行后快照二进制摘要仍相同。证据为 `binding-before-r2.json`、
`binding-after-r2.json`、`core-inventory-actual-r4.txt`。

已审查的最终核心输入：

| 文件 | r4 SHA256 |
| --- | --- |
| `crates/keelshell-core/src/server_archive.py` | `c38955fe645d7ab26af8a5538d63911af87dda24db0af125e361eb49976f3df2` |
| `crates/keelshell-core/src/server_archive.rs` | `215094b5cdc7f6cbb93ada567a6f34d717835c0d0f2b01065e063775a85802f4` |
| `crates/keelshell-core/src/archive.rs` | `786773b3839adaf01cb52f08e071e0d9ba8c70db97cfb4fdce4befbfdcd64d77` |
| `crates/keelshell-core/tests/server_archive.rs` | `4441aa67e701035bf3a8c3454d75dedc6af22bca613b6a151a02b9e55611cb5b` |

`snapshot-acl-candidate-r1/` 中保存的四个文件逐项匹配 r4；archive codec 没有改变。
Rust 协议实现差分仅为 rustdoc 的 macOS/Linux 和 ACL 范围说明。
Python 生产差分为 `private_acl`、平台能力限制和创建/复核处接入；
对应测试新增三项 Darwin 行为测试。没有重新扩大到 session/UI review。

执行环境仍为 macOS 27.2.1 arm64、Python 3.15.2。本 reviewer 未调用 Cargo、
编译、fmt/Clippy、修改源码、共享 target、commit、push 或 cleanup。
测试是**独立执行父预构建二进制**，不是独立编译或原生桌面验收。
所有自有写入只在 `review-core/` 下。

## 修复差分与 P2-001 关闭依据

### 1. 新对象先建立 ACL 边界，再写任何源字节

绝对路径及行号：

- [平台能力](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:55)：
  只接纳 Darwin/Linux，其他 POSIX 拒绝；Linux 还要求 fd xattr API。
- [private_acl](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:71)：
  Darwin 使用 extended ACL 的 fd API 清除并读回。
- [stage 接入](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:634)：
  stage fd 打开后清除/验证 ACL，再记录身份和校验 `0700`。
- [payload 接入](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:641)：
  payload fd 创建并包装后立即清除/验证 ACL，之后才构造 codec 并流式写源。
- [目录复核](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:574)、
  [读回前后复核](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:593)：
  复核不调用 clear，发现后来出现的 ACL 直接拒绝。

独立真实探针复用 r1 的攻击条件：先建立 `0600`、无 ACL 的自有源文件，再仅在
fixture 父目录添加 `everyone` 可继承读/搜索 ACE。未修改的 r4 helper 对 ZIP 和
tar.gz 均成功发布，`ls -led` 确认发布文件 mode 为 `0600` 且没有 ACE，
stage 已清理；源和父目录 ACL 的输出与执行前完全相同。

另外两项真实探针在同样继承条件下仅注入 hard-link `EOPNOTSUPP`。两个格式均
返回固定拒绝 exit 65、无成功 stdout；保留 stage 为 `0700`、payload 为 `0600`，
两者 ACL 均为空，源/父目录 ACL 不变。

三项新增 Darwin 风险测试又以单独、可见输出独立执行：**3 passed / 0 failed /
0 skipped**。其中真实继承测试在 `BoundedWriter` 构造之前观察到 payload 长度
仍为 `0`，stage/payload 均无 ACE；不是仅检查发布后的 mode。
相关测试位置为
[继承与写前观察](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/server_archive.rs:643)。

### 2. 空 ACL 返回值与 errno 没有使用通用 POSIX 假设

Darwin 分支先调用 `acl_valid`，再用 `ACL_FIRST_ENTRY` 查首项，仅在
`-1/EINVAL` 时确认这个有效 ACL 没有条目；存在首项的 `0` 返回会被拒绝。
`acl_get_fd_np` 返回 NULL 时仅接受 `ENOENT`，还要求该 fd 的 `fstat` 成功。
每次相关查询前将 ctypes errno 清零，因此不会用上一次调用留下的 errno 放行。

本次在实际 Darwin libc 上直接取得以下结果（数值为本机真实 errno）：

| 真实调用状态 | 返回 | errno | helper 结果 |
| --- | --- | --- | --- |
| `acl_init(0)` 产生的有效空 ACL 查询首项 | `-1` | `EINVAL = 22` | 空 ACL |
| 有效新文件 fd，没有 extended ACL | `NULL` | `ENOENT = 2` | 接受 |
| 实际 `chmod +a` 增加 allow ACE 后查询首项 | `0`，entry 非空 | `0` | 不清除模式拒绝 |
| 对有效 fd 清空后读回 | `NULL` | `ENOENT = 2` | 接受，`ls -led` 无 ACE |
| `acl_set_fd_np(-1, empty, EXTENDED)` | `-1` | `EBADF = 9` | 拒绝 |
| `acl_get_fd_np(-1, EXTENDED)` | `NULL` | `EBADF = 9` | 拒绝 |

API 位置为
[清空及读取](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:92)、
[严格空 ACL 判定](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:107)。
源码及实际结果一致；没有把 `EOPNOTSUPP`、`EACCES`、`EBADF` 等读取失败当无 ACL。

来源核对使用本机 Apple SDK
[sys/acl.h](/Library/Developer/CommandLineTools/SDKs/MacOSX27.2.sdk/usr/include/sys/acl.h:102)、
[acl_get_entry.3](/Library/Developer/CommandLineTools/SDKs/MacOSX27.2.sdk/usr/share/man/man3/acl_get_entry.3)、
[acl_get.3](/Library/Developer/CommandLineTools/SDKs/MacOSX27.2.sdk/usr/share/man/man3/acl_get.3)，
以及 Apple 官方开源 libc 的
[acl_entry.c 原件](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/review-core/official-references-r2/apple-acl-entry.txt:115)、
[acl_file.c 原件](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/review-core/official-references-r2/apple-acl-file.txt:72)。
头文件确认 `ACL_TYPE_EXTENDED = 0x100`、`ACL_FIRST_ENTRY = 0` 和 ctypes 签名；
官方实现确认 Darwin 的成功首项返回 `0`、空首项返回 `-1/EINVAL`。
无 ACL fd 的 ENOENT 解释另有上述本机真实调用证据，不仅依赖手册的路径措辞。

### 3. 设置/查询失败，以及后来新增 ACL，都保留失败状态

[真实 fd 错误测试](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/server_archive.rs:681)
分别使真实 `acl_set_fd_np` 和 `acl_get_fd_np` 使用非法 fd；两种情况均无目标、
无成功回执，保留空 stage。它们在二进制 14 项的嵌入 suite 中通过，也在单独
提取的 3 项 Darwin 测试中实际通过。

[中途 ACL 变化测试](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/tests/server_archive.rs:699)
分别用真实 `/bin/chmod +a` 给已经生成的 stage 和 payload 增加 ACE。
两个目标都被后续检查拒绝：没有发布目标或成功回执，非空 payload 保留。
这是实际权限变化；测试中的注入钩子只控制发生时点。

这些检查没有新增失败自动删除或重试分支，也没有改变 no-replace 发布协议。
原缺陷的修复前证据与修复后结果分别保留。

## Linux 保守合同及明确限制

代码位置：
[Linux fd xattr 分支](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:115)。

只删除已知的 `system.posix_acl_access`、`system.posix_acl_default`，且仅在
新 stage/payload 的 `clear=True` 调用；清除前发现名称含 `acl` 的其他家族立即
拒绝，不会先删除部分已知 ACL。第二次 `listxattr` 必须没有任何名称含 `acl`
的属性；不清除的复核同样拒绝已知 ACL。查询/删除错误直接传播到固定失败处理，
其他 xattr 不删除，其他 POSIX 平台在能力检查处拒绝。

独立受控分支探针覆盖：已知 access/default 清除、未知 NFS4 家族先拒绝、
只读复核不清除、list 不支持、remove 拒绝、清除后 ACL 重现和无关 xattr 保留；
另有其他 POSIX 的拒绝分支。**8 个分支组全部通过**。它们是在 macOS 上
模拟平台/API 返回的逻辑检查，**不是 Linux syscall 或文件系统验收**。

本次没有原生 Linux、default ACL 真实继承、其他 Linux 文件系统或旧 Python
证据。Linux `listxattr` 合同允许省略调用者无权访问的属性，因此本次只确认代码
对 fd-xattr 可见的已知/未知 ACL 的保守处理，不能把空清单解释为任意网络文件系统、
外部服务器 ACL 或所有权限模型都已验证。参考保存的
[Linux man-pages 原件](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/review-core/official-references-r2/linux-listxattr.txt)。
需要后续原生 Linux 及目标文件系统证据；本报告没有给它补签。

ADR 和产品范围已经同步收窄为 macOS/Linux、可验证私有权限，明确 Linux/旧 Python
运行验收未覆盖，见
[ADR 0135](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/adr/0135-reviewed-server-archive-packing.md:36)、
[服务器归档指南](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/docs/product/SERVER_ARCHIVES.md:17)。

## 实际执行及证据计数

| 命令/项目 | 实际结果 | 原件 |
| --- | --- | --- |
| r4 冻结及稳定二进制绑定、独立 `<binary> --list` | exit 0；553 inputs / 10 scripts 无差异；14 项逐字相同 | `binding-before-r2.json`、`core-inventory-actual-r4.txt` |
| `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/server-archive-20261115/prebuilt-r4/core --test-threads=1 --nocapture` | exit 0；14 passed / 0 failed / 0 ignored / 0 filtered；5.313s | `prebuilt-core-run-r4.json`、`prebuilt-core-r4.stdout.txt`、`prebuilt-core-r4.stderr.txt` |
| `python3 -B work/server-archive-20261115/review-core/acl_fix_probes_r2.py` | exit 0；14 probe groups passed / 0 failed / 0 skipped | `acl-fix-probes-r2.json` |
| 上述 probe groups | 6 组真实 Darwin（含单独执行 3 项 Python 风险测试），8 组模拟分支 | 同上 |
| 探针包装器实际子命令 | 37 个：35 个 exit 0，2 个预期拒绝 exit 65；0 timeout | 同上逐命令 argv、stdout/stderr、stdin SHA、耗时 |
| 单独提取的 Darwin 风险测试 | 3 passed / 0 failed / 0 skipped；0.551s | `acl-fix-probes-r2.json` 末项命令及 `darwin-risk-tests-r2.py` |
| 结束绑定与自有进程核对 | exit 0；553/10 仍零差异，binary 未变，r1 report/notice 未变，0 owned running workers | `binding-after-r2.json` |

这些数量属于不同层次，不把 3 个 Python 测试或 14 个探针组累加进 Rust 14 项，
也不叠加父线程此前执行。预构建 suite 包含完整嵌入 Python suite，但内部输出被
Rust 测试捕获；只单独声明本次可见执行的 3 个 Darwin 风险测试计数。

开始时曾尝试用父线程尚未更新的 `frozen-inputs-r3.json` 绑定修复候选，断言正确
exit 1，差异就是 helper 和 tests；没有把该候选签成 r3 通过。
父提供 r4 后才执行最终绑定和预构建验证，上表全部绑定 r4。
官方源码参考读取另有一个猜测文件路径 HTTP 404，不是行为测试失败；
主要语义由实际 SDK、成功取得的官方源码和真实 syscall 交叉核对。

二进制运行有 105 秒总期限；自有探针命令通常 12 秒，提取的 Python 风险 suite
为 35 秒，各测试内部 helper 有 10 秒期限。所有自有子进程都创建独立 process
group，并在 finally 中回收。TMPDIR/TMP/TEMP 统一指向 `review-core/tmp-r2`，
不生成项目外的 Python bytecode。没有遗留 reviewer 子进程；自有 fixture 和日志
保留，不运行 cleanup。

## 交付边界

P2-001 的 **r4 源码及本机真实 Darwin 行为复核通过**；r1 report/notice 不改。
新的 full gate、真实 OpenSSH、app/GPUI、session/UI 以及最终提交输入绑定由父线程
和另一 reviewer 按各自职责核验。没有本 reviewer 的独立编译或完整门禁证据。

Linux 原生、旧 Python、其他文件系统、三个桌面平台、IME/辅助技术及发行安装没有
在本次验证。检查仍是有界观察，不宣称抵御同 uid 恶意并发的全文件系统锁，
也不宣称清除并读回 ACL 后消除了所有并发时窗。
