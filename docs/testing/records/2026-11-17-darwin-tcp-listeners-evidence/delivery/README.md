# 源码交付、独立证据复核与资源回收

工程 `bd198a8ef2578ea9b96e0a603cd705bb2eadca13` 已普通提交／推送。
`2026-11-16T17:14:08Z` 本地、tracking、远端三方读回相同，工作区干净。
255 原件与 724 工程输入从准确提交的真实 Git blob 验证
`0/reaped/6.972s`，零 mismatch、测试重放 0。

本补充为 43 成员、344,639 字节原文，压缩 75,845 字节，SHA-256
`53f51c88d107cd08afc169aad4ba9db0e0fc33f6c6cb5424c5d79e1395661655`。
`manifest.json` 列出每个成员长度和摘要；封存时已逐项解包重算。
不含正常应用或测试二进制、Cargo target、密钥或 token。

- `feature-commit-r1.*`、`feature-push-r1.*`、`delivery-readback-r1.json`
  为真实提交、普通推送和三方读回。`delivery-source-r1.json` 保留提交后尚未收取
  push 结果的原时点；最终以 readback／`delivery-summary-r1.json` 为准。
- `verify-git-source-r1.*` 为准确源码与全部封存的验证，不运行 Cargo 或测试。
- `normal-package-commit-manifest-r1.json`、打包／结构检查回执绑定同一工程提交。
  正常包位于本地 `work/darwin-tcp-listeners-20261117/package-r1/KeelShell.app`，
  完整文件摘要与提交前包相等，arm64／minos 15.0 检查通过，未启动或安装。
- `owned-cleanup-r1.json` 与 `cleanup-execution-r1.*` 保存实际回收
  301 文件／88,633,128 字节的逐项 Git 原件依据；当前及上一正常包各六文件
  摘要、共享 target 目录身份前后不变。`cleanup_owned.py` 是本次实际工装。
- `delivery-review/report.md`、`receipt.json` 及完整派生回执保存新非作者 Locke
  的限定 **APPROVE，P0–P3 全 0**。两个文档验证器实际 exit 0/reaped，
  0.094828s／7.331875s；独立重算关键计数并核对 724 Git 输入和指定 11 项文档
  before/after 零漂移。没有 Cargo、功能测试、target 或 Git 写入。
  其两次计数诊断和报告生成诊断保留；结论只绑定原指定文档时点，
  不签署父级随后补记的推送、CI 或清理。Locke 已收取关闭，本批四位自有 agent 均关闭。
- `ci-list-r1.*` 和两个 `ci-snapshot` 保存当前源码 Quality `43916079699`：
  Ubuntu `133470463980`、macOS `133470464468`、Windows `133470464623`
  在捕获时均 in_progress。没有当前源码 CI 成功终态，也没有活动 watcher。
  旧 `6f28f5c` 三平台成功只签它自己的 721 输入。

源代码、真实 SSH、宿主完整门禁、精确 CI、包结构与原生桌面分别验收。
本补充不修改主 255 成员封存或上一批 CI 原件；完整产品及桌面／发行继续开放。
此补充提交并核对真实 Git blob 后，原件活动副本回收；
活动目录只保留必要正常包、最后交付工装和回执，不追加递归的“回执的回执”提交。
