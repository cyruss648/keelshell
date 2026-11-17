# 原 SSH 生命周期工程的精确 CI 终态

仅绑定工程 `cdf1e171f2f9b179f09db230f4739696d02fe80d`，
不代签后续 MCP、原生桌面或发行。
最终 API 观察为 UTC `2026-11-17T05:43:37.040413+00:00`，
三个 workflow 均自然进入 completed/success，没有 rerun/cancel。

| Workflow / 平台 | ID | 普通 Rust passed/failed/ignored | rustdoc | Python 主 run/skipped |
| --- | --- | --- | ---: | --- |
| Quality / Windows | `44020265369` / `133827817935` | 3422/0/66 | 14 | 526/53 |
| Quality / Ubuntu | `44020265369` / `133827818182` | 3428/0/69 | 14 | 526/3 |
| Quality / macOS | `44020265369` / `133827818303` | 3435/0/68 | 14 | 526/3 |
| Windows API | `44020265273` / `133827817731` | routing 1/0/0，9 filtered | — | lab 111/3 |
| Windows OpenSSH | `44020265248` / `133827817472` | native SSH 1/0/0 | — | lab 96/1 |

macOS/Ubuntu 各有真实 OpenSSH interoperability 18 项与 forwarding 6 项通过。
三个 Quality 各 95 个普通套件的逐名日志计数和 summary 相符；不是本机新执行
`--list`。Python quiet discovery 只有 run/skip 总数，没有逐名原件。
API 的 isolated-SSH job 仍 skipped、`ssh=UNRUN`；
两个 Windows artifact 均为 `native_gui=UNRUN`。
真实 SSH 的 20 个非 master 原对象自然终态包含 exit255/1，
早先 seal 的 master 仍存活为 HOLD；最终清理对象/handle 为 0，
master cleanup exit125 保留，不能描述为所有对象自然 exit0。

原件为 7 个 ZIP、172 个 ZIP 成员，4 个 artifact 摘要与 API digest 相等，
77 个父级离线检查通过、0 差异。首次 `gh` escape-guard 下载失败和原重取回执保留。
新的非作者原件复核由交付记录补充，不将父级检查称为独立审查。

Heisenberg 新非作者已对上述指定旧工程原件限定 **APPROVE** 并已关闭。
独立离线 13,543 项通过/0 失败，逐件核对 7 ZIP/172 成员、4 个 digest、
769 个旧 Git 输入及 314 件外层归档；没有重新执行产品或访问网络/target。
P0/P1/P2 均为 0；历史 checksum 范围 P3 为 1，已由本页实际澄清关闭，
未关闭 P0–P3 全 0。首轮审计工装的 argv0 双空格假设失败和原 SSH HOLD/
中间 handle/非零自然或清理退出均保留。原报告与工装见[独立复核](review/README.md)。

`originals.tar.gz` 保存 314 个文件、7,314,736 逻辑字节；
压缩后 2,046,994 字节，SHA-256
`88c25669ca00d7d0a6f6609ecad08e57feffcc06ac0c523248428615abccc4e2`。
外层 `manifest.json` 覆盖全部 314 件，包括两个 `SHA256SUMS` 文件。
原 collector 的根 checksum 表实际只有 312 条，另有根自身和
`bounded-snapshot-052139/SHA256SUMS`；其原 README “除自身覆盖全部”的表述
在此纠正，历史原文和旧快照字节保持。外层完整摘要不依赖这项原工装计数。

归档内 `snapshots/` 保存 API 与 GET 回执，`downloads/` 保存原 ZIP，
`inventories/` 保存逐名记录，`summary.json`/`verification.json` 保存派生检查；
`source-inputs.json` 及绑定回执记录 769 个旧工程 Git 输入。
可从[本批离线核验脚本](../../2026-11-17-mcp-client-history-evidence/verify_git_binding.py)
核对旧 769 输入及全部归档成员。脚本不重跑测试或访问网络。
