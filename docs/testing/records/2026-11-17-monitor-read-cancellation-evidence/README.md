# 监控读取取消的工程原件

本批基线`4c8654f219779f48afd485bec8ff1b284905fbf0`；
777工程输入r3 aggregate
`6d495b849c63e00dfc7ba8b867f3fa8b5ba3481de9e4b66669dee77f40fa22d5`。
设计与实际执行从[批次记录](../2026-11-17-monitor-read-cancellation.md)接续。

父级182成员、3,505,658逻辑字节，压缩归档734,239字节，SHA-256：
`eb3617e9356239c6ef2ed8225aeadab1dfaf724d62fce657938c141862e3f5f4`。
每件原路径、长度和摘要见[manifest.json](manifest.json)，原件在`originals.tar.gz`。
包含三轮输入、旧实现红控制、专项、两次失败、最终完整门禁、逐名库存、
最后输入复核与自有reviewer副本清理回执，不覆盖失败或预填交付。

最终完整门禁`0/reaped/709.145348s`：
普通Rust3466通过／0失败／68忽略，15rustdoc通过；
Python主526 run／3 skipped／0失败。格式、严格Clippy、依赖政策及三个controller通过。
96普通二进制逐名库存及前后摘要相等，最终777输入前后相等。
库存中的嵌入Python前缀规范化规则和三处原始行均保留。

分开封存的原件：

| 原件 | 成员 | 范围 |
| --- | ---: | --- |
| [源码／功能复核](review/README.md) | 63 | Turing the 2nd批准r2五文件；独立r1预编译12项，r2独立runtime0 |
| [工装修订复核](fixture-review/README.md) | 18 | Heisenberg the 2nd批准r3唯一Python修订；独立14项通过 |
| [空文件peer诊断](peer-diagnosis/README.md) | 81 | 固定上游发送条件的静态定位，未执行新传输；排除第三方正文 |
| [前批精确CI](../2026-11-17-mcp-client-history-evidence/ci-completion/README.md) | 131 | 只绑定f910e001，7原始ZIP／172内部成员和4个artifact摘要 |
| [前批CI复核](../2026-11-17-mcp-client-history-evidence/ci-completion/review/manifest.json) | 65 | Anscombe限定批准，独立2301离线检查通过 |

以上连同父级共540个归档成员；ZIP内部成员不重复加到此计数。
本批六位自有agent均已关闭，未创建新用户chat、worktree、分支或容器。
前批CI的SSH seal HOLD、非master自然255／1、master清理125、
API isolated-SSH skipped及native_gui=UNRUN保持，不能代签本批代码。
本批没有新正常包、GUI操作、真实Windows执行或发行验收。

提交后在仓库根目录执行：

```sh
python3 -B docs/testing/records/2026-11-17-monitor-read-cancellation-evidence/verify.py \
  --commit FULL_40_CHARACTER_DELIVERY_SHA
```

该工装离线核对提交内六个归档的完整成员集／摘要、r3的777个Git输入、
r2→r3唯一文件差异、两份独立批准、最终门禁／库存，以及前批f910的777个Git输入。
它不重跑测试、不访问target、网络或桌面，也不把历史CI重绑定到新工程。
后续普通推送、三方SHA读回与按字节回收记录另行补入交付封存；
只回收已封存自有副本，保留唯一target和两份正常应用包。
