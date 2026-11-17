# 本地转发与原 SSH 生命周期 — 2026-11-17

状态：实现／本机工程验证通过，远端交付进行中；
基线 `660645fbe77cee2b8af2740f3277a70e9f968048`。
本记录不关闭完整产品、持久化隧道阶段、FILE-05、三平台原生或发行。

## 设计和范围

见 [ADR 0159](../../adr/0159-local-forward-transport-lifetime.md)。
修复库级本地转发在 SSH 不可用后仍可占用 listener 的生命周期，
并增加受 loopback 限制的独立 OpenSSH 转发套件和 macOS／Linux CI 步骤。
不改变审核权限、命名规则或自动恢复策略。

## 当前验证

工装专项 r1：`33 run / 0 failed / 0 skipped`，实际
`0 / reaped / 0.245009s`；包括新增 forwarding 模式的真实配置生成、
默认禁用 TCP forwarding、目标限制和互斥选项检查。
这些是 mocked 工装测试，不是 SSH 互通。

当前本机 `rustc 1.100.0 (0ef1bc386 2026-11-09)`、
`cargo 1.100.0 (3acc79c0c 2026-11-05)`；项目继续不 pin toolchain，
不以此设置产品 MSRV，registry 依赖要求继续仅用 `x.y`。
首次实时依赖检查 r1 在 113 声明中 101 通过、7 项 registry 失败、5 项
因共享的 `uuid` 锁版本过旧失败。按官方结果将锁从 `1.29.2` 更新到 `1.29.3`，
声明仍为 `1.29`，未新增依赖或改变版本声明。修订后的完整新快照 r2 对
37 registry package／113 条声明全部通过，实际
`0 / reaped / 37.122483s`。该结果只签 2026-11-17 本次官方 metadata
快照，不覆盖间接依赖、Actions、未来新版本或历史 registry 失败。

原生桌面状态检查返回 Mac 锁定且自动解锁失败；GUI 操作／截图为 0。
未启动正常包，未安装或覆盖应用；完整紧凑尺寸、连接上下文及辅助技术矩阵保持开放。

库级专项 r1 实际 `0 / reaped / 8.513484s`：16 passed、0 failed、0 ignored、
408 filtered。新用例覆盖 Closing／Closed、bind 前后和等待中断、闲置 listener、
远端 disconnect、TCP loss 与已建立流、待确认 channel-open、sibling／独立会话、
新旧连接、watch 已读终态／sender 消失／Connected 更新／stale watch。
作者只运行 rustfmt／diff 检查，以上 Rust 编译和执行均由父级串行完成。

真实 Darwin OpenSSH r1：6 passed、0 failed、0 ignored，套件实际执行 0.44s，
完整 runner `passed / 2.205s`。local／remote 各自停止后在原有 sibling stream
再次交换 70,031 字节；SOCKS、SSH断线无面板轮询、已关闭拒绝新监听，以及
同地址显式新owner与旧owner迟到关闭均有独立断言。
18个观察到的原进程身份全部停止、无未知祖先、临时密钥目录已删除。
这些是同宿主真实OpenSSH，不是Podman、真实生产主机或原生GUI验收。

首位非作者 Pauli REQUEST_CHANGES，P0=0/P1=0/P2=1/P3=0：
原local和remote用例在停止first后才新建sibling流，不能证明已有sibling流存活。
父级已修为停止前建立并交换、停止后继续在同一sibling流再次交换。
Pauli的33工装＋12独立控制共45项通过只签其原审查源码；报告及P2保留，agent已关闭。

769个非docs工程输入r1已冻结，aggregate
`79cdd56f1f38a720b7e4800b0ee65bd9346b683a43a5996c1db0080ab42e5bec`。
相对前批765集合，本清单另纳入原有两份`.hallmark`元数据及两个新增Rust文件。
完整门禁r1在严格Clippy的测试helper `while_let_loop`停止，实际
`1 / reaped / 31.610392s`，此时Python主526 run／3 skipped已通过，
Rust workspace测试及controller尚未运行。原件保留，不计作完整通过。
将EchoServer的`loop + let Ok(..) else break`等价改为`while let`后，
769输入r2 aggregate
`83e2e1b00ba4d373a407030c79538f8a5039347931236344315ad8b70844a5b7`。
新非作者Curie对r2限定APPROVE，P0=0/P1=0/P2=0/P3=0，已实际关闭。
逐项核对769输入无漂移，确认原Pauli P2关闭；r1→r2仅上述等价循环修改。
7条验证命令中，Python两条unittest共45项通过；父级精确预编译session binary
先取摘要和完整`--list`，16项逐名匹配源码，独立复跑16/0/0。
这是预编译复跑，不是独立重编译。父级真实OpenSSH r2的6项、清理字段、
日志字节数与临时凭据目录删除均离线核验，未由reviewer重跑SSH。
依赖回执37包／113声明与当前manifest和direct lock edges离线相符。
完整门禁r2实际 `0 / reaped / 845.590925s`：

- 普通Rust：3435 passed、0 failed、68 ignored；rustdoc：14 passed。
- Python主discovery：526 run、0 failed、3 skipped。
- fmt、严格Clippy、直接依赖声明政策及三个完整controller gate全部通过。
- 95个普通套件与精确测试二进制`--list`逐名相等，实际结果数量和摘要相等，
  二进制SHA前后不变；库存检查实际 `0 / reaped / 1.746323s`。
- 两个`harness=false` controller没有普通libtest库存接口，未向其发送`--list`，
  由完整门禁的真实执行验证。
- 冻结前后769输入及aggregate相同，唯一target连续归属本checkout，不clean。

同一r2的真实OpenSSH6项再次通过：runner `passed / 2.197s`，18观察身份全部停止、
祖先无未知、临时密钥目录删除。新r2只修测试helper，不继承旧r1包或二进制作为新结果。
提交推送与自有资源清理进行中；精确新CI、Windows真实OpenSSH forwarding、
原生桌面与发行仍单独验收。

可在已有OpenSSH工具的macOS／Linux宿主上重新运行独立套件：

```sh
python3 -B scripts/openssh_interop.py --forwarding
```

每次生成新的隔离服务器和凭据，不复用旧receipt目录。若更换源码目录但共享target，
先遵循[共享target合同](../SHARED_TARGET.md)，不能仅从Cargo退出0推定新用例已编译。
