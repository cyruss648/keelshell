# SSH 上游代理验收

日期：2026-10-03。基线：`7f95c46`。状态：本机门禁、独立复审与 macOS 受控原生验收通过；新增提交的远程 CI 另行核对。设计见 [ADR 0013](../../adr/0013-upstream-ssh-proxies.md)。

## 验收内容

| 层次 | 必须验证的行为 |
|---|---|
| 配置与存储 | 严格 JSON、秘密字段拒绝、UTF-8 字节边界、旧配置与旧路线键兼容、规范化、导入/复制/恢复、上游变更后活动与回收站关联失效 |
| 主机与凭据 | 代理路线独立指纹；无直连指纹回退；旧版直连 vault 拒绝代理路线；新 vault 绑定每跳完整路线，代理密码不保存 |
| 协议 | SOCKS5 匿名/密码/IPv4/IPv6/域名；HTTP CONNECT 匿名/Basic/IPv6 authority；分段、同包 SSH banner、信息性响应、有限错误与大小边界 |
| 生命周期 | 单次绝对截止时间、拒绝不重试、取消与迟到结果隔离、逐跳代理、无直连回退、父 SSH 与专属链的资源清理 |
| 界面 | 折叠编辑/重开、双语与草稿、独立密码、Agent 代理认证、保存后解锁、当前路线编辑取消、窄布局和焦点 |
| 原生 | 在最终 macOS 构建中，实际操作两种代理、逐跳认证/指纹、终端与 SFTP、取消/失败/重连和退出清理 |

## 私有验收环境

ignored `work/proxy-native/fixture.py` 提供限时回环代理。两种协议只允许各自固定的虚构域名与端口，映射至一次性 SSH 夹具；用户名和密码只用于这套夹具。SSH 终端仅回显，不执行系统命令；SFTP 根是临时目录。记录不保存密码、原始认证请求或代理响应头。

该环境在产品原生验收之前独立通过两种代理的 TCP / SSH banner 冒烟检查；进程、监听和临时目录已清理，回执为 `work/proxy-native/smoke-receipt.json`。此项只证明夹具可以使用，不证明应用已经接入代理。

## 最终工程检查

| 检查 | 结果与本地证据 |
|---|---|
| 整仓依赖策略、格式与严格 Clippy | 通过；`work/proxy-full-gate.log`，使用 `--workspace --all-targets --locked -- -D warnings` |
| 整仓普通测试 | **523 通过、0 失败**；AI 47、app 173、core 166、session 137；4 个 opt-in OpenSSH 用例在普通运行忽略、另行补齐 |
| 文档测试 | **2 通过** |
| 独立真实 OpenSSH（既有 SFTP/exec 互通回归） | **4/4 通过**；`work/proxy-openssh-final/result.json` 与 `tests.log`；27 个被跟踪进程身份全部停止，无 ancestry 未验证项，临时目录已删除 |
| 打包回归 | **47/47 通过**；`work/proxy-packaging.log` |
| 应用专项完整运行 | **173/173 通过**；`work/upstream-proxy-app-tests-final-2.log`；最终整仓日志同样包含全部 173 项 |
| 独立审查 | core、session 和 app 交叉只读复审完成，无剩余生产阻断；session 夹具清理证据的审查发现已修复并回归 |

新增回归包括 core 19 项、session 协议单元 10 项与真实 TCP 9 项、app 13 项。session 的 `--all-targets` 专项还执行了 4 个 example 用例；上述整仓 523 项不包含这些 example 用例。应用新增用例覆盖 Save→Unlock、Agent+代理、秘密清空、迟到结果、编辑路线取消、旧指纹与旧 vault 隔离，以及三种尺寸和两种语言下实际 wheel→click→input。

## macOS 最终原生验收

最终阶段构建：`work/packages/proxy-final/KeelShell.app`。二进制 SHA-256：`bbc2e8e8de65f46ae15eb32b230874cafe0644f4a9f1a6167a2269b03a91bae8`。日志为 `work/proxy-native-build-final.log`；隔离状态与回执为 `work/proxy-native/native-checks.json`、`receipt.json`。没有替换已安装应用或访问生产服务器。

1. 在连接编辑界面分别配置 SOCKS5 跳板与 HTTP CONNECT 目标，实际滚动、点击代理用户名并保存。端点与用户名持久化，代理密码没有进入配置。
2. 使用错误 SOCKS5 密码，出现明确中文认证失败，未创建标签或最近记录；重新连接的两个秘密输入均为空。
3. 核对夹具给出的第一跳指纹并信任；重新询问秘密后进入第二跳。路线预览按“SOCKS5 → SSH 跳板 → HTTP CONNECT → SSH 目标”排列。
4. 在目标认证前取消，回执确认上游 SOCKS5 流已关闭。切换英文、重新进入连接，输入为空且路线上各步文案清晰。
5. 经跳板完成 HTTP CONNECT，核对目标独立指纹并信任，再次输入目标认证后生成唯一目标标签。终端实际回显 `proxy-chain-native-ok`；SFTP 列出 `welcome.txt`，编辑器读取 70 字节内容并正确显示中文。
6. 关闭目标、恢复中文并退出应用，退出码 0。检查仅有 1 条最终目标最近记录、2 条路线指纹和 0 条旧直连指纹；配置不含测试 SSH/代理秘密。
7. 全部代理请求已关闭，无夹具协议错误。停止本轮控制进程及两台 SSH 夹具后，逐一验证进程已退出、监听端口已关闭、两个临时 SFTP 根已删除。

夹具故意拒绝 exec，因此监控面板显示“不完整/服务器拒绝”，不能将本次终端回显和 SFTP 读取扩大为真实 Linux 监控验收。

## 失败记录与验证边界

- 初始完整 app 运行 172 通过、1 个旧英文标签断言失败；保留 `work/upstream-proxy-app-tests-final.log`，更新测试预期后定向及完整 173 项复跑通过。生产布局拆分后旧输入 ID 的测试预期也已同步；没有以跳过用例通过门禁。
- 初始原生“滚轮未移动”观察包含自动化工具自然滚动方向的因素；使用正确方向后可显示下方字段。这不构成原产品滚轮缺陷的证据。最终布局约束和实际滚动/点击回归仍保留，认证正文中的 SSH 保存按钮和代理输入均可达。
- 新增代理解锁取消测试在前台处理结果前取消，证明没有发起代理认证、创建标签或继承秘密；它没有强制后台先生成 `Unlocked` 再延迟交付的窄窗口。该分支另外经过 token 检查的静态复审与既有凭据回归验证。
- 新增代理 A/B 用例让 A 停在后续 hop、B 回到首 hop；相同步骤 A/B 的请求 UUID 隔离由既有跳板回归覆盖。
- 首次私有夹具状态使用错误 locale 值，应用拒绝并保留文件；修正测试输入后继续，记录 `work/proxy-native/setup-error.txt`。夹具与自动化输入问题不记作产品互通成功或未修复缺陷。
- 本机检查仍提示上游 `block 0.1.6` 的 future incompatibility；格式、严格 Clippy 与测试均通过。此记录没有宣称未来编译器兼容已解决。

## 当前边界

此切片提供 SOCKS5 与普通 HTTP CONNECT，不包括 HTTPS、PAC、NTLM、Kerberos、SOCKS4、外部 ProxyCommand 或代理密码保存。HTTP Basic 与 SOCKS5 密码认证本身没有传输加密。回环测试、macOS 原生界面、各平台 CI 与真实第三方代理兼容性是不同验证范围。
