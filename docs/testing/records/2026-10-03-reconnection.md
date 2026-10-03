# 2026-10-03 — 远程会话重连验收

本记录随实现维护。设计见 [ADR 0014](../../adr/0014-remote-session-reconnection.md)。本机集成和最终 macOS 原生构建已验收；精确提交的远端 CI 结果需另行核验，不作为完整产品或发布完成声明。

## 独立模块证据

- Core 完整 183 项普通测试与 1 项文档测试通过，新增重连定向 17 项全部通过；严格 Clippy 与格式通过。日志：`work/reconnect-core-full-tests.log`、`work/reconnect-core-tests-2.log`、`work/reconnect-core-clippy-2.log`。
- 首轮 Core Clippy 发现测试中的 `expect_err` 不符合工程 lint，已改用 Result 传播，保留原始失败日志。
- 集成时若其他模块的测试文件仍在编写，编译会报告缺失 module；对应 `work/reconnect-root-tests-1.log`、`work/reconnect-root-tests-2.log` 保留，最终完整门禁另行记录。

## 最终本机门禁

`work/reconnect-final-gate-1.log`：整仓格式、严格 Clippy、依赖策略通过；**578 项普通测试 + 2 项文档测试**通过，0 失败。按 crate 为 AI 47、App 201、Core 183、Session 147；默认门禁中的 4 项 OpenSSH ignored 已由独立夹具另行执行，不能重复计数。

最后增加归档 Open 按钮禁用、Delete 灰色以及“上次会话快照”文案后，重新通过格式、严格 app Clippy、12 项文件面板定向测试；确认层焦点调整后对应归档回归单独通过。最终打包回归 **47 / 47**；全新本机 OpenSSH **4 / 4**，28 个受管进程身份全部停止，临时目录移除、未验证祖先列表为空。汇总：`work/reconnect-closeout-counts.json`；OpenSSH 回执：`work/reconnect-closeout-openssh/result.json`。定向重复测试不增加独立总数。

新增覆盖包括真实 TCP 切断、保活超时、退出码/信号/通道关闭优先、EOF 半关闭、安静 shell Ready、满 UI 队列取消、阻塞写入后尾输出、完整两跳重连、共享分屏、取消与信任变更、30 秒预算、草稿同意文本变化拒绝旧结果、中英文窄表单、旧命令/AI 权限撤销及文件/监控/隧道快照。

面板测试在 worker 已计算成功但 GPUI 回调未处理时主动挂起，明确检查成功回执和远端真实字节，因此不会把取消成功冒充迟到成功隔离。自动调度 GPUI 用例明确注入 typed 终态，不能单独证明实际断网归因；真实协议与下面的原生故障注入补充此边界。模块交叉审查与最终独立复审未发现剩余阻断。

## 最终 macOS 原生验收

最终 staging：`work/packages/reconnect-final/KeelShell.app`；主程序 SHA-256：`f86d0b934ab2984cf96f5f65d4db42381ba28b4996fdf4854484319c15c42342`。构建日志：`work/reconnect-native-build-final.log`，与最终 `target/debug` 二进制哈希一致。

受控回环 SSH 后端经独立 TCP relay 接入，原生界面完成：

1. 核对真实夹具 SHA256 指纹后认证；密码每次临时输入。
2. 真实 TCP 切断后自动系列等待“继续认证”，保留编辑焦点；明确继续、重新认证，在原标签建立新 shell。
3. 原生搜索找到重连前输出；旧命令保留且执行按钮禁用，点击“用于当前会话”后才可手动发送。
4. SFTP 未保存草稿保留于上一会话快照，归档远端按钮禁用；中英文切换仍保留草稿。远端原始文件保持 70 字节，SHA-256 `0ff8547eb915849ef9ce51cdcdcef875a0ca03d43db46112e242b2a7ad1c4ecb`。
5. Ctrl-D 正常退出只显示手动重连；下一次替换有草稿的旧快照会要求确认。Command-W 仅关闭确认层，标签和草稿保留；明确同意并成功认证后才替换单份旧快照。
6. 再次断线后点击“停止重连”，提示保持手动可重连，之后正常退出应用。

操作清单、relay 回执与清理证据：`work/reconnect-native/native-checks.json`、`receipt.json`、`cleanup.json`。共三次故障注入、六个 TCP relay 连接，最终全部关闭；应用与夹具进程均退出，两个监听关闭，临时根目录清理。

首轮私有配置用 0644 创建，被应用正确拒绝；改为 0600 后重测。首轮构建后还补充确认层焦点及归档按钮视觉状态，因此重新打包并对最终构建重复完整流程。测试控制器最初以 SIGTERM 结束夹具，未触发临时目录析构；确认进程和监听全部退出后，显式删除唯一受管临时根，保留原失败回执，控制器后续改用 SIGINT。上述为验收夹具修正，不隐去原始失败。

## 发布历史核验

本轮提交前，独立重新扫描公开 `main` 的 58 个提交、1,143 个可达对象及 9,670 个历史文件路径，名称规则命中为零，strict fsck 通过。范围限当前 advertised refs 可达历史，不证明服务器缓存、隐藏引用或其他 clone 已清除；证据 `work/reconnect-history-independent-review.json`。本轮变更继续执行全公开内容与 Markdown 链接扫描，推送后核对新提交与 CI。

## 证明边界

夹具仅使用回环、临时目录与测试凭据，不执行用户 shell 命令；原生 UI 与测试日志只证明记录的受控路径。尚未进行 Windows/Linux 桌面操作、真实客户网络、多种 SSH 服务端及远程进程恢复验收。未创建发布标签。
