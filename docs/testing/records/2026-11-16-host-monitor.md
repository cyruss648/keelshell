# 2026-11-16：macOS 资源监控与 Linux 能力恢复

基线：本地 `539c172389b4450570026f023ff89d50df90f910`。
状态：`AUTHOR_GATE_PASS / ALL_NON_AUTHOR_SOURCE_SCOPES_APPROVED / LOCAL_FEATURE_COMMITTED / CLEANUP_CONFIRMED`。
本批新卡片的完整原生桌面验收仍为 `NOT_RUN`。
只做本地提交，不 push。设计见 [ADR 0143](../../adr/0143-darwin-resource-monitoring.md)。

## 归属与执行

父 checkout 继续持有唯一 Cargo target
`work/recovery-batch-integration-20261115/target`；没有切换源码归属。
两个受管 worktree 分别编写 Darwin 采集/解析与 Linux 恢复实验，作者不使用 Cargo、
不修改父 target。父负责 UI/模型、集中编译、真实联调和最终集成。
临时原件位于 `work/host-monitor-20261116`，提交前封存到独立持久证据入口。

Rust/Cargo 实测均为 `1.100.0`，默认 `stable-aarch64-apple-darwin`。
项目不 pin toolchain；本片无新增依赖。在线官方版本检查 r1 在 45 秒截止终止，
实际 `exit -15 / timed_out / reaped`，不能签全部依赖最新。

## 已执行

- `runner-unit-r1`：OpenSSH runner 的平台/套件互斥及既有资源边界单元测试，
  实际 `exit 0 / reaped`；这不是实际 SSH 验收。
- 本批曾处于 Rust、Darwin ABI 和 UI 集成阶段；最终复核及门禁按下方实际终态记账。
- 两个作者均已逐项 SHA 导入：Darwin 5 项源码/68 份原件，恢复实验 6 项源码/
  7 份原件。两个作者已关闭并提交受管归档；最终路径/注册回读另留证。
- session `monitor::` r1 为 `76/1`，旧 fixture 只识别旧命令；修后 r2 `79/0`，
  含 44 个 Darwin 用例。真实 `df` 的自动挂载来源含空格，父补有界唯一数值锚点
  解析与歧义拒绝两个回归，不以虚构成功容量绕过实际输出。
- app r1 `69/1` 复现非作者 APP-P2-01：新测试跨越暂停边界错误期待 CPU。
  修正首样未知/次样有值断言后 r3 `70/0`；生产暂停语义保持。
  一次 `--lib` 误选未生成 app binary 的启动失败保留为独立回执，不计测试执行。
- macOS disposable OpenSSH r1 实际 `5/1`；资源、内存、采样间隔、df、重连通过，
  进程测试对首次 Unsupported 错误类型预期错误，后续负缓存才是 typed capability。
  已修正测试，旧失败和 34 条 birth identity 清理证据保留。
- Linux 恢复 r1 实际 `exit 1 / 6.229s`，容器启动权限不足，与非作者 MR-01
  静态发现一致。三个自有资源及进程已清理；不签实际恢复测试通过。
  父已补 CHOWN/DAC_READ_SEARCH 精确 mask 和反例，待实际 r2。
- Darwin 非作者发现 PATH 可选中不受控解释器；已改固定绝对候选与 uname，
  加有界真实 shell 选择回归；作者 Python 30 项提升到默认工程测试集。
- 恢复 Python 首轮父集成 `23/23` 通过；权限 mask 修改后旧正例过时的失败保持，
  已更新精确内核 mask 与缺能力反例。Rust sha2 0.11 LowerHex 编译失败已窄修，
  实际 monitor_interop 重新编译通过，完整库存为两个 ignored。
- 后续两个非作者 P2 也已窄修：Linux 的 uname 固定候选同时支持 `/usr/bin` 与
  `/bin`，两处都缺失是 CommandMissing；Python 默认测试按实际 alarm API 能力
  跳过三个 Unix main-entry 用例，保留可移植解析测试。API 缺失模拟由非作者
  实际执行 `16 passed / 14 expected skipped / 0 errors`，不是 Windows 原生。
- 最终 session r4 `81/0`，含 45 Darwin 和两个平台模型/选择回归。
  实际 Mac SSH r3 `6/0`、33 条 birth identity 已停止、私有目录已删除；
  app `70/0` 保持。正常 arm64 GUI/MCP `normal-build-r1` 实际 exit 0，
  Mach-O 摘要单独保存；没有新桌面/键盘/IME/辅助技术验收。
- Linux r2 正确失败于现有 base 镜像无真实 ss，不生成替代成功。
  另建自有工具镜像，只在该镜像内安装 Debian iproute2，主实验继续离线。
  其工作 helper 首轮遗漏 `steps`，创建日志成功但记录/清理失败，原 UNKNOWN
  和 `cleanup=false` 保留；独立恢复通过原 Popen 子进程实际 reap/exit 0 确认
  创建终态，再核对精确 owner/full-ID/tag，无重放创建请求。
- Linux r3 实际 `PASSED / 14.692s`：两个 ignored 用例均实际执行通过，
  `df/ps/ss × missing/unsupported/permission` 九组降级/负缓存/修复后继续缓存/
  新连接隔离/显式 reset 恢复，以及 transport 分类通过。三个主实验资源、
  客户端及私有目录已清理；工具镜像在独立恢复回执中也已确认删除。
- 正常程序与前三份非作者批准来源为 713 项 r1 工程输入，集合 SHA-256
  `66936ab5115a3abe55ef6e9bd8e460f8bf4b9462252d249a028f35d10c9b5740`。
  三个非作者范围均批准、开放 P0–P3 为 0，六项 P2 全部关闭。
  Rust 使用父预构建独立执行，没有独立重编译；Linux reviewer 审计父实际运行，
  没有自行运行 Podman。原签署时门禁 PENDING 的报告不改写。
- `full-gate-r1` 实际 `exit 1 / reaped / 34.211s`：Python `284 run / 1 skipped`
  与格式通过，Clippy 拒绝两个新增测试模块的 15 处 unwrap/expect 断言；
  workspace 测试和三组 controller 未运行，不补造成功。
- 父只为两个 cfg(test) 文件增加带理由的局部 `expect` 属性；没有放宽断言、
  修改生产路径或全局 lint。完整 r2 仍 713 项，集合 SHA-256
  `8189595b7869f5bf6aadeeccfbe641529237b11267d167209ffc9272fa31f695`。
  移除新属性能逐 byte 恢复 r1 两文件；其余 711 项保持。
  新非作者窄审已批准；正常程序与真实联调继续绑定生产输入未变，
  原有预构建不代签修后完整门禁。
- `full-gate-r2` 实际 `exit 1 / reaped / 487.225s`：Python `284 run / 1 skipped`、
  格式、严格 Clippy 及 app `1276/0/2 ignored` 等通过，旧 disk monitor 集成测试
  以全文子串搜索把固定 Darwin Python 的 `child.kill()` 当作目标进程操作，
  该 target 为 `1 passed / 1 failed / 1 ignored`；后续 controller 未执行。
- 只修改该集成测试的采样脚本断言：排除精确 `include_str` 的固定 Darwin 正文，
  prefix/suffix 的全部 shell 字节继续禁止 kill/sudo/install；磁盘数值、命令一致、
  重启及未知值断言保持。未改变生产代码。未格式化中间 r3 专项 `2/0/1 ignored`
  通过，格式检查要求折行并实际退出 1，原中间冻结保持；最终 r4 格式通过、
  disk target `2/0/1 ignored`。其余 712 项相对 r2 不变。
  中间 r4 的 713 项集合 SHA-256 为
  `e128c4ed36afa923bc0c3e3ff326ad0b7af1cfc43792f7c4dce9def75c57df97`；
  后续 package 专项另列，不移签 r4。
- `session-regression-r4` 实际 `exit 101 / reaped / 46.702s`，发现第二处旧
  fixture 把完整跨平台命令直接等同旧 Linux 正文，target `3 passed / 1 failed`。
  只将该断言改为提取 Linux 分支与原正文逐 byte 相等；250ms 截止、
  200–700ms elapsed、单次 exec、server join 和 listener 关闭断言保持。
  最终该 target `4/0/0` 通过；r5 格式也通过。
- `session-regression-r5 --no-fail-fast` 完整收集 session target，实际
  `exit 101 / reaped / 192.371s`，唯一失败为 remote-only 架构 guard。
  新 Darwin 外部测试文件缺少该 guard 明确要求的首行 inner `#![cfg(test)]`；
  父模块原有 cfg 不被 guard 推断。只补该文件首行，不修改 guard 或测试体。
  r6 格式、remote-only `5/0/0`、session monitor `81/0/0` 通过；
  旧 r5 全包失败保留，不将专项闭合伪装成第二次全包成功。
- 最终 r6 仍为 713 项，集合 SHA-256
  `e0b6ae274c4fad8d1246df5cba5c21b779b8afa6341385818a99b0a2264effd3`。
  相对已批准 r2 仅三个测试文件变化，其余 710 项不变；相对初始 r1
  共四个测试文件变化，其余 709 项不变。生产、真实 SSH/Linux 实验及正常程序
  输入保持。最后新非作者窄审已批准，开放 P0–P3 为 0；最终门禁结果见下方。

## 最终集中门禁

`full-gate-r3` 实际 `exit 0 / reaped / 680.172s`，未超时。
普通 Rust **3157 passed / 0 failed / 48 ignored**，rustdoc **14/0/0**；
工程 Python **284 run = 283 passed + 1 skipped**，helper Python **25/25**。
格式、严格 workspace all-target Clippy、x.y 依赖政策与 readiness、observer、
small-stack 三组 AI controller 均通过。三组 controller 与 rustdoc 独立记账，
不重复加入普通 Rust 总数。

全部 **52 个新增默认 Rust 名**逐一在最终门禁通过，包含在 3157 中；
713 项完整路径集合、字节数和摘要在门禁后零漂移。完整计数与 target 清单见
`final-gate-summary-r3.json`。五份报告原签署的 PENDING 文本保持，由
`review-resolution-r3.json` 另行绑定同一最终输入及成功终态。
正常非 test GUI/MCP 构建仍为 r1 来源；相对 r1 仅四个测试文件变化，
实际程序摘要已再读回一致，没有声称重新构建字节可复现。

## 非作者范围与计数

| 范围 | 最终报告 | 实际独立检查与限制 |
| --- | --- | --- |
| app/UI、平台模型及父集成 | `review-app/report-r3.md` | 39 次通过、38 个不同用例；Rust 为父预构建独立执行，无独立重编译 |
| Darwin 采集器、解析与 session | `review-darwin/report-final-r1.md` | Rust 64 次通过、57 个不同用例，Python 30/30；无独立 Rust 重编译 |
| Linux 实验与恢复清理 | `review-recovery/source-review-final-r2.md` | 独立 Python 35/35；核对父实际 Linux 运行、原 CLI 完成与清理，没有自行操作 Podman |
| 两个测试模块 lint 增量 | `review-lint/report-r1.md` | 40 项静态/身份检查，行为测试 0；独立重算 713 项，移除新增属性精确恢复 r1 |
| 磁盘断言与首行 test cfg 增量 | `review-disk/report-r1.md` | 39 项静态/身份检查、6,876 项内存模型；父预构建独立运行 3 passed / 1 ignored / 3 filtered，无独立重编译 |

五份报告均限定 APPROVE，各自开放 P0–P3 为 0。六项历史 P2 为
`APP-P2-01/02`、`DARWIN-P2-01/02`、`MR-01/02`，全部关闭；
报告签署时的完整门禁 PENDING、预构建及原生限制保持，后续另加最终门禁绑定。
Darwin 的 alarm API 缺失模拟为 `16 passed / 14 expected skipped / 0 errors`，
不能作为 Windows 原生运行。内存模型不算编译后 Rust 或真实目标测试。
五个 reviewer 均已关闭，两个作者也已关闭。
两个作者受管 worktree 的路径/Git 注册和匹配分支均已确认 absent，
没有创建用户 chat；清理不触及父 chat 或其他任务。

## 交付收尾

- 1035 份原件已无损封存到[持久证据](2026-11-16-host-monitor-evidence/README.md)，
  713 项工程输入与全部 gzip/解压原文已通过 Git index/blob 逐项核对，零 mismatch。
  原文 `13,722,596` 字节、压缩 `5,539,192` 字节；18 个可重建省略件仅保留摘要。
- 本地源码提交为 `c5ad5dca621207644e39aa28f21f7f20a3116cdf`，没有 push。
  [提交绑定](2026-11-16-host-monitor-evidence/commit-input-binding.json)逐项核对精确
  Git blob 与当前文件，713 项输入、1035 份原件全部匹配。
- 完成绑定后回收本批 1053 个临时文件及稳定测试副本，逻辑 `898,623,036` 字节；
  18 个可重建省略件包含在删除数中，原失败、UNKNOWN 和来源摘要全部留在 Git。
  [清理回执](2026-11-16-host-monitor-evidence/cleanup-final.json)确认本批目录 absent，
  正常程序摘要、唯一有效 target、共享 base 与其他任务资源保留。
- [清理后绑定](2026-11-16-host-monitor-evidence/post-cleanup-commit-binding.json)
  再次核对同一源码提交、完整路径集合及全部原件，零 mismatch。
  两个作者、五个 reviewer、自有受管 worktree 与匹配分支均已清理，没有新建用户 chat。
  后续仅补文档回执，不重复输入未变的 Rust 门禁。

三平台完整桌面、IME/辅助技术、签名发行及完整产品目标继续开放。
