# 2026-10-05 AI 请求头名称已知秘密 E 修复

- 基线：`23310ee13286adb488a451277addf49b05cd476b`，完整继承 D 候选，不混入主树 MCP 后续提交。
- 状态：请求边界完整门禁通过并单独冻结，随后同片补齐已知秘密不能持久化的必要 Apply/写入边界；最终完整门禁与MAC15开发编译/只读检查通过。完整E候选与失败lineage冻结供fresh非作者审查；尚未独立复核或整合，没有提交或推送。
- 范围：自定义请求头名称的全已知秘密审核与网络前拒绝；保持 D 草稿生命周期、资源预算及显式值投递的用途绑定。

## 原失败与隔离

D fresh 非作者使用自有 loopback，实证合法 HTTP 头名称可以携带仍保留的已知秘密。三协议 Discover/Test/Ask 九个场景收到真实请求，原 HTTP 反例 exit101；原真实 GPUI 反例也 exit101。D 作者原冻结 193 proof/source41/input491 保持，不把作者通过改写为秘密边界通过；D 不整合。

E 修改前逐 SHA 核验 D seal 及全部 193 proof，独立复制作者冻结材料与新失败 lineage。D reviewer 完整 archive、manifest、seal 逐 bytes/SHA 校验，并完整读回 541 proof/542 archive members；原 manifest SHA `3c7da1c65379a1713204cae73bb436c63dbe9b8b2197de45bdb055aa218002c4`，93,336,921 字节 archive SHA `03371fc481bf62d04243f393986d9625d637faae7cc6d2fb4e7556a277a52764`。原 6958 字节 HTTP SHA `d6b040164eb0e7491c795627c19cb2390700161ed7193a0a4fd1ef6e1aeeb266` 与3378字节 GPUI SHA `f7741e4cab7d89ae0a935acf8b812e9ab03ccfc05528d4b39473c95555040999` 按原字节保存。

仅独占原作者 source/target，使用 E 专属绝对 TMP、mode0700 目录与0600日志/收据。每条命令有 owned PID、界限、输入前后 size/SHA 和原始日志。未重跑旧 C 受系统中断的审查探针，未重跑 D 泄露实现，没有真实用户凭据、生产机器、实际 CLI/供应商/云或已安装 GUI 操作。

## 实现与正式回归

共享 RequestOptions 名称检查消费自身已知值、全配置快照及当前调用才提供的 key，以 ASCII 忽略大小写的有界窗口匹配原名称与 HTTP 规范化名称；空值忽略，比128字节名称更长的值不生成副本。ContextDraft 在批准前拒绝，ProviderClient 的共同请求入口及阻塞 AiClient.send 在构造发送前再次拒绝。错误为 CredentialInContext。secret pool 只作拒绝/脱敏，原显式值交付权限不扩大；D 的独立草稿缓存、4096/1MiB/8MiB失败关闭、取消与迟到结果机制保持。

新增3个单元回归检查原/规范化名称、前后缀、当前头值与代理值、空/非匹配值及 sensitive 显式值。新增4个 HTTP 回归涵盖全秘密快照三协议九操作、晚传key九操作、blocking三协议，以及三协议安全名称/显式header/auth的实际交付正例。负面夹具在 typed rejection 后以 stop/join/重新bind断言0连接；即使guard回退也会正常响应以便完成任务后失败。原3378字节 GPUI 不改断言正式纳入，检查masked owner invalid、active resolve合法、prepare拒绝与0请求。

原HTTP诊断要求收包及正确方法，之后才作负面断言；E的一次原字节执行退出101、耗时7.522秒、原输入未变，日志为 owned fixture failed，夹具在3.01秒结束。该诊断没有 typed rejection 观测，不能标为通过。正式新增九场景独立证明拒绝类型和0HTTP；临时 Cargo 入口随后删除，原字节/日志/收据保留，未改断言或发送无秘密请求迎合诊断。

静态检查同时证实 Apply 和 Workspace 写入入口此前只做句法/引用校验，已知值可作为合法名字/model/endpoint/proxy元数据保存。第一轮完整请求门禁完成后，将该轮patch/source和输入收据单独冻结，再补同一禁止明文持久化边界；未边跑门禁边修改源码。新守卫检查全目录的可编辑/provider文本，固定协议标签、UUID、数字不扫描、不新读环境；后台写入前检查精确事件catalog/credentials快照。

追加2个单元回归覆盖9类可保存字段、清除释放和单值预算失败关闭。真实GPUI Apply检查已知头名不发事件、安全改名后发事件，并在pending save期间更换masked值、接收旧revision回执后确认新值仍已知/可见。真实Workspace/StateStore回归绕过panel直接发4类有效事件，验证全部拒绝、原磁盘字节不变；安全snapshot随后实际保存且metadata无秘密，合法已知pool保持。没有联网或启动CLI。

## 作者命令证据

| 收据 | 状态 | 范围 |
| --- | --- | --- |
| e-fmt-write-1 | write-only | cargo fmt exit0，runner90记录预期写格式，原GPUI SHA不变 |
| e-http-focused-1 | passed | 40.579秒/inputs unchanged，4正式HTTP tests覆盖9+9+3拒绝与3交付场景，0失败 |
| e-gpui-original-1 | passed | 116.030秒/inputs unchanged，原3378字节/原断言通过，prepared_ok=false/network_requests=0 |
| e-http-original-1 | expected non-pass | 7.522秒/inputs unchanged，原6958字节诊断exit101，不能计passed；原bytes/FAIL保留 |
| e-fullcheck-1 | passed, intermediate | 411.385秒/inputs unchanged，42普通harness/1117passed/0failed/11ignored、4doc/8passed，Python6/fmt/strictClippy/x.y/default与2MiB controller通过；之后才改保存边界，源码快照独立冻结 |
| e-fmt-write-2 | write-only | cargo fmt exit0，runner90记录预期格式写入，原GPUI字节不变 |
| e-save-focused-1 | passed with warning | 39.621秒/inputs unchanged，14项匹配metadata的正式回归/0失败，其中4项新增保存回归；unused测试import警告原样保留并随后移除，不冒充strictClippy通过 |
| e-fmt-write-3 | write-only | cargo fmt exit0，runner90记录预期格式写入；原3378字节GPUI不变 |
| e-fullcheck-2 | passed, final engineering | 257.783秒/inputs unchanged，42普通harness/1121passed/0failed/11ignored、4doc/8passed，Python6/fmt/strict workspace all-targets Clippy/x.y/default与2MiB controller通过，future分别4624bytes |
| e-macos-build-1 | passed | 36.901秒/inputs unchanged，显式MACOSX_DEPLOYMENT_TARGET=15.0构建app与MCP，未启动 |
| e-macos-inspect-1 | passed | 0.222秒/inputs unchanged，两者实际arm64 Mach-O/minos15.0，完整load commands与SHA保存，0程序启动 |

最终app为181442416字节，SHA `08a01597361c5352b5b695d881632d740fa3ca6866bc6d9687ee8fddcc4af5fb`；MCP为16655024字节，SHA `27f1a51413253ee552c326f5a3c7d0bdfca79e2449be2ab46c6b483292bfaa24`，二者minos15.0。MCP源码和有效MAC15产物在此切片没有变化，构建命令核验其依赖/产物；没有以重新编译或原生运行替代真实发生的范围。

最终文档补齐后，checked工程输入保持同一bytes/SHA，完整组合patch、source/input/proofmanifest与source/proof archives按原字节读回冻结。D作者193proof全部原样、D审查541proof完整archive及原HTTP/GPUI失败仍自包含；原HTTP预期非通过收据不计正式passed。所有owned命令结束、私有TMP无残留后才允许根只读clone，作者停止源/Cargo写入，提交/整合由根与fresh审查处理。

11项ignored是原2项供应商CLI选择和9项OpenSSH，不计执行通过。默认/2MiB独立controller结果不与普通harness重复计数，既有block0.1.6 future-incompatibility提示保留。编译/GPUI/回环不代表实际macOS窗口、Windows/Linux桌面、供应商兼容性、Release或安装更新验收。

## E 独立失败与 F 后续修订

E 非作者完整门禁1121普通+8doc+6脚本通过，但独立实际 GPUI 固定认证头保存反例确认 P2，最终判定失败。原小写可Apply、混合大小写不能Apply，两者Core合法且0HTTP；原2957字节反例和失败保持。718 proof/719归档经根完整读回，manifest `463b38269b686e6ffb68b1eb804dc9b31ea683716543fa97a64537b342c59572`、321,461,063字节 archive `741b2067b58ddbd8e9cd30d5cf16ca0a3777f4ac06dfe03aed1aabd3d6f8bfcb`。E 原样不整合。

F 最小大小写修订已通过独立差量复核并精确导入主树，见[F记录](2026-10-05-ai-auth-header-case.md)。该修订不追溯改写 E 失败；主树 F 完整门禁与原生独立记录。
