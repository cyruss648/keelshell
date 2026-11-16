# 路线入口整理与依赖现场复核

本批只维护文档和只读检查原件，没有修改工程源码、manifest、Cargo.lock 或工具链。
工程源码仍为 `f094de6ad2789d168f5a24bacb316c98b83608ac`。
此前文档提交 `9ad1cd92148ed3498a3e9882e707128e9c84b1b7` 已实际读回本地、
tracking、远端 SHA 一致，读回时工作区干净。

## 当前路线与历史

[ROADMAP](../../ROADMAP.md)由 1,311 行收敛为 105 行，当前交付、接续顺序、
FILE-05/M4 和跨平台原生／发行缺口分别可查。
[历史原文](../../ROADMAP_HISTORY_2026-11-16.md)与上述文档提交中的原 Git blob
逐字节相等：98,769 字节，SHA-256
`5b7681c41a80f9a2800ed84587aff3091a465caaf22d283814e196129276ba3a`。
旧“未推送”、待评审、失败、HOLD 和 UNKNOWN 都保留原时点，不再充当当前待办。

并行作者只修改这两份路线文档，完成后已关闭。父整合时移除临时任务指令，
补充本记录和最新依赖原件入口；实际代码／Rust 测试执行为 0。
这是文档整理，不构成代码或产品验收。

## 官方依赖与实际工具链

`python3 -B scripts/check_dependencies.py --timeout 4 --jobs 8` 实际退出
`0/reaped/9.586s`，报告完成于 `2026-11-16T11:51:26Z`：
37 个 registry 包、113 处要求／使用、6 个本地包全部通过，errors 为空。
检查覆盖直接 `x.y` 要求及锁定版本；没有修改依赖或 lock。
旧批 15 个查询失败的 UNKNOWN 原件不改写，新报告只说明本次查询时点。

本机现场观察：

- `rustc 1.100.0 (0ef1bc386 2026-11-09)`；
- `cargo 1.100.0 (3acc79c0c 2026-11-05)`；
- active toolchain 为 `stable-aarch64-apple-darwin (default)`。

项目没有 `rust-toolchain`／`rust-toolchain.toml`，所有成员 manifest 均没有
`rust-version`。本机版本不是产品 MSRV，也不证明所有 CI runner 版本相同。

## 未完成边界

本批记录时，精确 Quality `43859165104 / attempt 1` 的 Windows filesystem
和 archive backend 两个预检已 success，完整三平台 gate 仍在执行；
确切库存／计数仍待原日志，不代签完整 app、Windows 桌面或整个产品。
相关终态继续维护于[根目录发布记录](2026-11-16-windows-archive-root-publication.md)。

原生 inventory 再次明确报告 Mac 锁屏且自动解锁失败；没有启动应用，
GUI 操作和截图均为 0。英文工具栏原生 P2、三平台桌面、IME／辅助技术及发行
验收保持开放。未保存无关浏览器 inventory。

只读命令回执、完整依赖报告、工具链和锁屏观察及原文绑定收据位于
[本批原件](2026-11-16-route-and-dependency-followup-evidence/maintenance-receipt.json)。
本批不重跑已通过且输入未变的 Rust 门禁；文档提交不替换当前源码的 CI。
