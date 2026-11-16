# Kit 0.10 固定源码原生与 CI 证据封存

本目录封存源码 `3c331c1cc3cc428c33c606b9af578057325871b9` 的已结束
macOS 正常包观察及 Quality `43834927464 / attempt 1` 的三平台完整原日志。
封存是文件、回执和 Git blob 核对；本次构建、测试、GUI、网络、提交、推送及资源清理
执行均为 0。完整 Windows gate、完整原生与发行验收继续 HOLD。
场景、失败及未验收边界见[主记录](../2026-11-16-kit010-native-ci.md)。

## 归档与逐成员身份

| 项目 | 实际值 |
| --- | --- |
| 归档 | `originals.tar.gz` |
| 成员 | 111 个普通文件，无目录成员、链接或重复路径 |
| 压缩体积 | 5,571,935 bytes（5.314 MiB） |
| 成员正文合计 | 14,753,322 bytes（14.070 MiB；不含 tar 头及填充） |
| 归档 SHA-256 | `8b9413c275b33a845855e7bc841ea5d1c5c94ac89be167a5ebb9d57c49a3de97` |
| 工程输入 | 719 个唯一且排序的非 `docs/` Git 路径 |
| 工程集合 SHA-256 | `f0f64a6629b2441d63eef1e801189eaef69dd848aea0039eef4d09ff143b5164` |
| app SHA-256 | `f78cdfd3446bea3a40e3b390c4aa29af1d480c2a45c452118fc81f480971b790` |
| MCP SHA-256 | `5166352519c48c15f2d02db8986c4d9301ccd8e0d055c736cf9471bd88ce584b` |

`manifest.json` 列出每个成员的归档路径、来源相对路径、类别、原始字节数和
SHA-256，并绑定复制后的封存脚本、原 helper、原 verifier 和封存前核对摘要。
归档保留原字节，不改写日志、转码图片或重写旧结论；tar 元数据固定为
mtime=0、mode=0644，gzip mtime=0。

| 归档前缀 | 成员数 | 正文字节 | 来源与含义 |
| --- | ---: | ---: | --- |
| `delivery/` | 40 | 6,344,007 | 已有交付回执、完整三平台日志、Windows 去 ANSI 派生件及历史失败 |
| `windows-inventory/` | 5 | 472,748 | 已结束非作者 Windows 库存核对的报告、摘要、逐项库存和验证回执；是派生审阅材料 |
| `native-review/` | 3 | 104,025 | Pascal 的最终 `review-r2.md`、`summary-r2.json`，以及原字节保留的错误 `summary-r1.json` |
| `native/` | 63 | 7,832,542 | 正常构建/打包/进程/退出/清理回执、工程冻结、包清单、自有 fixture 定义、19 图和 21 AX |

选择范围与 `work/workbench-fit-20261116/seal_native_ci.py` 的原白名单完全一致。
该 helper 没有修改；仅在 `work/kit010-seal-20261116/` 的副本增加最终复核
`COMPLETE` 检查、封存前成员/字节/摘要冻结比对和 provenance 字段。
没有整体打包原生工作目录；排除 `private/`、`runtime/`、`sshd-public/`、
app/MCP 可执行文件和图标。构建包的五个清单文件只读重新散列，5/5 相符，
二进制正文不在本归档。

## 实际核对与保留的失败

- `verify.py` 直接读取本归档，核对全部成员、精确源码 Git 树及 719 个工程
  blob，实际退出 `0/reaped`；源码 mismatch=0、归档 mismatch=0，
  构建或测试重放=0。没有借用父正在修改的 Rust 工作树字节签署旧源码。
- 封存前独立核对 18 组命令 JSON 与原日志的字节数、SHA-256、实际退出值和
  reaped 状态；另核对七项 CI 抓取输出的回执绑定。
  三平台原日志头、checkout SHA、末尾 job 收口均存在，完整抓取的字节数和摘要
  与保存的抓取回执一致。
- Windows 原日志 3,970 行，70 名 preflight 与已提交预期逐项相同，
  实际 `70/0/0`；1,565 条 libtest 逐行结果与独立库存一致。
  app 为 `1325/4/2 ignored`，原八项归档为 `7/1/0`。完整 Windows gate HOLD，
  不能将 preflight 成功扩签为 app、未运行 targets、rustdoc 或后续 controller 成功。
- macOS 原日志 6,160 行，app `1340/0/2 ignored`；Ubuntu 原日志 6,312 行，
  app `1341/0/2 ignored`。两平台 job、Rust gate 及真实 OpenSSH step 均为 success；
  CI 执行不等同于两平台完整桌面验收。
- 19 张原文件名为 `.png` 的图片全部是 JPEG，SOF 尺寸逐张核对为 2880×1866；
  对应最终复核的解码元数据及逐文件摘要一致。21 份 AX 均原字节保留。
  01/21 只有 AX，不能补签不存在的截图。
- Pascal r2 为 `COMPLETE / APPROVE_LIMITED`，P0=0、P1=0、P2=1、P3=0；
  独立构建、测试、GUI 均 0。P2 英文工具栏裁切仍属于旧包。
  `summary-r1.json` 保留 `IN_PROGRESS` 和无效 PNG 尺寸字段，仅供错误历史追溯，
  SHA-256 为 `ccda49af162b794163df9174533c6ac0e7945e6ccf3e6de3cac14961e8b8f1a4`。
- `delivery/ci-fetch-r1.json` 的首次 Windows 日志抓取为 exit 1，对应 0-byte
  `windows-job-log-r1.log` 原样保留；完整日志是另次成功抓取的 r2。
  `delivery/seal-readiness-r1.json` 的 `2/reaped`、`source-ci-watch-r1.json`
  的 `1/reaped` 及精确 CI failure 均保留，后续成功回执不覆盖它们。
- 正常 app 由核实身份的 SIGTERM 结束，原回执为 `241/reaped`，不是正常退出 0。
  容器/镜像清理后的 absent 探针原 exit 1 也保留。本次没有再查 daemon、
  发送信号或进行清理。

## 验证入口与封存者回执

在仓库根运行下面的只读核对命令；不解压落盘、不执行归档内脚本，不调用 Cargo、
应用、GUI 或网络：

```sh
python3 -B docs/testing/records/2026-11-16-kit010-native-ci-evidence/verify.py
```

- `seal-audit-r1.json`：本封存者的成功封存前核对，完整列出 111 个来源及摘要、
  Git 输入、回执绑定、三平台日志、JPEG/AX 与历史失败范围。
- `seal-verification-r1.json`：本封存者的持久收口回执，绑定归档、manifest、
  README、主记录、verifier 和上述 audit；保留实际命令退出值、日志正文及摘要，
  并确认归档成员与仍在场的来源逐字节相同。它是派生验证件，不计入 111 个原归档成员。
- 自有 `audit-command-r1` 首轮因检查器把空映射与空列表直接比较而退出 1；
  修正只发生于自有脚本，`audit-command-r2` 实际 0/reaped。
  两轮回执均保留；该检查器错误不归为产品失败，也不改变任何原件。
  归档生成与原 `verify.py` 执行均实际 0/reaped。
- 自有脚本、原始执行日志和逐命令回执保留在 `work/kit010-seal-20261116/`；
  所需核对结论和实际回执另持久保存在本目录，不依赖临时原生工作目录继续存在。

本次写入仅限主记录、本证据目录与上述自有 work 目录。父正在修复的英文工具栏
及 Windows 失败不在此次签署范围；连接表单可视滚动、IME/RTL/VoiceOver、
完整主题/尺寸/语言、多平台桌面、模型请求、隧道流量、正常退出及发行仍保持各自
未验收边界。没有以本封存回执改变任何 HOLD。
