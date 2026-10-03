# 远端补全集成验收 — 2026-10-03

状态：本机实现、独立复审、整仓门禁、真实 OpenSSH 和 macOS 受控原生验收通过。补全提交 `8cd89fa132f7695400ca50b3297f533d5efcb8ee` 的三平台 Quality 已通过；该结果只属于此提交，不代替后续功能的 CI。

## 验收范围

对应 [ADR 0015](../../adr/0015-remote-command-completion.md)。已覆盖多行/UTF-8 光标、引用与中间词替换、SFTP/PATH 协议边界、取消和资源清理、会话/光标失效、真实 GPUI Undo/IME 以及 macOS 原生操作。真实 OpenSSH 互通在独立临时服务器运行。

## 证据规则

- 受控 GUI 夹具只回显终端输入；固定 PATH 响应与临时 SFTP 文件用于交互验证，不执行系统命令。
- OpenSSH 协议测试和受控 macOS GUI 验收分别记录，不能相互替代。
- Windows/Linux CI 的编译与测试不代表其桌面交互已验收。
- 以下构建、命令、计数和清理回执均属于本轮；原始日志保存在 ignored `work/`，没有将开发者配置或密钥纳入公开提交。

## 开发中发现并修复的问题

- 真实 shell 集成测试首轮误用了私有 module 路径，已改为公开 reexport；保留 `work/completion-shell-1.log`，后续重新编译和实际解析通过。
- 恰好达到扫描条目/名称字节上限且当前目录返回 EOF 时，后续 PATH 目录可能未扫描却没有显示截断；已补跨目录边界回归并修复 `limited`。
- SFTP NAME 缺少类型属性时，直接 STAT 无法区分原项是文件还是符号链接；已使用共享属性请求预算先 LSTAT，必要时再 STAT 目标。
- 初版宽度测试只调用窗口 resize 请求，实际测试视口没有改变。现改为模拟真实尺寸回调，并断言视口就是目标尺寸；之前的执行结果不作为小窗口证据。
- 交叉审阅发现普通参数 `cat a=fi` 被错误当成赋值值。普通参数应保留完整 basename；只有命令前赋值和明确的长选项值采用局部值替换。相应词法边界已纳入回归。

初次 Clippy 风格失败和修复日志同样保存在 ignored `work/`。这些记录用于溯源，不替代最终门禁。

## 实现范围与限额

命令栏提供“远端补全”与 `Ctrl+Space`。普通输入不会触发网络请求。命令名来自独立 SSH exec 环境的 PATH；路径来自明确的补全目录，可手动输入、取当前活跃文件面板目录或显式读取 SFTP 起点。选择候选仅替换光标词并保留 Undo，Enter 不执行；完整命令和目标仍需单独审核并点击执行。

查询使用固定 PATH 探针和 SFTP 字面请求。全程共享 5 秒截止时间，最多 16 KiB 探针输出、32 个 PATH 目录、8192 个条目、2 MiB 名称数据、64 次额外属性请求和 64 项候选。输入最多 65536 字节，路径 4096 字节，名称/前缀 1024 字节。缺失类型先 LSTAT，链接目标再 STAT，共用预算。截断、跳过和来源在界面可见。

候选绑定会话 EntityId、文本、revision、光标/选区、IME、目录及请求 UUID；在查询、接收和填入三处校验。仅移动光标、同文本程序重填、切换目标、模态、退出或实体替换均使旧结果失效。取消真正释放后端 future，并保留 worker 身份直到完成回调，避免旧查询与新查询重叠。

## 自动验证

| 检查 | 结果 | 本机证据 |
|---|---|---|
| `python3 scripts/check.py` | 依赖 x.y 策略、格式、workspace 全 targets 严格 Clippy、642 项普通测试与 2 项文档测试全部通过 | `work/completion-workspace-gate-1.log` |
| Core 补全单元 | 31 项，包括普通 `a=fi` 参数、赋值、长选项、引用和中间词 | `work/completion-core-tests-4.log`，整仓再次覆盖 |
| 真实 `/bin/sh` 解析 | 4 项，特殊名称为单个字面参数、多行边界、命令 basename、赋值与长选项 | 整仓日志中的 `completion_shell` |
| Session 完整普通测试 | 164 项，含 14 项新增真实 TCP 补全测试 | `work/completion-session-final-2.log`，整仓再次覆盖 |
| App 完整测试 | 213 项，含 12 项真实 GPUI + SSH/SFTP 补全测试 | 整仓日志 |
| 真实 OpenSSH 独立执行 | 5 项实际通过，3.253 秒；默认 ignored 的用例在此明确执行 | `work/completion-openssh-final/result.json` 和 `tests.log` |
| 打包回归 | 47 项通过 | `work/completion-packaging-1.log` |
| 注释澄清后格式与 core rustdoc | 格式通过，1 项文档测试通过 | `work/completion-core-rustdoc-fmt.log`、`work/completion-core-rustdoc-tests.log` |
| 最终应用构建 | `cargo build -p keelshell-app --locked` 成功 | `work/completion-build-freshness.log` |

普通测试计数按 AI 47、App 213、Core 218、Session 164 汇总。OpenSSH 的 5 项默认忽略未混算为普通测试通过；独立 runner 显式执行，并将最低通过数从 4 提升到 5。

12 项 GPUI 专项经过真实输入组件与回环 SFTP，覆盖 Unicode 多行局部替换、Undo 恢复光标、候选滚动、IME、Shift+Enter、相同内容的程序重填、等待中取消/编辑/换标签、模态和目标关闭、终端结束/新 Entity、Files 目录来源及中英文布局。小窗口使用 `simulate_window_resize` 并确认实际 viewport；900×580 与 1440×900、520/960 面板宽度均检查终端和输入/执行控件的可见边界。该模拟不等同于原生操作系统缩放验收。

Core 与 Session/App 由不同实现者交叉只读复审；词法参数、PATH 边界、LSTAT/STAT、取消所有权及票据失效修复后，无剩余阻断。最后的公开 API 注释补充了名称/路径限额、合法 ZWJ 名称被拒绝和 canonical 父目录的含义，没有改变行为。

## macOS 原生验收

完整受控流程使用 `work/packages/completion-native-2/KeelShell.app`，二进制 SHA-256：

```text
cb085ec66ebcdc2d54f009b4d9a1601465bac89994eeedec67cb10e94f6c1cbf
```

通过实际应用 UI 核验：

1. 独立核对测试服务器指纹、密码连接和 SFTP 根列表。
2. `dep` 查询显示 `deploy-demo` 与 `deploy-preview`，上下键/Enter 只填入，单步 Undo 恢复 `dep`，终端不产生输入。
3. 显式读取 SFTP 起点 `/`；多行命令第二行 `cat /报 --tail` 在中文词后查询，只替换为带引号的绝对文件路径，前后行与 `--tail` 完整保留。明确点击执行后才在 echo 终端出现审核文本。
4. 相对 `cat rep` 插入 `cat '/report'\''s draft.txt'`；含单引号与空格的 basename 保持字面语义。
5. `cd 中` 插入中文目录和结尾 `/`；后续输入落在闭合引号内。
6. 中文切换英文保留草稿与补全目录；仅移动光标撤销候选，Escape 收起候选，文件工具正文恢复。

AX 选区写入在实际 GPUI 组件不可用，自动化返回不支持；改用键盘定位后完成多行中间词验收。没有把工具能力限制计为产品失败，也没有跳过相关流程。

最后的 rustdoc 注释更新改变了 debug 二进制哈希，因此重新构建并打包 `work/packages/completion-native-3/KeelShell.app`。当前构建与该包的 SHA-256 完全相同：

```text
687efb8eb1a90d1fc4fe6515c15a7f2b55b7d6d951eae84083dd9a88e99f8c82
```

最终包另外实际完成启动、指纹核对/密码 SSH、SFTP 列表、PATH 候选/Enter/Undo、SFTP 起点、相对路径引号填入与英文状态保留；终端保持空输入。原生完整与最终冒烟回执分别保存在 `work/completion-native/native-checks.json` 和 `work/completion-native-final/native-checks.json`。

两次验收的应用与 fixture 均退出码 0；控制进程已结束，监听已关闭，临时 SFTP 根目录已删除。独立 OpenSSH 回执另确认 `owned_processes_stopped=true`、`ancestry_unverified=[]`、`temporary_directory_removed=true`。只保留 ignored 证据和隔离配置。

## 补全提交的跨平台 CI

[Quality 运行 37105632462](https://github.com/cyruss648/keelshell/actions/runs/37105632462) 对应 `8cd89fa132f7695400ca50b3297f533d5efcb8ee`，于 2026-10-03 07:21:46 UTC 完成，整体 `success`。macOS 26、Ubuntu 24.04、Windows 2025 三个原生 runner 的打包回归和 Rust quality gate 均成功；macOS 与 Ubuntu 的独立 OpenSSH 互通步骤及回执上传成功，Windows 按既定策略跳过 OpenSSH 脚本。

本次文档补录通过 GitHub CLI 重新读取了提交 SHA、job 与步骤状态，原始元数据保存在 ignored `work/completion-quality-37105632462-docs-recheck.json`。CI 原生 runner 构建/测试通过不代表 Windows/Linux 桌面交互验收，也不覆盖后续参数片段与批量 exec 增量。

## 证明边界

PATH 来自独立 exec 环境，不包含交互 PTY 的 alias/function 或后来修改的 PATH；补全目录不代表终端 cwd。目录父级 canonical 化不代表成员符号链接已展开。执行权限位只用于发现，不保证 ACL、挂载选项或解释器支持。非 UTF-8、U+FFFD 和部分合法零宽/ZWJ 名称保守跳过。

本机 GUI 夹具只回显，不执行任意 OS 命令。真实 shell 引用由独立 `/bin/sh` 测试验证，真实协议由 OpenSSH 测试验证。Windows/Linux 桌面、OS IME 候选窗、真实生产主机及可编程 shell 补全仍未验收。依赖 `block 0.1.6` 的上游 future-incompatibility 提示保留；严格 Clippy 当前通过。
