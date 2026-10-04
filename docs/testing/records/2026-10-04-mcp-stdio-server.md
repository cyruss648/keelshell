# 对外 MCP 服务端基础验证 — 2026-10-04

范围：独立 `keelshell-mcp` crate/binary、官方SDK协议、默认关闭、授权/撤权、有界读取与仅待审核的命令提案契约。设计见[ADR0037](../../adr/0037-external-mcp-stdio-server.md)。不存在通用第三方MCP客户端。

## 验证结果

本机macOS执行以下针对新增crate的检查，全部通过：

```sh
cargo fmt --all --check
cargo clippy -p keelshell-mcp --all-targets --locked -- -D warnings
cargo test -p keelshell-mcp --locked
python3 scripts/check.py --policy-only
git diff --check
```

- 4项单元测试：路径词法/完整组件、含newline输入边界与多消息重置、超量输入永久关闭。
- 15项公开库集成测试：默认关闭不dispatch、显式scope仍需connectedbackend、固定读取UTF-8、工具/连接/selection/重连/路线边界、遍历/sibling路径、未知字段和approve/exec/unlock/write/connect拒绝、唯一摘要命令提案、空/NUL/超量命令、后端目标/selection/提案不匹配、枚举泄漏/重复、字节与JSON转义输出边界、撤权/取消/超时释放ownedfuture、并发满不扩容、scope/runtime配置约束。
- 6项实际stdio子进程集成：独立手写JSON客户端（不用SDK client）验证2025-11-25初始化、七项工具schema、默认DISABLED、新版2026-07-28 discover与逐请求metadata、缺失metadata/不支持版本、错误请求形状/非法参数/未知方法、超量未结束输入、静态脱敏诊断、EOF正常退出、不回显argv及客户端保持stdin打开但不发数据时10秒启动截止后的进程退出。进程均由测试owned、kill-on-drop，未接触SSH/凭据。
- 4项同协议byte-stream集成：2025-06-18兼容握手、授权读取及pending命令提案、撤权后拒绝与无context回传、RPC cancellation notification释放拥有的pendingbackend并允许后续请求、关闭输入在service返回前释放in-flight backend。
- 1项rustdoc：默认关闭、明确安装桌面grant及撤权的公开API。

合计29项普通测试和1项doctest，0失败/忽略。严格目标Clippy通过；所有直接registry为x.y，rmcp解析3.5.0、tokio-util解析0.7.19。该worktree没有改动既有app/core/session/AI行为，合并后的整仓门禁和本提交三平台CI须由主线单独记录。

通过日志保存在ignored `work/mcp-tests-final-accepted.log`、`work/mcp-clippy-final-accepted.log`；最初20项通过记录为 `work/mcp-tests-first.log`。本地记录不提交请求内容、客户日志、地址或凭据。

## 发现与修复

新增wire取消测试首次错误地等待一个CANCELLED回复，导致3秒fixturedeadline失败；对应 `work/mcp-tests-wire.log` 保留。官方2026stdio规范及rmcp3.5.0源码明确取消后不继续发送消息，SDK移除该request ID并丢弃迟到响应。修正验收为证明pendingfuture释放、没有旧ID回复且下一tools/list正常；库直接调用仍验证typedCancelled。

有界reader新增read-to-end专项发现首次实现先填充调用方ReadBuf再返回InvalidData，违反AsyncRead报错契约，Tokio断言失败；对应 `work/mcp-tests-final.log` 保留。修复为先在独立8 KiB scratch读取/检查，只有合法chunk才发布到调用方buffer；再次读取失败stream仍返回固定错误。补测完整含newline边界及多消息重置通过，未放宽128 KiB限制。

补充关闭输入生命周期token，EOF/读错误/transport析构立即停止此连接的backend准入与future；同时binary显式使用有界runtime shutdown，避免Tokio不可取消的blocking stdin在线程池退出时无限等待。真实stdin保持打开的10秒启动deadline专项通过。

覆盖SDK默认unknown-method回显行为，服务端显式返回固定Method Not Found文案；新增私密method名称不进入错误内容的真实stdio断言。

前期编译发现sha2 0.11数组没有LowerHex及测试helper变量遮蔽，修复了摘要hex编码与helper命名；没有因此削弱授权断言。

## 尚未验收的边界

真实stdio启动/退出在macOS上验证；不代表Windows/Linux原生进程验收、外部Claude Code/Codex实际配置互通、应用内UI/IPC/SSH/SFTP/监控业务闭环。production binary默认关闭且没有bridge，受控backend仅返回固定测试值/待审核提案，不执行系统命令、不登录SSH、不读取用户文件或凭据、不调用模型。

DesktopBackend契约要求真实IPC认证、当前authority、准确捕获live handle、canonical/no-symlink/type检查、owned取消cleanup、桌面人工批准单次消费与过期。这些还没有实现，不能用契约测试声称已完成MCP-01..04。
