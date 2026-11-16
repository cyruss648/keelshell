# 隧道配置重新加载：精确证据

本目录保存本批执行原件、独立报告和输入摘要；各自的原失败与未验范围保持。
设计及解释见[测试记录](../2026-11-16-tunnel-configuration-reload.md)、
[ADR 0151](../../../adr/0151-tunnel-configuration-reload.md)。

`originals/` 保留原文件字节；`manifest.json` 在最终门禁结束后封存所有成员。
`verify.py` 仅重新派生证据、校验源码，不重放测试，也不操作 target。
原报告的绝对路径描述执行时位置；运行副本回收后，以此处 `originals/` 为原件入口。

```sh
python3 -B docs/testing/records/2026-11-16-tunnel-configuration-reload-evidence/verify.py
python3 -B docs/testing/records/2026-11-16-tunnel-configuration-reload-evidence/verify.py --source FULL_GIT_SHA
```

输入冻结排除 `docs/` 和 `.hallmark/`，覆盖其余全部 tracked／nonignored 文件，
含完整 Rust、manifest／lock、脚本、资源、打包、workflow、README 和工程配置。
r2 有 721 项，聚合
`442393e019c51a19b8fe49d5040813e44a06b5a77cc3a7eb4c9b162701a979f2`。
原 base 是 `c81f718ccc3ee5d6a8cce6b7988b1cbcab8ea694`；
开发、测试和非作者复核发生在其上的冻结工作区，不把修改追溯归入 base。

主要原件：

- 官方依赖 r1 失败及 r2 全部通过；Cargo 精确升级与 session／真实 OpenSSH 回执。
- app inventory r1 编译失败、r2 过渡库存、r3 最终 1364 唯一名与精确二进制摘要。
- panel r1 `13/1`、tunnel r1 `60/3` 原失败及 Workspace r2 `7/0/0`。
- Socrates 静态源码限定批准；Ptolemy 独立核对 721 输入与父二进制，
  1364 名逐项一致、63 项隧道执行通过，覆盖 21 新名。
- 完整门禁的原日志、退出／reap 回执及逐名库存与执行核对。
- Mac 锁屏通道错误；GUI、截图、安装、签名、公证和跨平台原生桌面验收未执行。

该目录不含应用二进制、私钥、生产日志或账户凭据。独立 reviewer 执行父编译
产物不等于独立编译；loopback 与 OpenSSH 不等于三平台桌面。正常包、精确远端 CI、
发布和完整产品范围分别验收。
