# MCP 精确交付与限定原生回执

源码 `f910e0013832643f09b112b08443675a956d5551` 已普通提交/推送，
local/tracking/remote 三方一致，读回时工作区干净。
777 个新输入、769 个旧 CI 输入及 574 个原件成员的精确 Git 绑定通过，
`0/reaped/0.197138s`。提交后的这份回执仅补文档，不改变已验工程输入。

`originals.tar.gz` 保存 90 文件/3,729,876 逻辑字节，压缩后 2,020,794 字节，
SHA-256：
`763e6eabc3bdc2e599f44423ea4f7f0cfcdf631eae4677b526925594a45b37b9`。
`manifest.json` 逐件记录源路径、成员、长度及摘要，生成时逐件读回核对。
`parent/` 是提交/推送/源码副本回收、正常包与原生观察回执，
`review/` 是新的原图非作者复核，`runtime/` 是本次隔离空库原件。
可执行文件和正常应用包不在归档中。

## 本批正常包与原生子集

正常双程序 release build `0/reaped/219.335732s`；
arm64/minos15.0 结构检查 `0/reaped/0.136847s`。
app SHA-256：
`70045d6ff4f278a8a9df8bea075dcaa6bb65b3304895924101af588c90c2f110`；
MCP SHA-256：
`8579a3188d4d4af85bcd2efc67d87a2891dc61ba7d7aa88c1497753985b4148e`。
构建来源和六个包文件见 `parent/normal-package-manifest.json`、
`parent/native-observations.json`；777 当前工程输入的后次核对见外层
`build-input-binding.json`。这是父级构建与冻结链，不声称非作者独立重建。

原 PID34098 在新隔离目录实际运行并经关闭审核正常退出：
`0/reaped/340.564409s`，后次 `ps` 确认原 PID 不存在，没有退出后再次读取应用。
11 组原 JPEG/AX 为 2880×1866，未转码或缩放。
三套推荐外观各中文/英文 MCP 空授权面板、客户端选中样式、单模态及固定动作可见。
三张原图可直接从[设计截图基线](../../../../design/screenshots/2026-11-17-mcp/README.md)查看。

Boole 新非作者实际用 `view_image` 查看全部 11 张图，对可见子集限定
**ship / APPROVE，P0–P3 全 0**，现已关闭。
独立原件审计 347/347 通过，报告读回 13/13 通过；不是产品测试或 GUI 重跑。
首轮工装把 `captures.json` 辅助清单误计为截图的失败保留并修正。
父级使用 stale AX34 被拒、按重新读取的 AX7 成功的事实也保留。

复制模板因无活动授权始终 disabled；本次没有原生复制成功、未知动作核对层、
真实 SSH/文件修改、供应商请求、最小/紧凑尺寸、完整 IME/辅助技术、
Windows/Linux 桌面或发行验收。截图中的零计数、客户端选中和焦点轮廓不在
AX 中完整表达，只按可见像素事实批准；完整 native finish 仍开放。

## 远端与清理范围

UTC `2026-11-17T06:20:45.514348+00:00`，本工程
Quality44026933453 的 macOS job133849410649 为 success，
Ubuntu133849410428 和 Windows133849410871 仍运行。
Windows API44026933510、OpenSSH44026933475 已 API 状态 success；
这些只是本批实际状态快照，尚未复核本批新 CI 原始测试/artifact，不与旧工程混用。

Git 绑定后，按完整清单及原字节回收源码原件副本两目录，
260 文件/3,457,053 逻辑字节；再按已提交旧 CI 原件核对回收 314 文件/
7,314,736 字节。唯一 target 身份、旧/新正常包摘要在对应回收前后相同。
本次作者、源码/功能 reviewer、CI 原件 reviewer、原图 reviewer 均已关闭；
没有新增用户 chat、worktree 或容器。交付展开副本及隔离 runtime 只供最终
Git 绑定后按原件核对回收；小型最终推送/清理回执留在
`work/mcp-client-finalize-20261117/`，不作为产品实现或完整验收。

离线核对全部四归档、两个工程输入、原生包/进程回执和三张设计副本：

```sh
python3 -B docs/testing/records/2026-11-17-mcp-client-history-evidence/delivery/verify.py \
  --commit <exact-40-character-docs-delivery-commit>
```

该脚本不访问网络/target、不运行产品或 GUI，也不依赖已经回收的临时目录。
