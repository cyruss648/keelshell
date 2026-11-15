# KeelShell app / SSH bridge / 原生审核 UI / Linux fixture 非作者审查 r1

结论：**REQUEST_CHANGES，暂不建议合入**。开放问题 **P0 0 / P1 0 / P2 3 / P3 1**。审查于 2026-11-15 在唯一目录 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell` 进行；基线 HEAD `721bad2157da8d3a0bc11a1eab5ac59b6616aa9c`，通过只读读取 `.git` 指针、HEAD 和 branch ref 核对，未调用 git。

输入为未提交的 r1 冻结，SHA-256 `0fb6bc9d7c1a845583c56c803796c92ccc73b0c590c9f3b29bcd83f292ca63d5`。开始时 570 个工程输入与 12 个脚本全部匹配。结束时 **1 个 session 互通测试发生漂移**，其余 569 个工程输入、12 个脚本匹配；本次审查的 app/bridge/UI/Linux 工具源码全部仍匹配 r1。对应原件为 `freeze-start.json` 和 `freeze-end.json`。本报告不覆盖漂移后的测试输入。

**可证实问题**

1. **[P2] 首个文件 offer 前结束的接收协议漏发 ResumeInput。** 主位置 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/terminal_zmodem.rs:369`，关键条件和交接在 373–400 行。分开的 PTY 事件先送入合法 ZRQINIT，再在 ZFILE 前取消/失败，即可进入此路径。session `zmodem.rs:585–618` 在接收方向建立 lease，但尚不发 UI offer；`ssh_bridge_zmodem.rs:181–189` 已据此设置 blocked 和 safety.active。没有 current 卡片的确定 Cancelled/Failed 在 UI 中直接落空，394 行的 ResumeInput 不执行；bridge 又只在 152–178 行接受精确交接后解除 active。因此普通输入持续被拒绝，正常关闭标签/窗口也持续被阻止，并且没有可取消的当前卡片。应独立跟踪协议 lease，并处理没有 offer 卡片的终态及等待阶段取消。**验证：源码控制流证明，未编译新增复现。**

2. **[P2] 挂起写入期间观察到 shell 结束会丢失末尾输出。** 主位置 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/ssh_bridge.rs:306`，相关分支 306–315 行。远端在普通写入受流控阻塞时发送最后 Data、exit_status 和 close，completion watch 可先被观察；此时 write_and_drain 直接返回 false，pump 退出、shell 清理使尚未交付的队列消失。`SshShell::subscribe_completion` 在 session `ssh.rs:1051–1052` 明确不保证此前输出已消费。稳定副本已有测试 `ssh_bridge::tests::blocked_input_preserves_tail_and_exit_when_write_fails` 报 **bridge lost tail**；bridge 组首次运行 5/6 通过、exit 101，单独重复 **5/5 次失败**。应停止旧写入且保留有界输出 drain owner，交付有序尾部和退出事件，不能重放已发送前缀。证据：`app-ssh-bridge-r1.log`、`app-tail-repeat-results.json`、五份独立重复日志。

3. **[P2] 重复取消的迟到拒绝会复活已结束卡片。** 主位置 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/terminal_zmodem.rs:334`，334–357 行。Cancel 在 Running 阶段仍可点击，421 行只比对 UUID；两次待处理 Cancel(id) 依次得到 Cancelled(id)、ActionRejected(id, Stale)，后者由 session `zmodem.rs:485–487` 产生。UI 先将卡片标为 Ended，再无条件将同 UUID 拒绝恢复成 Upload/Download，造成后端 lease 已结束而 UI 继续封锁输入和关闭。重新选择/取消仍会收到 stale，不能结束已复活卡片。应忽略已结束 transfer 的迟到非终态通知并防止重复待处理取消。**验证：源码控制流证明，未编译新增复现。**

4. **[P3] 下载的已验证文件回执被完成/下一 offer 覆盖。** 主位置 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/terminal_zmodem.rs:278`，278–306 行，覆盖点 379–381 行。FileSaved 只将真实路径、字节数和 SHA-256 写入 current.feedback；Finished 覆盖该字段，下一 DownloadOffered 又替换整个 current。没有保存回执集合，两类事件在同一次 foreground poll 被处理时，已验证回执甚至不会绘制。用户在传输结束后无法核对该回执，批量接收的前序文件记录也丢失。应以有界集合保留每个文件的验证回执，并将瞬时进度与最终状态另存。**验证：源码数据流证明，未编译新增复现。**

**执行证据与计数**

稳定只读副本 `stable-tests/app-r1` 的 SHA-256 为 `f93d0070b572a1208b3c21e43804803d940834070f5afd7705faeb46279d4046`，与父提供值和元数据一致；实际执行 `--list` 与 `app-inventory-r1.txt` **逐字相同，1217 个测试名**。父元数据标明来自本轮 `full-gate-r1` 的 r1 重编译，父日志有本目录 keelshell-app 的实际编译行。**reviewer 未独立重编译；这不是独立完整门禁或原生验收。**

| 有界专项 | 实际结果 | 退出码 |
| --- | --- | --- |
| app-r1 zmodem --test-threads=1 | 9 通过 / 0 失败 | 0 |
| app-r1 ssh_bridge::tests:: --test-threads=1 | 5 通过 / 1 失败 | 101 |
| 失败的 tail 测试 --exact，额外重复 5 次 | 每次 0 通过 / 1 失败 | 每次 101 |
| test_linux_file_interop.py | 10 通过 / 0 失败 | 0 |
| test_openssh_interop.py | 6 通过 / 0 失败 | 0 |

Rust 去重为 **12 项，11 通过 / 1 失败**；9 项 Zmodem 组与 bridge 组重叠 3 项。含重叠及重复的 Rust 实际执行为 20 次，14 次通过、6 次失败，不能写成 20 个独立用例。Python 共 **16/16**。未编写或编译 reviewer 新测试。详见 `focused-execution-r1.json`、`app-prebuilt-verification-r1.json`、专项日志。

**已检查的重点与边界**

- 唯一 PTY write lane 保留同一 write future，普通输入中途遇协议会停止轮询并保留前缀未知性；没有重放路径。协议/通知队列和字节数有界；精确 UUID 交接有现有回归，但开放问题 1/3 破坏 UI 终态路径。
- Finished 等待协议写队列清空和 in-flight 写实际完成，OO 失败转未知；已有专项通过。该专项手动驱动完成回调，不等同独立原生 peer 认证。
- 未知结果有独立集合及 epoch，后续成功不能清除；双次确认还检查 unread epoch、active 与 publication_running。未发现额外可证实的 publication_running 绕过。没有新增后台发布竞态专项。
- 标签、窗口、关闭确认及 reconnect 历史归档保留 safety 守卫；连接 endpoint 独立于 OSC title，相关专项通过。
- 审核正文包含路径/名称/长度/可用摘要/实际目标/限制，正文滚动、动作固定；Palette 与组件 token 跨 Frost、Graphite、Lavender、明暗及双语的现有测试通过，共 12 种组合。Windows 不支持的安全文件后端使选择按钮禁用并显示说明，未作为 Windows 传输验收。
- 审查 OpenSSH 新 zmodem 分支及 Linux fixture 的 PTY 参数、字面量转义、pinned keys、期限、进程 birth identity、容器 full ID + labels 清理及错误回执；没有额外可证实 P0–P3。16 个工具单元测试通过。reviewer 未启动容器、未运行 live SSH fixture、未清理父方资源。

父方 real-pty-r5 的现存回执核对为 7/7、outcome passed；未独立重跑，不覆盖 r1–r4 失败。父方报告 strict Clippy exit 0；统一 workspace gate 和新的完整 Linux fixture 周期由父方运行，本报告不宣称它们已最终通过。macOS/Windows/Linux 真实桌面、IME、辅助技术、包和发行全部未验证；没有新增设计原型，也不以 headless GPUI 结果代签原生通过。core/session 仅为 app 合约追踪而查阅，非完整协议/安全发布审查。

结束冻结的唯一漂移为 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/tests/zmodem_openssh.rs`：r1 期望 `7a7ee3b4dd063776d84f5cd462ebbbebf574bd9d630a751363128bfb3490cf6d`，末次实际 `819e4ae15f9779268f26094eaf8645644fb1abf9b8a5abe8c0d84a877498f7f1`。修复后需要新的冻结/稳定副本及针对开放问题的回归，再进行非作者差分复核。r1 的失败原件和本轮意见应继续保留。

仅写入本 review 目录的报告、专项日志和隔离测试临时文件；未编辑源码、运行 fmt/Cargo/git、接触共享 target 或执行资源清理。机器可读结论见 `report.json`，带摘要的源码片段见 `source-evidence-r1.json`。
