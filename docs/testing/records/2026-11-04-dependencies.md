# 2026-11-04 依赖更新与解析记录

状态：初始依赖作者限定记录如下；2026-11-04 后续已导入知识库/隧道/三主题组合，最新组合结果见本文件末段与[主题测试记录](2026-11-04-native-themes.md)。
基线为 `4cd1809b2e1b02ddfcae0c2cdef36a060a9affd7`，工作分支
`feature/dependencies-20261104`；本切片不提交、不推送。

## 版本来源与范围

按本轮用户要求，每次更新检查最新版本；直接 registry 依赖选取官方
crates.io API 中最高稳定、非 yanked 版本，manifest 保持 `x.y`。
本次复用同日独立只读研究在 **16:40–16:43 Asia/Shanghai**
逐包 `GET https://crates.io/api/v1/crates/<package>` 的结果，避免重复全量查询。
实施时核对本树 6 个 manifest、35 条显式版本要求、34 个 registry 包名，
包含 target/build/dev。`anyhow` 仅在 workspace 声明，成员实际直接使用 33 个包。
研究基线与实施前 lock SHA-256 均为
`fbeae2598384c700129e4ff8f7430e1c455d494cd88f1471d9eb725b8bba0294`。
完整研究为 `docs/research/2026-11-04-engineering-refresh.md`；
实施者已从主工作区只读核对，其文件尚未进入本依赖工作副本，
也不属于本切片的冻结文件清单。

下表的“解析版”均与上述官方最高稳定版相等；`toml` 的 `+spec-1.1.0`
是 build metadata，不是预发布标记。

| 包 | 更新后 requirement | 解析版 |
| --- | --- | --- |
| alacritty_terminal | 0.26 | 0.26.0 |
| anyhow（未被成员直接继承） | 1.0 | 1.0.104 |
| argon2 | 0.6 | 0.6.0 |
| base64 | 0.23 | 0.23.1 |
| chacha20poly1305 | 0.11 | 0.11.0 |
| chrono | 0.4 | 0.4.45 |
| directories | 6.0 | 6.0.0 |
| flate2 | 1.1 | 1.1.10 |
| gpui-kit | 0.9 | 0.9.0 |
| hmac | 0.13 | 0.13.0 |
| image（Linux） | 0.25 | 0.25.10 |
| nix（Unix） | 0.31 | 0.31.3 |
| process-wrap | 10.0 | 10.0.1 |
| rand（workspace / session dev） | 0.10 | 0.10.3 |
| reqwest | 0.13 | 0.13.5 |
| rmcp | 3.8 | 3.8.0 |
| russh | 0.65 | 0.65.1 |
| russh-sftp | 3.0 | 3.0.1 |
| rust-embed | 8.13 | 8.13.0 |
| serde | 1.0 | 1.0.229 |
| serde_json | 1.0 | 1.0.154 |
| sha2 | 0.11 | 0.11.1 |
| tar | 0.4 | 0.4.46 |
| tempfile | 3.27 | 3.27.0 |
| thiserror | 2.0 | 2.0.21 |
| tokio | 1.54 | 1.54.1 |
| tokio-util | 0.7 | 0.7.20 |
| toml | 1.2 | 1.2.0+spec-1.1.0 |
| url | 2.5 | 2.5.8 |
| uuid | 1.29 | 1.29.0 |
| win32job（Windows） | 2.0 | 2.0.3 |
| winresource（build） | 0.1 | 0.1.34 |
| zeroize | 1.9 | 1.9.1 |
| zip | 9.3 | 9.3.0 |

另由实施者读取 [Pillow 官方 PyPI API](https://pypi.org/pypi/Pillow/json)，
最高非 yanked 稳定版为 `13.0.0`，发布于 `2026-10-16T22:15:24Z`，
要求 Python `>=3.11`；`packaging/icon-requirements.txt` 更新为
`Pillow~=13.0`。未安装 Pillow 或重新生成图标。

## 镜像检查与解析

本机 Cargo 将 crates.io 替换为公开 `rsproxy-sparse`。先对
`gpui-kit`、`uuid`、`rmcp` 分别读取 rsproxy 与
`https://index.crates.io/` 的 sparse 条目：最高稳定版分别为
`0.9.0`、`1.29.0`、`3.8.0`，两端版本与 checksum 相同。
这是三项抽查，不能据此宣称镜像全量无滞后。本轮正常解析成功，
未使用额外镜像或本地 source replacement，未修改 Cargo 配置。

`cargo update` 成功，报告锁定 209 个新增或更新 package identity；
lock 总条目从 991 变为 977（集合差：增加 209、移除 223）。
随后 `cargo update --verbose` 成功，锁定 0 个包且 lock 字节保持；
没有恢复旧锁或用 `--precise` 回退版本。最终 lock SHA-256：
`edf2cdf319f4f28f1202cf8625e8dfe1feaacb97d9db43356d96e4c71884d08c`。
解析器使用开发环境 Rust `1.99.1` 的兼容性规则；项目未增加工具链 pin。

`cargo metadata --locked --format-version=1` 及离线读回均成功，
确认 5 个成员、977 个解析包，成员直接依赖边与上表一致。
GPUI 显式 tree-sitter 语言和 test-support features 保留；
图内含 `gpui-pre 0.3.11` 与 `gpui-fast 0.1.13`。

`cargo update --verbose` 报告的以下较新版本受上游精确约束限制；
metadata 已定位约束，另用官方 crates.io API 核对候选版本存在且非 yanked：

| 当前解析 | 更高候选 | 阻止直接更新的上游约束 |
| --- | --- | --- |
| generic-array 0.14.7 | 0.14.9 | crypto-common 0.1.7 要求 `=0.14.7` |
| pkcs1 0.8.0-rc.4 | 0.8.0 | russh 0.65.1 要求 `=0.8.0-rc.4` |
| rsa 0.10.0-rc.18 | 0.10.0-rc.20 | russh 0.65.1 要求 `=0.10.0-rc.18` |
| taffy 0.15.0 | 0.15.1 | gpui-fast 0.1.13 要求 `=0.15.0` |
| unicode-properties 0.1.3 | 0.1.5 | gpui-fast-web 0.1.13 要求 `=0.1.3` |

直接依赖是最新稳定版，不代表整个图全为稳定版。实际仍含上述 pkcs1/rsa，
以及 `ssh-key 0.7.0-rc.11`、`ropey 2.0.0-beta.1`、
`zed-font-kit 0.14.1-zed`、`zed-scap 0.0.8-zed`、`zed-xim 0.4.0-zed`。
上游精确约束未通过本机 patch、降低直接依赖或伪造版本绕过。

## 最小兼容修改与验证边界

依据已发布的 `rand 0.10.3` 源码，将 core vault 与 MCP IPC 的
`TryRngCore / OsRng` 改为 `TryRng / SysRng`；
继续调用可失败的 `try_fill_bytes`，分别保留 `VaultRandomness`、
`IpcFailure::Unavailable` 的错误传播。未改 AI/net 并行源码。

已通过：35 条 requirement 与官方版本表的 `x.y` 投影核对、
`python3 scripts/check.py --policy-only`、上述解析与 metadata、
两处兼容源码的文件级 `rustfmt --check`、`git diff --check`。

按并行构建协调，本切片**没有运行 Cargo build/check/test/Clippy**；
metadata 下载源码不构成编译验收。组合后需验证 GPUI UI/原生流程、
SSH/SFTP/隧道、vault/MCP、更新包 ZIP 读写及语言 grammar 构建
（`cc` 已从 `1.2.67` 更新为 `1.11.0`）。
Pillow 13 的图标格式、转换结果与视觉验收亦未执行。
`scripts/build-icons.py --verify` 只校验存量资产，不能代替用
Pillow 13 重新转换并验证生成结果；既有图标 manifest 记录的
Pillow 12.3.0 / zlib 1.3.1.zlib-ng 是历史生成环境。
本切片不更新公共状态文档；这些边界不能继承旧依赖组合的通过结果。

## 后续组合核验

已将冻结 manifest/lock、Actions/Python 3.15 和 freshness 检查器导入组合候选，保留后续 GPUI/russh/zip 兼容修复。2026-11-04T10:18:12Z 官方在线检查 **104 条声明/使用全部通过**，完整 JSON 保留在 `work/themes-20261104/dependency-freshness.json`。检查器新增 49 项无网络单测，已接入标准 unittest discover；`scripts/check.py --fresh-dependencies` 可在确定性门禁前显式执行在线检查。

## 最终组合与检查器复核

升级后的完整本机门禁退出 0，普通 Rust 2,007 passed/23 ignored、rustdoc 10 passed（1 项仅编译）、Python 初次 116 项/1 skipped，严格 Clippy、格式及额外控制器通过；最终 macOS arm64 staging 和有限 UI 检查另记于[组合记录](2026-11-04-native-themes.md)。

独立 reviewer Pascal 提出一个 P2：排除在工作区之外的 path package 的 dev 依赖不进入消费者根锁图，原检查器却要求其存在。最小离线 Cargo metadata 观察确认 excluded helper 的 dev/normal/build path 子包都不会自行变成 workspace member；普通/build 闭包仍属于根锁图，非成员 dev 闭包不属于。

修复先遍历声明，再独立计算成员闭包与锁图可达性。声明仍检查最新 x.y；非成员 dev-only 依赖只检查声明，不能伪造已锁定版本。新增 4 项回归覆盖 registry/legacy/target dev、dev-only path 闭包、普通/build path 闭包及另一入口独立加入成员后的严格锁检查。相同反例在旧检查器上实际失败，修后 **53 项检查器测试通过**；完整 Python suite **120 项/1 skipped**、x.y 策略通过。最后官方 freshness 从 **2026-11-04T11:45:03Z** 到 `11:45:19Z`，34 个 registry 包、104 条声明/使用、5 个成员全部通过。

这次末轮只修改两个 Python 文件；Rust 源码、manifest/lock、CI 配置和打包脚本未变，因此没有重复未受影响的 Rust/原生检查，也不把此前 929 输入快照误称为最后两个脚本的身份。原始反例、旧脚本、最后 JSON 和结果位于 `work/themes-20261104/dependency-review/`。远端精确提交 CI、其它原生平台和发行安装继续单独验收。

Pascal 最终只读复核关闭该 P2，P0/P1/P2 均为 0；独立运行 53 项检查器测试和 3 项额外隔离控制均通过，并用旧官方快照离线重放实际仓库的 104 行，结果逐行相同。两份脚本在复核期间摘要稳定；这份独立离线重放与父任务上面的新在线查询分别记录。

本机隔离 Python 3.15.2 venv 安装 Pillow **13.0.0**，`scripts/build-icons.py --verify` 实际通过，验证 35 个图标文件及 ICO/ICNS；图标产物未修改。Actions 新 SHA 和 Python 3.15 的远端执行结果仍需精确提交 CI，不能用本机结果替代。
