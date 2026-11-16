# 原源码的后续完整 CI 原件

精确源码 `f094de6ad2789d168f5a24bacb316c98b83608ac`，Quality `43859165104`，
已终态 failure。这里补充原封存之后的真实结果，不改写上级 62 成员的
NOT_RUN 时点或历史失败。

- Windows：filesystem `70/0/0`，archive backend `29/0/1 ignored`，
  30 名逐项相等；app `1330/0/2 ignored`，原四项失败均逐名 ok。
- Windows 后续 core archive `39/1/0`，权限重编码读回 `0664` 而要求 `0600`。
  后续完整 session 与 target 为 NOT_RUN，整个 Windows gate 没有通过。
- macOS／Ubuntu：完整 gate、真实 OpenSSH success，app 分别 `1341/0/2`、
  `1342/0/2 ignored`。
- 条件 Zmodem 静态候选仍未集成；其名称准确性 P2/HOLD 报告单独保留。

`originals.tar.gz` 保存 81 个成员，8,858,927 字节原文；归档 1,536,665 字节，
SHA-256 `9ab518f956776256855f8712c3eb38b413d2155b2d99d7b85b2015f51b26b36b`。
包括三平台完整原日志／回执、终态、watch、逐名结果、首次 CLI escape 拒绝、
Windows 分隔符初次派生解析偏差与修正结果。CLI／解析偏差不计为 Rust 失败。

验证：`python3 docs/testing/records/2026-11-16-windows-archive-root-publication-evidence/ci-completion/verify.py`。
该命令校验逐成员摘要、原日志／回执及上级精确 Git 源码绑定，不重放测试。
实际首次验证 `0/reaped/6.835s`，81 成员及 719 工程输入零 mismatch。
