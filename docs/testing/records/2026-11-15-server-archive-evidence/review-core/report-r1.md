# KeelShell server archive core 独立只读复核 — r1

结论：**HOLD**。

发现计数：**P0 = 0，P1 = 0，P2 = 1，P3 = 0**。唯一阻塞项为 P2-001：
Darwin 可继承 ACL 使 stage/payload/发布结果具有额外主体访问权限，
单独检查 `0700`/`0600` 及 uid 没有兑现 owner-only 合同。已先于本报告发出
`notice-P2-001.md`；父线程已告知停止并回收旧 gate，正在修复。本报告保留
r1 的原始结论，不把进行中的修复签成通过。

范围仅为 `keelshell-core` 的请求/清单协议、真实 Python helper、对应测试及必要
archive codec。没有重复 session/UI 审查，没有执行 Cargo、格式化、编译、提交、
push、cleanup 或修改共享 target。自有报告、源码快照和受控验证均在本目录。

## 输入及执行来源

基线 HEAD：`9112e232b6f0e022d299cba9e8498088c77b3d3f`。
冻结：`work/server-archive-20261115/frozen-inputs-r1.json`，
SHA256 `a7805633400c49c967499ada69eb4121cea8a76e0a258d10b7ad2dd96b57617a`。

审查开始时核对冻结的 553 个 input 和 10 个 script。r1 仅两个 app 测试文件
与当前字节不同；随后逐项核对 r1→r2，确实仅这两个 app 测试文件变化，
core/Python/session 不变。用户明确允许 core 继续按 r1 审核。相关 Cargo/core
输入共 102 项，开始时全部匹配。证据为 `binding-before-r1.json`、
`freeze-r1-r2-delta.json`。

核心文件 r1 SHA256：

| 文件 | SHA256 |
| --- | --- |
| `crates/keelshell-core/src/server_archive.rs` | `2be35019f791892d3796238113dc3985aea69aa89c616e1999dfe65391e77d2c` |
| `crates/keelshell-core/src/server_archive.py` | `96e06a6e5c51b062208bcb3108c720656a560a902c67ca85e2378e7b1570685b` |
| `crates/keelshell-core/src/archive.rs` | `786773b3839adaf01cb52f08e071e0d9ba8c70db97cfb4fdce4befbfdcd64d77` |
| `crates/keelshell-core/tests/server_archive.rs` | `b61f8fd17ecc542cab745f3b3099fb541c386a4c3b92ff8703e2085e7e7029e0` |

父预构建二进制：

```text
/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/work/recovery-batch-integration-20261115/target/debug/build/keelshell-core/3b4f8a5f97f7af36/out/server_archive-3b4f8a5f97f7af36
SHA256 ef7c3adfb0ad5943ac48e57aa8a67fd367f7dc036217957e8e4e690adf2b5214
```

二进制摘要匹配 `core-binary-r1.json`；预存清单摘要为
`776cf65a661dd9da33508358dcbd1e0f3cdf99628c91c9e1cdc7e29c453fc2b9`，
匹配回执。独立执行 `--list` 所得字节与 `core-inventory-r1.txt` 完全相同，
为 **14 tests / 0 benchmarks**，具体名称均逐项核对。

这是**独立执行父预构建产物**，不是独立编译、完整 gate 或原生验收。
运行平台为 macOS 27.2.1 arm64，实际 Python 3.15.2；没有据此宣称
Python 3.8、其他 POSIX 平台或三个桌面平台已实机通过。

## P2-001：真实继承 ACL 破坏 owner-only 边界

绝对路径及行号（均指 r1 精确字节；后续修复可能移动当前工作树行号）：

- [创建 stage](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:570)：以 `0700` 创建，但没有清除或拒绝继承 ACL。
- [创建 payload](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:575)：以 `0600` 创建，仍继承 ACL。
- [stage 权限检查](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:516)、
  [payload 权限检查](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:525)、
  [初始 payload 检查](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:579)：只验证 stat 元数据。
- [owner-only 对外合同](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.rs:182)：
  明确描述 owner-only `0700` stage 与 `0600` payload。

触发条件：归档父目录在 Darwin 文件系统上有授予额外主体读/搜索权限的可继承
ACL。不依赖同用户恶意并发或路径竞态。

真实复现先创建无 ACL、mode 为 `0600` 的自有源文件，再仅在其 fixture 父目录执行：

```text
/bin/chmod +a 'everyone allow read,execute,file_inherit,directory_inherit' <fixture-parent>
```

随后以原封不动的 r1 helper 执行 inspect 和 pack，tar.gz 打包返回 exit 0 和
合法成功回执。发布结果为 `-rw-------`，但 `ls -led` 同时显示：

```text
0: group:everyone inherited allow read,execute
```

在另一个自有 fixture 仅注入 publication `EOPNOTSUPP`，helper 正确返回 exit 65；
保留的 stage 为 `0700`、payload 为 `0600`，但分别存在：

```text
stage:   group:everyone inherited allow list,search,file_inherit,directory_inherit
payload: group:everyone inherited allow read,execute
```

影响：源内容可能通过成功结果或失败中间物授予其他主体读取权限。mode/uid/inode
检查没有检测到问题。真实证据为 `acl-probe-r1.json`、`independent-probes-r1.json`
中两个 `P2-001-private-source-*` case，以及保留的 fixture。未切换到其他 uid，
不将 ACL 读取误记为其他用户端到端读取验收。

修复验收需要在任何源字节写入之前，建立并读回验证 stage 与 payload 的私有 ACL
边界；失败必须 fail closed，并保持源/父目录 ACL 不变。需要真实继承 ACL
红回归、修复后新冻结/二进制及必要差分复核。重复 chmod 后只检查 mode 不足以关闭。

## 其余审查结果

未发现第二个 P0–P3 问题。以下是本次核对的实际边界：

- 精确绑定：请求保留源顺序、父目录、目标及格式；报告拒绝未知/重复/缺失字段，
  核对全部显式根/祖先、stat 类型与 nlink、大小、完整摘要和累计预算。
  参考 [请求](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.rs:112)、
  [清单](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.rs:353)。
- UTF-8 与 shell：路径置于 base64 JSON 参数，helper 和 stage 使用 POSIX 单引号转义；
  packing input 由严格解析后的 Rust 值重新序列化为 UTF-8。真实预构建用例执行了
  Unicode、单引号、`$(false)` 源路径和两个格式。参考
  [命令](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.rs:222)、
  [转义](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.rs:584)。
- no-follow 与漂移：逐级 descriptor-relative 打开保留祖先句柄，源文件检查打开前、
  打开后及关闭时快照，流式摘要与最终清单复核。独立探针确认父目录替换为 symlink
  或新 inode 均拒绝，socket 源不会阻塞，已有悬空目标保留。参考
  [打开](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:195)、
  [父锚定](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:211)、
  [扫描](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:293)。
- 编码与成员校验：helper 在写入时限制编码长度，并限制成员/名字/深度/展开字节；
  Rust 通过现有严格 codec 解码后比对所有成员的路径、类型、长度和完整内容摘要。
  ZIP32、GNU long-name tar.gz 的真实产物已通过这一组合；必要 codec 的完整 size、
  CRC/流终止、原始/有效路径和 manifest 预算已只读检查。参考
  [验证](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.rs:453)、
  [ZIP](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:433)、
  [TAR](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/archive.rs:619)。
- no-replace 与失败保留：发布只用 anchored `link`；失败不进入自动删除路径。独立
  syscall 故障探针覆盖不支持硬链接、发布后 fsync 失败、unlink/rmdir 失败、最后一次
  readback 失败及 payload/stage 被替换；均无成功回执，原源和未知对象保留。
  参考 [发布及已知对象清理](/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:563)。

## 实际命令及数量

所有 helper/二进制执行都指定本目录的 `tmp-r1` 为 TMPDIR，二进制运行额外设置
TMP/TEMP。Python 使用 `-B` 或 `PYTHONDONTWRITEBYTECODE=1`；自有验证的每个子进程有
12–15 秒期限，预构建总执行期限 105 秒。每个 process group 都在 finally 中回收。

| 命令/验证 | 退出或计数 | 原件 |
| --- | --- | --- |
| HEAD、冻结/摘要/清单绑定脚本；内部二进制 `--list` | scoped 核验 exit 0；14 项清单逐字相同 | `binding-before-r1.json`、`core-inventory-actual-r1.txt` |
| `<上述 exact binary> --test-threads=1 --nocapture` | exit 0；14 passed / 0 failed / 0 ignored / 0 filtered；13.563s | `prebuilt-core-run-r1.json`、`prebuilt-core-r1.stdout.txt`、`prebuilt-core-r1.stderr.txt` |
| 初次真实 ACL 复现 | chmod、inspect、pack、ls 共 4 个命令全部 exit 0；成功复现缺陷 | `acl-probe-r1.json` |
| `python3 -B work/server-archive-20261115/review-core/independent_probes_r1.py` | exit 0；13 probe assertions passed / 0 failed / 0 skipped | `independent-probes-r1.json` |
| 上述独立探针实际子命令 | 共 29；17 个 exit 0，12 个预期拒绝 exit 65，0 timeout | 同上，包含完整 argv、stdin SHA256、stdout/stderr、耗时 |
| 结束核验：HEAD、r1 输入差异、自有进程清单 | exit 0；0 owned running workers | `binding-after-r1.json` |

13 个独立探针中，11 个验证拒绝/保留行为，2 个用于成功复现 P2-001；
**13 个断言成功不表示权限合同通过**。预构建 14 项中的 Python adversarial suite
是一个 Rust 测试项，其内部成功输出由测试捕获，本报告不猜测其中 Python
skip 数量，也不把内部用例或作者此前执行重复累加为新的 Rust 通过数量。

首个全量冻结断言命令曾 exit 1，原因正是前述两个 app 测试文件已变化；
随后 r1→r2 差分及 core 相关 scoped 核验通过。该失败原样披露，不解释为
core 行为测试失败。

在 2026-11-15T06:23:21Z 的结束核验时，HEAD 未变、二进制 SHA 未变；
父线程已开始添加修复回归，`crates/keelshell-core/tests/server_archive.rs`
与 r1 不同。其余 101 项相关输入仍匹配。四个已审查文件的 r1 精确字节已保存
在 `snapshot-r1/`；后续修改需以新冻结/新二进制另行签署，不复用本报告替代。

## 风险和未验证范围

本报告签署时 P2-001 尚未完成修复验证。禁止将旧 14/14 或本报告改写为修复版通过。
未运行独立编译、fmt/Clippy/完整 workspace gate、真实 OpenSSH、session/UI、
GPUI 或任何桌面/发行安装验收；它们由父线程及另一 reviewer 按各自证据完成。

POSIX/Python 3.8+ 能力不足时现有代码拒绝，真实运行仅覆盖上述开发机；
未运行 Linux、其他 POSIX 文件系统、旧 Python 或 Windows。没有证明在服务器同
uid 恶意并发下形成全文件系统锁；检查与 unlink/rmdir 之间仍是文档限定的观察窗口。
独立替换探针验证的是身份检查发生前已替换时的拒绝，并非证明消除了所有竞态。

没有真实客户系统、凭据、外部上传、现有应用安装或共享 target 写入。
受控 fixture、原始 notice/失败证据均保留；结束核验没有本 reviewer 遗留子进程。
