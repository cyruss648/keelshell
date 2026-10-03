# SSH 凭据库与终端搜索集成验收 — 2026-10-03

本记录覆盖 SSH 凭据库 schema 2、显式保存/解锁界面、终端搜索与双语切换的整合工作区。测试使用独立配置目录、临时凭据和仅监听回环地址的 SSH/SFTP 夹具，没有访问用户服务器或真实模型服务。

## 本地检查

最终代码在配置冲突提示修正后执行：

```sh
python3 scripts/check.py
python3 -m unittest discover -s packaging -p 'test_*.py' -v
cargo build -p keelshell-app --locked
```

结果：依赖 `x.y` 规则、全仓格式检查、Clippy 全目标 `-D warnings` 均通过。Rust 测试 **236 passed / 0 failed / 0 ignored**：AI 47、应用 73、核心 71、会话 44、rustdoc 1。打包与发布脚本测试 **47 passed**。macOS ARM64 调试构建、`.app` 组装和实际启动成功。

本机原始日志保留于 ignored `work/vault-search-integration-final.log`、`work/vault-search-packaging-final.log` 和 `work/vault-search-native-build-final.log`。本地构建日志不进入公开仓库；此处记录可复现命令与结果。

## 原生 macOS 交互

通过应用的真实按钮和键盘输入完成：

1. 中文界面中打开隔离 SSH 配置，选择“保存到凭据库”，输入一次性测试 SSH 密码及测试主密码。保存完成后显示锁定状态，尚无 SSH 会话。
2. 输入错误主密码，界面显示解锁失败，未建立会话。重新输入正确主密码后，出现回环 SSH 横幅和 SFTP 目录列表。
3. `Cmd+F` 打开搜索，输入 `fixture`；Enter/Shift+Enter 定位匹配并显示高亮。终端内容没有出现搜索输入的回显。
4. 搜索打开时切换英文，查询仍为 `fixture`，占位文字切换为英文。
5. 回到搜索框按 Esc，随后输入 `search-focus-ok` 并回车。搜索关闭，回环终端收到该文本并显示下一提示符。
6. 重新构建包含最终配置冲突文案的应用，以同一隔离目录重启。已保存的配置和英文设置仍存在；连接操作重新要求主密码，输入后成功建立回环 SSH 会话。没有复用此前进程中的解锁状态。

步骤 1–5 的二进制 SHA-256 为 `1b159eeb3d308980000286c2b3b24122a80059ed1a0c17f38d91185f6c512ad9`；步骤 6 的最终二进制为 `f09cd3db6864c3fcc71cdc5b1d775124f04e1a8b0eff89a99bc5ce9b4a0bad89`。最终版本仅补充了配置冲突的恢复提示和对应断言；搜索代码未再变动。界面截图与可访问性观察保留在开发会话的工具证据中。

磁盘检查确认：`state.json` 仅有不透明凭据引用，`vault.json` 为 schema 2 且含一个加密条目。两个文件权限均为 0600，二者均不包含本次测试的明文 SSH 密码或主密码。测试应用和回环夹具在验收后退出。

## 失败修复与验证边界

- 早期搜索覆盖曾绕过真实快捷键，且 Esc 没有恢复终端焦点；真实 GPUI 输入测试复现后已修复，见 [搜索记录](2026-10-03-terminal-search.md)。
- 核心加固时的首次 Clippy 失败源于测试中使用 `expect`，已改为错误传播，失败日志保留，见 [加固记录](2026-10-03-vault-hardening.md)。
- 复核发现 state 冲突后重开弹窗不能刷新持久化 revision。最终提示要求先保存其他工作、重启加载配置后重试，并明确密码输入已清空；新增断言覆盖中文、英文与状态栏一致性，见 [界面记录](2026-10-03-vault-ui.md)。
- 回环终端只回显，不执行操作系统命令；监控面板准确显示夹具拒绝 exec，不能将其记为真实 Linux 监控成功。
- 本次原生流程验证密码认证。私钥 payload 的目标绑定有单元测试，实际私钥签名有独立传输测试；二者不等价于完整的原生“保存私钥口令后登录”验收。
- 本机结果不证明 Windows/Linux 桌面交互、生产 SSH 互通、OS Keychain、签名/公证或独立密码学审计。AI Key 仍只保存在内存中。
- 本次没有创建版本标签或发布 Release；六目标构建的旧结果仍归属其记录的提交。推送后 Quality 状态应以对应 GitHub 提交的运行记录为准。
