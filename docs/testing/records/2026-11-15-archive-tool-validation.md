# 2026-11-15 服务端工具版本与流式自测

状态：`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_APPROVED / NATIVE_DESKTOP_NOT_RUN`。

基线为本地 `6e2b0a7ee3493ace52ba157049ab668ba464c4f0`。后端和辅助程序在
受管 worktree 编写，父负责原生界面、控制器回归、真实 OpenSSH 接入和集成。
当前唯一有效 Cargo target 为 `work/recovery-batch-integration-20261115/target`，
归属没有切换，没有并发使用 Cargo。作者 worktree 不运行 Cargo；父在主工作树
统一集成与验证。最终冻结前的命令只证明各自输入的范围。
本地功能提交为 `df363691fa4ee238bdd1db826f83c916c7d18194`；
[精确提交绑定](2026-11-15-archive-tool-validation-evidence/commit-input-binding.json)
核对其 Git blob、607 项最终输入、156 份原件与当前文件，零 mismatch。

设计见 [ADR 0139](../../adr/0139-reviewed-archive-tool-validation.md)，操作和
限制见[工具指南](../../product/ARCHIVE_TOOL_INSPECTION.md)。七项版本观察与
tar/gzip/Python 固定内存流式检查分开呈现；其它四项功能保持未测试。

## 最终工程输入和集中门禁

最终 r2 共 607 项 Rust/Cargo/Python 工程输入，完整路径集合和内容在门禁前后
零漂移，集合 SHA-256：

```text
523506564206cbeca415ae2bc9a82ed670cdb210705ba49f3173a84250bd4c17
```

冻结清单自身 SHA-256 为
`12e7e0bf2032c3cb9534ccd685cd4013521d982c2bf5b5494579fc9574ce8280`。
冻结、命令、日志、完整库存、复核报告及保留失败从
[持久证据入口](2026-11-15-archive-tool-validation-evidence/README.md)接续；
原运行目录是历史位置，不要求继续保留临时副本。

最终 `python3 -B scripts/check.py` 实际 wait/reap，退出 0，耗时 674.299 秒；
结束于 2026-11-15 15:03:52 UTC（Asia/Shanghai 23:03:52）。
格式、严格 workspace all-target Clippy、`x.y` 依赖策略、Python discovery、
workspace Rust/rustdoc 及三组 AI controller 全部通过。

| 范围 | 实际结果 |
| --- | --- |
| workspace 普通 Rust | 2987 passed / 0 failed / 36 ignored，含 app 1244 / 0 / 2 |
| rustdoc | 14 passed / 0 failed |
| 工程 Python discovery | 187 run，186 passed / 1 skipped |
| helper Python | 25/25；默认 Unix Rust 桥接也实际运行同一 suite，不重复累计 |
| 新增默认 Rust | 29 项具体名称全部存在且在最终门禁实际通过，已包含在普通 Rust 总数 |
| 作者与非作者的 Rust 专项 | 后端 11 unit + 11 加密 SSH，app 7/7；与全仓重叠，不另加总 |
| 真实 disposable localhost OpenSSH r2 | 16/16，内部 runner 8.882 秒，外层 exit 0 / 8.937 秒 / reaped |
| 正常 GUI/MCP r2 构建 | exit 0 / 9.654 秒；均 arm64 Mach-O，摘要单独保存，不等于原生窗口验收 |
| Linux helper/Python r2 | 实际 helper exit 0；tar/gzip/Python 的版本和流式样本通过，Python 25/25 |
| 非作者源码与必要专项 | 两个范围均 APPROVE，P0/P1/P2/P3 开放均为 0，关闭两个 P2 |
| macOS/Windows/Linux 原生桌面 | 本批 0 / 0 / 0，NOT_RUN |

Linux 使用已缓存的精确 image
`77353e13649fe86ce434ffb958fcdf43317abe4ffdc0916fa7225edd407bb082`，
只读 root、禁用自动 tmpfs、init、UID/GID 65534、network none、cap-drop ALL、
no-new-privileges。r3 补充只读核对该 image 的 `linux/arm64` 与当前 Podman VM
的 `Linux aarch64`；不把描述字段当架构证明，也没有新增 Rust 或桌面运行。
zip/unzip/rz/sz 在此 image 未找到；四项功能仍显式 NotTested。
OpenSSH 测试验证完整 helper、当前连接后续可用及远端目录未改变；
83 个 kernel-birth process identities 的停止和临时目录清理均有回执。

## 独立复核与修复

后端与 app 各由新非作者严格只读复核。两份 r2 原报告签署时最终门禁仍为
PENDING；父随后取得相同 607 输入的成功终态及零漂移，满足其工程条件。
不改写原报告的签署时状态。复核实际独立执行必要风险测试，Rust 使用已核对
摘要、来源、完整库存与 helper 字节的父预构建副本，**没有独立重编译**。
后端 unit 库存为 205，SSH 为 11，app 为 1246 个描述符；库存不是执行数量。

1. 正常成功的工具可能留下关闭 stdio 的同组后台后代。原独立探针先红后绿。
   改用仍存活的 supervisor 保持组所有权，独立有界状态 pipe 报告真实 tool exit，
   正常和异常均先清组再 reap；reap 后只作有界 signal 0 观察。清理不能确认不出具
   报告。生产真实回归与 reviewer 原探针均通过；旧 macOS Python 3.9.6 也通过。
2. 本地 POSIX suite 原本无条件进入 Windows 默认 gate。Unix Rust 桥接和 Python
   类级 POSIX 条件已补齐；其余协议/SSH 测试保持跨平台。源码差分与 Unix 执行已核，
   Windows 原生执行仍为 0。

app 独立执行 7/7，覆盖完整命令、一次确认、取消、替换连接、迟到回复、失败刷新、
PATH/版本授权分离和三主题明暗双语。最终命令 14,892 字节、394 行，900×580
和 1440×900 固定确认/取消均经 headless GPUI 断言；不代签桌面视觉或辅助技术。
后端独立执行 Python 25/25、风险 11/11、原探针 1/1、Rust unit 11/11 和
SSH 11/11；桥接重复 Python 不再累计。原命令和各次退出见持久报告。

## 保留的失败与限制

- 早期 app check r1 exit 101：缺少 `Language` 导入；修复后 r2 exit 0 / 3.128 秒。
- 旧完整门禁 r1 因独立发现实际停止/reap，exit -15 / 499.942 秒。
  首次精确 argv 查找未匹配、未发信号；核对真实 Python 路径后停止正确自有组。
- 作者 audit hook 曾误把 FIFO 当文件写入，修正 fixture 后通过；旧原件保留。
- reviewer r2 信号 fixture 因 supervisor 引入仍向 `getppid()` 发信号，11 项中
  1 项失败；只改 reviewer fixture 为明确 helper PID 后 11/11。生产源码未因此改动。
- app 审计脚本的过宽「文档不得变化」断言退出 1；607 项工程输入无漂移，
  实际为预期的 ADR supervisor 说明变化，补充差分判断保留。
- 在线最新依赖检查 60 秒超时，TERM 后实际 wait/reap，exit -15，无版本报告；
  本批未新增依赖，不能声称全部依赖已核实最新。

本机工具链实测 Rust/Cargo 1.100.0，默认 stable。项目无工具链固定或新增 MSRV；
使用环境提供的受支持工具链；`rust-version` 不是选择编译器的机制。
既有 Windows 交叉验证与 Linux 本地后端结果不代签本批样本或桌面。恶意程序、
逃逸进程组、远端取消后实际终止、任意归档/工具真实性不由固定样本认证。
本批原生三平台、IME/辅助技术、完整并行传输/未保存草稿动态组合、
direct PTY strict OO `HOLD/PARTIAL`、用户中继、性能收益、FILE-05/M4 和完整产品
仍开放。只本地提交，不 push；证据 Git blob 绑定和自有资源清理见持久证据入口。
