# 重启后传输恢复日志测试记录 — 2026-11-14

日期：2026-11-14 至 2026-11-15（Asia/Shanghai）。当前源码与测试实现提交：
`d180ece6fb4c91517532938f3ab2d9dfa30ede64`。公司内网不可用，本片只做本地
commit，不执行 push。

## 结论

作者完整工程门禁和 workspace doctest 已在当前修复源码提交和独立 target 上实际
退出 `0`。修复前独立严格只读代码与功能复核固定在文档提交基线
`83db250d175845d084926c0742d2ab20620edc2f` 并返回 `REQUEST_CHANGES`，
P0/P1/P2/P3 为 `0/0/2/0`；修复后的本片状态为
`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_PENDING`。发现项见
[修复前复核记录](2026-11-15-transfer-recovery-review-before-fix.md)。

本记录区分源码测试、GPUI headless 行为和真实桌面/远端验收。它没有把本地
loopback、受控 GPUI 或完整工程门禁写成 macOS/Windows/Linux 原生桌面、真实
客户 SSH、Podman 长流程或发布验收。

## 实现范围

- core 新增独立 `TransferRecoveryRecord` 和 `TransferRecoveryStore`，使用
  schema version `1`、32 条记录上限、256 KiB 文档上限和单路径 4096 字节
  上限；
- 记录保存方向、路径、文件/目录类型、进度、审核已有字节、总字节和
  `Unfinished`、`OutcomeUnknown`、`Failed`、`Cancelled` 终态；
- 保存连接来源时写入 `connection_id`，快速连接和历史旧格式记录保持无绑定
  兼容，但恢复界面要求当前 SSH 主机确认；
- 存储使用 owner-only 文件、同目录锁、临时文件、`sync_all`、原子替换和
  Unix 父目录同步，拒绝符号链接、非普通文件、越界路径、重复 ID、未知
  schema 和超限文档；
- 应用在真实传输开始前持久化 `Unfinished`，在进度、失败、取消、未知结果、
  队列初始化失败和 SFTP 入队失败时更新日志；
- 每条恢复记录在审核与 worker 生命周期内使用独立的 OS 文件锁 claim，窗口和
  进程之间不能同时续传、覆盖或删除同一条记录；带 claim 的更新和删除会校验
  记录 ID、日志路径和 claim 所有权；
- 首次持久化 `Unfinished` 记录时先取得同一条记录的 OS claim，再公开记录；
  传输 worker、恢复审核和新窗口之间不会出现“记录已可见但尚未被占用”的
  间隙；
- 持久恢复的只读计划把 claim 交给后台 worker，计划完成后将 claim 返回审核
  层；取消审核、确认前置失败、worker 失败和终态路径都会释放它；
- 恢复路径必须经过当前会话的 `Inspect → Review → Confirm`，绑定不匹配在
  任何远端只读检查前拒绝；
- 没有保存 `connection_id` 的旧格式或快速连接记录，必须先确认当前 SSH 主机，
  然后才会生成只读远端检查计划；确认前不会访问远端；
- 只读计划重新计算的已有字节数会回写审核进度、总字节数和安全范围内的已传输
  字节数，避免界面审核值与持久记录分离；
- 成功的显式恢复和用户明确 `Discard` 才会删除记录；`Discard` 不建立连接、
  不发起远端检查、不写入远端；
- 队列初始化失败、取消、worker 启动失败、挂起、导航撤销和旧审核替换都会
  释放 claim；worker 只有在持有 claim 时才能写入恢复终态或删除成功记录；
- 所有恢复日志文件系统读写在后台 worker 中执行，GPUI 线程只处理事件和
  状态展示。

## 定向验证

定向 Cargo 验证使用 Rust/Cargo `1.100.0`、`--locked --offline`，并使用当前
checkout 的独立 target：
`work/transfer-recovery-fix-v3-20261115/target`。

| 验证 | 结果 |
| --- | --- |
| core transfer-recovery focused | `8 passed / 0 failed` |
| app persisted-recovery focused 回归 | `8 passed / 0 failed` |
| app recovery focused 回归 | `39 passed / 0 failed` |
| app parallel focused 回归 | `18 passed / 0 failed` |
| core strict Clippy | exit `0` |
| app strict Clippy | exit `0` |
| `cargo fmt --all -- --check` | exit `0` |
| `git diff --check` | exit `0` |
| `cargo check -p keelshell-app --locked --offline` | exit `0` |

定向回归覆盖：

- 恢复文档终态幂等性和未知结果保留；
- 旧格式无绑定记录的当前 SSH 主机确认；
- 三主题、中英文恢复卡片可达性；
- 显式丢弃恢复记录；
- 保存连接与恢复记录的绑定校验；
- 恢复日志不会携带运行时执行权。
- 旧 recovery load generation 不能覆盖较新的 session/generation；
- 真实持久 store queued upload 成功后删除记录；
- 真实持久 store 队列初始化失败保留 Failed；
- 真实持久 store 取消保留 Cancelled/OutcomeUnknown；
- 两个持久 queued worker 同时终态后恢复列表保持最新且为空。

## 完整工程门禁

命令：

```text
CARGO_TARGET_DIR=work/transfer-recovery-fix-v3-20261115/full-target \
python3 scripts/check.py
```

日志：
`work/transfer-recovery-fix-v3-20261115/full-gate-current.log`

workspace doctest 日志：
`work/transfer-recovery-fix-v3-20261115/doctest-current.log`

实际结果：

| 范围 | 通过 | 失败 | 忽略 |
| --- | ---: | ---: | ---: |
| Python scripts | 131 | 0 | 1 |
| keelshell-app | 1172 | 0 | 2 |
| keelshell-core | 644 | 0 | 0 |
| keelshell-ai | 194 | 0 | 5 |
| keelshell-session | 448 | 0 | 16 |
| keelshell-mcp | 77 | 0 | 0 |
| 普通 Rust 合计 | 2535 | 0 | 23 |
| workspace doctest 合计 | 14 | 0 | 0 |

完整门禁使用 `--locked` 并从当前本地 Cargo 缓存完成并退出 `0`；依赖版本策略、
`cargo fmt --all --check`、workspace 严格 Clippy、
readiness controller、observer controller 和 small-stack controller 均通过。
app 链接阶段出现已知 `__eh_frame section too large` warning，但命令实际
退出码为 `0`。doctest 使用相同的独立 target，14 项全部通过。

## 工具链与依赖边界

本机读回为：

```text
rustc 1.100.0 (0ef1bc386 2026-11-09)
cargo 1.100.0 (3acc79c0c 2026-11-05)
```

项目没有 `rust-toolchain.toml`、`.cargo/config*` 或 `rust-version`。这表示
Cargo 默认使用开发机或 CI runner 的 active toolchain；`Cargo.toml` 的
`rust-version` 是最低支持版本声明，不是“选择最新 Rust”的开关，所以本片
没有把本机 `1.100` 写成项目 MSRV。直接依赖版本策略仍要求 `x.y`，锁文件
保留解析出的 patch 版本。

在线 registry 最新版本检查尚未完成，因为公司内网仍不可用；本次门禁使用
现有 `Cargo.lock` 和离线缓存完成。

## 修复内容

修复提交 `d180ece6fb4c91517532938f3ab2d9dfa30ede64` 增加
`recovery_load_generation`。每次恢复记录异步加载都绑定当前 SSH session 和
单调 generation，旧的 `store.load()` 回调不能覆盖较新的加载、丢弃、终态刷新
或计划进度。

同一提交还补充了真实 `FilesPanel + TransferRecoveryStore + queued worker`
回归：成功上传后恢复记录删除，队列初始化失败保留 `Failed`，取消保留
`Cancelled` 或 `OutcomeUnknown`，两个持久 worker 同时完成后界面和磁盘恢复列表
都保持为空。这样把修复前 reviewer 指出的持久 store 与真实队列接线缺口纳入
可重复验证。

## 修复前独立严格只读复核

reviewer 使用独立 detached worktree 和独立 Cargo target，复核了源码提交
`3a61562f2cc5031b6b6e5d4d69a452e283ea4a73` 与文档提交基线
`83db250d175845d084926c0742d2ab20620edc2f` 的关系。实际 focused 结果为：

- core `transfer_recovery`：`8 passed / 0 failed / 0 ignored`；
- app `persisted_recovery`：`8 passed / 0 failed / 0 ignored`；
- app `recovery`：`37 passed / 0 failed / 0 ignored`；
- app `parallel`：`14 passed / 0 failed / 0 ignored`；
- `cargo fmt --all -- --check`、core/app 严格 Clippy：全部 exit `0`；
- P0/P1/P2/P3：`0/0/2/0`，merge recommendation：`REQUEST_CHANGES`。

reviewer 没有修改、提交、推送或创建项目文档；复核完成后 reviewer 会话、
worktree、独立 target 和临时文件已清理。app 测试链接阶段的
`__eh_frame section too large` warning 没有改变退出码。

## 未验证边界

以下范围仍保持未完成或需要独立证据：

- macOS、Windows、Linux 原生 GPUI 窗口与完整主题/语言/尺寸矩阵；
- 真实客户 SSH/SFTP、跨进程恢复和重启后的真实远端内容；
- Podman 长流程联调；
- IME、VoiceOver、NVDA 等完整输入与辅助技术验收；
- 签名、公证、安装升级、回退和发布验收；
- 在线依赖新版本检查。
