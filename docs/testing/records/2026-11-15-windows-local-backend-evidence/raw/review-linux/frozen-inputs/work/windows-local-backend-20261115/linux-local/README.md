# Linux 本地 Rust 文件后端专项交付

本任务状态：`AUTHOR_LINUX_LOCAL_PASS / PARENT_INTEGRATION_REVIEW_PENDING`。
只新增 `scripts/run_linux_local_backend.py`、
`scripts/test_linux_local_backend.py` 和本目录证据，没有提交。
父任务的 Windows 写集、Cargo/锁、原有 runner、Rust 文件、docs 主文件不属于本任务。

## 最终实测

最终运行 `native-r3` 实际 wait/reap，`exit 0 / 55.609s`。
使用缓存 `docker.io/library/rust:1.98.1-slim-bookworm`、
Rust/Cargo `1.98.1`、`aarch64-unknown-linux-gnu`，与 Podman VM `arm64` 相符。
编译和测试以 UID 1000 在真实 Linux 执行。没有使用宿主 Cargo、fmt 或 macOS target。

| 独立选择范围 | 通过 | 失败 | 忽略 |
| --- | ---: | ---: | ---: |
| session lib `zmodem::`，含本地文件与 publication hold | 25 | 0 | 0 |
| session lib 本地 archive no-replace primitive | 1 | 0 | 0 |
| session Zmodem integration | 25 | 0 | 0 |
| session archive transfer integration | 18 | 0 | 0 |
| core Zmodem codec | 102 | 0 | 0 |
| core archive codec | 40 | 0 | 0 |
| core PAX codec | 40 | 0 | 0 |
| **Rust 专项总计** | **251** | **0** | **0** |

Python 最终 `python-r2`：`32/32`，`exit 0 / 1.254s`（外层实测）。
覆盖 SHA/缓存完整性、测试库存、原生工具链判别、容器身份、
创建超时后恢复清理、真实自有子孙进程有界回收及持久失败回执。
错误回执两项在修复前 runner 实际为 `2 errors / exit 1`，
修复后两项及完整 32 项通过。它们是 runner 注入异常回归，不是平台失败。

`native-r1`、`native-r2` 都为同一 Rust 输入 `251/251`；
对应早期 runner（r2 后仅补充两种底层异常的 FAIL 回执处理），只作历史记录，
**不将三次重复执行累计为 753 项独立覆盖**。
`python-r1` 的 30 项也不叠加到最终 32 项。

## 精确输入及可复现命令

Rust 源码只来自：

```sh
git archive --format=tar --output=source.tar \
  a092dd5712c0a92ee4af18970fb0500fb230b760 \
  Cargo.toml Cargo.lock crates
```

没有从正在修改的工作树复制 Rust/Cargo 输入。
该工程快照为 573 个普通文件，Git tar comment 的提交 ID、
每个文件的 Git blob ID 与内容均已实际核对，见 `git-source-binding.json`。

- 提交：`a092dd5712c0a92ee4af18970fb0500fb230b760`
- 原始 tar SHA-256：`1588c0927d10cecf45839b30639ac14a54d5dc6e8f26aee82e97d99128236a3c`
- 573 文件清单 SHA-256：`3263de3f6412d10570cf68dcb281ee5301110d902db4e8ff95ba8eb95e8522fe`
- 保留的完整工程快照：`source-a092dd5712c0a92ee4af18970fb0500fb230b760.tar.gz`

每次容器都使用全新的 `/linux-target`、`/linux-cargo` 和只读 `/snapshot`，
不复用前次测试二进制。六份最终 `*-binary.json` 保留 ELF AArch64 头、
实际二进制 SHA-256、完整 named inventory。
`native-r3/test-results.json` 和 `receipt.json` 保留逐名执行及终态；
全部选择名与实际运行名精确一致，源码运行前后零漂移。

```sh
python3 scripts/run_linux_local_backend.py \
  --revision a092dd5712c0a92ee4af18970fb0500fb230b760 \
  --registry-cache "$HOME/.cargo/registry/cache/rsproxy.cn-e3de039b2554c837" \
  --registry-index "$HOME/.cargo/registry/index/rsproxy.cn-e3de039b2554c837" \
  --output work/windows-local-backend-20261115/linux-local/native-next \
  --budget-seconds 900

python3 -B -m unittest discover -s scripts \
  -p test_linux_local_backend.py -v
```

`--output` 必须是新目录。Python 需要 3.11+。
实际执行的全部 argv、独立预算、stdout、stderr、退出码和 wait/reap
证据见各运行目录 `receipt.json` 与日志。

## Registry 与工具链边界

978 个官方 crates.io 锁定包中 949 个归档已缓存且摘要全部匹配锁文件。
29 个缺失缓存包主要属于未编译的 GUI 平台路径；完整名单保留在
`native-r3/registry-manifest.json`，不宣称完整工作区离线缓存齐全。
稀疏索引只复制公开包条目；归档来自现有公开镜像缓存，但每份已与官方源
Cargo.lock 的 checksum 核验。Cargo home、config、凭据文件没有复制或读取。

容器 `--network=none`、`--pull=never`、`--http-proxy=false`，
Cargo `--locked --offline`，没有拉镜像、下载依赖、安装包或
`--ignore-rust-version`。所选测试的依赖由 Cargo 1.98.1 正常执行版本要求检查。
这是可用缓存工具链兼容性实测，不是全部依赖最新版本声明。

## 清理与保留证据

`native-r3/cleanup.json` 证明最终 owned 容器按完整 ID、唯一名称和双标签核验后删除，
容器内 target/Cargo home/scratch 同时回收，宿主自有子进程已全部停止。
此前 probe/r1/r2 的清理与缺席检查保留，最终检查见
`cleanup-evidence-final.json` 和 `final-audit-r2/`。

原始 `source.tar` 在压缩回读、573 文件 Git blob 核对后去重，
registry 传输 tar 在与对应 manifest 摘要核对后删除；
原始日志、失败记录、各次 runner 副本、named inventory 和 receipt 均保留。
没有删除镜像、卷、其他任务容器、工作树或用户会话。

首次 probe 的 Podman 模板 `.Id` 字段不兼容导致 exit 1；
修正为 `.ID` 后完成工具链探测和按标签回收，
失败 stderr 与恢复 receipt 保留在 `preflight-r1/`，没有抹去失败。

## 接续与未验收范围

这些结果限定为固定提交上的 **Linux arm64 本地 Rust 后端及列明 codec 专项**。
它们不签署 Linux GPUI 桌面、x64、Windows、新独立 rz/sz 互操作、
完整产品、发行或全部权限/继承 ACL 矩阵。原有严格 peer HOLD 不受本任务改变。
父后续如变更 Windows 写集，只能对实际不变的 Unix 路径与依赖做限定连续性绑定，
不能把本快照改称新工作树的完整平台验收。

父任务仍需集成两个新脚本、执行相关工程门禁并取得新的非作者复核；
本目录提供原始可审查证据，不替父做该签署。
