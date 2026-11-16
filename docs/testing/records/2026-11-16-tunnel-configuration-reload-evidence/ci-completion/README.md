# 6f28f5c 三平台精确 CI 终态

Quality `43901863960` 绑定 `6f28f5c424752a221746fbfb621e319c20ff38b9`，
completed/success；watch 实际 `0/reaped/1258.244s`，没有活动 watcher，
没有取消、重跑或把后续源码移签到此成绩。

| 平台／job | 普通 Rust passed / failed / ignored | app passed / failed / ignored | Python run / skipped | rustdoc passed |
| --- | --- | --- | --- | --- |
| Windows `133420285732` | `3233 / 0 / 46` | `1351 / 0 / 2` | `293 / 34` | 14 |
| macOS `133420286347` | `3246 / 0 / 48` | `1362 / 0 / 2` | `293 / 2` | 14 |
| Ubuntu `133420286130` | `3239 / 0 / 49` | `1363 / 0 / 2` | `293 / 2` | 14 |

三个 Rust quality gate 均 success，macOS／Ubuntu 真实 OpenSSH step 均 success；
Windows 的该 step 为 skipped。每平台 92 个普通 suite 的声明、唯一结果和汇总
相等。CI app 没有独立 `--list`，Python 没有逐名 `-v`，不补造这些证据。
Rust 调用的 Python archive helper 与无 libtest harness 的 controller 单列。
这些终态不等于三平台 GPUI 桌面、完整产品、独立 peer 或六架构发行验收。

归档为 25 成员、压缩 1,564,160 字节，SHA-256
`0f8f10f10e889820f87977045dd3f7257b9402ac4f2308a1984ecd6fba934f16`。
包含原始完整 job 日志、终态 API、watch 终态、派生结果和解析器。
GH CLI 两列前缀及 literal-caret ANSI 引发的两次派生诊断保留；只修正派生层，
原始日志不改、不重放测试。验证器会从原文重建时间戳日志并逐项重算结果。

`python3 -B .../ci-completion/verify.py` 实际 `0/reaped/7.658s`：
全部成员、摘要、终态和三平台结果相等，并通过原批次封存从精确 Git blob
核对全部 721 工程输入，零 mismatch。验证只读取封存和 Git，不运行 Cargo／测试。
本批记录中原先“进行中”的签署时点保持在交付封存内；当前状态以本终态为准。
