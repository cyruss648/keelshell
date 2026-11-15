# 传输恢复批量整理集中验证 — 2026-11-15

## 范围与状态

本批接续依赖基线 `59c59a8` 和集中验证流程提交 `858e7f1`，
集成恢复记录多选、有序待审核队列、逐条独立审核及显式批量丢弃。
开发中的受影响包编译与行为专项由实现 agent 执行，整仓门禁、真实协议
联调与新的独立代码/功能复核由主线程统一组织。

当前状态：`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_APPROVED`。
固定 `5cdbc96` 的独立复核发现的 worker 创建失败回队 P2 已修复，新增真实
GPUI 一条/两条故障回归先红后绿。最终完整门禁和新的独立 delta 复核均通过，
普通 Rust `2533/0/23 ignored`、rustdoc `14/0`，独立队列专项 `29/29`，
P0–P3 均为 `0`。下文保留原有门禁、真实本机 OpenSSH 和限定 macOS 窗口
短验的修复前源码归属。本地修复前集成提交为
`5cdbc9617164d6cfbd5e7a9a022539934221ee75`，保留实现提交为其第二父节点。
提交后的 524 个 Rust/Cargo 文件已与通过门禁的冻结输入逐项核对一致。
最终修复源码提交为 `e94f31e49897ac981f4c3d20d22bc39f0cfba3bb`，对应
`fix/commit-input-binding.json` 核对的 524 个 Rust/Cargo 和 10 个 gate 脚本
全部一致，同时绑定作者最终 gate、非作者 delta 复核和正常程序构建。
公司内网不可用，当前仅本地提交，不推送。

## 验证计划

- 核对编译清单中本批具体测试名和来源，不能继承历史测试计数。
- 统一运行依赖新鲜度检查、fmt、workspace 严格 Clippy、单元和集成测试、
  AI controller 专项与 rustdoc。
- 聚焦审核队列次序、主机/连接绑定、claim、取消、旧 session 与旧回调、
  记录消失、批量丢弃部分失败和三主题双语紧凑布局。
- 对本批最终源码执行 Podman/OpenSSH 的真实 SFTP 检查、暂停、取消、
  新会话续传、目录续传与内容不匹配拒绝。
- 新的非作者 agent 固定最终源码 SHA，只读检查风险与有意义的行为回归，
  明确记录发现级别、建议和未覆盖边界。

## 资源与证据

本批主线程日志位于 `work/recovery-batch-integration-20261115/logs/`。
关键回执、524 项输入摘要、逐 suite 计数、正常程序/窗口和协议证据已归档到
[持久证据目录](2026-11-15-transfer-recovery-batch-evidence/)。
`full-gate.log.gz` 与原始门禁日志解压后逐字节一致，文件校验摘要在
`sha256.json`，不会因后续清理可重建 target 而丢失测试原件。
共享第三方缓存可以保留，但重新验证时必须确认项目包产物来自当前源码；
切换 checkout 的 target 先按
[共享 target 指南](../SHARED_TARGET.md)使项目产物失效。

自有临时容器、网络、镜像、密钥、target、已结束 agent 和完成的工作树
在证据归档后清理；其它任务拥有的 WIP 和共享基础镜像保留。

## 独立复核发现与 worker 启动失败修复

第一次独立复核固定 `5cdbc9617164d6cfbd5e7a9a022539934221ee75`，
结果为 `REQUEST_CHANGES`，P0/P1/P2/P3 为 `0/0/1/0`。
[原始报告](2026-11-15-transfer-recovery-batch-review-before-fix-raw.md)
与独立日志保存在证据目录的 `review-before-fix/`；收口后的报告已重新复制并
逐字节核对，失败状态诊断 `2 passed / 1 failed` 也保留。

P2 的触发条件是逐条确认恢复记录后，OS worker 创建返回错误。在旧实现中，
`run_queued_transfer` 在调用 worker 前已经清空当前 active 标记，最后一条还
清空 session；错误分支的回队 helper 因此无法恢复当前项。持久日志没有丢失，
但单条队列变空，多条队列丢失当前队首。

修复只在 worker 成功创建后消费 active/session。面板局部、仅测试编译的启动
拒绝开关使完整 GPUI 确认路径确定地进入既有错误分支；不耗尽系统线程，也不
修改共享 worker registry。关闭未启动 closure 会真实释放其捕获的 claim。

作者先执行两个新增真实 GPUI 用例，修复前 `0 passed / 2 failed`，两者都在
队首恢复断言失败。修复后最终专项 `2/2`，完整 `parallel_tests` 为 `29/29`。
它们检查一条/两条记录的有序回队、session 保留、未写入目标、claim 可重新获取，
然后重新进入新的审核并逐条明确确认，实际完成 SFTP 续传和持久记录删除；
等待中的下一条没有被自动 claim 或执行。对应前缀分别为
`fix/worker-start-before-fix`、`fix/worker-start-after-fix-r4` 和
`fix/app-parallel-after-fix`。

中间 `worker-start-after-fix` 的两项失败来自测试读取全局 status，而错误实际
保存在任务结果行；`r2` 读取私有字段和 `r3` 错改既有测试的 context 参数造成
编译失败。最终测试删除不相关的全局 status 断言并恢复既有 context，没有修改
错误显示逻辑、放宽队列次序/claim/传输断言或改写失败日志。

最终候选冻结 524 个 Rust/Cargo 输入及 10 个 gate Python 脚本，清单为
`fix/frozen-inputs.json`，SHA-256
`f8d1acaa5dffdbf27fa0207dbceb03774cc154ffa84adde7f55b9befc98df724`。
作者完整门禁与新的独立 delta 复核并行完成；两者使用不同 target。
复核源码显式为 `5cdbc96` 加三文件未提交候选，524 个输入与冻结逐项匹配；
不把基线 HEAD 当成包含修复。正式提交已在门禁和复核完成后绑定相同字节。
最终 `python3 -B scripts/check.py` 实际退出 `0`，耗时 `608.170s`；524 个输入
和 10 个 gate 脚本结束后逐字节匹配。所有 10 个批量用例在完整门禁中为 `ok`，
app 实际清单为 `1186`。回执为 `fix/full-gate-after-fix.json`，完整日志
SHA-256 为 `d29aa437a3563e1fd8df104014c418e5e35412a7201f3349fa2b818be756fb82`。

| 最终作者检查 | 实际结果 |
| --- | --- |
| Python | 131 run / 1 skipped |
| app 普通 Rust | 1184 passed / 0 failed / 2 ignored |
| core 普通 Rust | 644 passed / 0 failed / 0 ignored |
| AI 普通 Rust | 194 passed / 0 failed / 5 ignored |
| session 普通 Rust | 434 passed / 0 failed / 16 ignored |
| MCP 普通 Rust | 77 passed / 0 failed / 0 ignored |
| 普通 Rust 合计 | 2533 passed / 0 failed / 23 ignored |
| workspace rustdoc | 14 passed / 0 failed |
| `x.y`、fmt、workspace strict Clippy、三组 AI controller | 全部通过 |

Rust 总通过 `2547`，其中 rustdoc `14` 不重复计入普通测试。
逐 suite 汇总为 `fix/validation-summary.json`；首次修复前结果独立保留在旧
`logs/validation-summary.json`，没有覆盖。

新的非作者独立 delta 复核结论为 `APPROVE`，原 P2 关闭，P0/P1/P2/P3
均为 `0`。[独立修复复核原文](2026-11-15-transfer-recovery-batch-review-final-raw.md)
和 `fix/review/` 的原始命令日志、源码完整性、编译归属及回执均归档。
其独立重编译清单为 `1186`，全部 10 个批量用例在清单及实际测试中；
实际传输队列专项为 `29 passed / 0 failed / 0 ignored`，app strict Clippy、
fmt 与 diff 检查均退出 `0`。清单、重复专项和作者结果没有重复累计。
开始/结束 524 个 Rust/Cargo 输入、1139 个 tracked 文件、HEAD、索引和原三文件
未提交状态均不变；不把此快照描述为已经存在的修复提交。
正式提交之后父线程已核对相同冻结字节，`fix/commit-input-binding.json`
记录源码提交、冻结清单与原始报告摘要、两项构建/gate 成功退出码及零 mismatch。
冻结清单中脚本键为绝对路径，Git/复核副本绑定时明确规范化为仓库相对路径；
原冻结原件及其摘要没有改写。

最终源码另执行正常程序构建：

```text
CARGO_TARGET_DIR=work/recovery-batch-integration-20261115/target \
CARGO_BUILD_JOBS=4 cargo build -p keelshell-app -p keelshell-mcp --locked --offline
```

退出 `0`，耗时 `24.691s`。两个输出均为真实 macOS arm64 Mach-O，
GUI SHA-256 `474eb40d66a17eb0037e496083160a10f13b66548b6cf430962f645ba968572b`，
MCP SHA-256 `cf89dbc2b7fb77958d16c225d7d94c536970aca4c3f7675e544696a8a5811c88`。
回执为 `fix/normal-build-after-fix.json` 和 `normal-build-artifacts.json`。
本次没有重复原生启动、打包或安装；旧窗口回执仍只归属修复前源码。

## 修复前集成门禁与真实协议联调

实现提交为 `c2040bbe03ea6f545ee4a9688d1994bd18cbcc7a`，已在维护 checkout
叠加到依赖刷新与集中验证基线。本批 524 个 Rust/Cargo 输入的摘要保存在
`work/recovery-batch-integration-20261115/logs/frozen-inputs.json`。
新 target 只复用第三方缓存，五个 workspace 包产物已先失效；实际重新编译的
app 清单为 1184 项，明确包含本批 8 个 `persisted_recovery_batch_*` 用例。

完整工程命令为：

```text
CARGO_TARGET_DIR=work/recovery-batch-integration-20261115/target \
CARGO_BUILD_JOBS=4 python3 -B scripts/check.py
```

实际退出 `0`，耗时 `697.114s`。所有 524 个冻结输入在门禁结束后逐字节
核对一致，8 个新增用例在本轮完整门禁中实际执行并通过。

| 检查 | 实际结果 |
| --- | --- |
| Python | 131 run / 1 skipped |
| app 普通 Rust | 1182 passed / 0 failed / 2 ignored |
| core 普通 Rust | 644 passed / 0 failed / 0 ignored |
| AI 普通 Rust | 194 passed / 0 failed / 5 ignored |
| session 普通 Rust | 434 passed / 0 failed / 16 ignored |
| MCP 普通 Rust | 77 passed / 0 failed / 0 ignored |
| 普通 Rust 合计 | 2531 passed / 0 failed / 23 ignored |
| workspace rustdoc | 14 passed / 0 failed |
| `x.y` 依赖策略、fmt、workspace 严格 Clippy | 全部通过 |
| readiness、observer、small-stack controller | 全部通过 |

Rust 总通过数为 `2545`，其中 rustdoc `14` 不再重复计入普通测试。
原始日志为 `logs/full-gate.log`，退出回执为 `logs/full-gate.exit.json`，
逐 suite 汇总为 `logs/validation-summary.json`。日志 SHA-256 为
`ded8b4bb3c6fc98dc807b801a8e8abb2df252d9d415cd600e3904080f0589a28`。
已知 app linker `__eh_frame section too large` warning 未改变任何退出码。

本次也复算了依赖刷新基线原始日志：普通测试实际为 `2523`，rustdoc 为 `14`，
合计 `2537`。此前历史文档中的“普通 2537 / session 448”把全部 14 项
rustdoc 归入了 session；正确的 session 普通数为 `434`。原始成功退出事实
保持不变，本批采用修正后的分类，不用旧汇总推导测试通过。

现有隔离 OpenSSH runner 已真实执行：

```text
CARGO_TARGET_DIR=work/full-gate-dependency-refresh-20261115/target \
CARGO_BUILD_JOBS=2 \
python3 -B scripts/openssh_interop.py \
  --output work/recovery-batch-integration-20261115/openssh --timeout 300
```

退出 `0`，runner 耗时 `61.129s`，`11 passed / 0 failed / 0 ignored`。
服务为本机独立 OpenSSH，监听 `127.0.0.1` 的随机端口，使用临时主机/客户端
密钥、固定主机公钥和隔离目录。它未改动用户 SSH 配置。

用例覆盖新连接上传/下载续传及审核后内容变化拒绝、双向目录续传、
已确认暂停与取消、inode 替换拒绝、并行原子替换与硬链接/规范路径、
只读完整内容检查、远程补全、批处理退出/超时未知态、依赖工作流和
连接身份/参数原文字节。

原始证据为 `work/recovery-batch-integration-20261115/openssh/result.json`、
`tests.log` 和主线程 `logs/openssh-runner.exit.json`。
清理回执确认 `owned_processes_stopped=true`、
`temporary_directory_removed=true`、`ancestry_unverified=[]`。
sshd 的 BSM audit 权限诊断未改变任何测试退出码；不能把它解释为完整
系统审计或生产服务验收。

## Podman 与官方依赖网络边界

Podman CLI 为 `6.2.0`。把 VM 启动、检查与清理保存在同一个执行会话后，
引擎实际读回 `6.1.2 / linux / arm64`；此前单独启动调用退出后 VM 不再
可连接的回执仍保留。原有业务容器在开始前为已退出状态，未被修改。

本次 `podman pull docker.io/library/alpine:latest` 因 Docker Hub HTTPS
超时退出 `125`，因此未构建镜像或开始容器协议测试。随机前缀自有 container、
network 和 image 的 `exists` 检查均退出 `1`，临时密钥目录已移除，VM 已停止。
证据为 `logs/podman-receipt.json` 和 `logs/podman-controlled.log`。
本机 OpenSSH 的 11 项通过不代签 Podman 通过。

官方在线依赖复查命令为
`python3 -B scripts/check_dependencies.py --timeout 30 --jobs 4`。
官方请求持续无响应，超过七分钟后仅中断本轮拥有的检查进程，实际退出
`130`；配置代理与直接请求官方 crates.io 的有界 HTTPS 探针均读回 `403`。
本轮不声明已核实最新版本，保留此前依赖升级输入；`x.y` 策略由确定性
工程门禁单独验证。网络观察摘要在 `logs/dependency-network-summary.json`。

## macOS arm64 正常程序与窗口启动短验

当前冻结源码的正常 GUI 与 MCP companion 已使用同一 checkout 的独立于门禁
target 构建，未借用其它 checkout 的包产物：

```text
CARGO_TARGET_DIR=work/full-gate-dependency-refresh-20261115/target \
CARGO_BUILD_JOBS=3 \
cargo build --locked --offline -p keelshell-app -p keelshell-mcp
```

退出 `0`，耗时 `28.798s`；两个二进制 SHA-256 为：

| 程序 | SHA-256 |
| --- | --- |
| GUI | `d0fc2f815b1f831f1225cde0dded6a6f7f35e6ff82e963d26143eca0d7a55563` |
| MCP | `3134c5ac0ff605d74f6499dd860c9a67b3b5b0fbd98ccecda82e04c4f20d810c` |

`packaging/package.py macos` 与 `packaging/inspect_native.py` 均退出 `0`，
使用真实 arm64 Mach-O；包没有安装、签名或覆盖现有应用。构建时源码仍处于
冻结待提交状态，因此包 manifest 的 `build.commit` 为 `null`，不会填入旧 HEAD
冒充最终提交；编译输入摘要与二进制摘要保留，提交后再核对代码字节归属。

通过 LaunchServices 新建独立实例，显式指定新建的隔离数据目录，观察 8 秒。
CoreGraphics 按新实例 PID `78670` 读到一个可见的 KeelShell 窗口：
`1440×933 at (144,126)`，`kCGWindowIsOnscreen=true`。
随后只退出该新实例，`owned_process_stopped=true`。
日志为 `logs/native-window.log`、`native-stage-receipt.json` 和
`native-build.exit.json`。

该短验只确认正常可执行文件、包结构和可见窗口启动，不包含本批恢复控件的
原生交互、真实主题切换、IME、完整辅助技术或其它平台窗口验收。

## 已完成的资源清理

实现 agent 完成后已关闭。其专用 target 由实现 agent 清理；主线程保留并
逐字节核对了 34 份聚焦测试/失败日志，再删除已合入的开发 worktree 与
`feature/persisted-recovery-batch-review-20261115` 分支。实现提交仍是集成
提交的祖先，不依赖临时分支保持可达。

真实 OpenSSH 的进程与临时密钥已按 runner 回执清理。Podman 自有资源核验
不存在，VM 保持 stopped；未改动原有业务容器。限定 macOS 实例已退出，
临时 package、隔离数据、Swift module cache 与两份临时 runner 脚本已删除。
实现及两轮独立 reviewer agent 均已关闭，最后一轮报告返回后即关闭。
报告及原始日志归档后，独占 reviewer target 和旧依赖刷新 target 已删除，
测试临时目录和状态 probe 可执行文件也已清理；当前主树有效 author target
保留供继续开发。原始源码、失败日志、冻结和正常程序摘要不依赖这些可重建
产物保留。

复核 worktree 仍保留。两次受管 `archive_worktree` 均返回
`Another active task still uses this worktree.`；第二次在重新附加主开发树后
仍被活动任务保护拒绝。没有恢复或删除这份检出的三文件候选，没有使用 shell
绕过受管保护，也没有删除任何其它任务或其 WIP。清理回执为 `fix/cleanup.json`，
保留 `PROTECTED_ACTIVE_TASK` 状态，不能将该 worktree 记录为已归档。

## 验收边界

本批工程与受控协议联调不代表三平台完整原生桌面、IME、完整辅助技术、
真实客户 SSH 服务、外部 AI provider、签名、公证、安装升级或发布验收。
这些边界继续保持待验状态。
