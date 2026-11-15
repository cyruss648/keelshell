# 传输恢复批量整理集中验证 — 2026-11-15

## 范围与状态

本批接续依赖基线 `59c59a8` 和集中验证流程提交 `858e7f1`，
集成恢复记录多选、有序待审核队列、逐条独立审核及显式批量丢弃。
开发中的受影响包编译与行为专项由实现 agent 执行，整仓门禁、真实协议
联调与新的独立代码/功能复核由主线程统一组织。

当前状态：`AUTHOR_GATE_PASS / NON_AUTHOR_REVIEW_PENDING`。
集中门禁、真实本机 OpenSSH 和限定 macOS 窗口启动短验已完成；独立复核和
最终资源清理仍待收口。本记录会补齐最终源码 SHA 与复核回执。
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
共享第三方缓存可以保留，但重新验证时必须确认项目包产物来自当前源码；
切换 checkout 的 target 先按
[共享 target 指南](../SHARED_TARGET.md)使项目产物失效。

自有临时容器、网络、镜像、密钥、target、已结束 agent 和完成的工作树
在证据归档后清理；其它任务拥有的 WIP 和共享基础镜像保留。

## 已完成的源码清单与真实协议联调

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

## 验收边界

本批工程与受控协议联调不代表三平台完整原生桌面、IME、完整辅助技术、
真实客户 SSH 服务、外部 AI provider、签名、公证、安装升级或发布验收。
这些边界继续保持待验状态。
