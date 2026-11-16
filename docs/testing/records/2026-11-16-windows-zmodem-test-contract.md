# Windows Zmodem 目录替换测试契约

## 实际输入与失败

工程基线 `01f6f6bd8272303fe049bc7b52976afefdb44c37` 已推送并读回三方一致。
随后文档 `b7d6e92a937b3ad994c13c37e19db0bd0d059c8f` 也已推送、三方一致且干净。
精确 Quality `43867112056` 的 Windows job `133298403995` 已 failure：

| 早期范围 | 实际结果 |
| --- | --- |
| filesystem | `70 passed / 0 failed / 0 ignored / 1.06s` |
| 完整 core archive target | `41 passed / 0 failed / 0 ignored / 17.11s` |
| 完整 session library | `327 passed / 2 failed / 3 ignored / 11.33s` |

session 的 332 个唯一编译库存名与实际执行逐项相等。
其中 Windows Zmodem 45 名为 `41 passed / 2 failed / 2 ignored`。
完整 Windows gate 因预检失败为 NOT_RUN。该 run 已 completed/failure；
watch 实际 `1/reaped/1580.816s`，没有取消或重跑。
macOS `133298403996`、Ubuntu `133298403683` 的完整 gate 与真实 OpenSSH
均 success，app 分别为 `1341/0/2 ignored`、`1342/0/2 ignored`。
两平台 core archive 均为 `41/0/0`，新 ZIP 权限回归实际为 ok。
这些结果绑定原源码，不代签当前修订。

失败的原名分别是：

- `zmodem_upload_local_confirmation_rejects_replaced_ancestor_even_with_same_bytes`；
- `zmodem_windows_reviews_reject_replaced_grandparent_with_original_leaf_and_source`。

两者原日志均为 typed `Os { code: 5, kind: PermissionDenied }`；
该日志没有独立 syscall 定位，不把显示文案当作额外原生证据。
原空父目录案例
`zmodem_local_publication_rejects_a_replaced_parent_without_writing_either_tree`
在该真实 Windows 运行中为 ok。

## 两种可达的攻击路径

前两例的审核保留了实际打开的后代源文件和祖先链；外层目录替换在系统 rename
阶段即被阻止。修订要求精确 `PermissionDenied` 和 `raw_os_error == Some(5)`，
同时核对原完整祖先、来源 identity、完整 bytes 和原审核仍有效。
第二例还重验原 destination，确认目标／staging 不存在；没有调用 publish，
因此不声称发生过“替换后应用拒绝发布”。

新名准确表达这个契约：

- `zmodem_windows_retained_source_blocks_ancestor_rename_and_preserves_review`；
- `zmodem_windows_retained_reviews_block_grandparent_rename_and_preserve_namespace`。

第三例没有打开的后代，原空目录替换后要求 `SourceChanged`、
`mutated == false` 和两棵树为空，正文逐字节保留。
其余 43 个测试正文、模块 prefix、45 名总数和原 2 个 ignored 均保持。
Unix 测试没有变化；不为了让攻击先成功而释放生产能力句柄。
原条件候选的名称准确性 P2 通过本批准确改名和 CI 两名映射修正处理，
原 HOLD 报告保持原文，不将其改写成首次批准。

## 冻结输入与实际验证

本批仅修改 Windows tests.rs 和 CI 的两个必需名；生产实现没有变化。
719 项工程冻结 SHA-256：
`96753dee615c9cd7711a48ad769ec5b14f49c45c581b0bb67323bb51ba508766`。
同一 checkout／target 的集中完整门禁已实际完成，没有切换归属或清理缓存：

| 范围 | 实际结果 |
| --- | --- |
| `python3 scripts/check.py` | `0/reaped/723.521s` |
| 普通 Rust | `3225 passed / 0 failed / 48 ignored` |
| rustdoc | `14 passed / 0 failed` |
| 其中 app | `1341 passed / 0 failed / 2 ignored`；1343 个唯一编译库存名与执行逐项相等 |
| Python 主 discovery | `290 run / 1 skipped` |
| 独立 archive helper | `30 tests`；不混加到主 discovery 数量 |
| 工程检查 | fmt、严格 workspace all-target Clippy、x.y 政策、三组 AI controller 全通过 |
| 输入读回 | 719 工程输入零漂移 |

新非作者 Turing 完成限定静态代码／功能复核，APPROVE，P0/P1/P2/P3 全 0；
独立确认 43 个原测试块不变、45 名映射、第三例保留、CI guard 准确及生产改动 0。
实际静态命令 12 次，11 个 exit 0、1 个 exit 1。非零是并行父任务辅助解析
脚本摘要变化，工程冻结与封存原件均无漂移；诊断原文保留。
独立 Rust／Windows／GUI 执行均 0，reviewer 已关闭。

宿主不会编译执行 Windows 条件后端；本批 Windows 实际执行仍为 0。
新名字及精确错误、身份和原审核断言须由推送后的精确 Windows CI 验证。
静态批准与宿主完整门禁不代签该结果。

## 原件与日志摘要修订

[本批证据](2026-11-16-windows-zmodem-test-contract-evidence/README.md)保存冻结、
两文件快照、diff、完整门禁／库存、旧 Windows 实际失败、旧三平台终态、
作者准备候选和新非作者原件。前批 CI 原始日志与错误 r1 派生摘要均保持。
旧解析器漏掉与 stderr 同行的 `archive_pax` 头，将两个套件的 81 项结果
归到 archive，并用 PAX 的 40 项摘要覆盖原 41 项。r2 根据实际 libtest
运行起点归属结果，允许下一 stderr 头先于上一次 stdout 摘要出现；
两个自定义 harness 与 rustdoc 单独记账。两平台各 92 个普通套件的声明数、
唯一结果名和摘要逐项相等，archive 为 41、PAX 为 40。只重建派生摘要，
没有重跑测试／CI、清理 Cargo 或修改产品源码。

原 Windows 失败和旧权限重编码失败保持各自源码身份。
当前 ZIP 修订的 Windows 41 项通过已实测，见
[上一批记录](2026-11-16-portable-zip-metadata.md)。
完整产品、FILE-05/M4、strict OO 独立 peer、三平台桌面和发行仍开放。
