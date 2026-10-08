# 开发与测试 SHA2 优化验证 — 2026-10-08

## 根工程实际验证

根以已推送 `f3c88209ae314f4eb3ae5268bcdf8fd1f7b8b8e7` 为基线精确导入三条候选路径，冻结 830 输入／16,279,685 字节。新增 124 字节仅改变 SHA2 0.11.* 的开发／继承测试优化；所有 Rust 源码、原断言、目录控制、请求与清理期限、四线程、lock、工具链及 release 保持。候选静态非作者复核无确认 P1/P2；本次根工程证据与最后状态文档已通过新的最终非作者独立复核，无确认 P1/P2；精确新提交 CI 仍待验。

实际 `cargo test --workspace --locked --no-run -vv` 返回 0，原 owner 89590 已 wait／reap、组不存在，耗时 261.402 秒。944,963 字节完整 raw 的 SHA256 为 `bd16a3b6006ab2ee90bf2404fc89ca9ebdc8052d4f462752595195f5e4dde6fc`。第 30、637 行实际 rustc 命令均为 SHA2 0.11.0、`-C opt-level=3`、`-C debug-assertions=on`；Cargo description 含换行，不能要求 Running 标题与命令在同一物理行。0.10.9 在 lock 的静态父节点只有 oo7 0.6.0，本机 macOS 图未编译到它，**未观察实际 0.10 flags，Linux 待验**。不能将静态 selector 排除当作新平台编译证明。

原 `python3 scripts/check.py` 全门禁实际 0，owner 231 已 wait／reap、组不存在，耗时 1,001.194 秒。完整 raw 458,989 字节／SHA256 `e57edc843f9284c71e87505ebf1eaadafae042d74b8d465abe2c06ceb385b3ae`，根全文读回并分类：

| 根实际检查 | 结果 |
| --- | --- |
| x.y、格式、全 workspace/all-targets 严格 Clippy | PASS |
| workspace 原四线程 | 1,812 普通 Rust、10 rustdoc PASS；22 ignored 未运行 |
| 脚本回归 | 48 PASS，无过滤／skip |
| 原默认／2 MiB 本地智能体控制 | 各 626 连续阶段完成，原断言保持 |
| 准备失败收尾／目录观察 IO | 十项准备控制及两个观察 IO 控制 PASS，原目录控制完成 |
| 本机进程与输入收尾 | 两个 owner 实际 wait0／reap／group absent，外层 session20459 已消费 exit0；私有 TMP 为空并删除；全部 830 输入前后相同 |
| 实际 0.10.9 flags／精确新提交 CI | 未观察／待运行 |
| 新原生 GUI、完整 CLI/MCP、产品发布安装 | 本切片未执行，不继承为通过 |

六组实际 POSIX 准备树均按原身份绑定后返回 ABSENT，parent0／7、SIGINT130、timeout124、drain/reap/active0 原条件保持；API 模型不是 Windows 结果。Cargo 既有 block0.1.6 future-incompatibility 提示完整保留，严格 Clippy 本次实际 0。

最新基线 [Quality 37721737467](https://github.com/cyruss648/keelshell/actions/runs/37721737467) 已结束：**Linux 成功，macOS／Windows 失败**。这是仍未增加本优化的 f3 源码；Linux 通过不能归因于本切片或倒填先前两次 Timeout 原因。Linux 完整 raw 640,572 字节／`abf67f482fc6a9018027f4dc00beaaaab3a479fc4177f39ace503fc9fb8ef489`，原门禁及 OpenSSH interoperability 通过；macOS 完整 raw 505,405 字节／`acbbcf96d8e70808de0484b3dd4fb806ebbebcb559679c4db39d88a52bef0d51`，workspace 已通过，但 small-stack Codex ProgressAsk 取消后端口连接断言失败，具体监听者身份仍 UNKNOWN。Windows 则有原 retained HANDLE LIVE 的真实控制器缺陷，见[诊断记录](2026-10-08-windows-native-crash-diagnostics.md)。三个原日志已完整读回，CLI／模型／客户场景、最终新提交三平台 CI 与原生产品验收保持各自边界。

旧 Unix cleanup v1 因新 await 后重入／取消／Drop 可能对复用 PGID 重发信号的 P2 被独立静态复核拒绝，未导入；新的 Unix 与 Windows 控制器候选在各自忽略目录准备，不混入本切片。该风险来自完整源码可达路径，没有本次实际复用或误杀证据。完整对外 MCP 与应用内 AI 仍是两个独立入口；MCP 由 KeelShell 供外部智能体调用。

## 作者准备范围（历史）

候选以 `437f89609f05e18f4ce3d695b0df53ee0bc763d5` 为精确基线。唯一工程改动是根 `Cargo.toml` 的 `[profile.dev.package."sha2@0.11"] opt-level = 3`，另新增 [ADR 0084](../../adr/0084-development-sha2-build-optimization.md) 与本记录。root 文件尚未导入；作者不运行 Cargo、不写 root/shared target，也不提交、推送或操作 GUI。

静态核对确认 Package ID selector 限定 SHA2 0.11.*；直接 x.y 依赖、锁定 SHA2 0.11.0 与另一个 0.10.9、工具链、release、debug assertions／overflow 设置和 Argon2 override 不改。原生产代码与测试逐字节保持，完整前后 hash 在 ignored 作者封包中。产品请求超时仍由用户配置，8 秒仅指本记录目录回归夹具的原 Ask 期限。不能把 TOML 解析当成根工程实际编译通过。

## 已有独立成本证据

ignored `work/linux-directory-timeout-source-review-20261008-v1/` 保留根已完整读回的自有微型对照。两份源文件和最终 lock 相同，十个实际 registry 包的版本／checksum 与应用 lock 一致；仅 SHA2 依赖开发优化不同，Rust 1.98.1，实际 macOS/aarch64，所需 SHA2 指令检测为 true，未强制软件后端。

| 单组完整 50 MiB 操作 | 默认 dev µs | 仅 SHA2 优化 µs |
| --- | ---: | ---: |
| 原件读取 | 187,469 | 42,686 |
| 原件读取并复制 | 156,385 | 40,758 |
| 副本读取 | 153,776 | 19,103 |
| 原件再次读取 | 160,284 | 17,840 |
| 副本再次读取 | 161,562 | 17,684 |
| 程序内总 wall | 819,710 | 138,349 |

两侧各完整哈希 262,144,000 B、复制 52,428,800 B。十个摘要均与独立 Python reference 相等；raw 与参考保留。编译日志实际显示 SHA2 0.11.0 由默认开发优化变为 opt-level=3，并保留 debug assertions。实际库 dispatch 未直接插桩；这只是本机单组测量，有顺序／缓存边界，不等于目录回归原 8 秒 Ask、Linux native 或完整产品接受。

两次编译与两次运行均实际 wait=0／reap／group absent，TMP 删除，输入不变；各自绝对 60 秒实验期限未触发。首次 prepared lock 被 `--locked` 拒绝的 actual 101 原日志保存；随后仅实验 lock 离线生成并核对应用版本／checksum，不改应用 lock。实验独立 30 秒内部边界不改变目录回归夹具原 8 秒 Ask 期限或产品的用户配置超时。

## 精确历史失败与待执行门禁

[Quality 37716374658](https://github.com/cyruss648/keelshell/actions/runs/37716374658) 的 `561eb9f` 与 [Quality 37717370699](https://github.com/cyruss648/keelshell/actions/runs/37717370699) 的 `437f896` 在相同目录源和 51,262,112 B 夹具上，均只有 version／help／features 后于 8 秒 Timeout，没有 Ask 启动，scratch 为空。完整 Linux raw 分别为 92,524 B／SHA256 `71e948a4d77d50923faf53ff2b9bdc53b93884259d8ac02db61433a8bbdb54c6` 和 95,057 B／`e851e46eb78f275d696c4df20fea5804c67241a25655bd85eaf2a4a3fff4534b`，仍保留失败；准确慢操作与 runner backend UNKNOWN，不因本机成本改善改写为已修复。后者另有 macOS ProgressAsk 连接断言失败，其具体端口／进程身份由独立调查处理。

| 本候选根验证 | 当前状态 |
| --- | --- |
| 实际根 test-profile 编译 flags：0.11 优化／0.10 未改变 | UNRUN |
| 格式、x.y、Clippy、完整 workspace 原四线程门禁 | UNRUN |
| Unix 20／Windows 15 目录控制与目录回归原 8 秒 Ask 期限 | 源码保持；本候选执行 UNRUN |
| 默认与 2 MiB 各 626 阶段、准备收尾、观察 IO 控制 | 源码及入口保持；本候选执行 UNRUN |
| 原可执行文件／目录／schema 篡改拒绝、取消与清理 | 源码保持；本候选执行 UNRUN |
| 精确新提交三平台 CI | UNRUN |
| 原生完整产品验收 | UNRUN；本切片不继承为接受 |

作者只封存静态范围与既有实验引用；根须将新实际结果与输入绑定补入本记录，再进行最终独立复核和提交。
