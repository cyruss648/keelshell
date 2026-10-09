# ADR 0085 — Unix 本地智能体清理终态和动作所有权

- 日期：2026-10-08
- 状态：有界EPERM契约通过非作者源码复核、根16控制及原完整门禁；最终证据复核与准确平台CI待完成，旧失败保持
- 范围：既有 Unix 本地 CLI 子进程清理；Windows 路径保持

## 背景

锁定 process-wrap 10.0.1 的组 wait 在 ECHILD 时结束，无法单凭该状态证明普通非直接后代已消失。spawn 时可从实际公开 ProcessGroupChild.pgid() 保存组号，直接 leader wait/reap 后 native id() 可能是 None。

先前候选增加 post-wait 只读观察，却在错误、取消与 Drop 时仍允许清理重入发 SIGKILL；数字 PGID 可能已复用。这是源码确认的条件风险，没有实际复用或误杀证据。单个 helper 不发信号，不能证明完整 owner 调用链不发信号。

精确历史 macOS CI 的单次连接失败没有原 listener PID/PGID/nonce/ACK，其原因仍 UNKNOWN；本决策不宣称修复历史 case。

## 决策

先建立 OwnedChild Drop owner，再从实际安装的 ProcessGroupChild 捕获有效 PGID；捕获失败仍为 SpawnFailed，并保留未开始 owner 的原首次 Drop 清理。

Unix 使用明确状态：Ready、Waiting（保存 deadline）、Observing（保存同一 deadline）、Failed、Complete。首次进入清理同步消耗信号动作许可并保存原三秒 CLEANUP_DEADLINE，之后开始 try_wait 和一次 SIGKILL 尝试。Waiting 或 Observing 的 future 被取消时，外层 cleanup 沿原状态和原 deadline 继续等待／观察；不能获取新期限或新信号许可。

非 ESRCH／NotFound／EPERM 的首次 signal 错误、实际 wait 错误／到期、除EPERM之外的未知观察错误和观察到期进入 Failed；之后清理继续返回 CleanupFailed，Drop 不重发信号。首次 EPERM 保持原期限内回收和只读确认，不按旧数字 PGID 重发。新的候选将 signal-zero 的 EPERM 视为仍未知，沿同一期限继续十毫秒有界观察。EPERM 本身不证明不存在；只有完成原leader实际 wait、且期限内 signal-zero 返回 ESRCH 才进入 Complete。原成员、复用后的未知成员或持续权限拒绝均不能建立成功，到期仍拒绝。

Drop 只在 Ready 状态允许一次原最佳努力动作，并同步消耗该许可。已开始、取消或失败的清理不能重新向原数字组号发信号；它也不把失败改写为已清理。Unix 完成状态独立于 Windows 的既有 cleaned marker。原 Windows cleanup 函数正文、observer 和 Windows Drop 分支保持原实现。

该设计关闭新增 post-wait await 导致的重入信号风险，不证明数字 PGID 永久归属于原组，不消除首次 signal 的既有数字身份竞争，也不保证恶意 setsid/setpgid 逃离组、OS init 回收时序或既有 blocking wait worker 的生命周期。观察到复用后的存在仅能拒绝；不会补发动作。

既有 nix x.y 0.31 保留 fs/poll/user，显式补 process/signal/socket features。安全 socket API 返回 OwnedFd，通过安全 fcntl 配置 CLOEXEC，再安全转换 UnixStream 并配置 nonblocking；macOS 使用 SockFlag::empty()。继承的 unsafe_code=forbid 保持，新夹具不含 unsafe block。socket feature 激活既有 memoffset 0.9.1，Cargo.lock 仅在 nix 0.31.3 增加 memoffset 依赖边（14 B），所有锁定版本、checksum 和其它依赖边不变。工具链不改；v4 实际 --locked 库编译通过，v5 新组合须独立重验。原 626 默认／2 MiB 控制、四线程、请求／准备期限及原端口断言保持。

## 有界未知观察契约候选 — 2026-10-08

这是明确的工程契约修改：旧signal-zero EPERM立即Failed并恰好观察一次；新候选保持Observing，仅在原三秒deadline内继续signal-zero。它不是把EPERM解释为组缺席，也不增加非零signal、重新获取动作许可、重置deadline或接受迟到ESRCH。EIO／EINVAL／ECHILD等其它errno仍首次拒绝；实际wait错误仍原样拒绝。取消和重入保存原Observing及deadline，Failed和Complete仍锁定，已开始的Drop不补发动作。

根隔离真实机制探针在本机观察到同一自有未reap Child的live组为present、确认zombie后signal-zero为EPERM，原Child实际wait0后为ESRCH。该结果证明一种退出过渡机制存在，不证明旧v7或v9曾处于该状态，也不证明新owner完整门禁通过。仍保留所有旧失败和测试前像，不能以一次绿重跑改写旧因果。

原`group_observation_permission_and_unknown_errors_fail_closed_without_signal`中EPERM“恰好一次”的断言域显式移至新的有界未知控制；其余三个errno及原断言保持。原owner首次signal／观察错误控制不删，其持续EPERM预期仍为typed失败，只是到期判定。新增四个控制分别验证瞬时注入EPERM后只接受真实native ESRCH、持续EPERM取消／重入沿原三秒到期、迟到合成ESRCH拒绝，以及EPERM观察取消后的实际OwnedChild Drop不能获得第二次动作许可。注入控制与本机真实机制探针分开记账，未伪装为相同OS场景。

候选尚未编译或执行Rust测试。需要新的非作者源码准入，再在根原target完成锁定编译、全部十六Unix控制、格式／严格Clippy／x.y及原完整check；默认／2MiB各626、原线程数与全部外部期限和断言保持。精确CI、真实非child zombie归属证明、原生CLI／MCP／桌面及发行安装仍OPEN。没有新增guardian／leader／member复杂探针，首次数字PGID动作竞态和主动detach边界保持。

## 验证与边界

原 11 项 Rust 控制使用真正私有 helper、真实 OwnedChild 和自有实际组，新增实际 leader wait 取消、post-wait 取消／重入／Drop、首次 signal 错误、观察错误及原三秒期限失败。动作／errno 注入明确只检验错误控制流，不能代表实际 OS 权限或 PGID 复用；实际 SIGKILL 成功路径独立覆盖。

自托管 exact ignored 入口只调用当前测试可执行文件，env_clear、0700 私有短根、0600 文件、原子完整身份、PID/PGID/nonce ACK；测试入口之外不访问模型、客户 SSH 或用户 CLI。夹具通过 /tmp 中随机 ks-ug- 前缀的 0700 私有目录创建 socket 根，独立于继承的长 TMPDIR；两条完整 socket 路径用安全 UnixAddr 校验。该短根在外层检查 scratch 之外，不能以外层 TMP 空推断回收。正常 close 与失败 fallback 记录其独立路径／状态，仍须 actual leader reap 与 ESRCH 才报告正常删除；失败私有根保留。另新增隔离长 TMPDIR 子环境控制，覆盖普通 member 和 held leader 实际启动与独立根删除，禁止进程全局环境修改。原 11 项控制正文保持，新增第 12 项仍 UNRUN。夹具 leader/member 有硬生命期，控制器八秒绝对期限与三秒 RAII fallback。fallback 使用 nonce 限定的自有 IPC 请求退出和只读终态观察，不再向数字 PGID 发信号；不确定时保留失败证据并报告 UNKNOWN，不能伪造 Complete。

Rust 行为、严格 Clippy、原完整控制、精确 macOS/Linux CI 与原生产品验收均 UNRUN。格式解析不等于编译；旧 Python 反例与旧候选不继承为 PASS。实际记录见[验证记录](../testing/records/2026-10-08-unix-local-agent-cleanup-terminal-state.md)。

## v5 夹具读取补充

v4 真实 held leader 控制失败并保留 accepted stream timeout 配置 EINVAL；准确生产 CleanupFailed 分支仍 UNKNOWN，独立标准库 setter 探针不证明原因。v5 仅将该 held leader 读取换为安全 nonblocking 完整 17 字节 helper，保留原单次 100 ms、父 10 秒绝对期限、2 ms 有界轮询以及失败拒绝。成员协议、所有十二控制正文、生产状态／三秒期限和 Windows 保持。新的非作者只准入编译、原控制和完整门禁；v4 失败与私有根证据保留，v5 尚无行为 PASS。详见对应记录。

## v6 仅测试诊断

v5 仍为原控制 11 PASS／1 FAIL，完整门禁未到达，两夹具 stderr 为空，准确 CleanupFailed 分支 UNKNOWN；新根终验拒绝功能提交。v6 仅在既有失败判定后输出 cfg(test) 阶段、原 errno、期限与 native ID；不增加动作／观察／重试，不改生产结果和期限。逆变换精确恢复 v5，新的非作者只准入编译及原四线程十二控制，当前诊断 PENDING。测试日志 I/O 交付及更早未覆盖分支仍未证明。详见记录。

## v7 工程写法修正

v6 原控制12 PASS但完整check在严格Clippy的byte_char_slices失败。v7仅将held leader helper写出的单字节数组换成相同Q字节的byte string；十二正文、协议、三秒和全部控制流保持，无allow或deadline放宽。后继完整门禁与最终独立复核待完成；旧held失败原因仍UNKNOWN。

## v7 最终验证边界

等价byte string修正后严格Clippy通过，原十二控制重跑通过。但原default integration仅到408／626阶段，在ClaudeCode继承管道后代场景返回CleanupFailed，准确wait／观察分支UNKNOWN；普通库构建没有cfg(test)日志。完整工程未通过，禁止以专项成功覆盖原失败。root仅更新四状态文档，其余830执行输入（含既有文档）保持。后继debug实验只用于定位，不放宽错误拒绝或任何期限，实际结果见下节。

## v8 临时诊断与精确恢复

新的非作者仅准入一次独立有界调试实验：四处失败hook临时选入test/debug构建，同步日志可能阻塞、panic或改变调度，不能进入功能提交或发行。原locked integration no-run和原默认controller实际wait0，626连续阶段0..625及controller end完整读回，0失败hook；这次未重现旧失败，准确原因仍UNKNOWN。实验后精确恢复60,718字节原源码及完整834输入，恢复后的同一locked no-run实际wait0；没有再次运行默认controller。旧完整check失败、未到达的门禁、功能提交阻断与其它原生范围保持。完整流水和恢复事实见Unix测试记录；新实际运行独立复核单独绑定。

## v9 原完整失败与后续分析边界

新的非作者准入临时诊断后，原完整check在长TMP控制的组观察EPERM处实际失败，AI库96PASS／1FAIL／1ignored，原626及后续门禁未到达。原流绑定19198、deadline未到期和Failed锁定，15个complete根删除／1个失败根保留，不沿用旧17根数量。全部834执行输入随后精确恢复，恢复后的locked no-run通过。没有改变本决策的错误拒绝、三秒、动作许可或任何原断言，旧v7原因仍未知。平台机制研究和新的受控证明必须分别记录；不能把可能的OS退出过渡解释直接变成放宽错误的功能修复。
