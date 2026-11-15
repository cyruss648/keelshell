# P2-001 — 继承 ACL 绕过临时目录与 payload 的 owner-only 权限检查

状态：OPEN；core r1 的可复现阻塞项。2026-11-15，独立 reviewer。

位置：

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:570`：`mkdir(..., 0700)` 后没有拒绝或清除继承 ACL。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:575`：payload 以 `0600` 创建，但同样可继承 ACL。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-core/src/server_archive.py:516`、`:525`、`:579`：只用 uid、mode、inode、nlink 等 stat 字段判断 owner-only，没有检查有效 ACL。

触发：远端 macOS POSIX 文件系统的归档父目录有给其他主体授予读/搜索权限的可继承 ACL。用户不需要恶意并发修改。`0700`/`0600` 不会清除此类 ACL，stage 和 payload 可以继承额外访问权限；发布的硬链接继续保留 payload 的 ACL。

影响：审核所承诺的私有 stage/owner-only payload 不成立；源数据可以通过中间 payload 或发布结果向额外主体暴露，helper 仍返回成功回执。这与 ADR 0135 的私有临时目录合同和 Rust `packing_command` 的 owner-only 文档不符。

真实证据：`review-core/acl-probe-r1.json`。在唯一自有 fixture 目录上执行：

```text
/bin/chmod +a 'everyone allow read,execute,file_inherit,directory_inherit' <fixture-parent>
<unmodified server_archive.py> inspect <base64-request>  -> exit 0
<unmodified server_archive.py> pack <base64-request> <stage> < <inspection-json> -> exit 0
/bin/ls -led <fixture-parent> <fixture-parent>/result.zip -> exit 0
```

最终文件输出为：

```text
-rw-------@ ... result.zip
 0: group:everyone inherited allow read,execute
```

helper 实际返回 `archive_bytes: 134` 及完整 SHA256，没有 stderr。使用的是冻结 r1 的未修改 helper，macOS 27.2.1 arm64、Python 3.15.2。以上证明真实继承权限；本次没有切换到其他 uid，不将其写成其他用户端到端读取验收。所有 fixture、日志只在 `review-core/` 内，没有真实客户数据，没有 Cargo。

建议父线程按阻塞 notice 停止旧 gate。修复可以拒绝无法验证 owner-only ACL 的环境，或用受支持平台的真实 ACL 接口，在任何源内容写入之前安全建立并验证私有目录和 payload；不能只重复 `chmod 0700/0600` 后检查 mode。保留失败与冻结 r1 证据，新增真实继承 ACL 回归。

补充证据（原始复现保留）：`review-core/independent-probes-r1.json` 中
`P2-001-private-source-published` 与 `P2-001-private-source-retained-stage`。
源文件均预先设为 `0600`、本身没有 ACL，再给 fixture 父目录增加继承 ACL。
未修改的 r1 helper 成功发布 tar.gz，结果仍有 `everyone inherited allow read,execute`。
另外仅注入 hard-link 不支持错误，保留下来的 `0700` stage 有
`everyone inherited allow list,search,file_inherit,directory_inherit`，`0600` payload
有 `everyone inherited allow read,execute`。因此问题同时影响成功结果和失败中间物，
并非源文件原本公开权限的延续。两项均使用自有数据，未切换 uid。
